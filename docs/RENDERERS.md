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

It was then broken for a sixth, and for two more in one renderer only.
`SetRecording` was documented in all nine binding READMEs and bound nowhere, so
the recorder could only be armed by a config field read once at start-up — the
one thing nobody can reach at the moment the market gives them a reason to. It
is `r` in both renderers now, and `wkterm --record` for a run that is starting.
`RemoveSource` and `ExportRecording` were answered by the TUI and dropped into
the browser's catch-all, which is the worst shape available: the key is in the
shared keymap, the config validates, and nothing happens. Both are controls in
the browser now, and
`docs_examples::every_bound_action_reaches_a_renderer` fails the build if a
seventh ever joins them.

`Feed` and `FeedDerivatives` are deliberately not in that list: they are how a
*host* pushes its own feed in, so their caller is an embedder rather than a
person at a keyboard. The web renderer uses `Feed` exactly that way, from its
Binance bridge.

## The browser's live feed is not the native one

Both renderers can watch a live market and they do not reach the same markets.
The native source connects through `wickra-exchange` and dispatches on the venue
name, so it opens Binance, Bybit, OKX, Bitget, KuCoin, Gate, HTX, Kraken,
Coinbase or Upbit, spot or derivatives. The browser cannot use that client at
all — it is native code with a socket and an HTTP stack — so it opens the venue's
public stream itself and feeds a `Manual` source through the `Feed` command, and
that hand-written bridge speaks one dialect. `live:` with any other venue is
refused with a message that says so, which is the whole of the difference: the
limit is stated where a user meets it rather than discovered on the second venue
they try.

Two things the bridge lacked and the native source always had, both of which
made the browser look like a different product rather than a second view:

- **It did not reconnect.** The socket carried `onmessage` and nothing else — no
  `onerror`, no `onclose`, no retry — so a dropped connection left the chart
  frozen at the last print with nothing said. It now runs the native source's
  backoff, a quarter of a second doubling to half a minute, and the header says
  `reconnecting…` while it does.
- **It had no history.** A `Manual` source has no `backfill`, so the browser
  chart opened empty on a market that has traded for years while the native one
  opened with the venue's klines behind it. The bridge fetches those klines and
  feeds them before the socket's first message. Best effort, exactly as the
  native backfill is: no history is a terminal that starts empty, never one that
  refuses to open the market.

## One keymap, both renderers

`layout.keybinds` is a map from action name to key name, and it sits in the
config precisely so that rebinding a key moves both front-ends. Only the TUI
read it, so that was true of the config and false of the product; the web
renderer reads the same map now.

The key names are the TUI's, because the config is shared: `backtab` means
Shift+Tab in both, even though crossterm reports it as a key of its own and the
browser reports Tab with a modifier. A key held with Ctrl, Cmd or Alt is left to
the browser — a terminal that stole Ctrl+R would be a worse citizen than one
with fewer shortcuts — and so is any key pressed inside a text field, or typing
a symbol would fire the bindings on its way past.

Where the two renderers differ, they differ in idiom rather than in meaning: an
action that opens a modal prompt in the TUI focuses the matching field in the
browser. Three do nothing in a browser and say so rather than pretending:
`quit`, because a tab is not the terminal's to close, and the panel-focus and
scroll pairs, because a web panel is a scrollable box the browser already
drives.

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
| Live market data | `wickra-exchange`, ten venues | browser WebSocket into a `Manual` source, **Binance spot only** |
| Paper fills, P&L, real orders | not built | not built |

Both renderers read. Neither opens an order, and there is nothing to gate: the
terminal has no simulator, no position and no order path. `LiveSource` connects
with empty credentials, because the exchange client it wraps is used here only
for public market data.

The browser cannot hold a secret, so if execution is ever added it will need a
backend on that side regardless. See [../THREAT_MODEL.md](../THREAT_MODEL.md) for
what the current surface does and does not expose.
