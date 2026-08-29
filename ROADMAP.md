# Roadmap

`wickra-terminal` is built out in phases, mirroring the structure of the Wickra
exchange and backtester repos. Each phase lands as reviewed, CI-green pull
requests.

## Phases

0. **Scaffold** — workspace, governance, supply-chain config, `.github`
   scaffolding. ✅
1. **`wickra-terminal-core`** — the `DataSource` trait, `AppState` (O(1) fold),
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
11. **Indicators** — tick-to-OHLCV aggregation and a generated registry of <!--indicator-count-->475<!--/indicator-count-->
    `wickra-core` indicators, configurable and changeable at run time from every
    binding. ✅

## Next

Nothing here is scheduled. These are the openings the current shape leaves, in
roughly the order they would pay off.

- **The remaining indicator families.** 29 of the 504 in `wickra-core` are not
  reachable, and the reason is now the same for nearly all of them:
  [`docs/INDICATORS.md`](docs/INDICATORS.md) lists them. Derivatives-tick (17),
  cross-section (15) and trade-quote (3) need feeds this repository has no source
  for, so they are blocked on a data source rather than on wiring. The remaining eight
  are output shapes the registry's fixed named-field model does not carry -- the
  profile and level outputs, whose bin lists change length bar to bar.
- **First release.** Blocked, and not on a decision — on a dependency. The chain,
  in full, because it has one link and no way round it:

  `wickra-terminal-core` depends on `wickra-exchange`, pinned to a git revision.
  `wickra-exchange` is not on crates.io. `cargo publish` refuses any crate with a
  git dependency, so `wickra-terminal-core` cannot be published:

  ```text
  $ cargo publish --dry-run -p wickra-terminal-core
  error: failed to prepare local package for uploading
  Caused by:
    no matching package named `wickra-exchange` found
    location searched: crates.io index
  ```

  Everything downstream follows from that one line. `cargo-publish` is the first
  job in `release.yml`; `github-release` waits on it and on every other publish
  job; `publish-release` waits on `github-release`. So the whole pipeline is
  blocked, not merely the crates.io half — and no tag should be pushed until
  `wickra-exchange` releases, because the run would fail at its first job.

  This is recorded rather than worked around on purpose. Vendoring the dependency
  or switching it to a path dependency would let `cargo publish` succeed while
  shipping a tree that is not the one that was tested, which trades a visible
  blocker for an invisible one.

  Two things have to be in place before that tag is worth pushing, and neither is
  today. The workflow itself audits clean — every action pinned to a full SHA,
  every job with a timeout, `contents: read` at the top with write granted only
  to the two jobs that attach release assets, and no checkout leaving credentials
  on disk. What is missing is credentials and an environment: `release.yml`
  references eight repository secrets (`CARGO_REGISTRY_TOKEN`, `PYPI_API_TOKEN`,
  `NPM_TOKEN`, `GO_MIRROR_TOKEN`, `CENTRAL_USERNAME`, `CENTRAL_PASSWORD`,
  `GPG_PRIVATE_KEY`, `GPG_PASSPHRASE`) and a `release` environment, and the
  repository has none of them. Every publish job would fail on authentication
  even if the dependency chain above were resolved.

  So the pipeline has never run, and a dispatch would not tell us much: it would
  stop at its first job. It is verified structurally instead, which is what
  catches the faults a run that dies early never reaches.

  Nine badges wait on that tag rather than sitting in the README claiming
  something. Eight are the registry badges -- crates.io, PyPI, npm and the rest --
  which would render as 404s until the packages exist. The ninth was in the README
  and read "provenance: attested", against zero releases, zero tags and an
  attestations API that answers 404: the artefacts it attests to are produced by
  `release.yml`, so the claim becomes true at exactly the moment the rest do. They
  are added together, in the `.github` profile repository where the badge assets
  live, when the release actually happens.
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
