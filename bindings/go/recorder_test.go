package wickraterminal

// The recorder, the scrubber and the host feed, end to end through the binding.
//
// Four commands sit on the boundary, are documented in all nine binding
// READMEs, and were driven by almost no binding: SetRecording and
// ExportRecording by none at all, ReplayPosition only by the C example,
// FeedDerivatives by none. The README completeness test proved the promise and
// nothing checked it was kept, so the recorder had never been executed outside
// Rust.
//
// The round trip is the point: arm the recorder, drive the terminal, export
// what it kept, and hand that straight back as a Replay dataset. A binding that
// mangled the export would be caught by the replay refusing it, which no
// assertion about a string shape would find.

import (
	"encoding/json"
	"fmt"
	"math"
	"strings"
	"testing"
)

// A derivatives indicator, so FeedDerivatives is observable in the frame rather
// than merely accepted.
const recorderConfig = `{"sources":["Manual"],"indicators":[{"kind":"FundingRate","params":[]}],` +
	`"layout":{"panels":[{"kind":"Chart","rect":{"x":0,"y":0,"w":100,"h":100}}]}}`

const recorderSymbol = "BTC/USDT"

type indicatorReading struct {
	Name  string   `json:"name"`
	Value *float64 `json:"value"`
}

type chartPanel struct {
	Panel      string             `json:"panel"`
	Last       float64            `json:"last"`
	Indicators []indicatorReading `json:"indicators"`
}

type framePanels struct {
	Panels []chartPanel `json:"panels"`
}

type replayPosition struct {
	Cursor int `json:"cursor"`
	Length int `json:"length"`
}

func replayConfig(dataset string) string {
	encoded, err := json.Marshal(dataset)
	if err != nil {
		panic(err)
	}
	return `{"sources":[{"Replay":{"dataset":` + string(encoded) + `}}],"indicators":[],` +
		`"layout":{"panels":[{"kind":"Chart","rect":{"x":0,"y":0,"w":100,"h":100}}]}}`
}

func feedTrade(t *testing.T, term *Terminal, price string, timestamp int) string {
	t.Helper()
	event := fmt.Sprintf(
		`{"type":"Feed","source":0,"event":{"type":"trade","symbol":{"base":"BTC","quote":"USDT"},`+
			`"price":"%s","quantity":"0.5","aggressor":"Buy","timestamp":%d}}`, price, timestamp)
	if _, err := term.Command(event); err != nil {
		t.Fatal(err)
	}
	raw, err := term.Command(`{"type":"Tick"}`)
	if err != nil {
		t.Fatal(err)
	}
	return raw
}

func chartOf(t *testing.T, raw string) chartPanel {
	t.Helper()
	var frame framePanels
	if err := json.Unmarshal([]byte(raw), &frame); err != nil {
		t.Fatal(err)
	}
	for _, panel := range frame.Panels {
		if panel.Panel == "chart" {
			return panel
		}
	}
	t.Fatalf("no chart panel in %s", raw)
	return chartPanel{}
}

func subscribedRecorder(t *testing.T) *Terminal {
	t.Helper()
	term, err := New(recorderConfig)
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(term.Close)
	if _, err := term.Command(`{"type":"Subscribe","source":0,"symbol":"` + recorderSymbol + `"}`); err != nil {
		t.Fatal(err)
	}
	return term
}

func exported(t *testing.T, term *Terminal) string {
	t.Helper()
	raw, err := term.Command(`{"type":"ExportRecording"}`)
	if err != nil {
		t.Fatal(err)
	}
	return raw
}

func TestRecorderRoundTripsThroughAReplay(t *testing.T) {
	term := subscribedRecorder(t)

	// Nothing is kept until the recorder is armed, and asking is not an error.
	if got := exported(t, term); got != "[]" {
		t.Fatalf("expected an empty recording, got %s", got)
	}

	if _, err := term.Command(`{"type":"SetRecording","capacity":64}`); err != nil {
		t.Fatal(err)
	}
	for i, price := range []string{"100", "101", "102", "103"} {
		feedTrade(t, term, price, i+1)
	}

	recording := exported(t, term)
	var events []map[string]any
	if err := json.Unmarshal([]byte(recording), &events); err != nil {
		t.Fatal(err)
	}
	if len(events) != 4 {
		t.Fatalf("expected 4 recorded events, got %d", len(events))
	}
	if events[0]["price"] != "100" || events[3]["price"] != "103" {
		t.Fatalf("recorded the wrong prints: %s", recording)
	}

	// Straight back in as a dataset: the shape Replay takes is the shape
	// ExportRecording answers with, which is what makes a session keepable.
	replay, err := New(replayConfig(recording))
	if err != nil {
		t.Fatal(err)
	}
	defer replay.Close()
	if _, err := replay.Command(`{"type":"Subscribe","source":0,"symbol":"` + recorderSymbol + `"}`); err != nil {
		t.Fatal(err)
	}
	var raw string
	for i := 0; i < 4; i++ {
		if raw, err = replay.Command(`{"type":"Tick"}`); err != nil {
			t.Fatal(err)
		}
	}
	if last := chartOf(t, raw).Last; last != 103 {
		t.Fatalf("the replay ended at %v rather than 103", last)
	}
}

