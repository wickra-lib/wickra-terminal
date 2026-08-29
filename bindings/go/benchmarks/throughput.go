// Throughput benchmark for the wickra-terminal Go binding.
//
// What this measures is the boundary, not the core. Every binding drives the
// same Rust terminal through one function -- a command JSON in, a frame JSON
// out -- so the number is the cost of crossing this boundary once per command.
// Go's row is worth reading next to C's: the work either side of cgo is the
// same, so the gap is what a cgo call and the string conversions cost.
//
// Build the C ABI first, then from the repository root:
//
//	cargo build -p wickra-terminal-c --release
//	go run ./bindings/go/benchmarks            # 20k commands
//	go run ./bindings/go/benchmarks -ticks 100000
package main

import (
	"flag"
	"fmt"
	"sort"
	"time"

	wickra "github.com/wickra-lib/wickra-terminal-go"
)

// Shared by all nine binding benchmarks, so the numbers compare.
const (
	config = `{"sources":[{"Synth":{"seed":1}}],` +
		`"layout":{"panels":[` +
		`{"kind":"Chart","rect":{"x":0,"y":0,"w":100,"h":40}},` +
		`{"kind":"Book","rect":{"x":0,"y":40,"w":50,"h":30}},` +
		`{"kind":"Tape","rect":{"x":50,"y":40,"w":50,"h":30}}]}}`
	subscribe = `{"type":"Subscribe","source":0,"symbol":"BTC/USDT"}`
	tick      = `{"type":"Tick"}`
	list      = `{"type":"ListIndicators"}`

	// The catalogue response is ~30 kB, so a hundred of them is a noisy sample.
	catalogueReps = 1000
)

// medianNs is the median of three timed runs, after one warmup.
func medianNs(term *wickra.Terminal, command string, count int) (float64, error) {
	drive := func() error {
		for i := 0; i < count; i++ {
			if _, err := term.Command(command); err != nil {
				return err
			}
		}
		return nil
	}
	if err := drive(); err != nil {
		return 0, err
	}
	samples := make([]float64, 3)
	for i := range samples {
		start := time.Now()
		if err := drive(); err != nil {
			return 0, err
		}
		samples[i] = float64(time.Since(start).Nanoseconds())
	}
	sort.Float64s(samples)
	return samples[1], nil
}

func main() {
	ticks := flag.Int("ticks", 20000, "commands per sample")
	flag.Parse()
	if *ticks < 100 {
		*ticks = 20000
	}

	term, err := wickra.New(config)
	if err != nil {
		panic(err)
	}
	defer term.Close()

	if _, err := term.Command(subscribe); err != nil {
		panic(err)
	}
	frame, err := term.Command(tick)
	if err != nil {
		panic(err)
	}
	catalogue, err := term.Command(list)
	if err != nil {
		panic(err)
	}

	tickNs, err := medianNs(term, tick, *ticks)
	if err != nil {
		panic(err)
	}
	listNs, err := medianNs(term, list, catalogueReps)
	if err != nil {
		panic(err)
	}

	fmt.Printf("wickra-terminal Go throughput - %d commands (median of 3)\n\n", *ticks)
	fmt.Printf("%-18s%14s%14s%12s\n", "Command", "per second", "us/command", "payload")
	fmt.Println("----------------------------------------------------------")
	fmt.Printf("%-18s%14.0f%14.2f%11dB\n", "Tick", float64(*ticks)/(tickNs/1e9),
		tickNs/float64(*ticks)/1e3, len(frame))
	fmt.Printf("%-18s%14.0f%14.2f%11dB\n", "ListIndicators", float64(catalogueReps)/(listNs/1e9),
		listNs/float64(catalogueReps)/1e3, len(catalogue))
	fmt.Print("\nOne command crosses the boundary once. Higher is better, and the numbers\n",
		"are machine-dependent -- compare bindings on one machine, never across two.\n")
}
