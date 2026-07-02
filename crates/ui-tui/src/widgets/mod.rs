//! ratatui widgets — one per [`PanelView`] variant.
//!
//! Each widget is a pure function from a view-model to a rendered area. The
//! renderer never inspects state; it only maps the core's view-models to
//! ratatui, which is exactly what makes the TUI one interchangeable renderer.

pub mod book;
pub mod chart;
pub mod tape;
pub mod watchlist;

use ratatui::layout::Rect;
use ratatui::Frame;
use terminal_core::PanelView;

/// Render one panel's view-model into `area`.
pub fn render_panel(frame: &mut Frame, area: Rect, panel: &PanelView) {
    match panel {
        PanelView::Chart(view) => chart::render(frame, area, view),
        PanelView::Book(view) => book::render(frame, area, view),
        PanelView::Tape(view) => tape::render(frame, area, view),
        PanelView::Watchlist(view) => watchlist::render(frame, area, view),
    }
}
