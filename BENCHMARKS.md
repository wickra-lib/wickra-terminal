# Benchmarks

A trading terminal's frame budget is dominated by the terminal's own CPU work —
folding feed events into state and building the per-frame view-models — not by
rendering (the TUI draws a few kilobytes; the browser canvas is GPU-composited).
The benchmarks here measure that **core work per tick**, so it never becomes the
bottleneck under a fast feed.

## What is measured

The `terminal-bench` crate (criterion) covers:

- **`fold`** — folding a single feed event into `AppState` (the O(1) hot path),
  in events per second.
- **Order-book diff apply** — applying an L2 depth diff to a symbol's `BookState`
  and re-deriving the top-of-book, in updates per second.
- **View-model build** — building a `Frame` (all active panels → `PanelView`s)
  for the focused symbol, in frames per second.
- **`command_json`** — the round-trip of a command through the data-driven FFI
  boundary (parse JSON → apply → serialise the frame), in commands per second.

## Methodology

Run on a single core against fixed, representative in-process inputs, so the
numbers are reproducible and contain no feed variance:

```bash
cargo bench -p terminal-bench
```

## Results

Figures are recorded here once the benchmark crate lands in the hardening phase
(see [ROADMAP.md](ROADMAP.md), phase 7), measured with `cargo bench -p
terminal-bench` (criterion, 100 samples per benchmark) and reported as the median
estimate. Treat them as orders of magnitude, not guarantees — they vary with CPU
and toolchain.

## Caveats

These figures bound the terminal's own per-tick overhead only. End-to-end frame
latency in a live session also depends on the feed's message rate and, for the
Web renderer, the browser's compositor — neither of which these benchmarks
capture.
