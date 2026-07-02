"""wickra-terminal: the data-driven trading-terminal core, in Python.

Build a :class:`Terminal` from a JSON config, drive it with command JSON and read
back the frame view-models as JSON — the exact same protocol the native TUI and
every other binding use::

    import json
    from wickra_terminal import Terminal

    term = Terminal(json.dumps({
        "sources": [{"Synth": {"seed": 1}}],
        "layout": {"panels": [{"kind": "Chart", "rect": {"x": 0, "y": 0, "w": 100, "h": 100}}]},
    }))
    term.command(json.dumps({"type": "Subscribe", "source": 0, "symbol": "BTC/USDT"}))
    frame = json.loads(term.command(json.dumps({"type": "Tick"})))
    print(frame["panels"][0])
"""

from __future__ import annotations

from ._wickra_terminal import Terminal, __version__

__all__ = ["Terminal", "__version__"]
