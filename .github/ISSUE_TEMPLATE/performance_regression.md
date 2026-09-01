---
name: Performance regression
about: Report a measurable slowdown in the fold, the frame budget, or a binding boundary.
title: "[perf] <code path> regressed in <version>"
labels: ["performance", "regression", "triage"]
assignees: []
---

## Summary

<!-- Which code path got slower, by how much, and since when? -->

## Affected code path

- [ ] Event fold (`AppState::apply`) — the per-event work
- [ ] Frame build (`Terminal::frame`) — panels to view-models
- [ ] Indicator set update — one tick across every tracked indicator
- [ ] A binding boundary — the JSON in, JSON out round trip
- [ ] TUI draw
- Which: `e.g. Terminal::tick with 5 panels and 20 indicators`

## Versions compared

| Version | Median | Notes |
| --- | --- | --- |
| `0.1.0` | `e.g. 41 µs/frame` | baseline (good) |
| `0.1.1` | `e.g. 96 µs/frame` | regressed |

## Benchmark / reproducer

<!--
The criterion invocation and its output, or the per-binding throughput harness
under bindings/<lang>/benchmarks/. Say how many symbols, panels and indicators
were configured — the frame cost is a function of all three, so a number without
them cannot be compared to anything.
-->

```bash
cargo bench -p wickra-terminal-bench -- --save-baseline new
```

```
tick/5-panels           time:   [95.8 µs 96.1 µs 96.5 µs]
                        change: [+133.1% +134.2% +135.4%] (p = 0.00 < 0.05)
                        Performance has regressed.
```

## Configuration measured

```json
{ "sources": [{ "Synth": { "seed": 1 } }], "timeframe": "1m",
  "indicators": [], "layout": { "panels": [] } }
```

## Hardware / environment

| Field | Value |
| --- | --- |
| CPU | `e.g. Ryzen 9 9950X` |
| OS / arch | `e.g. Linux 6.8 x86_64` |
| Toolchain | `rustc 1.x.y` |
| Build flags | `--release`, `RUSTFLAGS=...` |

## Suspected cause

<!-- Optional. Link the commit or PR if you have bisected it. -->
