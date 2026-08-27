# Benchmarks

A trading terminal's frame budget is dominated by the terminal's own CPU work —
folding feed events into state and building the per-frame view-models — not by
rendering (the TUI draws a few kilobytes; the browser canvas is GPU-composited).
The benchmarks here measure that **core work per tick**, so it never becomes the
bottleneck under a fast feed.

## What is measured

The `terminal-bench` crate (criterion) covers five paths, one benchmark each:

- **`fold_trade`** — folding one trade into `AppState`: the tape, the footprint,
  the price history, the candle builder and every configured indicator.
- **`book_delta`** — applying an L2 depth diff to a symbol's `BookState`,
  including level removals. The highest-rate message on a live feed: a venue
  sends far more depth updates than trades, so this is the fold path that decides
  whether a busy market keeps up.
- **`frame_build`** — building a `Frame` from state, every configured panel to a
  view-model, with no polling. What a renderer pays for a redraw that has no new
  data behind it.
- **`tick_synth`** — one full tick: poll the sources, fold what arrived, build
  the frame.
- **`command_json_tick`** — the same tick through the data-driven FFI boundary:
  parse the command JSON, apply it, serialise the frame. This is what every
  binding pays per call.

## Methodology

Run on a single core against fixed, in-process inputs, so the numbers are
reproducible and contain no feed variance:

```bash
cargo bench -p terminal-bench
```

## Results

Criterion defaults (100 samples per benchmark) on a Windows x86-64 laptop,
single-threaded, with the default indicator overlay of two price indicators.
Figures are the median estimate; treat them as orders of magnitude rather than
guarantees — they move with the CPU, the toolchain and the number of indicators
configured.

| Benchmark           | What                                          | Median   | Throughput |
|---------------------|-----------------------------------------------|----------|------------|
| `fold_trade`        | fold one trade into `AppState`                | 157 ns   | ~6.4 M/s   |
| `book_delta`        | apply an L2 depth diff (six levels, two removals) | 115 ns | ~8.7 M/s |
| `frame_build`       | build all five panels' view-models            | 9.8 µs   | ~102 K/s   |
| `tick_synth`        | poll + fold + build the frame                 | 10.7 µs  | ~94 K/s    |
| `command_json_tick` | the same tick across the FFI boundary         | 19.9 µs  | ~50 K/s    |

The takeaway: a full tick that rebuilds every panel's view-model costs about ten
microseconds, so the core sustains tens of thousands of frames per second — far
above any renderer's frame budget, which is the whole point of the O(1) fold.

Two readings are worth explaining rather than leaving to look odd.

`frame_build` and `tick_synth` are within noise of each other, and a tick does
strictly more work than a frame build. The polling and folding a tick adds are in
the hundred-nanosecond range, which is under a percent of the ten microseconds
the frame build costs, so the two are not separable at this sample size. That
also says where the time actually goes: building view-models, not folding events.

`command_json_tick` costs roughly twice a bare tick. The extra is JSON — parsing
the command and serialising a frame of five panels — not the terminal's work, and
it is the price every binding pays for the boundary being data rather than an
API. It is also why the frame is serialised once per tick rather than per panel.

## Caveats

These figures bound the terminal's own per-tick overhead only. End-to-end frame
latency in a live session also depends on the feed's message rate and, for the
Web renderer, the browser's compositor — neither of which these benchmarks
capture.

The indicator count is a direct multiplier on `fold_trade` and `tick_synth`: the
numbers above are the two-indicator default. A configuration tracking twenty
indicators does twenty indicators' work per trade, and the registry offers 421.
