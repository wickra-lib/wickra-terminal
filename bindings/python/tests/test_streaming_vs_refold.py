"""Streaming a feed and re-folding it in one batch reach the same frame.

The terminal reaches a state two ways. Streaming folds one event per tick as it
arrives; ``Seek`` throws the state away and re-folds the whole prefix in a single
batch. ARCHITECTURE.md calls that re-fold the moat -- it is what makes a rewind
deterministic and what lets the browser run the time-machine without an engine --
so the two must land on byte-identical frames.

Byte-identical, not merely equal: every binding returns the core's compact
``command_json`` string verbatim, so string equality here is the exact check with
no per-language JSON comparison in the way. That also makes this the per-language
half of the guarantee: the Rust suite proves the core re-folds correctly, and
this proves each binding carries the same bytes out.
"""

import json

import wickra_terminal as wt

TICKS = 4


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
    feed = json.dumps([_trade(str(100 + i), i + 1) for i in range(8)])
    return json.dumps(
        {
            "sources": [{"Replay": {"dataset": feed}}],
            "layout": {
                "panels": [{"kind": "Chart", "rect": {"x": 0, "y": 0, "w": 100, "h": 100}}]
            },
        }
    )


def _subscribed():
    term = wt.Terminal(_config())
    term.command(json.dumps({"type": "Subscribe", "source": 0, "symbol": "BTC/USDT"}))
    return term


def test_streaming_and_batch_refold_agree():
    streamed = _subscribed()
    frame = None
    for _ in range(TICKS):
        frame = streamed.command(json.dumps({"type": "Tick"}))

    # A second terminal runs the feed out, then re-folds the same prefix in one
    # batch. Running past the point first is what makes this a rewind rather
    # than a replay of state it still had.
    rewound = _subscribed()
    for _ in range(8):
        rewound.command(json.dumps({"type": "Tick"}))
    refolded = rewound.command(json.dumps({"type": "Seek", "source": 0, "index": TICKS}))

    assert frame == refolded


def test_the_frame_is_not_empty():
    # A guard on the guard: two empty frames are also byte-identical, and an
    # equality test that passes on nothing proves nothing.
    term = _subscribed()
    for _ in range(TICKS):
        raw = term.command(json.dumps({"type": "Tick"}))
    chart = next(p for p in json.loads(raw)["panels"] if p["panel"] == "chart")
    assert chart["last"] == float(100 + TICKS - 1)
