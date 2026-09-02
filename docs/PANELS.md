# Panels

A panel is a pure function from [`AppState`](../crates/wickra-terminal-core/src/state.rs)
and the focused market to a [`PanelView`](../crates/wickra-terminal-core/src/view.rs) —
a plain data description of what to draw (values, series, sides), never a
renderer command. Adding a panel makes it appear in **every** renderer at once,
because each renderer is only a mapping from `PanelView` to its own widget.

## Built-in panels

| Kind | View-model | Shows |
|------|------------|-------|
| `Chart` | `ChartView` | the bars of the configured timeframe, a tick-resolution price series, and the indicator readings for the focused market |
| `Book` | `BookView` | the top of the L2 order book (bids and asks) and the spread |
| `Tape` | `TapeView` | the most recent trade prints, coloured by aggressor side |
| `Footprint` | `FootprintView` | per-price buy and sell volume (a volume profile) |
| `Watchlist` | `WatchlistView` | the last price of every tracked `(source, symbol)` |

The `PanelView` enum is internally tagged by `panel` (`"chart"`, `"book"`, …);
each variant's struct fields are flattened alongside the tag, so a frame is a
plain, language-neutral JSON document.

## What each view-model carries

This is the surface a binding consumer actually reads, so it is worth stating
rather than leaving to be discovered from a sample frame.

**`chart`** — `symbol`, `last`, `bars`, `forming`, `series` and `indicators`.

`bars` is up to 120 closed bars of the configured timeframe, oldest first, each
an `open`, `high`, `low`, `close`, `volume` and the bar's opening `timestamp`.
It is omitted from the JSON until the first bar closes, which at an hourly
timeframe is the first hour of a session.

`forming` is the bar still accumulating, in the same shape. It is kept apart
from `bars` rather than appended, because it is the one bar that will still
change: an indicator never sees it — a reading that repainted as its bar filled
would be a different number on every print — but a chart that omitted it would
show the market frozen at the last close for a whole bar.

`series` is up to 120 recent prices, oldest first, one point per trade. It is
the finer of the two and does not wait for a bar to close, which is what makes
it the right thing to draw before any bar has.

Each entry in `indicators` carries its `name` (the label from its spec,
`Sma(20)`), its primary `value` (`null` while warming up), an optional `fields`
list for the multi-output ones, and an optional `series` of its own. See
[INDICATORS.md](INDICATORS.md).

`value` is the *first* field of a multi-output indicator, so that a consumer
wanting one line does not have to know which field that is -- and both reference
renderers read only it for a while, which drew `Macd(12,26,9)=1.42` and dropped
the signal line and the histogram. They read `fields` now and write
`Macd(12,26,9)[macd=1.42 signal=1.25 histogram=0.17]`, the same shape in both,
because one indicator read in two places should not look like two.

> Both reference renderers draw the candles and report the indicators as
> numbers, rather than drawing indicator lines over the candles. An indicator's
> `series` is sampled once per tick and the bars are one per bar, so the two do
> not share an x-axis; an average drawn on the candle axis would sit near, but
> not on, the bar it was computed from. Overlays are drawn only in the
> before-the-first-bar fallback, where the price series and the indicator series
> do share an axis.

**`book`** — `symbol`, `bids` and `asks` (up to 12 levels each, best first, every
level a `price` and a `quantity`), and `spread` (`null` until both sides have a
level).

**`tape`** — `symbol` and `prints`, up to 24 of them, newest first. Each print is
a `price`, a `quantity`, a `side` (`"buy"` or `"sell"`, the aggressor) and a
`timestamp`.

**`footprint`** — `symbol` and `levels`, each a `price` with its accumulated
`buy` and `sell` volume, highest price first. The levels are the ones nearest the
last trade, not the highest ever traded: anchoring them is what keeps the ladder
on the market after a move.

**`watchlist`** — `rows`, one per tracked market, each a `source` id, a `symbol`,
its `last` price, the venue's `bid`, `ask` and rolling base-asset `volume`, and
the percentage `change` from the first price the terminal folded for that market.
This is the only panel that does not render the focused market: it renders all of
them.

The `bid`, `ask` and `volume` come off the venue's ticker stream and are `0.0`
until the first ticker arrives, which is how a renderer tells "no quote yet" from
a genuine zero: it shows a dash rather than a spread of nothing. The book carries
a best bid too and it is not the same number — the book's is whatever the depth
stream has delivered, and a venue that publishes a truncated book publishes an
untruncated ticker.

`change` is measured from the first price this terminal saw for the market, not
from the venue's session open: a venue's day boundary is its own and the terminal
is not told where it falls. When a subscription is backfilled the open is the
oldest bar's, so the change covers the history the chart draws rather than
restarting at whatever tick arrived first. It is computed in the core rather than
in each renderer, so the terminal and the browser cannot derive it differently.

