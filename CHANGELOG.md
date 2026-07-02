# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Repository scaffolding: Cargo workspace, supply-chain configuration
  (`deny.toml`, `osv-scanner.toml`), lint configuration, `repo-metadata.toml`,
  and dual `MIT OR Apache-2.0` licensing.
- `terminal-core`: the data-driven core — the `DataSource` trait (Live, Replay,
  Synth), an O(1) `AppState` fold, panels (chart, book, tape, footprint,
  watchlist) that emit view-models, and the `Terminal` handle with the
  `command_json` boundary.
- `ui-tui`: the native TUI renderer (ratatui) with a runtime source/symbol menu.
- `web/`: the browser renderer (Vue + Vite over the WASM binding).
- Ten language bindings — Rust, Python (PyO3), Node.js (napi), WASM
  (wasm-bindgen), and the C ABI hub reaching C, C++, C#, Go, Java and R — each
  exposing the same `Terminal` + `command` + `version` surface.
- Test rigor: conformance, a byte-exact golden corpus (also the cross-language
  parity corpus), property-based invariants, four cargo-fuzz targets, and a
  criterion benchmark suite.
- One runnable example per language, a C/C++ CMake harness, and the full CI
  workflow matrix (all ten languages across three operating systems) plus
  CodeQL, Scorecard, zizmor and link checking.

[Unreleased]: https://github.com/wickra-lib/wickra-terminal/commits/main
