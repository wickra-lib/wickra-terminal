"""The indicator registry is reachable from Python.

The registry lives in the Rust core and the binding passes JSON through, so
nothing here needed new binding code. That is exactly why it is worth a test:
"no code changed" is also what a broken pass-through looks like.
"""

import json

import wickra_terminal as wt

# A non-default indicator, so finding it proves the config reached the registry
# rather than the built-in overlay happening to look right.
CONFIG = json.dumps(
    {
        "sources": [{"Synth": {"seed": 1}}],
        "indicators": [{"kind": "Rsi", "params": [14]}],
    }
)


def chart_indicators(term):
    frame = json.loads(term.command(json.dumps({"type": "Tick"})))
    chart = next(p for p in frame["panels"] if p["panel"] == "chart")
    return [i["name"] for i in chart["indicators"]]


def test_a_configured_indicator_reaches_the_chart():
    term = wt.Terminal(CONFIG)
    term.command(json.dumps({"type": "Subscribe", "source": 0, "symbol": "BTC/USDT"}))
    for _ in range(30):
        names = chart_indicators(term)
    assert names == ["Rsi(14)"], names


def test_indicators_can_be_added_and_removed_at_run_time():
    term = wt.Terminal(CONFIG)
    term.command(json.dumps({"type": "Subscribe", "source": 0, "symbol": "BTC/USDT"}))
    term.command(json.dumps({"type": "AddIndicator", "spec": {"kind": "Atr", "params": [14]}}))
    assert "Atr(14)" in chart_indicators(term)
    term.command(json.dumps({"type": "RemoveIndicator", "label": "Rsi(14)"}))
    assert chart_indicators(term) == ["Atr(14)"]


def test_the_catalogue_lists_the_whole_registry():
    term = wt.Terminal(CONFIG)
    catalogue = json.loads(term.command(json.dumps({"type": "ListIndicators"})))["indicators"]
    assert len(catalogue) >= 497, len(catalogue)
    # Every row carries the parameters needed to construct it, which is the
    # point of the catalogue: discovery without a second lookup.
    #
    # These are wickra's own reference parameters, not the terminal's default
    # overlay -- the catalogue answers "what can this build do", and the overlay
    # answers "what is it showing right now". Sma appears here as wickra pins it
    # (14), while the overlay runs Sma(20).
    by_kind = {row["kind"]: row["params"] for row in catalogue}
    assert len(by_kind["Sma"]) == 1, by_kind["Sma"]
    assert len(by_kind["MacdIndicator"]) == 3, by_kind["MacdIndicator"]
    assert by_kind["AdaptiveCycle"] == [], "a parameterless indicator carries an empty list"


def test_the_catalogue_lists_the_other_two_surfaces():
    # Profiles and alternative bar types are configurable by name, so a caller
    # outside Rust has to be able to find them. Until this landed the catalogue
    # carried the indicators only: `VolumeProfile` was constructible and
    # invisible, and the only way to learn it existed was to read the Rust.
    #
    # They are separate lists rather than more indicator rows because they
    # answer with different things -- a histogram and a stream of bars, against
    # an indicator's single number -- and merging them would make every consumer
    # filter before it could use any of them.
    term = wt.Terminal(CONFIG)
    catalogue = json.loads(term.command(json.dumps({"type": "ListIndicators"})))

    profiles = {row["kind"]: row["params"] for row in catalogue["profiles"]}
    assert "VolumeProfile" in profiles, sorted(profiles)
    assert len(profiles) >= 6, sorted(profiles)
    assert len(profiles["VolumeProfile"]) == 2, profiles["VolumeProfile"]

    bar_types = {row["kind"]: row["params"] for row in catalogue["bar_types"]}
    assert "RenkoBars" in bar_types, sorted(bar_types)
    assert len(bar_types) >= 10, sorted(bar_types)

    # A row is constructible as it stands: feeding one straight back as a config
    # is what discovery is for.
    spec = {"kind": "RenkoBars", "params": bar_types["RenkoBars"]}
    config = json.loads(CONFIG)
    config["bars"] = [spec]
    config["layout"] = {
        "panels": [{"kind": "Bars", "rect": {"x": 0, "y": 0, "w": 100, "h": 100}}]
    }
    built = wt.Terminal(json.dumps(config))
    built.command(json.dumps({"type": "Subscribe", "source": 0, "symbol": "BTC/USDT"}))
    frame = json.loads(built.command(json.dumps({"type": "Tick"})))
    panel = frame["panels"][0]
    assert panel["panel"] == "bars", frame
    # The label comes from the parameters the catalogue handed out, so it is not
    # spelled here: pinning "RenkoBars(3)" would assert the golden corpus's own
    # choice of box size rather than what discovery actually returned.
    assert panel["streams"][0]["label"].startswith("RenkoBars("), frame


def test_an_unknown_indicator_is_rejected_with_its_name():
    term = wt.Terminal(CONFIG)
    try:
        term.command(json.dumps({"type": "AddIndicator", "spec": {"kind": "NotReal"}}))
    except ValueError as err:
        assert "NotReal" in str(err), err
    else:
        raise AssertionError("an unknown indicator should be rejected")


def test_a_multi_output_indicator_reports_named_fields():
    term = wt.Terminal(
        json.dumps(
            {
                "sources": [{"Synth": {"seed": 1}}],
                "indicators": [{"kind": "MacdIndicator", "params": [12, 26, 9]}],
            }
        )
    )
    term.command(json.dumps({"type": "Subscribe", "source": 0, "symbol": "BTC/USDT"}))
    for _ in range(200):
        raw = term.command(json.dumps({"type": "Tick"}))
    chart = next(p for p in json.loads(raw)["panels"] if p["panel"] == "chart")
    macd = chart["indicators"][0]
    assert macd["name"] == "MacdIndicator(12,26,9)"
    assert len(macd["fields"]) > 1, macd
    # The primary value is the first field, so a caller wanting one line does
    # not have to know which field that is.
    assert macd["value"] == macd["fields"][0]["value"]
