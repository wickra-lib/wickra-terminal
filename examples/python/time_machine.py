"""A runnable Python example: rewind a recorded feed and watch state re-fold.

The time-machine is what makes a recording more than a slow synthetic feed:
``Seek`` throws the folded state away and rebuilds it from the recording, so a
rewind is deterministic rather than approximate. Nothing here is Python-specific
-- it is four JSON commands, and every binding drives the same four.

    maturin develop --release -m bindings/python/Cargo.toml
    python examples/python/time_machine.py
"""

import json

from wickra_terminal import Terminal

PRICES = [100, 101, 102, 103, 104, 105]


def _feed() -> str:
    return json.dumps(
        [
            {
                "type": "trade",
                "symbol": {"base": "BTC", "quote": "USDT"},
                "price": str(price),
                "quantity": "1",
                "aggressor": "Buy",
                "timestamp": i + 1,
            }
            for i, price in enumerate(PRICES)
        ]
    )


CONFIG = json.dumps(
    {
        "sources": [{"Replay": {"dataset": _feed()}}],
        "layout": {
            "panels": [{"kind": "Chart", "rect": {"x": 0, "y": 0, "w": 100, "h": 100}}]
        },
    }
)


def _chart(raw: str) -> dict:
    return next(p for p in json.loads(raw)["panels"] if p["panel"] == "chart")


def main() -> None:
    term = Terminal(CONFIG)
    term.command(json.dumps({"type": "Subscribe", "source": 0, "symbol": "BTC/USDT"}))

    raw = ""
    for _ in range(len(PRICES)):
        raw = term.command(json.dumps({"type": "Tick"}))
    print("played to the end:   last =", _chart(raw)["last"])

    where = json.loads(term.command(json.dumps({"type": "ReplayPosition", "source": 0})))
    print("position:            {cursor}/{length}".format(**where))

    # Rewind to just after the second trade. The state is rebuilt from the
    # recording rather than restored from a snapshot, which is why a rewind
    # lands on exactly the frame the forward pass had at that point.
    raw = term.command(json.dumps({"type": "Seek", "source": 0, "index": 2}))
    print("rewound to index 2:  last =", _chart(raw)["last"])
    print("series:             ", _chart(raw)["series"])

    # And forward again from there, over the same events.
    raw = term.command(json.dumps({"type": "Tick"}))
    print("one tick later:      last =", _chart(raw)["last"])


if __name__ == "__main__":
    main()
