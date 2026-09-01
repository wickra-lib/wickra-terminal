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

/// One named output of a multi-output indicator.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IndicatorField {
    /// The field name as wickra-core declares it (`macd`, `signal`, `histogram`).
    pub name: String,
    /// The field's latest value.
    pub value: f64,
}

/// One indicator's latest value (`None` while warming up).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IndicatorValue {
    /// The indicator's display label (`"Sma(20)"`), derived from its spec.
    pub name: String,
    /// The primary value, or `None` during warmup. For a multi-output indicator
    /// this is its first field, so a renderer that only wants one line does not
    /// have to know which field that is.
    pub value: Option<f64>,
    /// Every named output, in declaration order, for the multi-output
    /// indicators. Empty for single-output ones, and omitted from the JSON
    /// entirely when empty: a consumer written against the single-output shape
    /// sees exactly the object it saw before.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fields: Vec<IndicatorField>,
    /// A bounded recent series, oldest first, ending at the current tick, for
    /// renderers that draw the indicator as a line over the price.
    ///
    /// Indicators warm up at different lengths, so this is not always as long as
    /// the chart's own series. Both end at the same tick, so a renderer aligns
    /// this to the right. Empty while warming up, and then omitted from the JSON
    /// entirely rather than serialised as `[]`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub series: Vec<f64>,
}

/// One OHLCV bar, as a chart draws it.
///
/// Separate from [`crate::registry::AltBar`], which the bars panel carries: an
/// alternative bar has a direction and no time, because a Renko brick or a
/// point-and-figure column advances on price movement. This one is a bar of the
/// configured timeframe and is placed by its timestamp.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct OhlcBar {
    /// Opening price of the bar.
    pub open: f64,
    /// Highest price traded in the bar.
    pub high: f64,
    /// Lowest price traded in the bar.
    pub low: f64,
    /// Closing price of the bar.
    pub close: f64,
    /// Volume traded in the bar.
    pub volume: f64,
    /// The bar's opening timestamp (ms since the Unix epoch).
    pub timestamp: i64,
}

/// The chart panel's view-model: the bars, a tick-resolution price series, and
/// the indicator overlays.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChartView {
    /// The market shown.
    pub symbol: String,
    /// The last traded price.
    pub last: f64,
    /// A bounded recent price series, oldest first.
    ///
    /// One point per trade rather than per bar, so it is the finer of the two
    /// and does not wait for a bar to close. A renderer with a handful of
    /// columns draws this; one with room for candles draws `bars`.
    pub series: Vec<f64>,
    /// The closed bars of the configured timeframe, oldest first.
    ///
    /// Empty until the first bar closes, and omitted from the JSON entirely
    /// when empty, so a consumer written against the earlier shape sees exactly
    /// the object it saw before.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub bars: Vec<OhlcBar>,
    /// The bar still accumulating, if a trade has opened one.
    ///
    /// Kept apart from `bars` rather than appended to it, because it is the one
    /// bar that will still change: an indicator never sees it — a reading that
    /// repainted as its bar filled would be a different number every print —
    /// but a chart that omitted it would show the market frozen at the last
    /// close.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub forming: Option<OhlcBar>,
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

/// One profile's histogram, as a panel row.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProfileRow {
    /// The profile's label, as the spec names it.
    pub label: String,
    /// The histogram, in bin order. Empty until the profile has produced one.
    pub bins: Vec<f64>,
    /// The lowest price the bins cover, for a distribution over price.
    ///
    /// Absent for a distribution over TIME -- day of week, minute of session
    /// -- which has no price range. Reporting zeros there would be a claim
    /// about prices that the profile never made.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub price_low: Option<f64>,
    /// The highest price the bins cover, for a distribution over price.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub price_high: Option<f64>,
}

/// The profile panel's view-model.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProfileView {
    /// The market shown.
    pub symbol: String,
    /// The configured profiles, in configured order.
    pub profiles: Vec<ProfileRow>,
}

/// One alternative bar stream, as a panel row.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BarStreamView {
    /// The stream's label, as the spec names it.
    pub label: String,
    /// The most recent completed bars, oldest first.
    ///
    /// Empty until the stream completes one, which for a Renko brick or a
    /// point-and-figure column can take many candles: these charts advance on
    /// price movement rather than on time.
    pub bars: Vec<crate::registry::AltBar>,
}

/// The bars panel's view-model.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BarsView {
    /// The market shown.
    pub symbol: String,
    /// The configured streams, in configured order.
    pub streams: Vec<BarStreamView>,
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
    /// The configured distributions.
    Profile(ProfileView),
    /// The configured alternative charts.
    Bars(BarsView),
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
            bars: vec![OhlcBar {
                open: 99.0,
                high: 101.0,
                low: 98.0,
                close: 100.0,
                volume: 3.0,
                timestamp: 0,
            }],
            forming: None,
            indicators: vec![IndicatorValue {
                name: "Sma(20)".to_string(),
                value: None,
                fields: Vec::new(),
                series: Vec::new(),
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
