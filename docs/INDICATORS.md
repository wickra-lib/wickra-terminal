# Indicators

The terminal drives the [Wickra](https://github.com/wickra-lib/wickra) indicator
set directly: **<!--indicator-count-->497<!--/indicator-count--> of them**, constructible by name from a config or at run
time, in every binding.

## Naming one

An indicator is a registry name and its positional parameters:

```json
{
  "sources": [{ "Synth": { "seed": 1 } }],
  "timeframe": "1m",
  "indicators": [
    { "kind": "Sma", "params": [20] },
    { "kind": "Rsi", "params": [14] },
    { "kind": "MacdIndicator", "params": [12, 26, 9] },
    { "kind": "AdaptiveCycle" }
  ]
}
```

The name is the `wickra-core` type name, and the parameters are in the order
that type's constructor takes them — `Sma::new(period)` takes one, so `Sma`
takes one. An indicator with no parameters may omit `params` entirely.

Omitting `indicators` gives the default overlay, a short and a long moving
average. Omitting `timeframe` gives one-minute bars.

Each indicator is labelled from its spec: `Sma(20)`, `MacdIndicator(12,26,9)`,
`AdaptiveCycle`. That label is what the chart panel shows and what
`RemoveIndicator` takes.

## What a tick feeds

The terminal folds individual trades, but most of the indicator set reads
something other than the bare price. All nine families are driven from the same
tick, and each advances only on a tick that carries what it consumes:

| Input | Fed with | Advances |
|-------|----------|----------|
| price (`f64`) | the last traded price | on every trade |
| bar (`Candle`) | the bar that just closed | once per `timeframe` |
| tape (`Trade`) | the print, with its size and aggressor side | on every trade |
| book (`OrderBook`) | the locally maintained L2 book | on every trade |
| pairwise (`(f64, f64)`) | this market's price and a reference market's | on every trade |
| returns | the close-to-close return of the bar that just closed | once per `timeframe` |
| cross-section (`CrossSection`) | the breadth of a named universe of markets | once per `timeframe` |
| derivatives (`DerivativesTick`) | funding and open interest as fed, with the taker flow folded from the tape | on every trade |
| quoted trade (`TradeQuote`) | the print, with the mid it arrived against | on every trade |

The tape and book families read state the terminal already keeps, converted into
the core's types once per tick and shared across the whole set rather than
converted per indicator. The book is converted only when some indicator in the
set actually reads it, so the default price-and-bar configuration never walks the
book for nothing.

A book that is momentarily one-sided or crossed — an ordinary thing to see
between a snapshot and the diffs that follow it — yields no value that tick. The
book indicators simply do not advance, the same as while warming up.

## Pairwise indicators

The pairwise family measures one market against another — beta, correlation,
cointegration, spread z-score — so a spec has to say which market. That is the
`reference` field, written as a symbol:

```json
{ "kind": "Beta", "params": [20], "reference": "ETH/USDT" }
```

The reference is part of the label, because `Beta(20)` against BTC and the same
against ETH are different readings: this one shows as `Beta(20) vs ETH/USDT`,
which is also the label `RemoveIndicator` takes.

A pairwise kind with no reference is **refused** rather than given a default.
Which market it compares against changes what it measures, so a guessed one
would produce a plausible number about the wrong thing. `ListIndicators` marks
these rows with `"needs_reference": true`, so a caller can tell before it tries.

The reference market has to be one the terminal is tracking, and it has to have
printed: a pairwise indicator does not advance on a tick where its reference has
no price yet. Feeding it a placeholder would produce a reading that looks real.

Only **closed** bars reach an indicator. Feeding the bar in progress would make
every reading repaint as the bar fills — the last print of a minute silently
rewriting what the previous print produced. The forming bar is still available to
renderers that want to draw it; it is simply not what an indicator sees.

So a bar indicator on a quiet market reports nothing until a bar closes, and that
is correct rather than a stall. `Atr(14)` at a one-hour timeframe needs fourteen
hours of trading before its first value, however busy the tape is.

## Discovering what is available

`ListIndicators` answers with the catalogue — the one command that answers rather
than renders:

```json
{ "type": "ListIndicators" }
```

```json
{ "indicators": [{ "kind": "Sma", "params": [14] }, ...] }
```

Every row carries parameters that construct it, so discovery needs no second
lookup. Those are wickra's own reference values, not the terminal's default
overlay: the catalogue answers *what this build can do*, the overlay *what it is
showing right now*.

## Changing them while running

```json
{ "type": "AddIndicator", "spec": { "kind": "Atr", "params": [14] } }
{ "type": "RemoveIndicator", "label": "Atr(14)" }
```

The bar size can change too:

```json
{ "type": "SetTimeframe", "timeframe": "5m" }
```

That restarts the bar-derived state: each market opens a new bar and the
indicator set is rebuilt. Rebuilding all of them rather than only the bar ones is
deliberate — an indicator's history is a sequence of readings at one bar size,
and continuing it across a change would blend two, which is neither the smaller
size nor the larger one. The price history, tape, book and footprint are
untouched, since none of them comes from bars.

`AddIndicator` starts the new indicator cold. A market that has been running
keeps its price history, but the inputs this indicator missed are gone, so it
warms up from the next tick rather than pretending to have seen them. Adding a
label that is already tracked is rejected instead of quietly duplicating a row.

Both apply to every market at once, and to markets opened later — the indicator
set belongs to the terminal, not to one symbol.

## Multi-output indicators

84 of the registered indicators produce a struct rather than a number. They
report every field by name, in declaration order:

```json
{
  "name": "MacdIndicator(12,26,9)",
  "value": -1.42,
  "fields": [
    { "name": "macd", "value": -1.42 },
    { "name": "signal", "value": -1.19 },
    { "name": "histogram", "value": -0.23 }
  ]
}
```

`value` is the first field, so a renderer that wants one line does not have to
know which field that is. A single-output indicator omits `fields` from the JSON
entirely, so a consumer written against the simple shape sees exactly the object
it saw before.

## What is not registered, and why

1 of the 504 indicators in `wickra-core` is not reachable from the terminal.
The generator lists what it skipped, with a reason, every time it runs:

| Missing | Count | Why |
|---------|------:|-----|
| level output | 1 | `Footprint` answers with a list of price LEVELS, each with its own bid and ask volume |

`Footprint` is the one that looks like an omission and is not. The terminal
already renders a footprint -- from its own per-price state, as the `footprint`
panel -- so the indicator would be a second implementation of a view that
exists, in a shape the registry cannot carry.

## Profiles: a histogram is not a reading

Six indicators answer with a **distribution** rather than a number:
`VolumeProfile` and `TpoProfile` over price, and `DayOfWeekProfile`,
`IntradayVolatilityProfile`, `TimeOfDayReturnProfile` and `VolumeByTimeProfile`
over the clock.

They are reachable, and they are not in the registry. A registry entry promises
one name, one number and a fixed set of named fields; a distribution is none of
those, and its length changes as the session runs. Squeezing one in means
reporting a single bin under the whole indicator's name -- which is exactly what
`VolumeProfile` did before it was removed: it reported `price_low`, a price,
under a profile's name.

So they have a surface of their own, alongside the registry rather than inside
it. Configure them under `profiles` and show them with a `Profile` panel:

```json
{
  "profiles": [{ "kind": "VolumeProfile", "params": [20, 50] }],
  "layout": { "panels": [{ "kind": "Profile", "rect": { "x": 0, "y": 0, "w": 100, "h": 100 } }] }
}
```

The panel answers with one row per configured profile: its label, its bins in
order, and -- for the two that are distributions over price -- the range those
bins cover. A profile over time reports no range rather than zeros, so a
consumer can tell "spans no price" from "spans zero to zero".

`ListProfiles` is not a command; the six are a fixed set and the
`registry::PROFILES` constant carries them with the parameters the wickra golden
manifest pins them at.
## Alternative bars: not a reading at all

Ten of wickra's types are not indicators. `RenkoBars`, `KagiBars`,
`PointAndFigureBars`, `ThreeLineBreakBars`, `TickBars`, `VolumeBars`,
`DollarBars`, `RangeBars`, `ImbalanceBars` and `RunBars` implement a different
trait — `BarBuilder` — and answer with *bars*.

They are also not a function of time. One closed candle completes zero, one or
several of them: a quiet hour produces no Renko bricks and a fast one produces
a run. That unevenness is the character of the chart rather than a defect, and
it is why they cannot be a reading — there is nothing to report on a tick that
completed nothing.

Configure them under `bars` and show them with a `Bars` panel:

```json
{
  "bars": [{ "kind": "RenkoBars", "params": [2.0] }],
  "layout": { "panels": [{ "kind": "Bars", "rect": { "x": 0, "y": 0, "w": 100, "h": 100 } }] }
}
```

The ten emit ten different bar types, in two shapes: a two-point bar recording
where a move started and ended, and an OHLC bar recording a range. The panel
answers with one shape, `AltBar` — open, high, low, close, direction, and a
volume for the types that measure one. The mapping is written down per bar type
in the generator rather than guessed: a Kagi bar's `start` and `end` become open
and close with the high and low derived, and a point-and-figure column opens at
its low and closes at its high when it is rising, the other way round when it is
not.

`volume` is absent rather than zero for the types that have none. A Renko brick
is a price move, not a period; reporting zero would read as "no volume traded".

## What the terminal reaches, in full

| Surface | Count | What it answers with |
|---------|------:|----------------------|
| registry | <!--indicator-count-->497<!--/indicator-count--> | one number, plus named fields |
| profiles | 6 | a histogram |
| alternative bars | 10 | bars, zero or more per candle |
| the `footprint` panel | 1 | volume per price, from the terminal's own state |

That is **514** — every indicator and bar builder wickra ships. They are not one
list because they are not one kind of thing, and flattening them would mean a
catalogue whose entries answer with three different shapes and a consumer that
has to know which.

## Regenerating the registry

`crates/wickra-terminal-core/src/registry.rs` is generated. It reads the wickra-core
sources — the `Indicator` impls, their `new` signatures and their Output structs
— so it cannot drift from the library: a renamed constructor argument becomes a
compile error on the next regeneration rather than a wrong value at run time.

```bash
python tools/gen_registry.py --wickra ../wickra --out crates/wickra-terminal-core/src/registry.rs
cargo fmt --all
```

Do not edit it by hand.

The default parameters it emits come from wickra's `testdata/golden/golden_manifest.json`,
which is what the library pins its own reference outputs with.

`crates/wickra-terminal-core/tests/registry_completeness.rs` drives every registered
indicator, and guards against the set shrinking: the generator run against an
older or partial wickra checkout would emit a smaller file that still compiles
and whose every remaining entry still passes.

## See also

- [Panels](PANELS.md) — where indicator values are rendered
- [Sources](SOURCES.md) — what feeds the ticks
- [Streaming](STREAMING.md) — the O(1) fold the indicators sit on
- [Cookbook](Cookbook.md) — runnable configurations
