// The time-machine scenario: rewind a recorded feed and watch state re-fold.
//
// Seek throws the folded state away and rebuilds it from the recording, so a
// rewind is deterministic rather than approximate -- which is what makes a
// recording more than a slow synthetic feed. Nothing here is Go-specific: it is
// four JSON commands, and every binding drives the same four.
//
//	cd examples/go && go run . time-machine
package main

import (
	"encoding/json"
	"fmt"
	"strings"

	wickra "github.com/wickra-lib/wickra-terminal-go"
)

const trades = 6

// The recorded feed, as the JSON array a Replay source takes.
func replayConfig() string {
	events := make([]string, 0, trades)
	for i := 0; i < trades; i++ {
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

// The chart panel's last price, out of a frame.
func lastPrice(frame string) float64 {
	var decoded struct {
		Panels []struct {
			Panel string  `json:"panel"`
			Last  float64 `json:"last"`
		} `json:"panels"`
	}
	if err := json.Unmarshal([]byte(frame), &decoded); err != nil {
		panic(err)
	}
	for _, panel := range decoded.Panels {
		if panel.Panel == "chart" {
			return panel.Last
		}
	}
	return 0
}

func timeMachine() {
	term, err := wickra.New(replayConfig())
	if err != nil {
		panic(err)
	}
	defer term.Close()

	must := func(command string) string {
		out, err := term.Command(command)
		if err != nil {
			panic(err)
		}
		return out
	}

	must(`{"type":"Subscribe","source":0,"symbol":"BTC/USDT"}`)
	var raw string
	for i := 0; i < trades; i++ {
		raw = must(`{"type":"Tick"}`)
	}
	fmt.Printf("played to the end:   last = %v\n", lastPrice(raw))

	fmt.Println("position:           ", must(`{"type":"ReplayPosition","source":0}`))

	// Rewind to just after the second trade. The state is rebuilt from the
	// recording rather than restored from a snapshot, which is why a rewind
	// lands on exactly the frame the forward pass had at that point.
	raw = must(`{"type":"Seek","source":0,"index":2}`)
	fmt.Printf("rewound to index 2:  last = %v\n", lastPrice(raw))

	// And forward again from there, over the same events.
	raw = must(`{"type":"Tick"}`)
	fmt.Printf("one tick later:      last = %v\n", lastPrice(raw))
}
