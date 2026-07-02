"""Cross-language golden parity: build the terminal from the committed
``golden/config.json``, replay the feed, and assert the frame equals
``golden/expected/basic.min.json`` byte-for-byte. Because the binding returns the
core's compact ``command_json`` string verbatim, byte equality against that one
file is the exact cross-language parity check.
"""

import os

import wickra_terminal as wt


def _golden_dir():
    d = os.path.dirname(os.path.abspath(__file__))
    for _ in range(8):
        g = os.path.join(d, "golden")
        if os.path.isfile(os.path.join(g, "config.json")):
            return g
        d = os.path.dirname(d)
    raise AssertionError("golden/ not found")


def test_golden_parity_frame_is_byte_exact():
    g = _golden_dir()
    with open(os.path.join(g, "config.json"), encoding="utf-8") as f:
        config = f.read()
    with open(os.path.join(g, "expected", "basic.min.json"), encoding="utf-8") as f:
        expected = f.read().strip()

    term = wt.Terminal(config)
    term.command('{"type":"Subscribe","source":0,"symbol":"BTC/USDT"}')
    frame = ""
    for _ in range(32):
        frame = term.command('{"type":"Tick"}')
    assert frame.strip() == expected
