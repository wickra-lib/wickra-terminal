# Wickra Terminal — Python

Python bindings for the `wickra-terminal` data-driven core (PyO3). Build a
`Terminal` from a JSON config, drive it with command JSON, read back frame
view-models — the same protocol as the native TUI and every other binding.

## Install

```bash
pip install wickra-terminal
```

## Usage

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

`Terminal.version()` and `wickra_terminal.__version__` report the library version.

## Build from source

```bash
maturin develop --release
pytest tests -q
```
