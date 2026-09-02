"""The recorder, the scrubber and the host feed, end to end through the binding.

Four commands sit on the boundary, are documented in all nine binding READMEs,
and were driven by almost no binding: ``SetRecording`` and ``ExportRecording``
by none at all, ``ReplayPosition`` only by the C example, ``FeedDerivatives`` by
none. ``docs_examples::every_binding_readme_documents_every_command`` proved the
READMEs were complete, and nothing checked the promise was kept — so the
recorder had never been executed outside Rust.

The round trip is the point: arm the recorder, drive the terminal, export what
it kept, and hand that straight back as a ``Replay`` dataset. If the binding
mangles the export the replay refuses it, which no assertion about a string
shape would catch.
"""

import json

import wickra_terminal as wt

CONFIG = json.dumps(
    {
        "sources": ["Manual"],
        # A derivatives indicator, so `FeedDerivatives` is observable in the
        # frame rather than merely accepted.
        "indicators": [{"kind": "FundingRate", "params": []}],
        "layout": {"panels": [{"kind": "Chart", "rect": {"x": 0, "y": 0, "w": 100, "h": 100}}]},
    }
)

SYMBOL = "BTC/USDT"


def trade(price, timestamp):
    return {
        "type": "trade",
        "symbol": {"base": "BTC", "quote": "USDT"},
        "price": price,
        "quantity": "0.5",
        "aggressor": "Buy",
        "timestamp": timestamp,
    }


def chart(raw):
    return next(p for p in json.loads(raw)["panels"] if p["panel"] == "chart")


def drive(term, price, timestamp):
    term.command(json.dumps({"type": "Feed", "source": 0, "event": trade(price, timestamp)}))
    return term.command(json.dumps({"type": "Tick"}))


def test_the_recorder_round_trips_through_a_replay():
    term = wt.Terminal(CONFIG)
    term.command(json.dumps({"type": "Subscribe", "source": 0, "symbol": SYMBOL}))

    # Nothing is kept until the recorder is armed, and asking is not an error.
    assert json.loads(term.command(json.dumps({"type": "ExportRecording"}))) == []

    term.command(json.dumps({"type": "SetRecording", "capacity": 64}))
    for i, price in enumerate(["100", "101", "102", "103"]):
        drive(term, price, i + 1)

    recorded = json.loads(term.command(json.dumps({"type": "ExportRecording"})))
    assert len(recorded) == 4, recorded
    assert recorded[0]["price"] == "100"
    assert recorded[-1]["price"] == "103"

    # Straight back in as a dataset: the shape `Replay` takes is the shape
    # `ExportRecording` answers with, and that is what makes a session keepable.
    replay = wt.Terminal(
        json.dumps(
            {
                "sources": [{"Replay": {"dataset": json.dumps(recorded)}}],
                "indicators": [],
                "layout": {
                    "panels": [{"kind": "Chart", "rect": {"x": 0, "y": 0, "w": 100, "h": 100}}]
                },
            }
        )
    )
    replay.command(json.dumps({"type": "Subscribe", "source": 0, "symbol": SYMBOL}))
    for _ in range(4):
        raw = replay.command(json.dumps({"type": "Tick"}))
    assert chart(raw)["last"] == 103.0


def test_stopping_the_recorder_clears_what_it_held():
    # Both directions clear, so a capacity change never leaves a recording that
    # is part one size and part another.
    term = wt.Terminal(CONFIG)
    term.command(json.dumps({"type": "Subscribe", "source": 0, "symbol": SYMBOL}))
    term.command(json.dumps({"type": "SetRecording", "capacity": 64}))
    drive(term, "100", 1)
    assert json.loads(term.command(json.dumps({"type": "ExportRecording"})))

    term.command(json.dumps({"type": "SetRecording", "capacity": None}))
    assert json.loads(term.command(json.dumps({"type": "ExportRecording"}))) == []


def test_replay_position_answers_for_a_source_that_cannot_be_replayed():
    # `0/0` rather than an error, so a renderer can ask about whatever is
    # focused without first knowing what kind of source it is.
    term = wt.Terminal(CONFIG)
    term.command(json.dumps({"type": "Subscribe", "source": 0, "symbol": SYMBOL}))
    where = json.loads(term.command(json.dumps({"type": "ReplayPosition", "source": 0})))
    assert where == {"cursor": 0, "length": 0}


def test_replay_position_tracks_the_cursor_through_a_recording():
    term = wt.Terminal(CONFIG)
    term.command(json.dumps({"type": "Subscribe", "source": 0, "symbol": SYMBOL}))
    term.command(json.dumps({"type": "SetRecording", "capacity": 64}))
    for i, price in enumerate(["100", "101", "102", "103"]):
        drive(term, price, i + 1)
    recorded = term.command(json.dumps({"type": "ExportRecording"}))

    replay = wt.Terminal(
        json.dumps(
            {
                "sources": [{"Replay": {"dataset": recorded}}],
                "indicators": [],
                "layout": {
                    "panels": [{"kind": "Chart", "rect": {"x": 0, "y": 0, "w": 100, "h": 100}}]
                },
            }
        )
    )
    replay.command(json.dumps({"type": "Subscribe", "source": 0, "symbol": SYMBOL}))
    at_start = json.loads(replay.command(json.dumps({"type": "ReplayPosition", "source": 0})))
    assert at_start == {"cursor": 0, "length": 4}

    for _ in range(3):
        replay.command(json.dumps({"type": "Tick"}))
    moved = json.loads(replay.command(json.dumps({"type": "ReplayPosition", "source": 0})))
    assert moved == {"cursor": 3, "length": 4}


def test_fed_derivatives_reach_a_derivatives_indicator():
    # Accepting the command proves nothing on its own: the update is folded into
    # the market's microstructure and only reaches an indicator on the next
    # trade, so the reading is what says it arrived.
    term = wt.Terminal(CONFIG)
    term.command(json.dumps({"type": "Subscribe", "source": 0, "symbol": SYMBOL}))
    raw = drive(term, "100", 1)
    assert chart(raw)["indicators"][0]["value"] is None

    term.command(
        json.dumps(
            {
                "type": "FeedDerivatives",
                "source": 0,
                "symbol": SYMBOL,
                "update": {
                    "funding_rate": 0.0001,
                    # All three prices, or the tick is withheld: a mark without
                    # an index and a futures price is not a priced market.
                    "mark_price": 102.0,
                    "index_price": 100.0,
                    "futures_price": 104.0,
                    "open_interest": 1000.0,
                    "timestamp": 9,
                },
            }
        )
    )
    raw = drive(term, "101", 2)
    reading = chart(raw)["indicators"][0]
    assert reading["name"] == "FundingRate"
    assert abs(reading["value"] - 0.0001) < 1e-12, reading


def test_feeding_derivatives_to_an_untracked_market_is_an_error():
    term = wt.Terminal(CONFIG)
    try:
        term.command(
            json.dumps(
                {
                    "type": "FeedDerivatives",
                    "source": 0,
                    "symbol": SYMBOL,
                    "update": {"funding_rate": 0.0001, "timestamp": 1},
                }
            )
        )
    except ValueError:
        return
    raise AssertionError("expected a ValueError for a market that is not subscribed")
