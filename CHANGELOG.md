# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- The browser's live bridge reconnects. It carried `onmessage` and nothing else
  -- no `onerror`, no `onclose`, no retry -- so a dropped socket left the chart
  frozen at the last print with no indication that anything had happened, where
  the native source has had an escalating backoff since it was written. The
  bridge now runs the same one, a quarter of a second doubling to half a minute,
  and the header says `reconnecting…` while it does.
- A malformed frame no longer throws out of the message handler. A feed is not a
  contract; the frame is dropped and the rest of the stream survives.
- The browser subscribes the venue's ticker, so the quote and the rolling volume
  reach the core there too rather than only in the native terminal.
- A browser subscription starts with the venue's history. It feeds a `Manual`
  source, which has no `backfill`, so the chart opened empty on a market that
  has traded for years while the native one opened with the venue's klines
  behind it. The bridge fetches those klines and feeds them before the socket's
  first message -- best effort, exactly as the native backfill is.
- The browser feed says it is Binance-only. It always was: the native source
  dispatches on the venue name across ten venues, and the hand-written bridge
  speaks one dialect. The refusal message, the placeholder, `web/README.md` and
  `docs/RENDERERS.md` all name the limit now, so it is met where a user reads
  rather than discovered on the second venue they try.
- The recorder can be started while the terminal is running. `SetRecording` was
  documented in all nine binding READMEs and bound in neither renderer, so the
  only way to record was a config field read once at start-up -- which is the
  one thing nobody can reach at the moment the market gives them a reason to.
  `r` now takes a capacity in both front-ends and an empty answer stops it, and
  `wkterm --record <events>` arms a run that is starting. A capacity of zero is
  refused rather than treated as off: it is a ring that drops everything it is
  handed, and reporting it as recording would promise a file that comes back
  empty.
- `--backfill <bars>` alongside it, and both apply on top of `--config` rather
  than instead of it: a stored layout is a layout, and how far back this run
  reaches is a decision about this run.
- The web renderer answers `RemoveSource` and `ExportRecording`. Both were bound
  in the shared keymap, answered by the TUI, and dropped into the browser's
  catch-all -- the worst shape available, because the key looks configured, the
  config validates, and nothing happens. A source can be dropped from its
  watchlist row or with the bound key, and `save` hands the recording to the
  browser as the file `Replay` takes.
- `docs_examples::every_bound_action_reaches_a_renderer` fails the build if a
  bound action is answered by neither front-end. Three are deliberately
  unanswered in the browser and are named in the test rather than inferred, so
  adding a fourth is a decision someone writes down.
- The watchlist shows what a watchlist is for. A row carried a source, a symbol
  and a last price, so neither renderer could show a spread, a turnover or a
  move -- and the numbers had been arriving all along: the state fold took
  `last` off a `Ticker` and dropped the bid, the ask and the volume on the
  floor. All four are kept now, `WatchRow` carries them plus a percentage
  change, and both front-ends draw the columns: the change tinted by its sign,
  the volume abbreviated on the same thresholds in the terminal and the browser,
  and a dash rather than a spread of nothing where no ticker has arrived yet.

  The change is measured from the first price the terminal folded for the
  market, which is the window it has actually watched -- the oldest backfilled
  bar's open when a subscription is seeded, and otherwise the first price to
  arrive. Not the venue's session open: a venue's day boundary is its own and
  the terminal is not told where it falls, so calling it a session change would
  claim a boundary nothing here knows. It is computed in the core rather than in
  each renderer, so the terminal and the browser cannot derive it differently.
- A `ticker` scenario in the golden corpus. The corpus had never carried a
  `Ticker` event, so the three fields above would have been recorded as zero in
  every scenario and pinned there -- and a field that is zero everywhere is one
  the nine language suites cannot tell apart from a field that was dropped,
  which is exactly what the fold used to do with them.
