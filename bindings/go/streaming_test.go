package wickraterminal

import (
	"encoding/json"
	"fmt"
	"strings"
	"testing"
)

// Streaming a feed and re-folding it in one batch reach the same frame.
//
// The terminal reaches a state two ways. Streaming folds one event per tick as
// it arrives; Seek throws the state away and re-folds the whole prefix in a
// single batch. ARCHITECTURE.md calls that re-fold the moat -- it is what makes
// a rewind deterministic and what lets the browser run the time-machine with no
// engine behind it -- so the two must land on byte-identical frames.
//
// Byte-identical, not merely equal: the binding returns the core's compact
// command output verbatim, so string equality here is the exact check with no
// JSON comparison in the way. The Rust suite proves the core re-folds correctly;
// this proves the binding carries the same bytes out.

const (
	refoldTicks  = 4
	refoldEvents = 8
)

func refoldConfig() string {
	events := make([]string, 0, refoldEvents)
	for i := 0; i < refoldEvents; i++ {
		events = append(events, fmt.Sprintf(
			`{"type":"trade","symbol":{"base":"BTC","quote":"USDT"},`+
				`"price":"%d","quantity":"1","aggressor":"Buy","timestamp":%d}`,
			100+i, i+1))
	}
	feed, err := json.Marshal("[" + strings.Join(events, ",") + "]")
	if err != nil {
		panic(err)
	}
	return `{"sources":[{"Replay":{"dataset":` + string(feed) + `}}],` +
		`"layout":{"panels":[{"kind":"Chart","rect":{"x":0,"y":0,"w":100,"h":100}}]}}`
}

func subscribedReplay(t *testing.T) *Terminal {
	t.Helper()
	term, err := New(refoldConfig())
	if err != nil {
		t.Fatal(err)
	}
	if _, err := term.Command(`{"type":"Subscribe","source":0,"symbol":"BTC/USDT"}`); err != nil {
		t.Fatal(err)
	}
	return term
}

func TestStreamingAndBatchRefoldAgree(t *testing.T) {
	streamed := subscribedReplay(t)
	defer streamed.Close()
	var frame string
	for i := 0; i < refoldTicks; i++ {
		var err error
		frame, err = streamed.Command(`{"type":"Tick"}`)
		if err != nil {
			t.Fatal(err)
		}
	}

	// A second terminal runs the feed out, then re-folds the same prefix in one
	// batch. Running past the point first is what makes this a rewind rather
	// than a replay of state it still had.
	rewound := subscribedReplay(t)
	defer rewound.Close()
	for i := 0; i < refoldEvents; i++ {
		if _, err := rewound.Command(`{"type":"Tick"}`); err != nil {
			t.Fatal(err)
		}
	}
	refolded, err := rewound.Command(fmt.Sprintf(`{"type":"Seek","source":0,"index":%d}`, refoldTicks))
	if err != nil {
		t.Fatal(err)
	}

	if frame != refolded {
		t.Fatalf("streaming and re-fold disagree:\n stream: %s\n refold: %s", frame, refolded)
	}
}

func TestTheComparedFrameIsNotEmpty(t *testing.T) {
	// A guard on the guard: two empty frames are also byte-identical, and an
	// equality test that passes on nothing proves nothing.
	term := subscribedReplay(t)
	defer term.Close()
	var raw string
	for i := 0; i < refoldTicks; i++ {
		var err error
		raw, err = term.Command(`{"type":"Tick"}`)
		if err != nil {
			t.Fatal(err)
		}
	}
	want := fmt.Sprintf(`"last":%d`, 100+refoldTicks-1)
	if !strings.Contains(raw, want) {
		t.Fatalf("no %s in frame: %s", want, raw)
	}
}
