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
11. **Indicators** — tick-to-OHLCV aggregation and a generated registry of <!--indicator-count-->497<!--/indicator-count-->
    `wickra-core` indicators, configurable and changeable at run time from every
    binding. ✅
12. **The renderers catch up with the core** — candles with a price scale in both
    front-ends, all seven panels in both, panel depth in the config and
    panel-local scrolling, one keymap read by both, a recorder feeding the
    time-machine, and a live subscription that starts with the venue's history. ✅

    This entry also claimed *every command the boundary carries bound to a key or
    a control*, and that was not true when it was written. `SetRecording` was
    bound in neither front-end, so the recorder could only be armed by a config
    field read once at start-up; `RemoveSource` and `ExportRecording` were
    answered by the terminal and dropped into the browser's catch-all. The
    sentence is not repaired by deleting it — the claim is worth making, so it is
    made where a build can check it instead:
    `docs_examples::every_bound_action_reaches_a_renderer` fails if a bound
    action is answered by neither renderer, and requires the browser's
    deliberate refusals to be written down rather than read out of its silence.

    The same entry said *a live subscription that starts with the venue's
    history* while that was true of the native source alone. The browser fed a
    `Manual` source, which has no backfill, and it did not reconnect, subscribe a
    ticker or guard its parse either. It does all four now, and
    `docs/RENDERERS.md` records what is still genuinely different between the two
    — the browser bridge speaks one venue's dialect where the native source
    reaches ten.

13. **Panels at run time** — `AddPanel`, `RemovePanel` and `MovePanel`, bound in
    both renderers and held to byte parity across the nine language suites by a
    corpus scenario. The layout used to be read once when the terminal was built
    and never again, so a terminal opened with the wrong panels had to be
    restarted with a different config. ✅

14. **The bindings drive what the READMEs promise** — the recorder, the
    time-machine cursor and the host feeds are exercised by every binding suite,
    not just by Rust, and every example is run rather than parsed. Four commands
    had been documented in nine READMEs and executed nowhere but Rust; five
    languages had their examples byte-compiled, which a script that dies on its
    first call also survives.
    `docs_examples::every_documented_command_is_driven_by_a_binding_suite` keeps
    the first of those closed. ✅

## Next

Nothing here is scheduled. These are the openings the current shape leaves, in
roughly the order they would pay off.

- **The one indicator that is not reachable.** Exactly one of the indicators in
  `wickra-core` has no registry entry, and it is `Footprint`:
  [`docs/INDICATORS.md`](docs/INDICATORS.md) says why. It answers with a list of
  price levels, each with its own bid and ask volume, which the registry's fixed
  named-field model does not carry -- and the terminal already renders a
  footprint from its own per-price state, so registering it would be a second
  implementation of a view that exists.

  The profiles and the alternative bars are *not* in that count. They have
  surfaces of their own, alongside the registry rather than inside it, because a
  histogram and a bar are not a reading; `ListIndicators` lists all three.

  This paragraph used to say something else, and it was two claims that could
  not both be true: "1 of the 504 are not reachable", and then a list of
  seventeen derivatives-tick, fifteen cross-section, three trade-quote and eight
  profile outputs. The number had been corrected and the sentence under it had
  not.
- **The browser reaches one venue.** The native source dispatches on the venue
  name and opens any of ten through `wickra-exchange`; the browser cannot use
  that client at all -- it is native code with a socket and an HTTP stack -- so
  it opens the venue's public stream itself and feeds a `Manual` source. That
  bridge is hand-written and speaks Binance spot. Adding a venue means writing
  its stream dialect again in TypeScript, which is real work rather than a
  missing switch. The limit is stated in `docs/RENDERERS.md`, in
  `web/README.md`, in the app's own placeholder and in the message that refuses
  any other venue, so it is met where a user reads rather than found on the
  second venue they try.

- **Two copies of the indicator library in one binary.** `wickra-exchange` pins
  `wickra-core 0.9` and this crate builds against 1.x, so the lockfile carries
  both and cargo compiles both. Nothing is wrong at run time -- the version gap
  is crossed explicitly, by `into_core` in the live source and its test -- but it
  is a larger binary and a second set of the same types. It closes when
  `wickra-exchange` follows to 1.x; the work is there, not here.

- **A live feed for funding and open interest.** The `DerivativesTick` family is
  registered and drivable, and no source in this repository can drive it: the
  exchange layer defines `DerivativesFeed` and `DerivativesTickBuilder`, no
  venue implements them, and the `Exchange` trait exposes no way to subscribe to
  one. So the family is reachable only through the `FeedDerivatives` command,
  from a host with its own source -- which is why that command exists. The work
  is in `wickra-exchange`, not here.
- **First release.** Done at 0.1.0, and worth leaving written down, because what
  held it up was not a decision here and the same shape will recur.

  `wickra-terminal-core` depended on `wickra-exchange` by git revision, and
  `cargo publish` refuses any crate with a git dependency. So nothing could be
  published until that sibling had its own first release — and not merely the
  crates.io half of it: `cargo-publish` is the first job in `release.yml`,
  `github-release` waits on it and on every other publish job, and
  `publish-release` waits on `github-release`. A tag pushed before that point
  would have failed at the first job and produced no release assets at all.

  It was recorded and waited out rather than worked around, on purpose.
  Vendoring the dependency or switching it to a path dependency would have let
  `cargo publish` succeed while shipping a tree that is not the one that was
  tested — trading a visible blocker for an invisible one.

  The workflow itself audits clean — every action pinned to a full SHA, every job
  with a timeout, `contents: read` at the top with write granted only to the two
  jobs that attach release assets, and no checkout leaving credentials on disk.
  The credentials it needs now live at organisation level rather than in this
  repository, so a publish would authenticate; the dependency above is what is
  left.

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
