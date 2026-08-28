# Panels

A panel is a pure function from [`AppState`](../crates/terminal-core/src/state.rs)
and the focused market to a [`PanelView`](../crates/terminal-core/src/view.rs) —
a plain data description of what to draw (values, series, sides), never a
renderer command. Adding a panel makes it appear in **every** renderer at once,
because each renderer is only a mapping from `PanelView` to its own widget.

## Built-in panels

| Kind | View-model | Shows |
|------|------------|-------|
| `Chart` | `ChartView` | a recent price series and the configured indicator overlays for the focused market |
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

**`chart`** — `symbol`, `last`, `series` (up to 120 recent prices, oldest first),
and `indicators`. Each indicator carries its `name` (the label from its spec,
`Sma(20)`), its primary `value` (`null` while warming up), an optional `fields`
list for the multi-output ones, and an optional `series` of its own. See
[INDICATORS.md](INDICATORS.md).

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

**`watchlist`** — `rows`, one per tracked market, each a `source` id, a `symbol`
and its `last` price. This is the only panel that does not render the focused
market: it renders all of them.

Every number crosses the boundary as a JSON number. Internally prices and
quantities are `Decimal`; the conversion happens here, at the edge, because a
view-model is drawn rather than compared.

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

## Bounds

Every panel reads from a bounded structure, so a session that runs for a week
renders exactly as fast as one that just started:

| Panel | Bound | Where |
|-------|-------|-------|
| `chart` | 120 price points, 120 points per indicator | `SymbolState::history`, `IndicatorSet` |
| `book` | 12 levels a side | `DEFAULT_DEPTH` |
| `tape` | 24 rendered of 256 kept | `TapeRing` |
| `footprint` | 12 levels | `DEFAULT_DEPTH` |
| `watchlist` | one row per subscribed market | the watchlist itself |

## Adding a panel

1. Add a `PanelView` variant and its `*View` struct in
   `crates/terminal-core/src/view.rs` (derive `Serialize`/`Deserialize`).
2. Add a `PanelKind` and a `Panel` implementation in
   `crates/terminal-core/src/panels/`, and wire it into `build_panel`.
3. Map the new variant to a widget in `crates/ui-tui/src/widgets/` (TUI) and a
   canvas or DOM view in `web/src/` (Web).
4. Regenerate the golden corpus if the default layout changed:
   `WICKRA_REGEN=1 cargo test -p terminal-core --test golden`.

Steps 1 and 2 are the feature. Step 3 is twice, once per renderer, and that is
the whole cost of a second front-end — the core stays the single source of truth,
and the renderers only render.

See also: [INDICATORS.md](INDICATORS.md) · [SOURCES.md](SOURCES.md) · [RENDERERS.md](RENDERERS.md) ·
[../ARCHITECTURE.md](../ARCHITECTURE.md).
