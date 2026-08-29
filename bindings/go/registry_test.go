package wickraterminal

import (
	"encoding/json"
	"strings"
	"testing"
)

// The indicator registry is reachable from Go.
//
// The registry lives in the Rust core and this binding passes JSON through the
// C ABI, so nothing here needed new binding code. That is exactly why it is
// worth a test: "no code changed" is also what a broken pass-through looks like.

// A non-default indicator, so finding it proves the config reached the registry
// rather than the built-in overlay happening to look right.
const registryConfig = `{"sources":[{"Synth":{"seed":1}}],"indicators":[{"kind":"Rsi","params":[14]}]}`

type frame struct {
	Panels []struct {
		Panel      string `json:"panel"`
		Indicators []struct {
			Name   string   `json:"name"`
			Value  *float64 `json:"value"`
			Fields []struct {
				Name  string  `json:"name"`
				Value float64 `json:"value"`
			} `json:"fields"`
		} `json:"indicators"`
	} `json:"panels"`
}

func chartIndicators(t *testing.T, term *Terminal) []string {
	t.Helper()
	raw, err := term.Command(`{"type":"Tick"}`)
	if err != nil {
		t.Fatal(err)
	}
	var f frame
	if err := json.Unmarshal([]byte(raw), &f); err != nil {
		t.Fatal(err)
	}
	for _, p := range f.Panels {
		if p.Panel == "chart" {
			names := make([]string, 0, len(p.Indicators))
			for _, i := range p.Indicators {
				names = append(names, i.Name)
			}
			return names
		}
	}
	t.Fatal("no chart panel in the frame")
	return nil
}

func subscribedTerminal(t *testing.T) *Terminal {
	t.Helper()
	term, err := New(registryConfig)
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(term.Close)
	if _, err := term.Command(`{"type":"Subscribe","source":0,"symbol":"BTC/USDT"}`); err != nil {
		t.Fatal(err)
	}
	return term
}

func TestConfiguredIndicatorReachesTheChart(t *testing.T) {
	term := subscribedTerminal(t)
	var names []string
	for i := 0; i < 30; i++ {
		names = chartIndicators(t, term)
	}
	if len(names) != 1 || names[0] != "Rsi(14)" {
		t.Fatalf("got %v, want [Rsi(14)]", names)
	}
}

func TestIndicatorsCanBeAddedAndRemovedAtRunTime(t *testing.T) {
	term := subscribedTerminal(t)
	if _, err := term.Command(`{"type":"AddIndicator","spec":{"kind":"Atr","params":[14]}}`); err != nil {
		t.Fatal(err)
	}
	names := chartIndicators(t, term)
	if !contains(names, "Atr(14)") {
		t.Fatalf("Atr(14) missing from %v", names)
	}
	if _, err := term.Command(`{"type":"RemoveIndicator","label":"Rsi(14)"}`); err != nil {
		t.Fatal(err)
	}
	names = chartIndicators(t, term)
	if len(names) != 1 || names[0] != "Atr(14)" {
		t.Fatalf("got %v, want [Atr(14)]", names)
	}
}

func TestCatalogueListsTheWholeRegistry(t *testing.T) {
	term := subscribedTerminal(t)
	raw, err := term.Command(`{"type":"ListIndicators"}`)
	if err != nil {
		t.Fatal(err)
	}
	var catalogue struct {
		Indicators []struct {
			Kind   string    `json:"kind"`
			Params []float64 `json:"params"`
		} `json:"indicators"`
	}
	if err := json.Unmarshal([]byte(raw), &catalogue); err != nil {
		t.Fatal(err)
	}
	if len(catalogue.Indicators) < 495 {
		t.Fatalf("only %d entries in the catalogue", len(catalogue.Indicators))
	}
	// Every row carries the parameters needed to construct it: discovery
	// without a second lookup.
	byKind := map[string][]float64{}
	for _, row := range catalogue.Indicators {
		byKind[row.Kind] = row.Params
	}
	if len(byKind["Sma"]) != 1 {
		t.Fatalf("Sma params: %v", byKind["Sma"])
	}
	if len(byKind["MacdIndicator"]) != 3 {
		t.Fatalf("MacdIndicator params: %v", byKind["MacdIndicator"])
	}
}

func TestUnknownIndicatorIsRejectedWithItsName(t *testing.T) {
	term := subscribedTerminal(t)
	_, err := term.Command(`{"type":"AddIndicator","spec":{"kind":"NotReal"}}`)
	if err == nil {
		t.Fatal("an unknown indicator should be rejected")
	}
	if !strings.Contains(err.Error(), "NotReal") {
		t.Fatalf("error does not name the indicator: %v", err)
	}
}

func TestMultiOutputIndicatorReportsNamedFields(t *testing.T) {
	term, err := New(`{"sources":[{"Synth":{"seed":1}}],"indicators":[{"kind":"MacdIndicator","params":[12,26,9]}]}`)
	if err != nil {
		t.Fatal(err)
	}
	defer term.Close()
	if _, err := term.Command(`{"type":"Subscribe","source":0,"symbol":"BTC/USDT"}`); err != nil {
		t.Fatal(err)
	}
	var raw string
	for i := 0; i < 200; i++ {
		if raw, err = term.Command(`{"type":"Tick"}`); err != nil {
			t.Fatal(err)
		}
	}
	var f frame
	if err := json.Unmarshal([]byte(raw), &f); err != nil {
		t.Fatal(err)
	}
	for _, p := range f.Panels {
		if p.Panel != "chart" {
			continue
		}
		macd := p.Indicators[0]
		if macd.Name != "MacdIndicator(12,26,9)" {
			t.Fatalf("got %q", macd.Name)
		}
		if len(macd.Fields) < 2 {
			t.Fatalf("expected named fields, got %v", macd.Fields)
		}
		// The primary value is the first field, so a caller wanting one line
		// does not have to know which field that is.
		if macd.Value == nil || *macd.Value != macd.Fields[0].Value {
			t.Fatalf("value %v does not match the first field %v", macd.Value, macd.Fields[0])
		}
		return
	}
	t.Fatal("no chart panel in the frame")
}

func contains(haystack []string, needle string) bool {
	for _, s := range haystack {
		if s == needle {
			return true
		}
	}
	return false
}
