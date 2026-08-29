//! ratatui widgets — one per [`PanelView`] variant.
//!
//! Each widget is a pure function from a view-model to a rendered area. The
//! renderer never inspects state; it only maps the core's view-models to
//! ratatui, which is exactly what makes the TUI one interchangeable renderer.

pub(crate) mod book;
pub(crate) mod chart;
pub(crate) mod footprint;
mod profile;
pub(crate) mod tape;
pub(crate) mod watchlist;

use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::widgets::Block;
use ratatui::Frame;
use wickra_terminal_core::PanelView;

/// The bordered block every panel draws itself into.
///
/// The focused one is highlighted. Focus has to be visible somewhere or the
/// keybinds that move it are indistinguishable from keybinds that do nothing —
/// which is what `tab` and `backtab` were until now.
#[must_use]
pub(crate) fn panel_block(title: String, focused: bool) -> Block<'static> {
    let block = Block::bordered().title(title);
    if focused {
        block.border_style(Style::new().cyan().bold())
    } else {
        block
    }
}

/// Render one panel's view-model into `area`.
pub(crate) fn render_panel(frame: &mut Frame, area: Rect, panel: &PanelView, focused: bool) {
    match panel {
        PanelView::Chart(view) => chart::render(frame, area, view, focused),
        PanelView::Book(view) => book::render(frame, area, view, focused),
        PanelView::Tape(view) => tape::render(frame, area, view, focused),
        PanelView::Watchlist(view) => watchlist::render(frame, area, view, focused),
        PanelView::Footprint(view) => footprint::render(frame, area, view, focused),
        PanelView::Profile(view) => profile::render(frame, area, view, focused),
    }
}
