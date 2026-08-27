# Architecture

`wickra-terminal` is **one core with two renderers**. A single
data-driven core (`terminal-core`) folds market events into state and emits
view-models; two reference front-ends render those view-models — a native TUI
(ratatui) and a Web app (WASM + Vue). They are separate programs over one
core, not two modes of one binary. The core
is exposed as a JSON-over-C-ABI data API in ten languages, so a developer in any
language can build their own front-end on the same core.

## The layers

```
RENDERERS   TUI: crates/ui-tui (ratatui)      ·      Web: web/ (Vue) over bindings/wasm
      ▲ view-models (JSON / structs)
CORE   crates/terminal-core
       DataSource(Live | Replay | Synth)  ·  AppState<(SourceId,Symbol), SymbolState> (O(1) fold)  ·  Panels → view-models
      ▼ exposed as a data API in ten languages (like wickra-backtest's run_json)
BINDINGS   python · node · wasm · c (ABI hub → c/c++/c#/go/java/r)
CORES   wickra-core (<!--indicator-count-->460<!--/indicator-count--> of its indicators reached) · wickra-exchange (Live)
```

## The core is renderer-agnostic

Panels return **view-models** — values, series and colours — never renderer
commands. That is the single rule that keeps one logic driving N front-ends: the
TUI maps a `PanelView` to a ratatui widget, the Web app maps the same `PanelView`
to a canvas draw, and neither can smuggle rendering decisions into the core.

## The data-driven boundary

The FFI surface is deliberately tiny and data-shaped, exactly like the
backtester's `run_json`:

```
Terminal::new(config_json)          construct from a JSON config
Terminal::command_json(cmd_json)    apply a command, return the next frame as JSON
Terminal::version()                 the crate version
```

Commands (subscribe, set-focus, add-source, add-indicator, set-timeframe…) and frames (the
active panels' view-models) are JSON. No callbacks cross the C ABI — every
language drives its own loop and drains frames, so streaming is as trivial to
carry as a synchronous call, R included.

## Sources are activatable modules

The `DataSource` trait unifies three source kinds behind one symbol-tagged
`poll()`:

- **`Live`** — wraps `wickra-exchange` (the ten-venue connectivity layer).
- **`Replay`** — a recorded feed with a time-machine `seek`. It keeps the whole
  event list and re-folds from the start, so a rewind is deterministic. Nothing
  is read from disk and no engine is involved, which is what lets it run in the
  browser.
- **`Synth`** — a deterministic synthetic feed for demos and tests.

Multiple sources run at once, can be added/removed/hot-swapped at runtime, and
every symbol is keyed by `(SourceId, Symbol)` for O(1) multi-symbol state.

## State is the moat

`AppState` folds each event in O(1) — order-book diffs into a `BookState`, prints
into a bounded `TapeRing`, indicator updates into an `IndicatorSet` — and never
recomputes over history. Golden tests pin the produced `Frame` byte-for-byte and
cross-language, so a refactor that corrupts the fold fails loudly everywhere.

## Where "real money" splits the form

| Layer | TUI (native) | Web (browser) | Status |
|---|---|---|---|
| Live charts + indicators | ✅ | ✅ core → WASM | shipped |
| Recorded replay + seek | ✅ | ✅ | shipped |
| Live market data | ✅ `wickra-exchange` | ✅ browser WebSocket → `Feed` | shipped |
| Paper fills and P&L | — | — | not built |
| Real orders | — | — | not built |

The terminal reads. It opens no orders, holds no credentials and has no position
of any kind: `LiveSource` connects with empty credentials, and the exchange
client it wraps is used only for public market data. Paper trading and execution
are plausible next layers, not features behind a flag — there is no simulator and
no order path to gate.

That is also why [SECURITY.md](SECURITY.md) and [THREAT_MODEL.md](THREAT_MODEL.md)
describe a read-only surface: the assets an execution-capable terminal would have
to protect do not exist here yet.

## Integration with the rest of Wickra

A Rust build depends on `wickra-core` and `wickra-exchange` as Cargo crates and
composes them in one binary, no FFI. `terminal-core` re-exports
`Symbol` and `Event` from `wickra-exchange` so the source layer speaks the
exchange's types directly.
