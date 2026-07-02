# Panels

A panel is a pure function from [`AppState`](../crates/terminal-core/src/state.rs)
and the focused market to a [`PanelView`](../crates/terminal-core/src/view.rs) —
a plain data description of what to draw (values, series, sides), never a
renderer command. Adding a panel makes it appear in **every** renderer at once,
because each renderer is just a mapping from `PanelView` to its own widget.

## Built-in panels

| Kind | View-model | Shows |
|------|------------|-------|
| `Chart` | `ChartView` | a recent price series + indicator overlays (SMA, EMA) for the focused market |
| `Book` | `BookView` | the top-of-book L2 order book (bids/asks) + spread |
| `Tape` | `TapeView` | the most recent trade prints, coloured by aggressor side |
| `Footprint` | `FootprintView` | per-price buy/sell volume (a volume profile) |
| `Watchlist` | `WatchlistView` | the last price of every tracked `(source, symbol)` |

The `PanelView` enum is internally tagged by `panel` (`"chart"`, `"book"`, …);
each variant's struct fields are flattened alongside the tag, so a frame is a
plain, language-neutral JSON document.

## Adding a panel

1. Add a `PanelView` variant and its `*View` struct in
   `crates/terminal-core/src/view.rs` (derive `Serialize`/`Deserialize`).
2. Add a `PanelKind` and a `Panel` implementation in
   `crates/terminal-core/src/panels/`, and wire it into `build_panel`.
3. Map the new variant to a widget in `crates/ui-tui/src/widgets/` (TUI) and a
   canvas/DOM view in `web/src/` (Web).

The core stays the single source of truth; the renderers only render.

See also: [SOURCES.md](SOURCES.md) · [RENDERERS.md](RENDERERS.md) ·
[../ARCHITECTURE.md](../ARCHITECTURE.md).
