# Cookbook

Short, task-focused recipes. Every recipe drives the same `Terminal` handle; see
the runnable [examples](../examples/) for full programs per language.

## Run the TUI over a live feed

```bash
cargo run -p wickra-terminal -- --render tui --source live:binance:BTC/USDT
```

Keys: `s` add a source · `a` subscribe a symbol · `d` unsubscribe · `x` remove a
source · `←/→` cycle the focused symbol · `q` quit.

## Run the TUI over a deterministic synthetic feed (no network)

```bash
cargo run -p wickra-terminal -- --render tui --source synth:1
```

## Drive the core from a config file

```toml
# terminal.toml
[[sources]]
[sources.Synth]
seed = 1

[layout]
[[layout.panels]]
kind = "Chart"
[layout.panels.rect]
x = 0; y = 0; w = 100; h = 100
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

## Build the browser renderer

```bash
( cd bindings/wasm && wasm-pack build --target web )
( cd web && npm install && npm run dev )   # http://localhost:5173
```

See also: [PANELS.md](PANELS.md) · [SOURCES.md](SOURCES.md) ·
[STREAMING.md](STREAMING.md).
