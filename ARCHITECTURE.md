# Architecture

`wickra-terminal` is **one product with a selectable renderer**. A single
data-driven core (`terminal-core`) folds market events into state and emits
view-models; two reference front-ends render those view-models — a native TUI
(ratatui) and a Web app (WASM + Vue) — chosen with `--render tui|web`. The core
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
CORES   wickra-core (indicators) · wickra-exchange (Live) · wickra-backtest (Replay)
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

Commands (subscribe, set-focus, add-source, set-timeframe…) and frames (the
active panels' view-models) are JSON. No callbacks cross the C ABI — every
language drives its own loop and drains frames, so streaming is as trivial to
carry as a synchronous call, R included.

## Sources are activatable modules

The `DataSource` trait unifies three source kinds behind one symbol-tagged
`poll()`:

- **`Live`** — wraps `wickra-exchange` (the ten-venue connectivity layer).
- **`Replay`** — wraps `wickra-backtest`, driving a recorded feed with a
  time-machine `seek`.
- **`Synth`** — a deterministic synthetic feed for demos and tests.

Multiple sources run at once, can be added/removed/hot-swapped at runtime, and
every symbol is keyed by `(SourceId, Symbol)` for O(1) multi-symbol state.

## State is the moat

`AppState` folds each event in O(1) — order-book diffs into a `BookState`, prints
into a bounded `TapeRing`, indicator updates into an `IndicatorSet` — and never
recomputes over history. Golden tests pin the produced `Frame` byte-for-byte and
cross-language, so a refactor that corrupts the fold fails loudly everywhere.

## Where "real money" splits the form

| Layer | TUI (native) | Web (browser) |
|---|---|---|
| Live charts + indicators | ✅ | ✅ core → WASM |
| Live signals (backtest streaming) | ✅ | ⚠️ needs the backtest WASM streaming export |
| Paper (sim fills + P&L) | ✅ `PaperExchange` | ✅ WASM + `localStorage` |
| **Real orders** | ✅ native (keys server-side) | ❌ browser holds no secret → backend |

Real execution is opt-in and USER-GO gated; both renderers default to
read-only / paper. See [THREAT_MODEL.md](THREAT_MODEL.md).

## Integration with the rest of Wickra

A Rust build depends on `wickra-core`, `wickra-backtest` and `wickra-exchange` as
Cargo crates and composes them in one binary, no FFI. `terminal-core` re-exports
`Symbol` and `Event` from `wickra-exchange` so the source layer speaks the
exchange's types directly.
