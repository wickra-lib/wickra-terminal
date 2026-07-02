package wickraterminal

import (
	"strings"
	"testing"
)

const config = `{"sources":[{"Synth":{"seed":1}}],"layout":{"panels":[{"kind":"Chart","rect":{"x":0,"y":0,"w":100,"h":100}}]}}`

func TestVersion(t *testing.T) {
	if Version() == "" {
		t.Fatal("empty version")
	}
}

func TestSubscribeThenTickReturnsChartFrame(t *testing.T) {
	term, err := New(config)
	if err != nil {
		t.Fatal(err)
	}
	defer term.Close()

	if _, err := term.Command(`{"type":"Subscribe","source":0,"symbol":"BTC/USDT"}`); err != nil {
		t.Fatal(err)
	}
	var raw string
	for i := 0; i < 30; i++ {
		raw, err = term.Command(`{"type":"Tick"}`)
		if err != nil {
			t.Fatal(err)
		}
	}
	if !strings.Contains(raw, `"panel":"chart"`) {
		t.Fatalf("expected a chart panel, got: %s", raw)
	}
}

func TestInvalidConfig(t *testing.T) {
	if _, err := New("not json"); err == nil {
		t.Fatal("expected an error for an invalid config")
	}
}

func TestInvalidCommand(t *testing.T) {
	term, err := New(config)
	if err != nil {
		t.Fatal(err)
	}
	defer term.Close()
	if _, err := term.Command(`{"type":"Nope"}`); err == nil {
		t.Fatal("expected an error for an unknown command")
	}
}
