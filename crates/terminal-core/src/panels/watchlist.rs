//! The watchlist panel — every tracked market at a glance.

use rust_decimal::prelude::ToPrimitive;

use super::{Panel, PanelKind};
use crate::source::{SourceId, Symbol};
use crate::state::AppState;
use crate::view::{PanelView, WatchRow, WatchlistView};

/// A multi-market watchlist. Unlike the other panels it spans every tracked
/// market, not just the focused one, so it ignores `focus`.
pub struct WatchlistPanel;

impl Panel for WatchlistPanel {
    fn kind(&self) -> PanelKind {
        PanelKind::Watchlist
    }

    fn view(&self, state: &AppState, _focus: (SourceId, &Symbol)) -> PanelView {
        let rows = state
            .watchlist
            .iter()
            .map(|key| WatchRow {
                source: key.0,
                symbol: key.1.to_string(),
                last: state
                    .get(key)
                    .map_or(0.0, |st| st.last.to_f64().unwrap_or(0.0)),
            })
            .collect();
        PanelView::Watchlist(WatchlistView { rows })
    }
}
