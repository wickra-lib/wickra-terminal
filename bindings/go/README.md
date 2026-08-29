<p align="center">
  <a href="https://wickra.org"><img src="https://raw.githubusercontent.com/wickra-lib/.github/main/profile/wickra-banner.webp?v=514" alt="Wickra Terminal — the data-driven trading terminal for Go" width="100%"></a>
</p>

[![Built on Wickra](https://img.shields.io/badge/built%20on-wickra-3b82f6)](https://github.com/wickra-lib/wickra)
[![CI](https://raw.githubusercontent.com/wickra-lib/.github/main/profile/badges/wickra-terminal/ci.svg)](https://github.com/wickra-lib/wickra-terminal/actions/workflows/ci.yml)
[![codecov](https://raw.githubusercontent.com/wickra-lib/.github/main/profile/badges/wickra-terminal/codecov.svg)](https://codecov.io/gh/wickra-lib/wickra-terminal)
[![Go module](https://raw.githubusercontent.com/wickra-lib/.github/main/profile/badges/wickra-terminal/go.svg)](https://pkg.go.dev/github.com/wickra-lib/wickra-terminal-go)
[![License: MIT OR Apache-2.0](https://raw.githubusercontent.com/wickra-lib/.github/main/profile/badges/wickra-terminal/license.svg)](https://github.com/wickra-lib/wickra-terminal#license)

# Wickra Terminal — Go

---

> **▶ Web renderer:** the same core drives a browser front-end (WASM + Vue) as a second renderer — see [`web/`](https://github.com/wickra-lib/wickra-terminal/tree/main/web).

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
ABI library for every platform, so `go get` + `go build` needs nothing else (a C
compiler is still required, as the binding uses cgo). Running what you build
needs one more step on Windows, below:

```bash
go get github.com/wickra-lib/wickra-terminal-go
```

```go
import wickra "github.com/wickra-lib/wickra-terminal-go"
```

`wickra-terminal-go` is generated from this directory by the release pipeline: it
mirrors the Go sources, the vendored C ABI header (`include/wickra_terminal.h`)
and the prebuilt libraries under `lib/<goos>_<goarch>/`.

### Windows needs one more step

Building works everywhere; **running** does not. The cgo directives carry
`-Wl,-rpath` on Linux and macOS, so the bundled library is found next to the
module at run time. Windows has no PE equivalent: the loader searches the
executable's directory and `PATH`, and the DLL is in neither.

A binary built against the module therefore starts and immediately exits with
`exit status 0xc0000135` — `STATUS_DLL_NOT_FOUND`, with no message naming the
library. Point Windows at the bundled directory, or copy the DLL next to your
executable:

```powershell
$dir = go list -m -f '{{.Dir}}' github.com/wickra-lib/wickra-terminal-go
$env:PATH = "$dir\lib\windows_amd64;$env:PATH"
```

CI cannot catch this: `ci.yml` puts the library directory on `PATH` before
running the Go tests, which is exactly the step a `go get` consumer has no reason
to take.

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

## Building from source (contributors)

This section applies to the [wickra-terminal] source repository, not to the
published module: the released module vendors the libraries and needs none of
this. It is here because the same file is the module's page on pkg.go.dev, and a
reader who arrived there should not be sent looking for directories the module
does not contain.

In a `wickra-terminal` checkout, compile the C ABI hub and stage the library into
the per-platform directory cgo links against — paths are from the repository
root:

```bash
cargo build -p wickra-terminal-c --release
mkdir -p bindings/go/lib/linux_amd64                       # match your GOOS_GOARCH
cp target/release/libwickra_terminal.so    bindings/go/lib/linux_amd64/    # Linux
cp target/release/libwickra_terminal.dylib bindings/go/lib/darwin_arm64/   # macOS (arm64)
cp target/release/wickra_terminal.dll      bindings/go/lib/windows_amd64/  # Windows
```

Then, with the library on the loader path, run `go test ./...` from
`bindings/go`.

[wickra-terminal]: https://github.com/wickra-lib/wickra-terminal

## The command protocol

Every binding drives the same thirteen commands, and the frame that comes back is
the same JSON in all of them:

| Command | Effect |
|---------|--------|
| `Tick` | Poll every source, fold what arrived, return the frame |
| `Subscribe` / `Unsubscribe` | Add or drop a market on one source |
| `SetFocus` | Choose the market the panels render |
| `AddSource` / `RemoveSource` | Attach or detach a feed at run time |
| `Seek` | Rewind or fast-forward a replay source (the time machine) |
| `Feed` | Hand an event to a `Manual` source from the host |
| `FeedDerivatives` | Fold a derivatives update -- funding, open interest, positioning, mark/index/futures -- into a market |
| `AddIndicator` / `RemoveIndicator` | Track or drop an indicator on every market |
| `SetTimeframe` | Set the bar size the candle-input indicators are fed at |
| `ListIndicators` | The catalogue: every registry name with its default parameters |

`ListIndicators` is the one command that answers rather than renders, and each
row carries `needs_reference`, which marks the pairwise indicators that compare
two markets and so require a `reference` symbol in their spec. A row for one of
the two friendly aliases also carries `alias_of` naming the canonical kind it
builds, so `Macd` and `MacdIndicator` read as one indicator rather than two.

A frame is `{"panels": [...]}`, one entry per configured panel, each tagged with
its `panel` kind — `chart`, `book`, `tape`, `watchlist`, `footprint`, `profile`, `bars`. See
[`docs/`](https://github.com/wickra-lib/wickra-terminal/tree/main/docs) for the panel and source references.

## Cross-language equality

The same config and the same command sequence produce a byte-identical frame in
Rust, Python, Node.js, WASM, C, C++, C#, Go, Java and R. That is not an aspiration:
[`golden/`](https://github.com/wickra-lib/wickra-terminal/tree/main/golden) holds a recorded feed and the expected frame,
and every binding's test suite asserts its own output against that one file.

## Documentation

- **Repository:** <https://github.com/wickra-lib/wickra-terminal>
- **Panels, sources, renderers, streaming:** [`docs/`](https://github.com/wickra-lib/wickra-terminal/tree/main/docs)
- **Cookbook:** [`docs/Cookbook.md`](https://github.com/wickra-lib/wickra-terminal/blob/main/docs/Cookbook.md)
- **Built on Wickra:** <https://github.com/wickra-lib/wickra> · <https://docs.wickra.org>

## Security

Found a security issue? **Please don't open a public issue.** Report it privately
via the repository's *Security* tab (*"Report a vulnerability"*) or email
**support@wickra.org**. Full policy: <https://github.com/wickra-lib/wickra-terminal/blob/main/SECURITY.md>.

## Disclaimer

Not a trading system, and not financial advice. The terminal renders market data
and derived view-models; what you do with them is your own risk. Provided **as
is**, without warranty of any kind.

## License

Dual-licensed under [MIT](https://github.com/wickra-lib/wickra-terminal/blob/main/LICENSE-MIT) or
[Apache-2.0](https://github.com/wickra-lib/wickra-terminal/blob/main/LICENSE-APACHE), at your option.
