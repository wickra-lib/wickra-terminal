<p align="center">
  <a href="https://wickra.org"><img src="https://raw.githubusercontent.com/wickra-lib/.github/main/profile/wickra-banner.webp?v=514" alt="Wickra Terminal — the data-driven trading terminal for WebAssembly" width="100%"></a>
</p>

[![Built on Wickra](https://img.shields.io/badge/built%20on-wickra-3b82f6)](https://github.com/wickra-lib/wickra)
[![CI](https://raw.githubusercontent.com/wickra-lib/.github/main/profile/badges/wickra-terminal/ci.svg)](https://github.com/wickra-lib/wickra-terminal/actions/workflows/ci.yml)
[![codecov](https://raw.githubusercontent.com/wickra-lib/.github/main/profile/badges/wickra-terminal/codecov.svg)](https://codecov.io/gh/wickra-lib/wickra-terminal)
[![License: MIT OR Apache-2.0](https://raw.githubusercontent.com/wickra-lib/.github/main/profile/badges/wickra-terminal/license.svg)](https://github.com/wickra-lib/wickra-terminal#license)

# Wickra Terminal — WASM

---

> **▶ Web renderer:** the same core drives a browser front-end (WASM + Vue) as a second renderer — see [`web/`](https://github.com/wickra-lib/wickra-terminal/tree/main/web).

WebAssembly bindings for the [wickra-terminal](https://github.com/wickra-lib/wickra-terminal) data-driven core, built
with wasm-bindgen. This is the binding the web renderer runs on: build a
`Terminal` from a JSON config, drive it with command JSON, read back frame
view-models — the same protocol as the native TUI and every other binding.

The native exchange client cannot run in a browser sandbox, so the core's `live`
feature is disabled here. The web renderer opens its own WebSocket and hands the
events in through the `Feed` command, which is why `Feed` matters more to this
binding than to any other.

## Requirements

- `wasm-pack` and the `wasm32-unknown-unknown` target
- A bundler or an ES-module-capable browser for the `web` target

## Build

```bash
wasm-pack build bindings/wasm --target web                        # for the browser
wasm-pack build bindings/wasm --target nodejs --out-dir pkg-node  # for the test suite
```

The `web` target is what [`web/`](https://github.com/wickra-lib/wickra-terminal/tree/main/web) imports; the `nodejs`
target is the only form `require()` can load, which is what the tests run against.

## Usage (browser)

```js
import init, { Terminal, version } from "./pkg/wickra_terminal_wasm.js";

await init();

const term = new Terminal(JSON.stringify({
  sources: [{ Synth: { seed: 1 } }],
  layout: { panels: [{ kind: "Chart", rect: { x: 0, y: 0, w: 100, h: 100 } }] },
}));

term.command(JSON.stringify({ type: "Subscribe", source: 0, symbol: "BTC/USDT" }));
const frame = JSON.parse(term.command(JSON.stringify({ type: "Tick" })));
console.log(frame.panels[0], version());
```

## API

| Name | Returns |
|------|---------|
| `new Terminal(configJson)` | A handle; throws on an invalid config |
| `term.command(cmdJson)` | The frame JSON; throws on an invalid command |
| `term.version()` | The library version |
| `version()` | The same string, as a free function |
| `term.free()` | Releases the handle |

`free()` is not optional housekeeping: wasm-bindgen hands out a pointer with no
finaliser, so a terminal dropped without it leaks its linear-memory allocation
for the lifetime of the page. Anything that rebuilds a terminal — a config
change, a source swap — must release the old one.

## Test

```bash
node --test bindings/wasm/tests/*.test.cjs
```

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
| `ListIndicators` | The catalogue: every indicator, profile and bar type this build accepts, each with its default parameters |

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
