//! The price-chart panel.

use rust_decimal::prelude::ToPrimitive;

use super::{Panel, PanelKind};
use crate::source::{SourceId, Symbol};
use crate::state::AppState;
use crate::view::{ChartView, IndicatorField, IndicatorValue, OhlcBar, PanelView};

/// The number of price points the chart series carries.
const CHART_POINTS: usize = 120;

/// The number of closed bars the chart carries.
///
/// Fewer than the series because a bar is wider than a tick: a hundred and
/// twenty candles is already more than a terminal column or a canvas draws
/// legibly, and the renderer trims to what it has room for.
const CHART_BARS: usize = 120;

/// A core candle as the view-model carries it.
fn ohlc(candle: &wickra_core::Candle) -> OhlcBar {
    OhlcBar {
        open: candle.open,
        high: candle.high,
        low: candle.low,
        close: candle.close,
        volume: candle.volume,
        timestamp: candle.timestamp,
    }
}

/// A price chart with the focused market's indicator overlays.
#[derive(Debug)]
pub struct ChartPanel;

impl Panel for ChartPanel {
    fn kind(&self) -> PanelKind {
        PanelKind::Chart
    }

    fn view(&self, state: &AppState, focus: (SourceId, &Symbol)) -> PanelView {
        let symbol = focus.1.to_string();
        let chart = match state.get(&(focus.0, focus.1.clone())) {
            Some(st) => ChartView {
                symbol,
                last: st.last.to_f64().unwrap_or(0.0),
                series: st.series(CHART_POINTS),
                bars: st.ohlc(CHART_BARS).iter().map(ohlc).collect(),
                forming: st.forming().as_ref().map(ohlc),
                indicators: st
                    .indicators
                    .snapshot()
                    .into_iter()
                    .map(|mut reading| IndicatorValue {
                        name: reading.label,
                        value: reading.value,
                        fields: reading
                            .fields
                            .into_iter()
                            .map(|(name, value)| IndicatorField {
                                name: name.to_string(),
                                value,
                            })
                            .collect(),
                        // Trimmed to the chart's own window: the set keeps the
                        // same number of points, but a panel configured for
                        // fewer should not ship more than it draws.
                        series: reading
                            .series
                            .split_off(reading.series.len().saturating_sub(CHART_POINTS)),
                    })
                    .collect(),
            },
            None => ChartView {
                symbol,
                last: 0.0,
                series: Vec::new(),
                bars: Vec::new(),
                forming: None,
                indicators: Vec::new(),
            },
        };
        PanelView::Chart(chart)
    }
}
