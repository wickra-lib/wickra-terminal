# Cookbook

Short, task-focused recipes. Every recipe drives the same `Terminal` handle; see
the runnable [examples](../examples/) for full programs per language.

## Run the TUI over a live feed

```bash
cargo run -p wickra-terminal -- --source live:binance:BTC/USDT
```

Keys: `s` add a source · `a` subscribe a symbol · `d` unsubscribe · `x` remove a
source · `←/→` cycle the focused symbol · `q` quit.

## Run the TUI over a deterministic synthetic feed (no network)

```bash
cargo run -p wickra-terminal -- --source synth:1
```

## Drive the core from a config file

```toml
# terminal.toml
timeframe = "1m"

[[sources]]
[sources.Synth]
seed = 1

[[indicators]]
kind = "Sma"
params = [20]

[[indicators]]
kind = "Rsi"
params = [14]

[[layout.panels]]
kind = "Chart"
[layout.panels.rect]
x = 0
y = 0
w = 100
h = 100
```

```bash
cargo run -p wickra-terminal -- --config terminal.toml
```

## Drive the core from any language

```python
import json
from wickra_terminal import Terminal

term = Terminal(json.dumps({
    "sources": [{"Synth": {"seed": 1}}],
    "layout": {"panels": [{"kind": "Chart", "rect": {"x": 0, "y": 0, "w": 100, "h": 100}}]},
}))
term.command(json.dumps({"type": "Subscribe", "source": 0, "symbol": "BTC/USDT"}))
frame = json.loads(term.command(json.dumps({"type": "Tick"})))
print(frame["panels"][0])
```

The same protocol works from Node.js, Go, C#, Java, R, C/C++ and the browser —
see [RENDERERS.md](RENDERERS.md).

## Add a source and a symbol at runtime

```json
{"type":"AddSource","spec":{"Synth":{"seed":2}}}
{"type":"Subscribe","source":1,"symbol":"ETH/USDT"}
```

Multiple sources coexist and hot-swap while the terminal runs.

## Rearrange the panels at runtime

The layout is read from the config when the terminal is built, and these three
change it afterwards. A panel is named by its index in the layout, counting from
zero:

```json
{"type":"AddPanel","spec":{"kind":"Book","rect":{"x":50,"y":0,"w":50,"h":100},"depth":24}}
{"type":"MovePanel","index":1,"rect":{"x":0,"y":50,"w":100,"h":50}}
{"type":"RemovePanel","index":1}
```

`AddPanel` appends, so the index it takes is the length before the call. An index
past the end is refused with the count rather than acted on, which is what a
renderer holding an index across a shrinking layout needs.

`MovePanel` changes the rectangle only. To change a panel's depth, remove it and
add it again -- the depth is what the panel was built with, and rebuilding is
what changing it means.

## Record a session and play it back

The recorder is off unless the config asks for it or something turns it on:

```json
{"type":"SetRecording","capacity":50000}
{"type":"Tick"}
{"type":"ExportRecording"}
{"type":"SetRecording","capacity":null}
```

`ExportRecording` answers with the events in exactly the shape `Replay` takes, so
the output goes straight back in as a `dataset`. The core stays filesystem-free —
it has to, to run in a browser — so writing the file is the host's job: the TUI
binds `w` to writing it beside itself, and the browser's `save` button hands the
same bytes to the browser as a download.

Both directions of `SetRecording` clear what is already held, so a capacity
change never leaves a recording that is part one size and part another. The
terminal binds `r` to it and the browser has a `record` field, so a session can
be kept without having decided to before it began.

## Rewind a recorded feed (the time-machine)

A `Replay` source records the whole feed, so `Seek` can rewind it and re-fold
state; playback then resumes forward:

```json
{"type":"Subscribe","source":0,"symbol":"BTC/USDT"}
{"type":"Tick"}
{"type":"Seek","source":0,"index":50}
```

`Seek` re-folds deterministically from the recorded feed (a market's streaming
indicators are not cloneable, so there is no state snapshot to restore). Seeking
a live or synthetic source is an error.

## Drive a source from your own feed (`Manual` + `Feed`)

Add a host-fed `Manual` source and push events into it; each is folded on the
next `Tick`:

```json
{"type":"AddSource","spec":"Manual"}
{"type":"Subscribe","source":1,"symbol":"BTC/USDT"}
{"type":"Feed","source":1,"event":{"type":"trade","symbol":{"base":"BTC","quote":"USDT"},"price":"64000","quantity":"0.1","aggressor":"Buy","timestamp":1}}
{"type":"Tick"}
```

This is how any embedder drives the terminal from a feed it already has — the
same commands in every language.

## Run a live feed in the browser

The WASM core cannot open sockets, so the browser opens the exchange WebSocket
itself and bridges it into a `Manual` source through `Feed`. The web renderer
ships a Binance bridge; type into the "add source" box:

```
live:binance:BTC/USDT
```

Public market data only — no API keys. See
[`web/src/binance.ts`](../web/src/binance.ts) and [SOURCES.md](SOURCES.md).

## Build the browser renderer

```bash
( cd bindings/wasm && wasm-pack build --target web )
( cd web && npm install && npm run dev )   # http://localhost:5173
```

See also: [INDICATORS.md](INDICATORS.md) · [PANELS.md](PANELS.md) · [SOURCES.md](SOURCES.md) ·
[STREAMING.md](STREAMING.md).