- A recorder-and-scrubber suite in every binding. Counted across the eight
  non-Rust surfaces, `SetRecording` and `ExportRecording` were driven by none of
  them, `ReplayPosition` only by the C example and `FeedDerivatives` by none --
  four commands documented in all nine binding READMEs and executed nowhere but
  Rust. Each suite now arms the recorder, drives a market, exports what it kept
  and builds a second terminal from exactly those bytes: a binding that mangled
  the export is caught by the replay refusing it, which no assertion about a
  string shape would find. Python, Node, WASM, Go, C#, Java, R and the C hub
  through `examples/c`.
- `docs_examples::every_documented_command_is_driven_by_a_binding_suite` keeps
  it that way. `every_binding_readme_documents_every_command` checked the
  promise was complete; nothing checked it was kept, which is exactly how those
  four came to be described in nine places and run in none.
- The examples are run rather than parsed. `examples-smoke` byte-compiled the
  Node, Python, Java, R and Go examples, and a script that compiles and dies on
  its first call passed that in five languages at once. Each of those languages
  now runs its examples in the job that already has the binding built, which
  costs a second and is the only check that says they work. What is left in the
  parse gate is the WASM example, which is a page and needs a browser.
- Both renderers show every output of a multi-output indicator. `value` is the
  first field, so a readout that showed only it drew `Macd(12,26,9)=1.42` and
  dropped the signal line and the histogram -- the two numbers the indicator
  exists to be read against. The core had carried them across the boundary all
  along, two binding suites tested them, and neither renderer drew them. Both
  write the same shape now, because one indicator read in two places should not
  look like two.
- The one piece of policy inside the TUI event loop is testable now. What a key
  means depends on whether a prompt is open -- a prompt takes the keyboard whole,
  because mapping keys to actions while one is open fires `quit` for the `q` in a
  symbol -- and that decision sat four levels deep in a function no test can
  enter, needing a terminal and an event stream to reach. `on_key` needs neither,
  and five tests now cover the prompt, the action path, and the press filter that
  keeps a held key from repeating its action on every report the terminal sends.
- The book panel's empty-market branch and the terminal guard have tests. The
  guard is the one piece the renderer must get right however the event loop
  exits, and it was at zero.
- The browser writes a changed layout back to `localStorage`. `web/README.md`
  said the layout is persisted, and the config was written once and never again
  -- so a panel added while the terminal ran was gone on the next reload, which
  made the panel controls something to use and then lose. The whole config is
  rewritten rather than a layout stored beside it: two places holding a layout is
  one more than can be kept in step. A config that does not parse is handed back
  unchanged rather than replaced, so a display fault cannot become data loss.
- A `host_feed` scenario in the golden corpus. `Feed` was driven by two binding
  suites and the runtime indicator commands by none of the corpus at all, so the
  frames they produce were never held to byte parity across languages -- a
  binding that serialised a fed event or a changed indicator set differently
  would have passed everywhere. The corpus now reaches fifteen of the nineteen
  commands; the four it does not are the four it cannot, because each answers
  with something other than a frame and the corpus records the last answer as
  one. Those are driven per binding instead, and
  `every_documented_command_is_driven_by_a_binding_suite` holds the whole set.
- The README and the cookbook say what the two renderers do not share. Live
  market data reaches ten venues natively and one in the browser, which was true
  before and written nowhere a user would meet it. The cookbook gains recipes for
  the layout commands and the recorder.
- The layout can be changed while the terminal runs. `Config.layout.panels` was
  read once when the terminal was built and never again, in both renderers, so a
  terminal opened with the wrong panels had to be restarted with a different
  config -- which is not something a person does while watching a market move.
  `AddPanel`, `RemovePanel` and `MovePanel` change it, and a panel is named by
  its index: `AddPanel` appends rather than inserting, because inserting would
  renumber the ones a caller already holds, and an index past the end is refused
  with the count rather than acted on.
