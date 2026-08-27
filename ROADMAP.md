# Roadmap

`wickra-terminal` is built out in phases, mirroring the structure of the Wickra
exchange and backtester repos. Each phase lands as reviewed, CI-green pull
requests.

## Phases

0. **Scaffold** — workspace, governance, supply-chain config, `.github`
   scaffolding. ✅
1. **`terminal-core`** — the `DataSource` trait, `AppState` (O(1) fold),
   `Panel`/`PanelView` view-models, and the `Terminal` handle with the
   data-driven `command_json` boundary. ✅
2. **TUI renderer** — `crates/ui-tui`: a ratatui front-end (`wickra-terminal`
   binary) with a RAII terminal guard and a widget per panel. ✅
3. **Bindings** — native Python, Node and WASM, plus the C ABI hub reaching C,
   C++, C#, Go, Java and R; each exposes the `Terminal` handle + `command` +
   `version`, held to the header by a surface-parity check. ✅
4. **Web renderer** — a Vue/Vite front-end over the WASM binding, sharing the
   core's view-models and reading the same layout from the same config. ✅
5. **Module toggle + multi-symbol** — sources added, removed and hot-swapped at
   runtime, multiple sources at once, dynamic watchlist subscribe/unsubscribe. ✅
6. **Iteration panels** — synthetic source, time-machine replay seek, and the
   panel set (chart, book, tape, watchlist, footprint) — added once in the core,
   so they appear in both renderers. ✅
7. **Hardening** — conformance suite, golden corpus (byte-exact,
   cross-language), property tests, fuzz targets, benchmarks. ✅
8. **ABI harness + examples** — cbindgen header sync-check and one runnable
   example per language. ✅
9. **CI/CD** — the full workflow matrix (all languages), OpenSSF Scorecard,
   Best Practices, link check, release and web deploy. ✅
10. **README, badges, docs** — the banner and badge treatment, and the docs
    guides. ✅
11. **Indicators** — tick-to-OHLCV aggregation and a generated registry of 421
    `wickra-core` indicators, configurable and changeable at run time from every
    binding. ✅

## Next

Nothing here is scheduled. These are the openings the current shape leaves, in
roughly the order they would pay off.

- **The remaining indicator families.** 83 of the 504 in `wickra-core` are not
  reachable yet, and the reasons differ:
  [`docs/INDICATORS.md`](docs/INDICATORS.md) lists them. Order-book (7) and trade
  (9) indicators need only a conversion from the book and tape this terminal
  already holds; pairwise (24) needs a reference symbol to be configurable. The
  other three families need feeds this repository has no source for.
- **First release.** Blocked on `wickra-exchange` reaching crates.io: it is a git
  dependency, and `cargo publish` rejects those.
- **Panel-local keys.** Panel focus moves and is drawn; nothing acts on it yet —
  scrolling the tape or the book would be the first use.

## Not planned

- **Real execution.** The terminal reads. There is no simulator, no position, no
  order path and no credential handling, and adding any of it would change the
  threat model before it changed the code — see
  [THREAT_MODEL.md](THREAT_MODEL.md). This is not a gated feature; it is a
  different product decision, and it has not been made.
- **A second product.** Web and TUI are two renderers of one core. They are
  separate programs — there is no renderer flag, and there was never a coherent
  one — but they share the state, the view-models and the config.
- **Secrets in the browser.** The browser cannot hold one. If execution is ever
  added, that side needs a backend regardless.
- **Renderer-specific logic in the core.** Panels emit view-models, never
  renderer commands, so every front-end stays a thin view over the same state.
