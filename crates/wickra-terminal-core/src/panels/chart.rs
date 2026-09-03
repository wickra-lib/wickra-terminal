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

#[cfg(test)]
mod tests {
    use super::super::CHART_POINTS;
    use super::*;

    /// A panel focused on a market the state has never folded still answers.
    ///
    /// The empty answer is not a formality: focus moves to a market the moment
    /// it is subscribed, which is before its first event arrives, so this is the
    /// view every live market is drawn from for its first frame. A renderer that
    /// was handed nothing there would have to guess.
    #[test]
    fn a_market_with_no_state_yet_charts_as_empty() {
        let sym = Symbol::new("BTC", "USDT");
        let state = AppState::default();

        let PanelView::Chart(view) = (ChartPanel {
            points: CHART_POINTS,
        })
        .view(&state, (0, &sym)) else {
            panic!("the chart panel answers with a chart")
        };

        assert_eq!(view.symbol, "BTC/USDT");
        assert!(
            view.last.abs() < f64::EPSILON,
            "an unfolded market has no last price"
        );
        assert!(view.series.is_empty());
        assert!(view.bars.is_empty());
        assert!(view.forming.is_none());
        assert!(view.indicators.is_empty());
    }
}