- The renderers bind them in their own idiom. The TUI has a focused panel, so
  `p` adds one and focuses it, `o` takes the focused one off and `m` moves it,
  each from the same shorthand the source and indicator prompts use --
  `Book 70 0 30 35`, or `Tape 0 70 100 30 48` with the depth. The browser has no
  focused panel, so it binds the add to its panel field and removes with the `x`
  on the panel's own heading; `move_panel` joins the panel-focus pair it already
  declines, and the guard requires that refusal to be written down rather than
  inferred from silence.
- A rectangle that runs off the grid is refused rather than drawn clipped, in
  both renderers. It is a typo every time, and trimming it silently would leave
  a panel the config says is one size and the screen says is another.
- The renderers' own per-panel state follows the layout now. The TUI kept one
  scroll offset per panel and a focused-panel index, both sized once at
  construction -- an invariant that held for exactly as long as the layout could
  not change, and that an earlier change in this cycle leaned on. `sync_panels`
  restores both after every layout change.
- A `panels` scenario in the golden corpus, so the nine language suites hold the
  three commands to byte parity: a panel added with a depth, moved, and another
  taken off, with the frame recorded after each.
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
- `scripts/check_version_sync.py`, wired into CI: the version lives in 23
  declarations across six package managers, and a bump that misses one
  publishes an npm package pinning a native binary that was never built --
  which surfaces at install time, on a user's machine, after the tag is
  irreversible.
- `scripts/check_readme_links.py`, wired into CI: each binding README is the
  long description of a published package, so a link that leaves the package
  resolves on GitHub and nowhere else, and nothing else in the build would say
  so, because the file it points at does exist.
- `scripts/check_license_copies.py`, wired into CI, and the copies it demands:
  `LICENSE-MIT` and `LICENSE-APACHE` inside `wickra-terminal-core`, `ui-tui` and
  `bindings/python`. Every manifest declared `MIT OR Apache-2.0`, but an SPDX
  expression is a reference to two documents rather than the documents, so each
  published package left whoever received it with terms to go and find. Cargo
  decides what to package from git, so the copies have to be committed --
  and committed copies drift, which is what the check watches.
- The npm packages stage the same two texts at publish time and name them in
  `files`. Both halves are needed: npm silently drops a file its `files` list
  does not mention, and the pack dry-run in `release.yml` now proves each of the
  seven packages would carry them.
- A CI check that the committed napi loader still matches what napi generates.
  `bindings/node/index.js` and `index.d.ts` are generated and committed so
  consumers need no toolchain, and nothing compared the two -- napi rewrites
  them only when somebody rebuilds, and a rebuild is not part of committing.
- `java-publish` uploads the jar it just deployed, so the release page carries a
  Java artefact like every other language, and provenance has it to attest.
- `go-mirror` builds and runs the module before pushing it. The tree is
  assembled by copying files and rewriting an import path, and was published to
  pkg.go.dev on that basis alone -- a wrong header, a missing library or a
  botched rewrite would have surfaced on a user's machine. A smoke test now
  constructs a terminal, subscribes, ticks and reads a frame against the staged
  Linux library.
- Build provenance covers the `.nupkg` and the `.jar`. Both were published with
  no attestation while the README badge claimed provenance for the release.
- `[package.metadata.docs.rs] all-features = true` on `ui-tui`, so the setting is
  uniform across everything this workspace publishes.
- The chart view-model carries the bars. `ChartView` gains `bars` -- up to 120
  closed OHLCV bars of the configured timeframe -- and `forming`, the bar still
  accumulating. Both are omitted from the JSON while empty, so a consumer
  written against the earlier shape sees exactly the object it saw before. The
  state keeps a bounded ring of 256 closed bars, which the candle builder did
  not: it held the bar in progress and handed each closed one to the indicators,
  which read it and kept only their own state, so a renderer could draw the last
  price and nothing else.

### Changed