func TestStoppingTheRecorderClearsWhatItHeld(t *testing.T) {
	// Both directions clear, so a capacity change never leaves a recording that
	// is part one size and part another.
	term := subscribedRecorder(t)
	if _, err := term.Command(`{"type":"SetRecording","capacity":64}`); err != nil {
		t.Fatal(err)
	}
	feedTrade(t, term, "100", 1)
	if got := exported(t, term); got == "[]" {
		t.Fatal("the armed recorder kept nothing")
	}

	if _, err := term.Command(`{"type":"SetRecording","capacity":null}`); err != nil {
		t.Fatal(err)
	}
	if got := exported(t, term); got != "[]" {
		t.Fatalf("stopping the recorder left %s", got)
	}
}

func TestReplayPositionAnswersForASourceThatCannotBeReplayed(t *testing.T) {
	// 0/0 rather than an error, so a renderer can ask about whatever is focused
	// without first knowing what kind of source it is.
	term := subscribedRecorder(t)
	raw, err := term.Command(`{"type":"ReplayPosition","source":0}`)
	if err != nil {
		t.Fatal(err)
	}
	var where replayPosition
	if err := json.Unmarshal([]byte(raw), &where); err != nil {
		t.Fatal(err)
	}
	if where.Cursor != 0 || where.Length != 0 {
		t.Fatalf("expected 0/0, got %+v", where)
	}
}

func TestReplayPositionTracksTheCursorThroughARecording(t *testing.T) {
	term := subscribedRecorder(t)
	if _, err := term.Command(`{"type":"SetRecording","capacity":64}`); err != nil {
		t.Fatal(err)
	}
	for i, price := range []string{"100", "101", "102", "103"} {
		feedTrade(t, term, price, i+1)
	}

	replay, err := New(replayConfig(exported(t, term)))
	if err != nil {
		t.Fatal(err)
	}
	defer replay.Close()
	if _, err := replay.Command(`{"type":"Subscribe","source":0,"symbol":"` + recorderSymbol + `"}`); err != nil {
		t.Fatal(err)
	}

	position := func() replayPosition {
		raw, err := replay.Command(`{"type":"ReplayPosition","source":0}`)
		if err != nil {
			t.Fatal(err)
		}
		var where replayPosition
		if err := json.Unmarshal([]byte(raw), &where); err != nil {
			t.Fatal(err)
		}
		return where
	}

	if start := position(); start.Cursor != 0 || start.Length != 4 {
		t.Fatalf("expected 0/4 at the start, got %+v", start)
	}
	for i := 0; i < 3; i++ {
		if _, err := replay.Command(`{"type":"Tick"}`); err != nil {
			t.Fatal(err)
		}
	}
	if moved := position(); moved.Cursor != 3 || moved.Length != 4 {
		t.Fatalf("expected 3/4 after three ticks, got %+v", moved)
	}
}

func TestFedDerivativesReachADerivativesIndicator(t *testing.T) {
	// Accepting the command proves nothing on its own: the update is folded into
	// the market's microstructure and reaches an indicator only on the next
	// trade, so the reading is what says it arrived.
	term := subscribedRecorder(t)
	if before := chartOf(t, feedTrade(t, term, "100", 1)); before.Indicators[0].Value != nil {
		t.Fatalf("a reading appeared before any derivatives arrived: %+v", before.Indicators[0])
	}

	// All three prices, or the tick is withheld: a mark without an index and a
	// futures price is not a priced market.
	update := `{"type":"FeedDerivatives","source":0,"symbol":"` + recorderSymbol + `",` +
		`"update":{"funding_rate":0.0001,"mark_price":102.0,"index_price":100.0,` +
		`"futures_price":104.0,"open_interest":1000.0,"timestamp":9}}`
	if _, err := term.Command(update); err != nil {
		t.Fatal(err)
	}

	reading := chartOf(t, feedTrade(t, term, "101", 2)).Indicators[0]
	if reading.Name != "FundingRate" {
		t.Fatalf("expected FundingRate, got %q", reading.Name)
	}
	if reading.Value == nil || math.Abs(*reading.Value-0.0001) > 1e-12 {
		t.Fatalf("the funding rate did not reach the indicator: %+v", reading)
	}
}

func TestFeedingDerivativesToAnUntrackedMarketIsAnError(t *testing.T) {
	term, err := New(recorderConfig)
	if err != nil {
		t.Fatal(err)
	}
	defer term.Close()

	update := `{"type":"FeedDerivatives","source":0,"symbol":"` + recorderSymbol + `",` +
		`"update":{"funding_rate":0.0001,"timestamp":1}}`
	_, err = term.Command(update)
	if err == nil {
		t.Fatal("expected an error for a market that is not subscribed")
	}
	if !strings.Contains(err.Error(), "subscribe") {
		t.Fatalf("the message does not say what to do: %v", err)
	}
}
