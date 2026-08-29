# Streaming and state

The terminal is streaming-native: it folds a feed of events into state one event
at a time, in **O(1) per event**, and never recomputes over history. That is the
whole moat — it is what lets the core sustain a hundred thousand frames per
second regardless of how long a session has run.

## The fold

[`AppState::fold`](../crates/wickra-terminal-core/src/state.rs) applies a single event
to one `(SourceId, Symbol)`'s state incrementally:

- **Trade** → update `last`, push into the bounded `TapeRing`, add to the
  `Footprint` (per-price buy/sell volume), fold into the `CandleBuilder`, advance
  the `IndicatorSet` with the resulting tick, and append to a bounded price
  history.
- **Ticker** → update `last`.
- **BookSnapshot / BookDelta** → apply to the local L2 `BookState` (a
  `BTreeMap`-backed book; a zero-quantity level is a removal).
- Account / lifecycle events do not affect per-symbol market state.

Every buffer is bounded — the tape ring at 256 prints, the price history at 512,
each indicator's series at 120, the footprint at 1,024 price levels, a manual
source's pending queue at 4,096 events — so memory is bounded regardless of
session length, and regardless of a host that feeds without ticking. The book is bounded by the market rather
than by a constant: it holds the levels a venue publishes.

The footprint was the exception until recently, keeping an entry per distinct
price ever traded. A 200k-print walk left 2,926 levels still climbing, and a
BTC/USDT feed quoting to the cent gives hundreds of thousands. It now evicts
whichever end is furthest from the price being traded, so the profile follows the
market instead of accumulating every price a session has touched.

## Bars, from the same fold

More than half the Wickra indicator set reads a `Candle`, and a terminal has
ticks. The same trade that advances the price indicators is folded into a
[`CandleBuilder`](../crates/wickra-terminal-core/src/candle.rs), which returns a closed
bar on the tick that crosses a bar boundary and nothing on every other tick. So a
price indicator advances once per trade and a bar indicator once per bar, from
one code path and one pass.

Only closed bars reach an indicator. Feeding the bar in progress would make every
reading repaint as the bar fills — the last print of a minute silently rewriting
what the previous print produced. The forming bar is available separately, for a
renderer that wants to draw it.

## Determinism and the golden corpus

The fold is deterministic: the same recorded feed always produces the same frame.
[`golden/`](../golden/) pins this byte-for-byte — a recorded feed
(`replay/basic.json`) drives the terminal and must produce the exact frame
view-models in `expected/basic.json`. Because the same command protocol crosses
every binding, these fixtures are also the **cross-language parity corpus**: a
binding replaying the feed must reproduce the same frame.

Regenerate the fixtures after an intentional schema change:

```bash
WICKRA_REGEN=1 cargo test -p wickra-terminal-core --test golden
```

## Property and fuzz coverage

- `tests/proptest_invariants.rs` — arbitrary event streams keep the tape ring
  within its cap, track the last price and keep the book top-of-book ordered.
- `fuzz/` — four cargo-fuzz targets (`feed_event`, `state_fold`, `view_model`,
  `config_parse`) drive arbitrary bytes through the parsing and fold paths; none
  may panic. The footprint accumulator saturates rather than overflowing on
  adversarial volumes.

## Performance

Measured with `cargo bench -p wickra-terminal-bench` (see [../BENCHMARKS.md](../BENCHMARKS.md)):
folding one trade ~142 ns, applying an L2 depth diff ~107 ns, a full tick (poll +
fold + build every panel) ~9.7 µs.

The split matters more than the totals: the fold is nanoseconds and building the
view-models is microseconds, so the O(1) fold is not what a renderer waits on.
The indicator count is a direct multiplier on the fold — those figures are the
two-indicator default, and the registry offers <!--indicator-count-->461<!--/indicator-count-->.
