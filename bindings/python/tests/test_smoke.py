"""Smoke test: build a terminal, subscribe, tick and read a frame."""

import json

import wickra_terminal as wt

CONFIG = json.dumps(
    {
        "sources": [{"Synth": {"seed": 1}}],
        "layout": {
            "panels": [{"kind": "Chart", "rect": {"x": 0, "y": 0, "w": 100, "h": 100}}]
        },
    }
)


def test_version_is_a_string():
    assert isinstance(wt.__version__, str)
    assert wt.Terminal.version() == wt.__version__


def test_subscribe_then_tick_returns_a_chart_frame():
    term = wt.Terminal(CONFIG)
    term.command(json.dumps({"type": "Subscribe", "source": 0, "symbol": "BTC/USDT"}))
    for _ in range(30):
        raw = term.command(json.dumps({"type": "Tick"}))
    frame = json.loads(raw)
    charts = [p for p in frame["panels"] if p["panel"] == "chart"]
    assert charts, "expected a chart panel in the frame"
    assert charts[0]["last"] > 0.0


def test_invalid_config_raises():
    try:
        wt.Terminal("not json")
    except ValueError:
        return
    raise AssertionError("expected ValueError for an invalid config")


def test_invalid_command_raises():
    term = wt.Terminal(CONFIG)
    try:
        term.command(json.dumps({"type": "Nope"}))
    except ValueError:
        return
    raise AssertionError("expected ValueError for an unknown command")
