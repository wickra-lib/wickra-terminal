# Roadmap

`wickra-terminal` is built out in phases, mirroring the proven structure of the
Wickra exchange and backtester repos. Each phase lands as reviewed, CI-green pull
requests. Status below is updated as phases complete.

## Phases

0. **Scaffold** — workspace, governance, supply-chain config, `.github`
   scaffolding. *In progress.*
1. **`terminal-core`** — the `DataSource` trait, `AppState` (O(1) fold),
   `Panel`/`PanelView` view-models, and the `Terminal` handle with the
   data-driven `command_json` boundary. Near-total coverage via inline tests.
2. **TUI renderer** — `crates/ui-tui`: a ratatui front-end (`wickra-terminal`
   binary) with a RAII terminal guard, feed threads, and a widget per panel.
3. **Bindings** — native Python, Node and WASM, plus the C ABI hub reaching C,
   C++, C#, Go, Java and R; each exposes the `Terminal` handle + `command` +
   `version`, with a completeness guard.
4. **Web renderer** — a Vue/Vite front-end over the WASM binding, sharing the
   core's view-models; layout persisted in `localStorage`.
5. **Module toggle + multi-symbol** — sources added/removed/hot-swapped at
   runtime, multiple sources at once, dynamic watchlist subscribe/unsubscribe.
6. **Iteration panels** — synthetic source, time-machine replay seek, and richer
   panels (footprint, radar, scanner, split-panes) — added once in the core, so
   they appear in both renderers.
7. **Hardening** — conformance suite, golden (byte-exact, cross-language),
   property tests, fuzz targets, benchmarks.
8. **ABI harness + examples** — cbindgen header sync-check and one runnable
   example per language.
9. **CI/CD** — the full workflow matrix (all languages), OpenSSF Scorecard,
   Best Practices, link check, release and web deploy.
10. **README, badges, docs** — the banner + badge treatment and the docs guides.
11. **Real execution** *(optional, USER-GO gated)* — opt-in live orders via the
    exchange layer, testnet-first, keys server-side.

## Non-goals

- **A second product.** Web and TUI are two renderers of one core, selected with
  `--render web|tui`; they are not separate applications.
- **Secrets in the browser.** The Web renderer holds no keys; real execution from
  a browser needs a backend (see the architecture docs).
- **Renderer-specific logic in the core.** Panels emit view-models, never renderer
  commands, so every front-end stays a thin view over the same state.
