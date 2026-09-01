# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Repository scaffolding: Cargo workspace, supply-chain configuration
  (`deny.toml`, `osv-scanner.toml`), lint configuration, `repo-metadata.toml`,
  and dual `MIT OR Apache-2.0` licensing.
- `wickra-terminal-core`: the data-driven core — the `DataSource` trait (Live, Replay,
  Synth, Manual), an O(1) `AppState` fold, panels (chart, book, tape, footprint,
  watchlist) that emit view-models, and the `Terminal` handle with the
  `command_json` boundary.
- Host-fed sources: a `Manual` source plus the `Feed` command let a host push
  events into the core. The web renderer uses this to bridge a Binance market
  WebSocket into the WASM core (which cannot open native sockets).
- Time-machine: the `Seek` command rewinds a `Replay` source to a recorded
  position and deterministically re-folds state, so every binding and both
  renderers can scrub through a recorded feed.
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
- Indicators: **497 of the wickra-core set**, constructible by name from a config
  or at run time, across nine input families — price, bar, tape, order book,
  pairwise, returns, cross-section, derivatives and trade-against-quote.
  `tools/gen_registry.py` reads the library's sources and emits the registry, so
  it cannot drift from what wickra actually offers; whatever it cannot reach is
  printed with a reason on every regeneration rather than dropped in silence.
- Profiles: the six indicators whose output is a histogram get a surface of their
  own rather than a registry entry. A registry entry promises one number plus
  named fields, and a distribution over price levels is neither — squeezing one
  in meant reporting a single bin under the whole indicator's name.
- Alternative bars: the ten bar builders — Renko, Kagi, line-break, range,
  point-and-figure, tick, volume, dollar, imbalance and run — get a third
  surface. They are not indicators, and one closed candle completes zero, one or
  several of them; that unevenness is the character of the chart, not a defect.
- With the `footprint` panel, which renders the one indicator none of the three
  surfaces fit, that is **514** — every indicator and bar builder wickra ships.
- Derivatives: the `Feed` command accepts funding and open-interest updates, and
  the taker flow is folded out of the terminal's own tape rather than taken on
  faith from a feed that does not carry it.
- Tick-to-OHLCV aggregation (`CandleBuilder`, `Timeframe`), which is what makes
  the 256 bar-input indicators reachable at all. Only closed bars reach an
  indicator, so a reading never repaints as its bar fills.
- The tape and book indicator families read the book and print stream the
  terminal already keeps, converted into wickra-core's `Trade` and `OrderBook`
  once per tick and shared across the set. The book conversion is skipped
  entirely when no indicator in the set reads it.
- The pairwise family compares two markets. `IndicatorSpec` gains a `reference`
  symbol, which is part of the indicator's label because the same indicator
  against a different market is a different reading. A pairwise kind with no
  reference is refused rather than defaulted, and `ListIndicators` marks the
  rows that need one.
- Commands `AddIndicator`, `RemoveIndicator`, `ListIndicators` and
  `SetTimeframe`. The registry is reachable from every binding with no binding
  code, because the surface was already JSON in, JSON out.
- Multi-output indicators report their named fields, and every indicator carries
  a bounded series so a renderer can draw it as a line rather than a number. Both
  are omitted from the JSON when empty, so the single-output shape is unchanged.
- The web renderer draws indicator overlays on the price canvas, and places its
  panels from the config's layout rects like the TUI always has.
- `tab` and `backtab` move panel focus in the TUI, with a highlighted border.
- Tests: a WASM suite (the one binding that had none), a per-language registry
  suite, a config round-trip suite, guards against the registry silently
  shrinking, a web suite over the pure mappings, and a test that extracts every
  example from the README and docs and runs it.
- `binding-surface` and `examples-smoke` CI jobs, plus a C golden-parity example
  so the C ABI hub is held to the same corpus as the eight languages on it.
- Python type stubs and the PEP 561 marker.
- `docs/INDICATORS.md`.
- `LICENSES/MIT.txt` and `LICENSES/Apache-2.0.txt` — the REUSE-conformant second
  copy that licence tooling looks for, alongside the root full texts.
- `docs/README.md`: the signpost to the per-binding reference at
  terminal.wickra.org, and the reason the six guides beside the code stay there —
  each describes a contract that changes in the same commit as the code it
  documents.
- Five further issue templates (detailed bug report, detailed feature request,
  performance regression, documentation, question) and a long-form pull-request
  template for changes that touch the core contract, several bindings at once or
  the release pipeline.