- `CITATION.cff` names the maintainer address the other repositories carry.
- The citation guard now checks the pairing rather than one half of it.
  `version` and `date-released` are what GitHub's citation box and Zenodo
  present as the thing being cited: while the changelog shows no released
  section both keys must be absent, and the moment one is cut both must be
  present and agree with it. Cutting a release now fails until the citation is
  brought along, instead of shipping one that dates nothing.
- Four error arms that nothing could reach are gone, each replaced by the
  reasoning that made it unreachable. Changing the timeframe and seeking a
  replay both handled a failure their own guards had already excluded -- the
  indicator specs were validated when they were added, and `replay_position`
  had just answered -- and saving a recording handled a failure of a constant
  command with no failing path. `parse_indicator` guarded against `vs` with no
  market after it, which the leading `trim` makes impossible: the pattern's
  trailing space cannot match at the end of the text, so a dangling `vs` never
  splits and is reported as the parameter it is not. A branch no input can take
  is not defence; it is an untested path that reads like one.
- `wickra-core` resolves to 1.0.4. The lockfile sat on 1.0.1 against a published
  1.0.4, and the drift notice reported it every Monday. The registry did not move
  with it and was not supposed to: wickra-core carried the same 514 indicator
  modules at 1.0.1, regenerating against 1.0.4 produces the same 497 registered
  indicators and the same file, and the note that claimed ten unseen indicators
  upstream was a miscount. It is corrected where it was written rather than left
  to send the next reader looking for them.
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

- A buffer overflow in the C examples, found by CodeQL's
  `cpp/overflowing-snprintf` on the first run after C/C++ entered the analysis
  matrix. `snprintf` returns the length it *would* have written, so accumulating
  that return value walks the offset past the buffer on truncation -- and
  `sizeof buf - at` then underflows to an enormous `size_t`, handing the next
  call a length far larger than the space that is left. Both files use a guarded
  append that refuses instead.

- An indicator period is bounded, so an absurd one is refused rather than
  allocated. A period is a length and the indicator allocates it: a parameter of
  10^20 cast cleanly to `usize`, the indicator asked for a `Vec` that size, and
  the process aborted on a capacity overflow no caller can catch. The new
  `registry_drive` fuzz target found it in under a minute. The ceiling is a
  million -- a million one-minute bars is two years -- and the refusal names the
  indicator and the number.
- `upstream-drift.yml`, which watches whether the generated registry has fallen
  behind the wickra-core it was made from. It had: the lockfile sat on 1.0.1
  while crates.io carried 1.0.4, so ten indicators existed upstream that the
  generator had never seen, and the repository's own guard catches shrinking
  only -- growth upstream was invisible by construction.

  A weekly notice that opens and edits one issue, rather than a check on every
  pull request, and that is deliberate: following upstream is blocked on
  wickra-exchange, which pins wickra-core 0.9, so bumping this side while that
  stands compiles two copies of the indicator library into one binary. A gate
  that failed every pull request until somebody else's release landed would be
  turned off within a week, and then it would be watching nothing. It reports
  both copies, because the duplicate is the blocker.
- A second scenario in every language's examples: `time_machine` plays a
  recorded feed to its end, rewinds to the second trade and shows the frame the
  forward pass had at that point. One scenario per language showed how to open a
  terminal and read a frame; it could not show the capability the repository is
  named around, and a reader could not guess it.
- `examples/wasm/`, which did not exist. Every other reach had an example and
  the browser had only the full Vue renderer -- which is the product rather than
  the shape. One HTML file, no build tool, the same three calls.
- Historical backfill. A fresh subscription fetches bars from the venue and
  seeds the chart, the price history and every bar-derived indicator with them.
  Without it every bar came from ticks the terminal saw itself, so `Atr(14)` on
  an hourly timeframe was silent for fourteen hours and the chart opened empty
  on a market that has traded since 2017. 200 bars by default, 0 to turn it off,
  and a failed fetch is not a failed subscription.

  The book, the tape and the footprint are not seeded: a bar records that
  trading happened rather than the prints it was made of, and inventing those
  would put numbers on screen no venue published.
