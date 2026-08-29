"""Throughput benchmark for the wickra-terminal Python binding.

What this measures is the boundary, not the core. Every binding drives the same
Rust terminal through one function -- a command JSON in, a frame JSON out -- so
the number here is the cost of crossing the Python/Rust boundary once per
command, plus the cost of building and parsing the JSON on each side. It is not
a speed claim and not a cross-library ratio: there is nothing to compare
against, because the other side of every one of these calls is the same code.

The Rust core runs the identical loop with no boundary at all --
`cargo run -p wickra-terminal-example --bin throughput --release`. That is the
floor every binding here is measured against. The criterion benchmark in
`crates/wickra-terminal-bench` times the same call for tracking regressions,
but its per-iteration harness costs enough that it is not comparable to these.

Two commands are timed, because they differ in what crosses:

  Tick             the steady-state call. A small command in, a frame out whose
                   size depends on the panels configured.
  ListIndicators   the catalogue, and by far the largest payload the boundary
                   ever carries -- every registered indicator with its default
                   parameters. It is timed separately because a binding that
                   looks fine on small frames can be slow on a large one.

Install the binding first (`maturin develop --release` in bindings/python), then
run from the repository root:

    python -m benchmarks.throughput            # 20k ticks (default)
    python -m benchmarks.throughput --ticks 100000
"""

from __future__ import annotations

import argparse
import json
import time

from wickra_terminal import Terminal

# Shared by all nine binding benchmarks, so the numbers compare. A synth source
# needs no network and no fixture, and is deterministic given its seed; three
# panels make the frame representative rather than trivially small.
CONFIG = json.dumps(
    {
        "sources": [{"Synth": {"seed": 1}}],
        "layout": {
            "panels": [
                {"kind": "Chart", "rect": {"x": 0, "y": 0, "w": 100, "h": 40}},
                {"kind": "Book", "rect": {"x": 0, "y": 40, "w": 50, "h": 30}},
                {"kind": "Tape", "rect": {"x": 50, "y": 40, "w": 50, "h": 30}},
            ]
        },
    }
)
SUBSCRIBE = json.dumps({"type": "Subscribe", "source": 0, "symbol": "BTC/USDT"})
TICK = json.dumps({"type": "Tick"})
LIST = json.dumps({"type": "ListIndicators"})

# The catalogue response is ~30 kB, so a hundred of them is a noisy sample --
# noisy enough that the no-boundary baseline once looked slower than a binding.
CATALOGUE_REPS = 1000


def median_ns(run, reps: int = 3) -> float:
    """Median elapsed ns over a few repetitions, after one warmup pass."""
    run()
    samples = []
    for _ in range(reps):
        start = time.perf_counter_ns()
        run()
        samples.append(time.perf_counter_ns() - start)
    samples.sort()
    return samples[len(samples) // 2]


def main() -> None:
    parser = argparse.ArgumentParser(description="wickra-terminal Python throughput")
    parser.add_argument("--ticks", type=int, default=20_000, help="commands per sample")
    args = parser.parse_args()
    ticks = args.ticks if args.ticks >= 100 else 20_000

    term = Terminal(CONFIG)
    term.command(SUBSCRIBE)
    frame_bytes = len(term.command(TICK))
    catalogue_bytes = len(term.command(LIST))

    def tick_loop() -> None:
        for _ in range(ticks):
            term.command(TICK)

    def list_loop() -> None:
        for _ in range(CATALOGUE_REPS):
            term.command(LIST)

    tick_ns = median_ns(tick_loop)
    list_ns = median_ns(list_loop)

    print(f"wickra-terminal Python throughput - {ticks:,} commands (median of 3)\n")
    print(f"{'Command':<18}{'per second':>14}{'us/command':>14}{'payload':>12}")
    print("-" * 58)
    print(
        f"{'Tick':<18}{ticks / (tick_ns / 1e9):>14,.0f}"
        f"{tick_ns / ticks / 1e3:>14.2f}{frame_bytes:>11,}B"
    )
    print(
        f"{'ListIndicators':<18}{CATALOGUE_REPS / (list_ns / 1e9):>14,.0f}"
        f"{list_ns / CATALOGUE_REPS / 1e3:>14.2f}{catalogue_bytes:>11,}B"
    )
    print(
        "\nOne command crosses the boundary once. Higher is better, and the numbers\n"
        "are machine-dependent -- compare bindings on one machine, never across two."
    )


if __name__ == "__main__":
    main()
