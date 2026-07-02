//! The price-chart panel.

use rust_decimal::prelude::ToPrimitive;

use super::{Panel, PanelKind};
use crate::source::{SourceId, Symbol};
use crate::state::AppState;
use crate::view::{ChartView, IndicatorValue, PanelView};

/// The number of price points the chart series carries.
const CHART_POINTS: usize = 120;

/// A price chart with the focused market's indicator overlays.
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
                indicators: st
                    .indicators
                    .values()
                    .into_iter()
                    .map(|(name, value)| IndicatorValue { name, value })
                    .collect(),
            },
            None => ChartView {
                symbol,
                last: 0.0,
                series: Vec::new(),
                indicators: Vec::new(),
            },
        };
        PanelView::Chart(chart)
    }
}