Every number crosses the boundary as a JSON number. Internally prices and
quantities are `Decimal`; the conversion happens here, at the edge, because a
view-model is drawn rather than compared.

## Changing the layout while it runs

`Config.layout.panels` was read once when the terminal was built and never
again, so a terminal opened with the wrong panels had to be restarted with a
different config -- which is not something a person does while watching a market
move. Three commands change it:

```json
{"type":"AddPanel","spec":{"kind":"Book","rect":{"x":50,"y":0,"w":50,"h":100},"depth":24}}
{"type":"MovePanel","index":1,"rect":{"x":0,"y":50,"w":100,"h":50}}
{"type":"RemovePanel","index":1}
```

A panel is named by its index in the layout, counting from zero, and `AddPanel`
appends rather than inserting: the index is how the other two name their target,
and inserting would renumber the ones a caller is already holding. An index past
the end is refused with the count, so a renderer that kept an index across a
frame in which the layout shrank gets an error rather than acting on the wrong
panel.

`MovePanel` changes the rectangle and nothing else. A panel's depth is what it
was built with, so changing that means building a different panel -- a remove
and an add, which says plainly that the old one's state goes with it.

The config moves with the layout, because the config is what a renderer reads to
place the panels a frame carries. A host that persists its config after a
session gets the layout the session ended on.

## Focus

All panels but the watchlist render one market — the focused `(source, symbol)`.
Focus is set by `SetFocus`, moves with the arrow keys in the TUI, and defaults to
the first market subscribed. A frame built with nothing subscribed has no panels
at all rather than empty ones, which is what lets a renderer draw a "no market"
hint instead of five blank boxes.

Panel *focus* is a different thing entirely, and lives in the renderer: which
panel carries a highlighted border, moved with `tab`. The core has no notion of
it, because each renderer decides for itself what focus means to it.

## Layout

A panel's position is data, not markup. Each `PanelSpec` carries a `RectSpec` in
percent of the viewport:

```json
{ "kind": "Chart", "rect": { "x": 0, "y": 0, "w": 70, "h": 70 } }
```

Both renderers honour it: the TUI maps it onto a ratatui `Rect`, the browser onto
absolute CSS percentages. Percentages rather than a grid, because a rectangle can
express layouts a row-and-column template cannot — overlapping panels, a panel
that spans an odd fraction, a layout with gaps.

A layout that omits a panel kind simply does not render it. Omitting the whole
`layout` gives the standard five.

## Bounds, and how deep a panel carries

Every panel reads from a bounded structure, so a session that runs for a week
renders exactly as fast as one that just started:

| Panel | Default depth | Ceiling |
|-------|--------------|---------|
| `chart` | 120 bars, 120 price points, 120 points per indicator | 256 bars kept, 512 price points |
| `book` | 12 levels a side | the book itself |
| `tape` | 24 prints | 256 kept |
| `footprint` | 12 levels | 1024 levels kept |
| `bars` | 12 bars a stream | 256 kept |
| `watchlist` | one row per subscribed market | — |
| `profile` | one row per bin | — |

A panel spec may set its own depth:

```json
{ "kind": "Book", "rect": { "x": 70, "y": 0, "w": 30, "h": 35 }, "depth": 40 }
```

One number rather than a name per panel, because every panel that has a bound
has exactly one: book levels a side, tape prints, footprint levels, chart points
and bars, alternative bars per stream. The watchlist and the profile panel have
none and ignore it. Zero is refused rather than honoured — a panel carrying
nothing renders blank with no error, which reads as a broken feed rather than as
a configuration — and the value is clamped to 512, above which the state's own
rings are the real ceiling anyway.

It is the **carried** depth, not the drawn one. A renderer draws what fits and
scrolls through the rest, so asking for more here is what makes scrolling
possible at all: with twelve book levels there is nothing underneath them to
scroll to. In the TUI that is `↑` / `↓` on the focused panel; in the browser the
panel is a scrollable box and the browser does it.

## Adding a panel

1. Add a `PanelView` variant and its `*View` struct in
   `crates/wickra-terminal-core/src/view.rs` (derive `Serialize`/`Deserialize`).
2. Add a `PanelKind` and a `Panel` implementation in
   `crates/wickra-terminal-core/src/panels/`, and wire it into `build_panel`.
3. Map the new variant to a widget in `crates/ui-tui/src/widgets/` (TUI) and a
   canvas or DOM view in `web/src/` (Web).
4. Regenerate the golden corpus if the default layout changed:
   `WICKRA_REGEN=1 cargo test -p wickra-terminal-core --test golden`.

Steps 1 and 2 are the feature. Step 3 is twice, once per renderer, and that is
the whole cost of a second front-end — the core stays the single source of truth,
and the renderers only render.

See also: [INDICATORS.md](INDICATORS.md) · [SOURCES.md](SOURCES.md) · [RENDERERS.md](RENDERERS.md) ·
[../ARCHITECTURE.md](../ARCHITECTURE.md).
