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
`RemoveSource`, `Seek`, `Feed`, `FeedDerivatives`, `AddIndicator`,
`RemoveIndicator`, `SetTimeframe`, `ListIndicators`, `ReplayPosition`) and the
returned frame (the active panels' view-models) are JSON. No callbacks cross the
boundary, so streaming is as trivial to carry as a synchronous call — across all
ten languages. `Seek` is the time-machine (rewind a replay source and re-fold),
and `Feed` pushes an external event into a host-fed `Manual` source — both are
just data (see [SOURCES.md](SOURCES.md)).

Two commands answer rather than render. `ListIndicators` returns the catalogue,
and `ReplayPosition` returns where a replayable source stands:

```json
{"type":"ReplayPosition","source":0}
```

```json
{"cursor":128,"length":512}
```

A source that is not a recording answers `0/0` rather than an error — a live
feed has no recorded length, and a renderer can ask about whatever is focused
without first knowing what kind of source it is. The position is a command
rather than a field on the frame because it belongs to a source, not to a panel:
in the frame it would sit in front of every consumer that has no replay at all.

## Everything the boundary offers is reachable from both renderers

That is a rule, not an observation, and it was broken for five commands.
`AddIndicator`, `RemoveIndicator`, `SetTimeframe`, `ListIndicators` and `Seek`
were carried by the boundary, exercised by the bindings' tests, and bound to
nothing in either front-end — so the registry could only be configured from a
file, and the time-machine had no control anywhere. In the TUI they are `i`,
`k`, `t`, `l` and `,` / `.`; in the web renderer they are the second control
bar. The keymap is data, shared by both renderers, so a rebinding moves both.

`Feed` and `FeedDerivatives` are deliberately not in that list: they are how a
*host* pushes its own feed in, so their caller is an embedder rather than a
person at a keyboard. The web renderer uses `Feed` exactly that way, from its
Binance bridge.

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
