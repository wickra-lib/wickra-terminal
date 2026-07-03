<p align="center">
  <a href="https://wickra.org"><img src="https://raw.githubusercontent.com/wickra-lib/.github/main/profile/wickra-banner.webp?v=514" alt="Wickra Terminal — the data-driven trading terminal for Go" width="100%"></a>
</p>

[![Built on Wickra](https://img.shields.io/badge/built%20on-wickra-3b82f6)](https://github.com/wickra-lib/wickra)
[![CI](https://raw.githubusercontent.com/wickra-lib/.github/main/profile/badges/wickra-terminal/ci.svg)](https://github.com/wickra-lib/wickra-terminal/actions/workflows/ci.yml)
[![codecov](https://raw.githubusercontent.com/wickra-lib/.github/main/profile/badges/wickra-terminal/codecov.svg)](https://codecov.io/gh/wickra-lib/wickra-terminal)
[![Go module](https://raw.githubusercontent.com/wickra-lib/.github/main/profile/badges/go.svg)](https://pkg.go.dev/github.com/wickra-lib/wickra-terminal-go)
[![License: MIT OR Apache-2.0](https://raw.githubusercontent.com/wickra-lib/.github/main/profile/badges/wickra-terminal/license.svg)](https://github.com/wickra-lib/wickra-terminal#license)

# Wickra Terminal — Go

---

> **▶ Web renderer:** the same core drives a browser front-end (WASM + Vue) as a selectable renderer — see [`web/`](https://github.com/wickra-lib/wickra-terminal/tree/main/web).

**The data-driven trading-terminal core for Go, over the Wickra C ABI hub via cgo.**

[Wickra Terminal](https://github.com/wickra-lib/wickra-terminal) is one streaming
trading-terminal core with a native **TUI** and a **Web** front-end as two
renderers of the same logic. The core folds market events into an O(1) state and
turns panels into **view-models**; every language drives it through one tiny,
data-shaped surface — a JSON config in, command JSON in, frame view-models out.
This package is the Go binding: it consumes the C ABI hub through cgo and exposes
the `Terminal` handle with the same protocol as the native TUI and every other
binding.

## Install

Use the published **`wickra-terminal-go`** module, which bundles the prebuilt C
ABI library for every platform, so `go get` + `go build` works with no extra
steps (a C compiler is still required, as the binding uses cgo):

```bash
go get github.com/wickra-lib/wickra-terminal-go
```

```go
import wickra "github.com/wickra-lib/wickra-terminal-go"
```

`wickra-terminal-go` is generated from this directory by the release pipeline: it
mirrors the Go sources, the vendored C ABI header (`include/wickra_terminal.h`)
and the prebuilt libraries under `lib/<goos>_<goarch>/`. On Windows the DLL must
be discoverable at run time (next to the executable or on `PATH`).

## Quick start

```go
package main

import (
	"fmt"

	wickra "github.com/wickra-lib/wickra-terminal-go"
)

func main() {
	// Build a terminal from a JSON config (a synthetic source + a chart panel).
	term, err := wickra.New(`{"sources":[{"Synth":{"seed":1}}],` +
		`"layout":{"panels":[{"kind":"Chart","rect":{"x":0,"y":0,"w":100,"h":100}}]}}`)
	if err != nil {
		panic(err)
	}
	defer term.Close()

	// Subscribe a market, then tick: the returned frame is the panels' view-models.
	term.Command(`{"type":"Subscribe","source":0,"symbol":"BTC/USDT"}`)
	frame, _ := term.Command(`{"type":"Tick"}`)
	fmt.Println(frame)
	fmt.Println(wickra.Version())
}
```

The same command protocol works from every binding and both renderers — `Tick`,
`Subscribe`, `Unsubscribe`, `SetFocus`, `AddSource`, `RemoveSource`, `Seek` (the
time-machine) and `Feed` (a host-fed source). See the
[docs](https://github.com/wickra-lib/wickra-terminal/tree/main/docs).

## Building from this repository (contributors)

This `bindings/go` directory is the development source. To build it directly,
compile the C ABI hub and stage the library into the per-platform directory cgo
links against:

```bash
cargo build -p wickra-terminal-c --release
mkdir -p bindings/go/lib/linux_amd64                       # match your GOOS_GOARCH
cp target/release/libwickra_terminal.so    bindings/go/lib/linux_amd64/    # Linux
cp target/release/libwickra_terminal.dylib bindings/go/lib/darwin_arm64/   # macOS (arm64)
cp target/release/wickra_terminal.dll      bindings/go/lib/windows_amd64/  # Windows
```

Then, with the library on the loader path, run `go test ./...` from this
directory.

## License

Dual-licensed under [MIT](https://github.com/wickra-lib/wickra-terminal/blob/main/LICENSE-MIT)
or [Apache-2.0](https://github.com/wickra-lib/wickra-terminal/blob/main/LICENSE-APACHE), at your option.
