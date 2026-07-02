package wickraterminal

// Cross-language golden parity: build the terminal from the committed
// golden/config.json, replay the feed, and assert the frame equals
// golden/expected/basic.min.json byte-for-byte. The binding returns the core's
// compact command_json string verbatim, so byte equality against that one file is
// the exact cross-language parity check.

import (
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func goldenDir(t *testing.T) string {
	t.Helper()
	dir, err := os.Getwd()
	if err != nil {
		t.Fatal(err)
	}
	for i := 0; i < 8; i++ {
		g := filepath.Join(dir, "golden")
		if _, err := os.Stat(filepath.Join(g, "config.json")); err == nil {
			return g
		}
		dir = filepath.Dir(dir)
	}
	t.Fatal("golden/ not found")
	return ""
}

func TestGoldenParity(t *testing.T) {
	g := goldenDir(t)
	config, err := os.ReadFile(filepath.Join(g, "config.json"))
	if err != nil {
		t.Fatal(err)
	}
	expectedBytes, err := os.ReadFile(filepath.Join(g, "expected", "basic.min.json"))
	if err != nil {
		t.Fatal(err)
	}
	expected := strings.TrimSpace(string(expectedBytes))

	term, err := New(string(config))
	if err != nil {
		t.Fatal(err)
	}
	defer term.Close()

	if _, err := term.Command(`{"type":"Subscribe","source":0,"symbol":"BTC/USDT"}`); err != nil {
		t.Fatal(err)
	}
	var frame string
	for i := 0; i < 32; i++ {
		frame, err = term.Command(`{"type":"Tick"}`)
		if err != nil {
			t.Fatal(err)
		}
	}
	if strings.TrimSpace(frame) != expected {
		t.Fatalf("frame mismatch\n got: %s\nwant: %s", frame, expected)
	}
}
