"""The time-machine ``Seek`` command flows through the binding unchanged: it is
just another JSON command on the data-driven boundary, so every binding gets it
for free. Drive a replay feed, rewind it, and check the frame reflects the
earlier state.
"""

import json

import wickra_terminal as wt


def _trade(price, ts):
    return {
        "type": "trade",
        "symbol": {"base": "BTC", "quote": "USDT"},
        "price": price,
        "quantity": "1",
        "aggressor": "Buy",
        "timestamp": ts,
    }


def _config():
    feed = json.dumps([_trade("100", 1), _trade("101", 2), _trade("102", 3)])
    return json.dumps(
        {
            "sources": [{"Replay": {"dataset": feed}}],
            "layout": {
                "panels": [{"kind": "Chart", "rect": {"x": 0, "y": 0, "w": 100, "h": 100}}]
            },
        }
    )


def test_seek_rewinds_the_replayed_frame():
    term = wt.Terminal(_config())
    term.command(json.dumps({"type": "Subscribe", "source": 0, "symbol": "BTC/USDT"}))
    for _ in range(3):
        raw = term.command(json.dumps({"type": "Tick"}))
    chart = next(p for p in json.loads(raw)["panels"] if p["panel"] == "chart")
    assert chart["last"] == 102.0

    # Rewind to position 1: only the first trade (price 100) remains folded.
    raw = term.command(json.dumps({"type": "Seek", "source": 0, "index": 1}))
    chart = next(p for p in json.loads(raw)["panels"] if p["panel"] == "chart")
    assert chart["last"] == 100.0
    assert chart["series"] == [100.0]
