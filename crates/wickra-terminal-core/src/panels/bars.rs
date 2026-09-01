//! The bars panel: the configured alternative charts for the focused market.
//!
//! Renko, Kagi, point-and-figure and the rest are built from the same closed
//! candles everything else reads, but they are not a function of time: a quiet
//! hour completes no bars and a fast one completes several. The panel shows the
//! most recent of each stream, which is what a chart of them is.

use super::{Panel, PanelKind};
use crate::source::{SourceId, Symbol};
use crate::state::AppState;
use crate::view::{BarStreamView, BarsView, PanelView};

/// The configured alternative bar streams for the focused market.
#[derive(Debug)]
pub struct BarsPanel {
    /// How many completed bars each stream carries.
    pub(crate) bars: usize,
}

impl Panel for BarsPanel {
    fn kind(&self) -> PanelKind {
        PanelKind::Bars
    }

    fn view(&self, state: &AppState, focus: (SourceId, &Symbol)) -> PanelView {
        let symbol = focus.1.to_string();
        let streams = state
            .get(&(focus.0, focus.1.clone()))
            .map(|st| {
                st.bars
                    .streams(self.bars)
                    .into_iter()
                    .map(|(label, bars)| BarStreamView {
                        label: label.to_string(),
                        bars,
                    })
                    .collect()
            })
            .unwrap_or_default();
        PanelView::Bars(BarsView { symbol, streams })
    }
}
