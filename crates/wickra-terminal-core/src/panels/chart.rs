//! The price-chart panel.

use rust_decimal::prelude::ToPrimitive;

use super::{Panel, PanelKind};
use crate::source::{SourceId, Symbol};
use crate::state::AppState;
use crate::view::{ChartView, IndicatorField, IndicatorValue, OhlcBar, PanelView};

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
pub struct ChartPanel {
    /// How many price points, bars and indicator points this panel carries.
    ///
    /// One number for all three: they are three views of the same window, and a
    /// config that could set them apart would let a chart draw a bar the price
    /// line does not reach.
    pub(crate) points: usize,
}

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
                series: st.series(self.points),
                bars: st.ohlc(self.points).iter().map(ohlc).collect(),
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
                            .split_off(reading.series.len().saturating_sub(self.points)),
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
