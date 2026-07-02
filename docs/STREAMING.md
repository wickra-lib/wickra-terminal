# Streaming and state

The terminal is streaming-native: it folds a feed of events into state one event
at a time, in **O(1) per event**, and never recomputes over history. That is the
whole moat — it is what lets the core sustain tens of thousands of frames per
second regardless of how long a session has run.

## The fold

[`AppState::fold`](../crates/terminal-core/src/state.rs) applies a single event
to one `(SourceId, Symbol)`'s state incrementally:

- **Trade** → update `last`, push into the bounded `TapeRing`, add to the
  `Footprint` (per-price buy/sell volume), advance the `IndicatorSet`, and append
  to a bounded price history.
- **Ticker** → update `last`.
- **BookSnapshot / BookDelta** → apply to the local L2 `BookState` (a
  `BTreeMap`-backed book; a zero-quantity level is a removal).
- Account / lifecycle events do not affect per-symbol market state.

Every buffer is bounded (the tape ring, the price history), so memory is bounded
regardless of session length.

## Determinism and the golden corpus

The fold is deterministic: the same recorded feed always produces the same frame.
[`golden/`](../golden/) pins this byte-for-byte — a recorded feed
(`replay/basic.json`) drives the terminal and must produce the exact frame
view-models in `expected/basic.json`. Because the same command protocol crosses
every binding, these fixtures are also the **cross-language parity corpus**: a
binding replaying the feed must reproduce the same frame.

Regenerate the fixtures after an intentional schema change:

```bash
WICKRA_REGEN=1 cargo test -p terminal-core --test golden
```

## Property and fuzz coverage

- `tests/proptest_invariants.rs` — arbitrary event streams keep the tape ring
  within its cap, track the last price and keep the book top-of-book ordered.
- `fuzz/` — four cargo-fuzz targets (`feed_event`, `state_fold`, `view_model`,
  `config_parse`) drive arbitrary bytes through the parsing and fold paths; none
  may panic. The footprint accumulator saturates rather than overflowing on
  adversarial volumes.

## Performance

Measured with `cargo bench -p terminal-bench` (see [../BENCHMARKS.md](../BENCHMARKS.md)):
folding one event ~333 ns, a full tick (poll + fold + build every panel) ~25 µs.
