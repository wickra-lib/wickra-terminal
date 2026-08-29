//! The profile panel: the configured distributions for the focused market.
//!
//! Where the footprint panel shows volume by price for the current session, a
//! profile shows whatever distribution its indicator computes — volume by price
//! over a window, time-price opportunity, or a shape over the clock rather than
//! over price. They share nothing but the word, so they are different panels.

use super::{Panel, PanelKind};
use crate::source::{SourceId, Symbol};
use crate::state::AppState;
use crate::view::{PanelView, ProfileRow, ProfileView};

/// The configured profiles for the focused market.
#[derive(Debug)]
pub struct ProfilePanel;

impl Panel for ProfilePanel {
    fn kind(&self) -> PanelKind {
        PanelKind::Profile
    }

    fn view(&self, state: &AppState, focus: (SourceId, &Symbol)) -> PanelView {
        let symbol = focus.1.to_string();
        let profiles = state
            .get(&(focus.0, focus.1.clone()))
            .map(|st| {
                st.profiles
                    .readings()
                    .into_iter()
                    .map(|(label, reading)| ProfileRow {
                        label: label.to_string(),
                        // A profile that has not produced a histogram yet keeps
                        // its row with an empty one, rather than dropping out:
                        // rows that appear and disappear as a session warms up
                        // make a renderer relayout for no reason.
                        bins: reading.map(|r| r.bins.clone()).unwrap_or_default(),
                        price_low: reading.and_then(|r| r.price_low),
                        price_high: reading.and_then(|r| r.price_high),
                    })
                    .collect()
            })
            .unwrap_or_default();
        PanelView::Profile(ProfileView { symbol, profiles })
    }
}
