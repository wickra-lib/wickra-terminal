"""A host can push events into a ``Manual`` source through the ``Feed`` command,
which flows through the binding unchanged (it is just another JSON command on the
data-driven boundary). Feed a trade, tick, and check the frame reflects it.
"""

import json

import wickra_terminal as wt

CONFIG = json.dumps(
    {
        "sources": ["Manual"],
        "layout": {"panels": [{"kind": "Chart", "rect": {"x": 0, "y": 0, "w": 100, "h": 100}}]},
    }
)

TRADE = {
    "type": "trade",
    "symbol": {"base": "BTC", "quote": "USDT"},
    "price": "64000",
    "quantity": "0.1",
    "aggressor": "Buy",
    "timestamp": 1,
}


def test_feed_then_tick_folds_the_event():
    term = wt.Terminal(CONFIG)
    term.command(json.dumps({"type": "Subscribe", "source": 0, "symbol": "BTC/USDT"}))
    term.command(json.dumps({"type": "Feed", "source": 0, "event": TRADE}))
    raw = term.command(json.dumps({"type": "Tick"}))
    chart = next(p for p in json.loads(raw)["panels"] if p["panel"] == "chart")
    assert chart["last"] == 64000.0


def test_feed_unsubscribed_market_raises():
    term = wt.Terminal(CONFIG)
    # No subscription on the manual source: the fed event is rejected.
    try:
        term.command(json.dumps({"type": "Feed", "source": 0, "event": TRADE}))
    except ValueError:
        return
    raise AssertionError("expected a ValueError feeding an unsubscribed market")
