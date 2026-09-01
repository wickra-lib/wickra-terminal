//! Panels — pure functions from state to a view-model.
//!
//! A [`Panel`] reads [`AppState`] and the focused market and returns a
//! [`PanelView`]; it holds no renderer state and issues no draw commands. Adding
//! a panel here makes it appear in every renderer at once, because each renderer
//! is just a mapping from `PanelView` to its own widget.

pub mod bars;
pub mod book;
pub mod chart;
pub mod footprint;
pub mod profile;
pub mod tape;
pub mod watchlist;

use serde::{Deserialize, Serialize};

use crate::config::PanelSpec;
use crate::source::{SourceId, Symbol};
use crate::state::AppState;
use crate::view::PanelView;

pub use bars::BarsPanel;
pub use book::BookPanel;
pub use chart::ChartPanel;
pub use footprint::FootprintPanel;
pub use profile::ProfilePanel;
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
    /// The configured distributions: volume by price, TPO, time-of-day shapes.
    Profile,
    /// The configured alternative charts: Renko, Kagi, point-and-figure.
    Bars,
}

/// A panel: a pure mapping from state to a view-model.
pub trait Panel {
    /// The panel's kind.
    fn kind(&self) -> PanelKind;

    /// Build this panel's view-model for the focused market.
    fn view(&self, state: &AppState, focus: (SourceId, &Symbol)) -> PanelView;
}

/// Build the panel a spec describes, carrying the depth it asks for.
///
/// The depth is read here rather than in each panel's `view`, so a panel holds
/// the one number it needs and the spec is not threaded through the whole
/// rendering path.
#[must_use]
pub fn build_panel(spec: &PanelSpec) -> Box<dyn Panel> {
    match spec.kind {
        PanelKind::Chart => Box::new(ChartPanel {
            points: spec.depth_or(CHART_POINTS),
        }),
        PanelKind::Book => Box::new(BookPanel {
            depth: spec.depth_or(DEFAULT_DEPTH),
        }),
        PanelKind::Tape => Box::new(TapePanel {
            rows: spec.depth_or(TAPE_ROWS),
        }),
        PanelKind::Watchlist => Box::new(WatchlistPanel),
        PanelKind::Footprint => Box::new(FootprintPanel {
            depth: spec.depth_or(DEFAULT_DEPTH),
        }),
        PanelKind::Profile => Box::new(ProfilePanel),
        PanelKind::Bars => Box::new(BarsPanel {
            bars: spec.depth_or(DEFAULT_DEPTH),
        }),
    }
}

/// The number of levels/rows a panel shows by default.
pub(crate) const DEFAULT_DEPTH: usize = 12;

/// The default number of prints the tape carries.
pub(crate) const TAPE_ROWS: usize = DEFAULT_DEPTH * 2;

/// The default number of price points and bars the chart carries.
pub(crate) const CHART_POINTS: usize = 120;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::RectSpec;

    /// Every panel kind, so this test drives the whole enum.
    ///
    /// The list was written with five and stayed at five when Profile and Bars
    /// were added, which left them the only panels nothing ever built.
    const EVERY_KIND: [PanelKind; 7] = [
        PanelKind::Chart,
        PanelKind::Book,
        PanelKind::Tape,
        PanelKind::Watchlist,
        PanelKind::Footprint,
        PanelKind::Profile,
        PanelKind::Bars,
    ];

    #[test]
    fn a_panel_carries_the_depth_its_spec_asks_for() {
        // Book depth, tape rows and chart points were `const` in the code, so a
        // config could set exactly one thing per panel -- its rectangle. That
        // also meant a renderer had nothing underneath the twelve rows it drew,
        // which is what made panel scrolling impossible rather than merely
        // unimplemented.
        let mut spec = PanelSpec::new(PanelKind::Book, RectSpec::new(0, 0, 100, 100));
        assert_eq!(spec.depth_or(12), 12, "no depth means the default");
        spec.depth = Some(40);
        assert_eq!(spec.depth_or(12), 40);
    }

    #[test]
    fn a_depth_of_zero_is_refused_rather_than_honoured() {
        // A panel configured to carry nothing renders blank with no error, which
        // reads as a broken feed rather than as a configuration.
        let mut spec = PanelSpec::new(PanelKind::Tape, RectSpec::new(0, 0, 100, 100));
        spec.depth = Some(0);
        assert_eq!(spec.depth_or(24), 1);
    }

    #[test]
    fn a_depth_beyond_the_ceiling_is_clamped() {
        let mut spec = PanelSpec::new(PanelKind::Tape, RectSpec::new(0, 0, 100, 100));
        spec.depth = Some(1_000_000);
        assert_eq!(spec.depth_or(24), crate::config::MAX_PANEL_DEPTH);
    }

    #[test]
    fn build_panel_matches_the_spec_kind() {
        for kind in EVERY_KIND {
            let spec = PanelSpec {
                kind,
                rect: RectSpec::new(0, 0, 100, 100),
                depth: None,
            };
            assert_eq!(build_panel(&spec).kind(), kind);
        }
    }
}
