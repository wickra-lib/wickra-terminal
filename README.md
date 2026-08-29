<p align="center">
  <a href="https://wickra.org"><img src="https://raw.githubusercontent.com/wickra-lib/.github/main/profile/wickra-banner.webp?v=514" alt="Wickra — streaming-first trading terminal" width="100%"></a>
</p>

[![Built on Wickra](https://img.shields.io/badge/built%20on-wickra-3b82f6)](https://github.com/wickra-lib/wickra)
[![Status](https://img.shields.io/badge/status-pre--release-orange)](https://github.com/wickra-lib/wickra-terminal)
[![CI](https://raw.githubusercontent.com/wickra-lib/.github/main/profile/badges/wickra-terminal/ci.svg)](https://github.com/wickra-lib/wickra-terminal/actions/workflows/ci.yml)
[![CodeQL](https://raw.githubusercontent.com/wickra-lib/.github/main/profile/badges/wickra-terminal/codeql.svg)](https://github.com/wickra-lib/wickra-terminal/actions/workflows/codeql.yml)
[![codecov](https://raw.githubusercontent.com/wickra-lib/.github/main/profile/badges/wickra-terminal/codecov.svg)](https://codecov.io/gh/wickra-lib/wickra-terminal)
[![GitHub release](https://raw.githubusercontent.com/wickra-lib/.github/main/profile/badges/wickra-terminal/release.svg)](https://github.com/wickra-lib/wickra-terminal/releases/latest)
[![crates.io](https://raw.githubusercontent.com/wickra-lib/.github/main/profile/badges/wickra-terminal/crates.svg)](https://crates.io/crates/wickra-terminal)
[![PyPI](https://raw.githubusercontent.com/wickra-lib/.github/main/profile/badges/wickra-terminal/pypi.svg)](https://pypi.org/project/wickra-terminal/)
[![npm](https://raw.githubusercontent.com/wickra-lib/.github/main/profile/badges/wickra-terminal/npm.svg)](https://www.npmjs.com/package/wickra-terminal)
[![NuGet](https://raw.githubusercontent.com/wickra-lib/.github/main/profile/badges/wickra-terminal/nuget.svg)](https://www.nuget.org/packages/WickraTerminal)
[![Maven Central](https://raw.githubusercontent.com/wickra-lib/.github/main/profile/badges/wickra-terminal/maven.svg)](https://central.sonatype.com/artifact/org.wickra/wickra-terminal)
[![Go module](https://raw.githubusercontent.com/wickra-lib/.github/main/profile/badges/wickra-terminal/go.svg)](https://pkg.go.dev/github.com/wickra-lib/wickra-terminal-go)
[![R-universe](https://raw.githubusercontent.com/wickra-lib/.github/main/profile/badges/wickra-terminal/r-universe.svg)](https://wickra-lib.r-universe.dev)
[![License: MIT OR Apache-2.0](https://raw.githubusercontent.com/wickra-lib/.github/main/profile/badges/wickra-terminal/license.svg)](#license)
[![OpenSSF Scorecard](https://raw.githubusercontent.com/wickra-lib/.github/main/profile/badges/wickra-terminal/scorecard.svg)](https://scorecard.dev/viewer/?uri=github.com/wickra-lib/wickra-terminal)
[![OpenSSF Best Practices](https://raw.githubusercontent.com/wickra-lib/.github/main/profile/badges/wickra-terminal/best-practices.svg)](https://www.bestpractices.dev)
[![Build provenance](https://raw.githubusercontent.com/wickra-lib/.github/main/profile/badges/wickra-terminal/provenance.svg)](https://github.com/wickra-lib/wickra-terminal/attestations)
[![Docs](https://raw.githubusercontent.com/wickra-lib/.github/main/profile/badges/wickra-terminal/docs.svg)](https://terminal.wickra.org)
[![Verified across 10 languages](https://raw.githubusercontent.com/wickra-lib/.github/main/profile/badges/wickra-terminal/verified.svg)](golden/)
[![Live demo](https://img.shields.io/badge/live%20demo-live.wickra.org-3b82f6)](https://live.wickra.org)

---

**One core. Ten languages. Two renderers.** A streaming trading terminal built on
the [Wickra](https://github.com/wickra-lib/wickra) core — live charts, order-book,
tape and <!--indicator-count-->475<!--/indicator-count--> streaming indicators — with a native **TUI** and a **Web** front-end
as a *second renderer* of the same logic, driven by the same config.

> **▶ Live demo:** the Wickra library's own 514 indicators over real Binance market data, computed live in your browser — **[live.wickra.org](https://live.wickra.org)** · zero backend, powered by `wickra-wasm`.

> **Part of the [Wickra ecosystem](https://github.com/wickra-lib):** the same data-driven core and ten-language binding surface also power [wickra-exchange](https://github.com/wickra-lib/wickra-exchange), [wickra-backtest](https://github.com/wickra-lib/wickra-backtest), [wickra-screener](https://github.com/wickra-lib/wickra-screener) and 20 more — see [the full list](https://github.com/wickra-lib).

The heart is a single data-driven core, [`wickra-terminal-core`](crates/wickra-terminal-core):
it folds market events into an O(1) `AppState` and turns panels into
**view-models** (values, series, colours) — never renderer commands. The TUI maps
a view-model to a ratatui widget; the Web app maps the *same* view-model to a
canvas draw. One logic, N front-ends.

Data arrives through the `DataSource` trait, an activatable module:

- **`Live`** — the [wickra-exchange](https://github.com/wickra-lib/wickra-exchange)
  connectivity layer over the ten largest venues.
- **`Replay`** — a recorded feed with a time-machine seek: the whole event list
  is kept, so `Seek` rewinds and re-folds state deterministically. It reads no
  files and holds no engine, which is why it runs in the browser too.
- **`Synth`** — a deterministic synthetic feed for demos and tests.

The core is exposed as a **JSON-over-C-ABI data API** (`Terminal::command_json`)
in **Rust, Python, Node.js, WASM, C, C++, C#, Go, Java and R** — so a developer in
any language builds their own front-end on the same core.

## Why this shape

Most terminals are an application with a data layer inside. This one is a data
layer with two applications outside it, and that difference is what the repository
is actually about.

- **The renderer is not where the logic lives.** Panels emit view-models — values,
  series, rows — never draw calls. The TUI maps them to ratatui and the browser to
  a canvas and some tables, and neither can diverge in behaviour because neither
  has any.
- **The boundary is data, not an API.** A config JSON in, a command JSON in, a
  frame JSON out. That is why <!--indicator-count-->475<!--/indicator-count--> indicators became reachable from ten languages
  without a line of binding code, and why a third renderer needs no core change.
- **One core, checked across ten languages.** Not "ports that should agree" — one
  Rust core behind a C ABI, with every binding asserting the same recorded feed
  produces the same frame byte for byte.
- **O(1) per event, bounded everywhere.** State folds forward and never
  recomputes over history; the tape, the price series and each indicator's series
  are all capped. A feed that never stops does not grow the process.
- **Exact where it matters.** Prices and quantities are `Decimal` through the
  market layer, converted to `f64` only at the view-model edge, where they are
  drawn rather than compared.

## Status

**Pre-release — CI-verified, not yet published.** The core, both renderers, all
ten language bindings, the indicator registry, the runtime source/symbol toggle,
the panel set, the byte-exact golden corpus, property and fuzz tests, benchmarks
and one runnable example per language are in place and green across the full CI
matrix (10 languages x 3 OS).

Not yet on any registry: the terminal depends on `wickra-exchange` as a git
dependency and `cargo publish` rejects those, so the first release waits on that
crate. [ROADMAP.md](ROADMAP.md) has what is done, what is open and what is not
planned.

> **Read-only.** The terminal renders market data. It places no orders, holds no
> credentials and keeps no position — the live source connects to public
> endpoints with empty credentials. Execution is not a flag that is off; it is a
> layer that is not built. See [THREAT_MODEL.md](THREAT_MODEL.md).

## Documentation

- [Architecture](ARCHITECTURE.md) — the core, the renderer split, the data-driven boundary.
- [docs/INDICATORS.md](docs/INDICATORS.md) · [docs/PANELS.md](docs/PANELS.md) · [docs/SOURCES.md](docs/SOURCES.md) · [docs/RENDERERS.md](docs/RENDERERS.md) · [docs/STREAMING.md](docs/STREAMING.md) · [docs/Cookbook.md](docs/Cookbook.md).
- [ROADMAP.md](ROADMAP.md) · [BENCHMARKS.md](BENCHMARKS.md) · [SECURITY.md](SECURITY.md).

## Quickstart

```bash
# Native TUI renderer over a live Binance feed:
cargo run -p wickra-terminal -- --source live:binance:BTC/USDT

# Or a deterministic synthetic feed (no network):
cargo run -p wickra-terminal -- --source synth:1

# Or replay a recorded feed. `replay:` takes the JSON itself, not a path, so
# a recorded file is passed through the shell:
cargo run -p wickra-terminal -- --source "replay:$(cat golden/replay/basic.json)"

# Anything beyond one source -- panels, indicators, timeframe -- comes from a
# config file, which overrides `--source`:
cargo run -p wickra-terminal -- --config my-terminal.toml
```

The three `--source` shorthands are `synth:<seed>`, `live:<venue>:<BASE/QUOTE>`
and `replay:<json>` — the last one taking the recorded events inline rather than
a filename. See [docs/SOURCES.md](docs/SOURCES.md) for what each one does and
[docs/Cookbook.md](docs/Cookbook.md) for worked config files.

## Renderers

| Renderer | Where | How |
|---|---|---|
| **TUI** | native terminal | `crates/ui-tui` (ratatui), `cargo run -p wickra-terminal` |
| **Web** | browser | `web/` (Vue) over `bindings/wasm`, `cd web && npm run dev` |

Both consume the identical `Frame` of view-models from `wickra-terminal-core`.

The Web row has a prerequisite the TUI row does not: `web/` depends on
`file:../bindings/wasm/pkg`, which `wasm-pack` produces and no clone contains, so
`npm install` fails until it has been built once.

```bash
( cd bindings/wasm && wasm-pack build --target web )
cd web && npm install && npm run dev
```

## Install

> **Pre-release.** Nothing is published yet — the terminal depends on
> `wickra-exchange` as a git dependency and `cargo publish` rejects those, so the
> first release waits on that crate. Until then, build from source (below). The
> commands here are what they will be.

| Binding | Install | Example |
|---------|---------|---------|
| Rust (TUI binary) | `cargo install wickra-terminal` | `examples/rust/src/main.rs` |
| Python (PyO3) | `pip install wickra-terminal` | `examples/python/synth_terminal.py` |
| Node.js (napi-rs) | `npm install wickra-terminal` | `examples/node/synth_terminal.js` |
| Browser / WASM | `npm install wickra-terminal-wasm` | [`web/`](web) — the Vue renderer |
| C / C++ (C ABI) | header + library, see [`bindings/c`](bindings/c) | `examples/c/synth.c` |
| C# (C ABI) | `dotnet add package WickraTerminal` | `examples/csharp/Program.cs` |
| Go (cgo, C ABI) | `go get github.com/wickra-lib/wickra-terminal-go` | `examples/go/synth_terminal.go` |
| Java (FFM, C ABI) | Maven Central `org.wickra:wickra-terminal` | `examples/java/SynthTerminal.java` |
| R (`.Call`, C ABI) | `install.packages("wickraterminal", repos = "https://wickra-lib.r-universe.dev")` | `examples/r/synth_terminal.R` |

[`examples/README.md`](examples/README.md) is the cross-language index; every one
of them drives the same core through the same JSON commands.

C and C++ link the C ABI directly; the header is generated and committed at
[`bindings/c/include/wickra_terminal.h`](bindings/c/include/wickra_terminal.h).

## Use in any language

The same `Terminal` handle — construct from a JSON config, drive with
`command(json) -> json`, read `version` — is reachable from every binding:

```python
import json
from wickra_terminal import Terminal

term = Terminal(json.dumps({"sources": [{"Synth": {"seed": 1}}]}))
term.command(json.dumps({"type": "Subscribe", "source": 0, "symbol": "BTC/USDT"}))
frame = json.loads(term.command(json.dumps({"type": "Tick"})))
print(frame["panels"][0])          # the chart panel's view-model
```

## Performance

The frame budget is dominated by the terminal's own CPU work — folding events and
building view-models — not by rendering. Criterion medians on a Windows x86-64
laptop, single-threaded, with the default two-indicator overlay:

| Path | Median | Throughput |
|------|--------|------------|
| Fold one trade into state | 142 ns | ~7.0 M/s |
| Apply an L2 depth diff | 107 ns | ~9.3 M/s |
| Build all five panels' view-models | 8.9 µs | ~113 K/s |
| One full tick (poll + fold + build) | 9.7 µs | ~103 K/s |
| The same tick across the FFI boundary | 17.7 µs | ~56 K/s |

A full tick costs about ten microseconds, so the core sustains a hundred thousand
of frames per second — far above any renderer's budget, which is the point of the
O(1) fold. The indicator count is a direct multiplier: those numbers are two
indicators, and the registry offers <!--indicator-count-->475<!--/indicator-count-->. See [BENCHMARKS.md](BENCHMARKS.md).

## Project layout

```
crates/wickra-terminal-core   the data-driven core (DataSource, AppState, panels → view-models)
crates/ui-tui          the native TUI renderer (bin: wickra-terminal)
crates/wickra-terminal-bench  criterion benchmarks
bindings/{python,node,wasm,c,go,csharp,java,r}   the ten-language surface
web/                   the Vue/Vite Web renderer over the WASM binding
golden/                recorded feeds + byte-exact expected frames (cross-language parity)
fuzz/                  cargo-fuzz targets (feed_event, state_fold, view_model, config_parse)
examples/              one runnable example per language
docs/                  indicators, panels, sources, renderers, streaming, cookbook
```

## Building from source

```bash
# Rust core + tests + lints
cargo build --workspace
cargo test  --workspace --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo run -p wickra-terminal -- --source synth:1

# Python binding (requires a Rust toolchain + maturin)
( cd bindings/python && maturin develop --release ) && pytest bindings/python/tests -q

# Node binding (requires @napi-rs/cli)
( cd bindings/node && npm install && npm run build && npm test )

# WASM binding (requires wasm-pack) — two targets: `web` for the renderer,
# `nodejs` because it is the only form `require()` loads
wasm-pack build bindings/wasm --target web
wasm-pack build bindings/wasm --target nodejs --out-dir pkg-node
node --test bindings/wasm/tests/*.test.cjs

# C ABI (cdylib + staticlib + the generated header)
cargo build -p wickra-terminal-c --release
.github/scripts/check-cbindgen.sh            # header still in sync?

# C# binding (requires the .NET 8 SDK; links the C ABI above)
dotnet test bindings/csharp/WickraTerminal.Tests

# Go binding (requires a C compiler for cgo; links the C ABI above)
( cd bindings/go && go test ./... )

# Java binding (requires JDK 22+ and Maven; links the C ABI above)
mvn -f bindings/java/pom.xml test

# R binding (requires a C toolchain / Rtools; links the C ABI above)
R CMD INSTALL bindings/r && Rscript bindings/r/tests/run_tests.R

# Web renderer (requires the `web` wasm target above)
( cd web && npm install && npm test && npm run build )
```

## Testing

Every language asserts its own output against [`golden/`](golden/) — one
recorded feed and the frame it must produce, compared byte for byte. Rust,
Python, Node, WASM, Go, C# , Java and R each run it in their own suite; C and C++
run it through the C ABI under ctest. Ten languages, one file: that is what makes
"the same core everywhere" checkable rather than asserted.

Alongside it: property tests over the fold invariants, four `cargo-fuzz` targets
across the parsing paths, a conformance suite pinning the trait shapes, a
registry suite that constructs and drives all <!--indicator-count-->475<!--/indicator-count--> indicators, and a test that
extracts every example from this README and the docs and runs it.

```bash
cargo test --workspace --all-features
python scripts/check_binding_surface.py     # every binding matches the C ABI header
```

## Requirements

- **Rust** ≥ 1.88 to build the `wickra-terminal` TUI (ratatui pulls
  `instability`/`darling`), and to build the Node binding. The library crate
  `wickra-terminal-core` keeps the workspace MSRV of ≥ 1.86.
- Renderer/binding toolchains as needed: Node ≥ 22, Python ≥ 3.9, a C toolchain,
  .NET 8, JDK 22+, Go 1.23, R — see each `bindings/<lang>/README.md`.

## Ecosystem

Part of the [Wickra](https://github.com/wickra-lib/wickra) family — each one a
data-driven core with a CLI and the same ten-language binding surface:

- [**wickra**](https://github.com/wickra-lib/wickra) — main library (Rust core + Python / Node.js / WASM bindings + a C ABI for C / C++ / C# / Go / Java / R)
- [**wickra-playground**](https://github.com/wickra-lib/wickra-playground) — a polyglot strategy playground: one StrategySpec live side by side in Python, Rust, JS and Go, entirely in the browser
- [**wickra-exchange**](https://github.com/wickra-lib/wickra-exchange) — unified market-data + execution across ten crypto exchanges
- [**wickra-backtest**](https://github.com/wickra-lib/wickra-backtest) — event-driven backtester over the Wickra core
- [**wickra-screener**](https://github.com/wickra-lib/wickra-screener) — parallel multi-symbol screening over 514 streaming indicators
- [**wickra-xray**](https://github.com/wickra-lib/wickra-xray) — market-microstructure explorer: footprint, order-book heatmap, liquidation map, funding/OI divergence
- [**wickra-radar**](https://github.com/wickra-lib/wickra-radar) — perp-universe alert radar: OI delta, funding flip, book imbalance, liquidation clusters, OI/price divergence
- [**wickra-copilot**](https://github.com/wickra-lib/wickra-copilot) — local market copilot grounded in real order-book, liquidation and funding microstructure
- [**wickra-shazam**](https://github.com/wickra-lib/wickra-shazam) — match an asset's current microstructure fingerprint against its entire history
- [**wickra-benchmark**](https://github.com/wickra-lib/wickra-benchmark) — reproducible, golden-verified benchmark suite — recompute any (strategy, dataset, report) in ten languages and confirm it byte-for-byte
- [**wickra-strategy-ci**](https://github.com/wickra-lib/wickra-strategy-ci) — Jest for trading strategies: golden-pin the report, catch regressions in CI, property-test against fuzzed data
- [**wickra-verify**](https://github.com/wickra-lib/wickra-verify) — confirm or refute a claimed backtest report against its strategy and data, in ten languages
- [**wickra-proof**](https://github.com/wickra-lib/wickra-proof) — Proof-of-Backtest: deterministic (spec, data) → report + blake3 hash, recomputable byte-for-byte in ten languages
- [**wickra-zk**](https://github.com/wickra-lib/wickra-zk) — prove a backtest zero-knowledge — on-chain-verifiable performance without revealing the data or the strategy
- [**wickra-impact**](https://github.com/wickra-lib/wickra-impact) — the backtester that knows you would have moved the market: agent-based fills on the real historical L2 order book
- [**wickra-darwin**](https://github.com/wickra-lib/wickra-darwin) — evolutionary strategy search at millions of backtests per second, mutating and crossing JSON specs across the 514-indicator space
- [**wickra-gym**](https://github.com/wickra-lib/wickra-gym) — a Gymnasium-compatible, microstructure-aware backtest environment with O(1) steps for deterministic RL rollouts
- [**wickra-feature-store**](https://github.com/wickra-lib/wickra-feature-store) — OHLCV and microstructure streams into ML-ready feature matrices over 514 O(1) streaming indicators
- [**wickra-genome**](https://github.com/wickra-lib/wickra-genome) — a vector database of the whole market: every asset a 514-dim live vector, for similarity search, clustering and anomaly detection
- [**wickra-timemachine**](https://github.com/wickra-lib/wickra-timemachine) — scrub the whole market like a video — every symbol, full order book, rewound to any moment via deterministic re-fold
- [**wickra-synth**](https://github.com/wickra-lib/wickra-synth) — deterministic synthetic market microstructure: OHLCV, order book, trades and funding from a single seed
- [**wickra-compile**](https://github.com/wickra-lib/wickra-compile) — compile a strategy spec into a standalone deployable: a WASM module, a self-contained binary, or a `no_std` artifact
- [**wickra-embed**](https://github.com/wickra-lib/wickra-embed) — allocation-free, `no_std` streaming indicators for bare-metal and HFT, byte-for-byte identical to the core
- [**wickra-pico**](https://github.com/wickra-lib/wickra-pico) — the O(1) indicator core running bare-metal on a $5 Raspberry Pi Pico — the LED blinks on the EMA cross

Docs at [docs.wickra.org](https://docs.wickra.org); the marketing site and
in-browser demo at [wickra.org](https://wickra.org).

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) and [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md).
Commits are signed and in English; open a PR against `main`.

## Security

See [SECURITY.md](SECURITY.md) and [THREAT_MODEL.md](THREAT_MODEL.md). Report
vulnerabilities privately — never in a public issue.

## License

Dual-licensed under either [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE), at
your option.

## Disclaimer

This software is provided "as is", without warranty of any kind. It is a research
and engineering tool, **not financial advice**. Trading carries risk of loss. Run
in paper mode and against exchange testnets, and review the code before risking
real capital.

---

<p align="center">
  <a href="https://github.com/wickra-lib/wickra-terminal">
    <img alt="GitHub stars" src="https://raw.githubusercontent.com/wickra-lib/.github/main/profile/badges/wickra-terminal/stars.svg">
  </a>
  <a href="https://github.com/wickra-lib/wickra-terminal/network/members">
    <img alt="GitHub forks" src="https://raw.githubusercontent.com/wickra-lib/.github/main/profile/badges/wickra-terminal/forks.svg">
  </a>
  <a href="https://github.com/wickra-lib/wickra-terminal/issues">
    <img alt="GitHub issues" src="https://raw.githubusercontent.com/wickra-lib/.github/main/profile/badges/wickra-terminal/issues.svg">
  </a>
</p>

<p align="center">
  Built on <a href="https://github.com/wickra-lib/wickra">Wickra</a>. If it saved you time, the cheapest way to say thanks is to ⭐ the repo.
</p>

<p align="center">
  <img alt="wickra-terminal star history" width="640"
       src="https://raw.githubusercontent.com/wickra-lib/.github/main/profile/badges/wickra-terminal/star-history.svg">
</p>
