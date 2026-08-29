package wickraterminal

import (
	"encoding/json"
	"os"
	"path/filepath"
	"strings"
	"testing"
)

// Cross-language golden parity, driven by golden/manifest.json.
//
// Each scenario names a config and a command sequence; replaying it must produce
// the frame in its expected file, byte for byte. Because the binding returns the
// core's compact command_json string verbatim, byte equality against that one
// file is the exact parity check.
//
// Reading the manifest rather than naming one scenario is what makes the corpus
// extensible: a scenario added in the Rust suite is picked up here, and in the
// seven other language suites, with no change to any of them.

type scenario struct {
	Name     string `json:"name"`
	Config   string `json:"config"`
	Expected string `json:"expected"`
	Commands string `json:"commands"`
}

func goldenDir(t *testing.T) string {
	t.Helper()
	dir, err := os.Getwd()
	if err != nil {
		t.Fatal(err)
	}
	for i := 0; i < 8; i++ {
		candidate := filepath.Join(dir, "golden")
		if _, err := os.Stat(filepath.Join(candidate, "manifest.json")); err == nil {
			return candidate
		}
		dir = filepath.Dir(dir)
	}
	// Skip rather than fail. The release job mirrors this directory into
	// wickra-terminal-go as a standalone module, which ships the tests but not
	// the corpus that lives at the repository root, so `go test ./...` on the
	// published module would otherwise fail for a reason that has nothing to do
	// with the module.
	//
	// This does not lose the guard: the corpus is driven from the source repo,
	// where CI runs these tests with golden/ present, and eight other language
	// suites read the same manifest.
	t.Skip("golden/ not found: this is the published module, which ships no corpus")
	return ""
}

func manifest(t *testing.T) (string, []scenario) {
	t.Helper()
	golden := goldenDir(t)
	raw, err := os.ReadFile(filepath.Join(golden, "manifest.json"))
	if err != nil {
		t.Fatal(err)
	}
	var parsed struct {
		Scenarios []scenario `json:"scenarios"`
	}
	if err := json.Unmarshal(raw, &parsed); err != nil {
		t.Fatal(err)
	}
	return golden, parsed.Scenarios
}

func TestGoldenParity(t *testing.T) {
	golden, scenarios := manifest(t)
	for _, sc := range scenarios {
		t.Run(sc.Name, func(t *testing.T) {
			config, err := os.ReadFile(filepath.Join(golden, filepath.FromSlash(sc.Config)))
			if err != nil {
				t.Fatal(err)
			}
			expected, err := os.ReadFile(filepath.Join(golden, filepath.FromSlash(sc.Expected)))
			if err != nil {
				t.Fatal(err)
			}

			raw, err := os.ReadFile(filepath.Join(golden, filepath.FromSlash(sc.Commands)))
			if err != nil {
				t.Fatal(err)
			}
			var commands []string
			for _, line := range strings.Split(string(raw), "\n") {
				if trimmed := strings.TrimSpace(line); trimmed != "" {
					commands = append(commands, trimmed)
				}
			}
			if len(commands) == 0 {
				t.Fatalf("%s: no commands", sc.Name)
			}

			term, err := New(string(config))
			if err != nil {
				t.Fatal(err)
			}
			defer term.Close()

			var frame string
			for _, command := range commands {
				if frame, err = term.Command(command); err != nil {
					t.Fatalf("%s: %v", command, err)
				}
			}
			if strings.TrimSpace(frame) != strings.TrimSpace(string(expected)) {
				t.Fatalf("frame does not match %s", sc.Expected)
			}
		})
	}
}

func TestCorpusCoversMoreThanOneScenario(t *testing.T) {
	// A manifest that silently shrank to one entry would leave every parity test
	// passing while checking a fraction of what it used to.
	_, scenarios := manifest(t)
	if len(scenarios) < 12 {
		t.Fatalf("only %d scenarios in the manifest", len(scenarios))
	}
	names := map[string]bool{}
	for _, sc := range scenarios {
		names[sc.Name] = true
	}
	for _, want := range []string{"basic", "book_deltas", "footprint", "indicators", "seek"} {
		if !names[want] {
			t.Fatalf("%s missing from the manifest", want)
		}
	}
}
