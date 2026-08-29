# Benchmarks

A trading terminal's frame budget is dominated by the terminal's own CPU work —
folding feed events into state and building the per-frame view-models — not by
rendering (the TUI draws a few kilobytes; the browser canvas is GPU-composited).
The benchmarks here measure that **core work per tick**, so it never becomes the
bottleneck under a fast feed.

## What is measured

The `wickra-terminal-bench` crate (criterion) covers six paths, one benchmark each:

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

- **`command_json_bench_config`** — the same call on the three-panel config the
  per-binding benchmarks build, so the section below has a no-boundary number
  measured on identical work. `command_json_tick` above uses the default
  five-panel layout with its default indicators, which is a heavier tick and
  not comparable to them.

## Methodology

Run on a single core against fixed, in-process inputs, so the numbers are
reproducible and contain no feed variance:

```bash
cargo bench -p wickra-terminal-bench
```

## Results

Criterion defaults (100 samples per benchmark) on a Windows x86-64 laptop,
single-threaded, with the default indicator overlay of two price indicators.
Figures are the median estimate; treat them as orders of magnitude rather than
guarantees — they move with the CPU, the toolchain and the number of indicators
configured.

Measured under `[profile.bench]`, which carries the same `lto = "fat"` and single
codegen unit as `[profile.release]`. That matters: an earlier set of figures here
was taken before those settings existed, so it described a binary nobody ships
and understated the released one by roughly a tenth.

| Benchmark           | What                                          | Median   | Throughput |
|---------------------|-----------------------------------------------|----------|------------|
| `fold_trade`        | fold one trade into `AppState`                | 142 ns   | ~7.0 M/s   |
| `book_delta`        | apply an L2 depth diff (six levels, two removals) | 107 ns | ~9.3 M/s |
| `frame_build`       | build all five panels' view-models            | 8.9 µs   | ~113 K/s   |
| `tick_synth`        | poll + fold + build the frame                 | 9.7 µs  | ~103 K/s    |
| `command_json_tick` | the same tick across the FFI boundary         | 17.7 µs  | ~56 K/s    |
| `command_json_bench_config` | the same, on the three-panel config the per-binding benchmarks use | 16.2 µs | ~62 K/s |

The takeaway: a full tick that rebuilds every panel's view-model costs about ten
microseconds, so the core sustains a hundred thousand frames per second — far
above any renderer's frame budget, which is the whole point of the O(1) fold.

Two readings are worth explaining rather than leaving to look odd.

`tick_synth` sits just above `frame_build`, and the gap is small because a tick
adds polling and folding to a frame build and those cost hundred-nanosecond
amounts against a nine-microsecond baseline. Under the older, un-LTO'd figures
the two were not separable at all; they are now, by about eight percent, which is
roughly what the added work should cost. That is also where the time goes:
building view-models, not folding events.

`command_json_tick` costs roughly twice a bare tick. The extra is JSON — parsing
the command and serialising a frame of five panels — not the terminal's work, and
it is the price every binding pays for the boundary being data rather than an
API. It is also why the frame is serialised once per tick rather than per panel.

## Per-binding throughput — what the boundary costs

The figures above are the core with no language boundary in the way. This
section answers the question the README's "one core in ten languages" invites
and never had a number for: what does the boundary cost?

Each binding ships a `throughput` benchmark under `bindings/<lang>/benchmarks/`.
All ten build the same three-panel synth config, subscribe once, and time a
tight loop of `Tick` commands — median of three runs after a warmup. The Rust
row is the same loop with no boundary at all
(`cargo run -p wickra-terminal-example --bin throughput --release`), which is
what the other nine are measured against.

50,000 commands, one Windows x86-64 desktop, release builds throughout. Same
machine for every row; that is the only way these compare.

| Surface | commands/s | µs/command | over the floor |
|---------|-----------:|-----------:|---------------:|
| **Rust — no boundary** | 67,321 | 14.85 | — |
| Node (napi-rs) | 65,709 | 15.22 | +0.4 |
| C (the ABI itself) | 65,142 | 15.35 | +0.5 |
| Java (FFM) | 62,449 | 16.01 | +1.2 |
| Python (PyO3) | 62,142 | 16.09 | +1.2 |
| C# (P/Invoke) | 60,711 | 16.47 | +1.6 |
| Go (cgo) | 60,008 | 16.66 | +1.8 |
| C++ (RAII header) | 59,599 | 16.78 | +1.9 |
| R (`.Call`) | 44,696 | 22.37 | +7.5 |
| WASM (wasm-bindgen) | 36,224 | 27.61 | +12.8 |

**The headline is the first eight rows, and it is that the boundary is nearly
free.** Eight of the nine sit within 2 µs of a Rust call that crosses nothing,
and the top four are inside a microsecond. That is not because the bindings are
fast; it is because the command dominates. Roughly 15 µs goes on parsing the
command JSON, folding the tick and serialising a three-panel frame — work every
row pays identically — and the crossing itself is a fraction of a microsecond on
top. Choose a language for the ecosystem you want, not for this table.

The two rows that do separate are the two whose boundary is not a function call:

- **WASM** copies every string into and out of linear memory in both directions.
  At 341 bytes a frame that is measurable, and on the ~30 kB catalogue response
  it is the largest gap in the whole set.
- **R** pays for the interpreted loop around `.Call`, not for `.Call` itself.

**C++ is the one row worth reading against its neighbour.** It calls the same
five functions as C, so the 1.4 µs between them is the ownership layer:
`wickra_terminal.hpp` copies the returned frame into a `std::string` and frees
the original. That is the price of not freeing by hand on every path including
the ones that throw, and it is visible here precisely so the trade is explicit.

Each harness also times `ListIndicators`, the ~30 kB catalogue and the largest
payload the boundary ever carries. Those numbers are printed but deliberately
not tabulated: on a shared desktop their run-to-run spread exceeds the
differences between bindings — enough that the no-boundary baseline sometimes
measured slower than a binding, which cannot be true. Read them locally to see
how a surface handles a large response; do not read them as a ranking.

### Running them

```bash
cargo build -p wickra-terminal-c --release        # the C ABI, for six of the ten

cargo run -p wickra-terminal-example --bin throughput --release
cmake -S bindings/c/benchmarks   -B bindings/c/benchmarks/build   && cmake --build bindings/c/benchmarks/build   --config Release
cmake -S bindings/cpp/benchmarks -B bindings/cpp/benchmarks/build && cmake --build bindings/cpp/benchmarks/build --config Release
(cd bindings/python && python -m benchmarks.throughput)
(cd bindings/node   && node benchmarks/throughput.js)
node bindings/wasm/benchmarks/throughput.mjs
(cd bindings/go && go run ./benchmarks)
dotnet run --project bindings/csharp/benchmarks -c Release
Rscript bindings/r/benchmarks/throughput.R
```

Java compiles as a single file against the built binding rather than carrying a
second Maven module; the command is in the header of
`bindings/java/benchmarks/Throughput.java`.

Every harness takes a command count as its one argument, so a longer run is
`... 100000`.

## Caveats

These figures bound the terminal's own per-tick overhead only. End-to-end frame
latency in a live session also depends on the feed's message rate and, for the
Web renderer, the browser's compositor — neither of which these benchmarks
capture.

The indicator count is a direct multiplier on `fold_trade` and `tick_synth`: the
numbers above are the two-indicator default. A configuration tracking twenty
indicators does twenty indicators' work per trade, and the registry offers <!--indicator-count-->475<!--/indicator-count-->.
