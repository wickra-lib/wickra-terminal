# Renderers

Web and TUI are **two renderers of one core** —
not two products. Both consume the identical `Frame` of view-models from
`wickra-terminal-core`; neither contains market logic.

## The data-driven boundary

Every renderer and every language binding drives the core through the same tiny,
data-shaped surface — the same idea as the backtester's `run_json`:

```
Terminal::new(config_json)          construct from a JSON config
Terminal::command_json(cmd_json)    apply a command, return the next frame as JSON
Terminal::version()                 the crate version
```

Commands (`Tick`, `Subscribe`, `Unsubscribe`, `SetFocus`, `AddSource`,
`RemoveSource`, `Seek`, `Feed`, `AddIndicator`, `RemoveIndicator`,
`SetTimeframe`, `ListIndicators`) and the returned frame (the active panels'
view-models) are JSON. No callbacks cross the boundary, so streaming is as
trivial to carry as a synchronous call — across all ten languages. `Seek` is the
time-machine (rewind a replay source and re-fold), and `Feed` pushes an external
event into a host-fed `Manual` source — both are just data (see
[SOURCES.md](SOURCES.md)).

## The two reference renderers

| Renderer | Where | How it maps a `PanelView` |
|----------|-------|---------------------------|
| **TUI** | native terminal | `crates/ui-tui` (ratatui) — a widget per variant; a RAII guard restores the terminal on exit/panic. |
| **Web** | browser | `web/` (Vue) over `bindings/wasm` — the chart to a `<canvas>`, the tabular panels to the DOM. |

Because both map the same view-models, a feature added once in the core (a new
panel, a new source) appears in both renderers with no per-renderer logic.

## Building your own front-end

Any language binding exposes the same `Terminal` handle + `command` + `version`.
A developer in Python, Go, C#, Java, R, C/C++ or the browser can build a bespoke
front-end on the core by feeding it command JSON and rendering the returned
frames — see the [examples](../examples/).

## What each renderer can do

| Layer | TUI (native) | Web (browser) |
|-------|--------------|---------------|
| Live charts + indicators | yes | yes (core → WASM) |
| Recorded replay + seek | yes | yes |
| Live market data | `wickra-exchange` | browser WebSocket into a `Manual` source |
| Paper fills, P&L, real orders | not built | not built |

Both renderers read. Neither opens an order, and there is nothing to gate: the
terminal has no simulator, no position and no order path. `LiveSource` connects
with empty credentials, because the exchange client it wraps is used here only
for public market data.

The browser cannot hold a secret, so if execution is ever added it will need a
backend on that side regardless. See [../THREAT_MODEL.md](../THREAT_MODEL.md) for
what the current surface does and does not expose.
