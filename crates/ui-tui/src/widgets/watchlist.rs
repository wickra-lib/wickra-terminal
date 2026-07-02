//! The watchlist widget: every tracked market and its last price.

use ratatui::layout::Rect;
use ratatui::text::Line;
use ratatui::widgets::{Block, Paragraph};
use ratatui::Frame;
use terminal_core::view::WatchlistView;

/// Render the watchlist panel.
pub fn render(frame: &mut Frame, area: Rect, view: &WatchlistView) {
    let lines: Vec<Line> = view
        .rows
        .iter()
        .map(|row| {
            Line::from(format!(
                "[{}] {:<12} {:>12.2}",
                row.source, row.symbol, row.last
            ))
        })
        .collect();
    frame.render_widget(
        Paragraph::new(lines).block(Block::bordered().title("Watchlist")),
        area,
    );
}
