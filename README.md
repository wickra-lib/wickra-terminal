<p align="center">
  <a href="https://wickra.org"><img src="https://raw.githubusercontent.com/wickra-lib/.github/main/profile/wickra-banner.webp?v=514" alt="Wickra — streaming-first trading terminal" width="100%"></a>
</p>

[![Built on Wickra](https://img.shields.io/badge/built%20on-wickra-3b82f6)](https://github.com/wickra-lib/wickra)
[![Status](https://img.shields.io/badge/status-pre--release-orange)](https://github.com/wickra-lib/wickra-terminal)
[![CI](https://raw.githubusercontent.com/wickra-lib/.github/main/profile/badges/wickra-terminal/ci.svg)](https://github.com/wickra-lib/wickra-terminal/actions/workflows/ci.yml)
[![CodeQL](https://raw.githubusercontent.com/wickra-lib/.github/main/profile/badges/wickra-terminal/codeql.svg)](https://github.com/wickra-lib/wickra-terminal/actions/workflows/codeql.yml)
[![codecov](https://raw.githubusercontent.com/wickra-lib/.github/main/profile/badges/wickra-terminal/codecov.svg)](https://codecov.io/gh/wickra-lib/wickra-terminal)
[![License: MIT OR Apache-2.0](https://raw.githubusercontent.com/wickra-lib/.github/main/profile/badges/wickra-terminal/license.svg)](#license)
[![OpenSSF Scorecard](https://raw.githubusercontent.com/wickra-lib/.github/main/profile/badges/wickra-terminal/scorecard.svg)](https://scorecard.dev/viewer/?uri=github.com/wickra-lib/wickra-terminal)
[![OpenSSF Best Practices](https://raw.githubusercontent.com/wickra-lib/.github/main/profile/badges/wickra-terminal/best-practices.svg)](https://www.bestpractices.dev/)
[![Build provenance](https://raw.githubusercontent.com/wickra-lib/.github/main/profile/badges/wickra-terminal/provenance.svg)](https://github.com/wickra-lib/wickra-terminal/attestations)
[![Docs](https://raw.githubusercontent.com/wickra-lib/.github/main/profile/badges/wickra-terminal/docs.svg)](https://wickra.org)
[![Live demo](https://img.shields.io/badge/live%20demo-live.wickra.org-3b82f6)](https://live.wickra.org)

---

**One core. Ten languages. Two renderers.** A streaming trading terminal built on
the [Wickra](https://github.com/wickra-lib/wickra) core — live charts, order-book,
tape and 514 streaming indicators — with a native **TUI** and a **Web** front-end
as a *selectable renderer* of the same logic (`--render tui|web`).

> **▶ Live demo:** all 514 indicators over real Binance market data, computed live in your browser — **[live.wickra.org](https://live.wickra.org)** · zero backend, powered by `wickra-wasm`.

> **Part of the [Wickra ecosystem](https://github.com/wickra-lib):** the same data-driven core and ten-language binding surface also power [wickra-exchange](https://github.com/wickra-lib/wickra-exchange), [wickra-backtest](https://github.com/wickra-lib/wickra-backtest), [wickra-terminal](https://github.com/wickra-lib/wickra-terminal), [wickra-screener](https://github.com/wickra-lib/wickra-screener), [wickra-xray](https://github.com/wickra-lib/wickra-xray), [wickra-radar](https://github.com/wickra-lib/wickra-radar), [wickra-copilot](https://github.com/wickra-lib/wickra-copilot) and [wickra-shazam](https://github.com/wickra-lib/wickra-shazam).

The heart is a single data-driven core, [`terminal-core`](crates/terminal-core):
it folds market events into an O(1) `AppState` and turns panels into
**view-models** (values, series, colours) — never renderer commands. The TUI maps
a view-model to a ratatui widget; the Web app maps the *same* view-model to a
canvas draw. One logic, N front-ends.

Data arrives through the `DataSource` trait, an activatable module:

- **`Live`** — the [wickra-exchange](https://github.com/wickra-lib/wickra-exchange)
  connectivity layer over the ten largest venues.
- **`Replay`** — the [wickra-backtest](https://github.com/wickra-lib/wickra-backtest)
  engine, driving a recorded feed with a time-machine seek.
- **`Synth`** — a deterministic synthetic feed for demos and tests.

The core is exposed as a **JSON-over-C-ABI data API** (`Terminal::command_json`)
in **Rust, Python, Node.js, WASM, C, C++, C#, Go, Java and R** — so a developer in
any language builds their own front-end on the same core.

## Status

**Pre-release — functionally complete, CI-verified, not yet published.** The core,
both renderers (TUI + Web), all ten language bindings, the runtime source/symbol
toggle, the panel set, the byte-exact golden corpus, property + fuzz tests,
benchmarks and one runnable example per language are in place and green across
the full CI matrix (10 languages × 3 OS). Not yet released to any registry —
track progress in [ROADMAP.md](ROADMAP.md).

> ⚠️ **Real orders move real money.** Live execution is opt-in, testnet-first, and
> keys stay server-side (native renderer only). Both renderers default to
> read-only / paper mode. See [THREAT_MODEL.md](THREAT_MODEL.md).

## Documentation

- [Architecture](ARCHITECTURE.md) — the core, the renderer split, the data-driven boundary.
- [docs/PANELS.md](docs/PANELS.md) · [docs/SOURCES.md](docs/SOURCES.md) · [docs/RENDERERS.md](docs/RENDERERS.md) · [docs/STREAMING.md](docs/STREAMING.md) · [docs/Cookbook.md](docs/Cookbook.md).
- [ROADMAP.md](ROADMAP.md) · [BENCHMARKS.md](BENCHMARKS.md) · [SECURITY.md](SECURITY.md).

## Quickstart

```bash
# Native TUI renderer over a live Binance feed:
cargo run -p wickra-terminal -- --render tui --source live:binance:BTC/USDT

# Or a deterministic synthetic feed (no network):
cargo run -p wickra-terminal -- --render tui --source synth:1
```

## Renderers

| Renderer | Where | How |
|---|---|---|
| **TUI** | native terminal | `crates/ui-tui` (ratatui), `--render tui` |
| **Web** | browser | `web/` (Vue) over `bindings/wasm`, `--render web` |

Both consume the identical `Frame` of view-models from `terminal-core`.

## Use in any language

The same `Terminal` handle — construct from a JSON config, drive with
`command(json) -> json`, read `version` — is reachable from every binding:

```python
from wickra_terminal import Terminal
t = Terminal('{"sources":[{"Synth":{"seed":1}}]}')
frame = t.command('{"Tick":{}}')   # JSON frame of panel view-models
```

## Project layout

```
crates/terminal-core   the data-driven core (DataSource, AppState, panels → view-models)
crates/ui-tui          the native TUI renderer (bin: wickra-terminal)
crates/terminal-bench  criterion benchmarks
bindings/{python,node,wasm,c,go,csharp,java,r}   the ten-language surface
web/                   the Vue/Vite Web renderer over the WASM binding
golden/                recorded feeds + byte-exact expected frames (cross-language parity)
fuzz/                  cargo-fuzz targets (feed_event, state_fold, view_model, config_parse)
examples/              one runnable example per language
docs/                  architecture, panels, sources, renderers, streaming, cookbook
```

## Building from source

```bash
cargo build --workspace
cargo test  --workspace --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo run -p wickra-terminal -- --render tui --source synth:1
```

## Requirements

- **Rust** ≥ 1.86 (workspace MSRV; the Node binding needs ≥ 1.88).
- Renderer/binding toolchains as needed: Node ≥ 22, Python ≥ 3.9, a C toolchain,
  .NET 8, JDK 22+, Go 1.23, R — see each `bindings/<lang>/README.md`.

## Ecosystem

Part of the [Wickra](https://github.com/wickra-lib/wickra) family — each one a
data-driven core with a CLI and the same ten-language binding surface:

- [**wickra**](https://github.com/wickra-lib/wickra) — the core library: 514 O(1) streaming indicators across ten languages
- [**wickra-exchange**](https://github.com/wickra-lib/wickra-exchange) — unified market-data + execution across ten crypto exchanges
- [**wickra-backtest**](https://github.com/wickra-lib/wickra-backtest) — event-driven backtester over the Wickra core
- [**wickra-terminal**](https://github.com/wickra-lib/wickra-terminal) — the trading terminal: a TUI and a browser renderer over the stack
- [**wickra-screener**](https://github.com/wickra-lib/wickra-screener) — parallel multi-symbol screening over 514 streaming indicators
- [**wickra-xray**](https://github.com/wickra-lib/wickra-xray) — market-microstructure explorer: footprint, order-book heatmap, liquidation map, funding/OI divergence
- [**wickra-radar**](https://github.com/wickra-lib/wickra-radar) — perp-universe alert radar: OI delta, funding flip, book imbalance, liquidation clusters, OI/price divergence
- [**wickra-copilot**](https://github.com/wickra-lib/wickra-copilot) — local market copilot grounded in real order-book, liquidation and funding microstructure
- [**wickra-shazam**](https://github.com/wickra-lib/wickra-shazam) — match an asset's current microstructure fingerprint against its entire history

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