- A live source can open a derivatives book. `market` picks spot, USD-margined,
  coin-margined or margin, and the shorthand takes it as a third segment
  (`live:binance:BTC/USDT:usdm`). It was hard-coded to spot, so a perpetual
  could not be opened at all. Funding and open interest still have no live feed
  and that is upstream: `wickra-exchange-core` defines `DerivativesFeed` but no
  venue implements it and the `Exchange` trait exposes no subscription, so the
  `FeedDerivatives` command remains the only path -- which is why it exists.
- A recorder, so the time-machine has something to rewind. `Replay` takes a feed
  and nothing in the repository could produce one: no session was ever written
  out, and `dataset` takes the events themselves rather than a path, so the only
  way to get a recording was to already have one. A config now sets `record` to
  a capacity and the terminal keeps that many recent events; `ExportRecording`
  hands them back in exactly the shape `Replay` takes, and `SetRecording` turns
  it on or off at run time. The TUI binds `w` to writing them beside itself.

  A ring rather than a log, because a terminal left running overnight must not
  grow without limit. Recorded as events are polled rather than folded, because
  `fold` is also how a seek re-folds a recording -- recording there would append
  the replayed events back on and every rewind would double it.
- The web renderer reads the shared keymap. `layout.keybinds` sits in the config
  expressly so both front-ends share one, and only the TUI ever read it -- so
  rebinding a key moved half the product. Key names are the TUI's, a key held
  with Ctrl, Cmd or Alt is left to the browser, and a key pressed inside a field
  is left to the field. `quit` and the panel-focus and scroll pairs are
  deliberately unhandled in a browser, which the docs now say rather than leave
  to be discovered.
- Panels carry a configurable depth. Book levels, tape prints, footprint levels,
  chart points and bars per stream were `const` in the code, so a config could
  set exactly one thing per panel -- its rectangle. A `PanelSpec` now takes an
  optional `depth`, clamped to 512 and refusing zero, and every panel that has a
  bound reads it.
- Panel focus finally means something. It was drawn and acted on nothing: `tab`
  moved a border and no key did anything with it. `↑` / `↓` now scroll the
  focused panel through the rows it carries, which is also why the depth had to
  become configurable -- with twelve book levels there was nothing underneath
  them to scroll to. The browser already scrolls its panels; a terminal has to
  be told.
- A streaming-versus-re-fold equivalence test in every binding: Python, Node,
  WASM, Go, Java, C#, R, C and C++. The terminal reaches a state two ways --
  streaming folds one event per tick as it arrives, `Seek` throws the state away
  and re-folds the whole prefix in one batch -- and ARCHITECTURE.md calls that
  re-fold the moat. The Rust suite proved the core re-folds correctly; nothing
  proved each binding carries the same bytes out, which is what these check, by
  string equality on the compact command output rather than a per-language JSON
  comparison. Each carries a second assertion that the frames compared are not
  empty ones: two empty frames are also byte-identical.
- Every command the boundary carries is now reachable from both renderers.
  `AddIndicator`, `RemoveIndicator`, `SetTimeframe`, `ListIndicators` and `Seek`
  were bound to nothing in either front-end, so the registry could only be
  configured from a file and the time-machine had no control anywhere. The TUI
  binds `i`, `k`, `t`, `l` and `,` / `.`; the web renderer gets a second control
  bar with the same five. The keymap is data and shared, so a rebinding moves
  both. `Feed` and `FeedDerivatives` stay unbound on purpose -- they are how a
  host pushes its own feed in, so their caller is an embedder rather than a
  person at a keyboard.
- `Terminal::add_indicator`, `remove_indicator`, `set_timeframe` and
  `replay_position`. The first three existed only inside `command_json`, so a
  Rust embedder had to assemble JSON to reach its own registry; `command_json`
  now calls them, and the config stays in step either way.