- `actionlint.yml`: the workflow files are now checked for whether they work at
  all -- unknown contexts, invalid `needs`, dead matrix keys -- and, through the
  bundled shellcheck, so is the shell inside every `run:` block. zizmor reads
  the same files for security; nothing read them for correctness.
- `codspeed.yml`: instruction counts on every pull request, so a performance
  regression is reported when it lands rather than found weeks later by someone
  re-measuring by hand.
- `.github/codeql/codeql-config.yml`, and CodeQL now analyses Go, C/C++, C# and
  Java as well -- the four bindings that hold a native handle or hand a pointer
  to C, and so the only four where a memory mistake is possible at all. All four
  are compiled rather than read as source, because without a build no call
  resolves and the analysis is reported as low quality.
- An `osv-scanner` step in the supply-chain job. `osv-scanner.toml` recorded
  advisories assessed as not affecting this project and no workflow ever read
  it; cargo-deny covers the Rust graph only, and npm, PyPI, Maven, NuGet, Go and
  R had no vulnerability scanning at all.
- A `semver` job holding the two published crates to their public surface. It
  probes the crates.io index first: with no release yet there is no baseline, so
  it says so and passes, and becomes a real gate with the first publish.
- A dependabot entry for `/fuzz`. The fuzz targets are their own cargo
  workspace, so the root entry never reached `fuzz/Cargo.lock`.
- `timeout-minutes` on every job of `codeql`, `links`, `scorecard`,
  `sync-metadata` and `zizmor`, which were unbounded and could hang for
  GitHub's six-hour default.

### Changed

- `CITATION.cff` names the maintainer address the other repositories carry.
- The citation guard now checks the pairing rather than one half of it.
  `version` and `date-released` are what GitHub's citation box and Zenodo
  present as the thing being cited: while the changelog shows no released
  section both keys must be absent, and the moment one is cut both must be
  present and agree with it. Cutting a release now fails until the citation is
  brought along, instead of shipping one that dates nothing.
- README section headings follow the fixed order the repository blueprint sets:
  `## Performance` is now `## Benchmarks`, and `## Building from source` is now
  `## Building everything from source`.
- `ci.yml` builds pull requests against `main` only. It had no branch filter, so
  a pull request against any branch started the full matrix.
- `criterion` in `wickra-terminal-bench` resolves to `codspeed-criterion-compat`.
  Outside a CodSpeed runner it behaves exactly as criterion does; inside one it
  is the difference between the benches reporting and running silently.

- `wickra-core` tracked from `1`; the `wickra-exchange` git dependencies are
  pinned to an explicit `rev`.
- The synthetic source advances its clock one second per poll rather than one
  millisecond. At the default one-minute timeframe a bar previously closed once
  every sixty thousand polls, leaving every bar-input indicator permanently
  warming up on the default demo source.
- The `--render` flag is gone. `--render web` printed an instruction and exited;
  the two renderers are separate programs over one core, which is what the
  documentation now says.
- Coverage measures both product crates, and the upload is guarded on the token
  being present rather than reporting success after uploading nothing.
- Every binding README, the main README, ARCHITECTURE, ROADMAP, SECURITY,
  THREAT_MODEL, BENCHMARKS and the docs guides now describe the code that exists.
  Removed claims include a `PaperExchange`, order execution, server-side keys, a
  `Replay` source backed by the backtester, a `set-timeframe` command that was
  not one, and an indicator count of 514 against a core that wired two.

### Fixed

- The README's Python example used a command shape the deserialiser rejects and
  ticked without subscribing; the cookbook's TOML was not valid TOML; and
  `docs/SOURCES.md` fed a source that cannot be fed. All three are now executed
  by a test.
- Two benchmarks the documentation described did not exist, and the published
  figures predated the indicator work.
- An indicator added before a market was subscribed never reached it, and
  rewinding a replay reset every market to the default overlay.
- The C# example could not load its native library on macOS.

### Security

- `Swatinem/rust-cache` re-pinned to a commit a tag still points at.
- cargo-deny scans the feature-expanded graph, so the `live` tree is checked at
  all; the dead `RUSTSEC-2024-0436` suppression was removed from both files.
- The gated live-integration test fails on no data instead of passing, so the
  nightly job can report a real result.

[Unreleased]: https://github.com/wickra-lib/wickra-terminal/commits/main
