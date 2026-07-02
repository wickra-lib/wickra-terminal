"""A runnable Python example: drive a synthetic feed and print a frame.

    maturin develop --release -m bindings/python/Cargo.toml
    python examples/python/synth_terminal.py
"""

import json

from wickra_terminal import Terminal

CONFIG = json.dumps(
    {
        "sources": [{"Synth": {"seed": 1}}],
        "layout": {
            "panels": [{"kind": "Chart", "rect": {"x": 0, "y": 0, "w": 100, "h": 100}}]
        },
    }
)


def main() -> None:
    term = Terminal(CONFIG)
    term.command(json.dumps({"type": "Subscribe", "source": 0, "symbol": "BTC/USDT"}))
    raw = ""
    for _ in range(20):
        raw = term.command(json.dumps({"type": "Tick"}))
    print("wickra-terminal", term.version())
    print(json.dumps(json.loads(raw)["panels"][0], indent=2))


if __name__ == "__main__":
    main()