- A `ReplayPosition` command, the second that answers rather than renders. The
  `DataSource` trait has carried `cursor` and `event_count` from the start and
  nothing read them, so no renderer could show where in a recording it stood. A
  source that is not a recording answers `0/0` rather than an error.
- Both renderers draw candles. The TUI chart was two lines -- a sparkline of
  eight block glyphs and a row of indicator text -- on the panel that occupies
  seventy percent of the default layout: no axis, no price scale, no bar
  structure. It now draws the bars on a braille canvas with a price scale beside
  them, and the web canvas draws the same bars. Both fall back to the tick
  series until the first bar closes, which at an hourly timeframe is the first
  hour of a session.

  Indicators stay a numeric readout rather than lines over the candles, and that
  is deliberate: an indicator's series is sampled once per tick while the
  candles are one per bar, so the two do not share an x-axis. Overlays are drawn
  only in the fallback, where the price series and the indicator series do.
- The web renderer knows all seven panels. `Profile` and `Bars` were added to
  the core, given view-models and TUI widgets, and never taught to the web
  front-end: `PanelKind` listed five kinds, `PanelView` had five variants, and
  the frames for the other two arrived on every tick and were discarded without
  an error anywhere. ARCHITECTURE.md says adding a panel to the core makes it
  appear in every renderer at once, and for those two that was not true.
  A guard in the core now reads its own `PanelKind` and fails when a renderer
  has not been taught a panel — the golden corpus could not catch this, because
  the frames were correct and it was the reader that was missing.

- The seven shell problems actionlint found on its first run, each real. Three
  publish steps used `A && B || C`, where C also runs when A succeeded and B
  failed -- so a publish that worked could report itself as an
  already-published skip, or exit 1. Two used `local x=$(...)` / `export
  x=$(...)`, which make the assignment's exit status the builtin's and hide a
  failed command behind an empty variable. One counted release assets with `ls
  | wc -l`, and one passed an unquoted `find` expansion to `javac`.

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

- The live source holds a `dyn MarketData`, not a `dyn Exchange`. `connect`
  hands back the whole exchange -- order placement and balances alongside the
  reads -- and the source kept it, so nothing but review stood between an edit
  and an order path in a terminal that is documented not to have one. The
  narrower type says it structurally: the method is not there to call.
- `Swatinem/rust-cache` re-pinned to a commit a tag still points at.
- cargo-deny scans the feature-expanded graph, so the `live` tree is checked at
  all; the dead `RUSTSEC-2024-0436` suppression was removed from both files.
- The gated live-integration test fails on no data instead of passing, so the
  nightly job can report a real result.
- Unsubscribing a live market stops the terminal folding it. `LiveSource`'s
  `unsubscribe` was a comment noting that the exchange client has no per-symbol
  unsubscribe, and doing nothing -- but the fold creates state for whatever
  market an event names, so the dropped market came straight back on the next
  poll and was folded for the rest of the session, invisible because the
  watchlist no longer listed it. The source filters its own output now, the way
  the replay, synthetic and manual sources already did. The socket is still the
  venue's to close; the work is not.
- `scripts/update-lockfiles.sh` no longer pipes `astral.sh/uv/install.sh` into a
  shell. That ran whatever was behind the URL at that moment, with the
  privileges of everyone who regenerated a lockfile. uv is now installed by hand
  or, with `WICKRA_BOOTSTRAP_UV=1`, fetched as one pinned release archive that is
  refused unless its checksum matches.
- `osv-scanner.toml` records the pytest tmpdir advisory (GHSA-6w46-j5rx-g56g)
  with its assessment. pytest is a CI-only dependency that never reaches a
  published wheel, and the flaw needs a second local user on the machine, which
  an ephemeral single-user runner does not have. It cannot simply be upgraded:
  pytest 9 requires Python >= 3.10 while the 3.9 row exists to test the abi3
  floor, so only that row stays at 8.4.2. Written here rather than left as prose
  in the requirements file, because this file is now actually read.

[Unreleased]: https://github.com/wickra-lib/wickra-terminal/commits/main
