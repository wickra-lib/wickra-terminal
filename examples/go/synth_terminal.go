// A runnable Go example: drive a synthetic feed and print a frame.
//
//	cargo build --release -p wickra-terminal-c
//	# stage the library under bindings/go/lib/<goos>_<goarch>/ (CI does this)
//	cd examples/go && go run .
package main

import (
	"fmt"

	wickra "github.com/wickra-lib/wickra-terminal-go"
)

const config = `{"sources":[{"Synth":{"seed":1}}],` +
	`"layout":{"panels":[{"kind":"Chart","rect":{"x":0,"y":0,"w":100,"h":100}}]}}`

func main() {
	term, err := wickra.New(config)
	if err != nil {
		panic(err)
	}
	defer term.Close()

	if _, err := term.Command(`{"type":"Subscribe","source":0,"symbol":"BTC/USDT"}`); err != nil {
		panic(err)
	}
	var raw string
	for i := 0; i < 20; i++ {
		raw, err = term.Command(`{"type":"Tick"}`)
		if err != nil {
			panic(err)
		}
	}

	fmt.Println("wickra-terminal", wickra.Version())
	fmt.Println(raw)
}
