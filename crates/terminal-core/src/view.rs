//! View-models — the renderer-agnostic output of the core.
//!
//! A [`Frame`] is what one `tick` produces: a list of [`PanelView`]s, each a
//! plain data description of what to draw (values, series, sides) — never a
//! renderer command. The TUI maps a `PanelView` to a ratatui widget; the Web app
//! maps the same `PanelView` to a canvas draw. Because these are `serde` types,
//! they are also the exact bytes the cross-language golden corpus pins and the
//! payload `Terminal::command_json` returns.

use serde::{Deserialize, Serialize};

use crate::source::SourceId;

/// One indicator's latest value (`None` while warming up).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IndicatorValue {
    /// The indicator's display name (e.g. `"SMA(20)"`).
    pub name: String,
    /// The latest value, or `None` during warmup.
    pub value: Option<f64>,
}

/// The chart panel's view-model: a recent price series with indicator overlays.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChartView {
    /// The market shown.
    pub symbol: String,
    /// The last traded price.
    pub last: f64,
    /// A bounded recent price series, oldest first.
    pub series: Vec<f64>,
    /// The indicator overlays.
    pub indicators: Vec<IndicatorValue>,
}

/// One order-book level.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Level {
    /// Price of the level.
    pub price: f64,
    /// Resting quantity at the level.
    pub quantity: f64,
}

/// The order-book panel's view-model.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BookView {
    /// The market shown.
    pub symbol: String,
    /// Bid levels, best (highest) first.
    pub bids: Vec<Level>,
    /// Ask levels, best (lowest) first.
    pub asks: Vec<Level>,
    /// The spread, or `None` if a side is empty.
    pub spread: Option<f64>,
}

/// One tape print in a view-model, with the aggressor side as a semantic hint
/// (`"buy"` / `"sell"`) the renderer colours.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TapePrint {
    /// Execution price.
    pub price: f64,
    /// Executed quantity.
    pub quantity: f64,
    /// Aggressor side hint: `"buy"` or `"sell"`.
    pub side: String,
    /// Venue timestamp (ms since the Unix epoch).
    pub timestamp: i64,
}

/// The tape (time-and-sales) panel's view-model.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TapeView {
    /// The market shown.
    pub symbol: String,
    /// The most recent prints, newest first.
    pub prints: Vec<TapePrint>,
}

/// One watchlist row.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WatchRow {
    /// The source the market belongs to.
    pub source: SourceId,
    /// The market, in `BASE/QUOTE` form.
    pub symbol: String,
    /// The last traded price.
    pub last: f64,
}

/// The watchlist panel's view-model.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WatchlistView {
    /// The tracked markets.
    pub rows: Vec<WatchRow>,
}

/// One footprint level: volume traded at a price, split by aggressor side.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FootprintLevel {
    /// The price level.
    pub price: f64,
    /// Buy-aggressor volume at this price.
    pub buy: f64,
    /// Sell-aggressor volume at this price.
    pub sell: f64,
}

/// The footprint (volume-profile) panel's view-model.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FootprintView {
    /// The market shown.
    pub symbol: String,
    /// Price levels, highest price first.
    pub levels: Vec<FootprintLevel>,
}

/// One panel's view-model, tagged by kind.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "panel", rename_all = "snake_case")]
pub enum PanelView {
    /// A price chart.
    Chart(ChartView),
    /// An order book.
    Book(BookView),
    /// A time-and-sales tape.
    Tape(TapeView),
    /// A multi-market watchlist.
    Watchlist(WatchlistView),
    /// A footprint / volume profile.
    Footprint(FootprintView),
}

/// The output of one `tick`: every active panel's view-model.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Frame {
    /// The panels, in layout order.
    pub panels: Vec<PanelView>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn panel_view_is_tagged_in_json() {
        let view = PanelView::Chart(ChartView {
            symbol: "BTC/USDT".to_string(),
            last: 100.0,
            series: vec![99.0, 100.0],
            indicators: vec![IndicatorValue {
                name: "SMA(20)".to_string(),
                value: None,
            }],
        });
        let json = serde_json::to_string(&view).unwrap();
        assert!(json.contains("\"panel\":\"chart\""));
        assert_eq!(serde_json::from_str::<PanelView>(&json).unwrap(), view);
    }

    #[test]
    fn frame_round_trips() {
        let frame = Frame {
            panels: vec![PanelView::Watchlist(WatchlistView { rows: vec![] })],
        };
        let json = serde_json::to_string(&frame).unwrap();
        assert_eq!(serde_json::from_str::<Frame>(&json).unwrap(), frame);
    }
}
