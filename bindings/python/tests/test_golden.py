"""Cross-language golden parity, driven by ``golden/manifest.json``.

Each scenario names a config and a command sequence; replaying it must produce
the frame in its expected file, byte for byte. Because the binding returns the
core's compact ``command_json`` string verbatim, byte equality against that one
file is the exact parity check — no per-language JSON deep-equal needed.

Reading the manifest rather than naming one scenario is what makes the corpus
extensible: a scenario added in the Rust suite is picked up here, and in the
seven other language suites, with no change to any of them.
"""

import json
import os

import pytest

import wickra_terminal as wt


def _golden_dir():
    d = os.path.dirname(os.path.abspath(__file__))
    for _ in range(8):
        g = os.path.join(d, "golden")
        if os.path.isfile(os.path.join(g, "manifest.json")):
            return g
        d = os.path.dirname(d)
    raise AssertionError("golden/ not found")


def _read(*parts):
    with open(os.path.join(*parts), encoding="utf-8") as f:
        return f.read()


def _scenarios():
    g = _golden_dir()
    manifest = json.loads(_read(g, "manifest.json"))
    return [(g, s) for s in manifest["scenarios"]]


@pytest.mark.parametrize(
    "golden,scenario", _scenarios(), ids=[s["name"] for _, s in _scenarios()]
)
def test_golden_parity_frame_is_byte_exact(golden, scenario):
    config = _read(golden, *scenario["config"].split("/"))
    expected = _read(golden, *scenario["expected"].split("/")).strip()
    commands = [
        line
        for line in _read(golden, *scenario["commands"].split("/")).splitlines()
        if line.strip()
    ]
    assert commands, scenario["name"]

    term = wt.Terminal(config)
    frame = ""
    for command in commands:
        frame = term.command(command)
    assert frame.strip() == expected, scenario["name"]


def test_the_corpus_covers_more_than_one_scenario():
    # A manifest that silently shrank to one entry would leave every parity test
    # passing while checking a fraction of what it used to.
    names = [s["name"] for _, s in _scenarios()]
    assert len(names) >= 7, names
    for expected in ("basic", "book_deltas", "footprint", "indicators", "seek"):
        assert expected in names
