//! Panels — pure functions from state to a view-model.
//!
//! A [`Panel`] reads [`AppState`] and the focused market and returns a
//! [`PanelView`]; it holds no renderer state and issues no draw commands. Adding
//! a panel here makes it appear in every renderer at once, because each renderer
//! is just a mapping from `PanelView` to its own widget.

pub mod book;
pub mod chart;
pub mod footprint;
pub mod tape;
pub mod watchlist;

use serde::{Deserialize, Serialize};

use crate::config::PanelSpec;
use crate::source::{SourceId, Symbol};
use crate::state::AppState;
use crate::view::PanelView;

pub use book::BookPanel;
pub use chart::ChartPanel;
pub use footprint::FootprintPanel;
pub use tape::TapePanel;
pub use watchlist::WatchlistPanel;

/// Which panel a [`PanelSpec`] builds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PanelKind {
    /// A price chart with indicator overlays.
    Chart,
    /// An order book.
    Book,
    /// A time-and-sales tape.
    Tape,
    /// A multi-market watchlist.
    Watchlist,
    /// A footprint / volume profile.
    Footprint,
}

/// A panel: a pure mapping from state to a view-model.
pub trait Panel {
    /// The panel's kind.
    fn kind(&self) -> PanelKind;

    /// Build this panel's view-model for the focused market.
    fn view(&self, state: &AppState, focus: (SourceId, &Symbol)) -> PanelView;
}

/// Build the panel a spec describes.
#[must_use]
pub fn build_panel(spec: &PanelSpec) -> Box<dyn Panel> {
    match spec.kind {
        PanelKind::Chart => Box::new(ChartPanel),
        PanelKind::Book => Box::new(BookPanel),
        PanelKind::Tape => Box::new(TapePanel),
        PanelKind::Watchlist => Box::new(WatchlistPanel),
        PanelKind::Footprint => Box::new(FootprintPanel),
    }
}

/// The number of levels/rows a panel shows by default.
pub(crate) const DEFAULT_DEPTH: usize = 12;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::RectSpec;

    #[test]
    fn build_panel_matches_the_spec_kind() {
        for kind in [
            PanelKind::Chart,
            PanelKind::Book,
            PanelKind::Tape,
            PanelKind::Watchlist,
            PanelKind::Footprint,
        ] {
            let spec = PanelSpec {
                kind,
                rect: RectSpec::new(0, 0, 100, 100),
            };
            assert_eq!(build_panel(&spec).kind(), kind);
        }
    }
}
