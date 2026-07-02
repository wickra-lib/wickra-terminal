# Wickra Terminal — Go

Idiomatic Go bindings for the `wickra-terminal` data-driven core over its C ABI
hub. Build a `Terminal` from a JSON config, drive it with command JSON, read back
frame view-models — the same protocol as the native TUI and every other binding.

## Install

```bash
go get github.com/wickra-lib/wickra-terminal-go
```

## Usage

```go
package main

import (
	"fmt"

	wickra "github.com/wickra-lib/wickra-terminal-go"
)

func main() {
	term, err := wickra.New(`{"sources":[{"Synth":{"seed":1}}],` +
		`"layout":{"panels":[{"kind":"Chart","rect":{"x":0,"y":0,"w":100,"h":100}}]}}`)
	if err != nil {
		panic(err)
	}
	defer term.Close()

	term.Command(`{"type":"Subscribe","source":0,"symbol":"BTC/USDT"}`)
	frame, _ := term.Command(`{"type":"Tick"}`)
	fmt.Println(frame)
	fmt.Println(wickra.Version())
}
```

## Native library

cgo links the prebuilt C ABI library `wickra_terminal`, staged per platform under
`./lib/<goos>_<goarch>/`, with the header vendored under `./include`. Build the
library with `cargo build -p wickra-terminal-c --release` and place it (e.g.
`wickra_terminal.dll` on Windows, `libwickra_terminal.so` on Linux) under the
matching `lib/` directory. CI stages these per platform.
