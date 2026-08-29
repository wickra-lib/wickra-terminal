//! The order-book widget: asks above, bids below, split by the spread.

use ratatui::layout::Rect;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::widgets::Paragraph;
use ratatui::Frame;
use wickra_terminal_core::view::{BookView, Level};

fn level_line(level: &Level) -> String {
    format!("{:>12.2} {:>12.4}", level.price, level.quantity)
}

/// Render the order-book panel.
pub(crate) fn render(frame: &mut Frame, area: Rect, view: &BookView, focused: bool) {
    let mut lines: Vec<Line> = Vec::new();
    // Asks worst-first so the best ask sits just above the spread line.
    for level in view.asks.iter().rev() {
        lines.push(Line::from(level_line(level)).red());
    }
    let spread = view
        .spread
        .map_or_else(|| "spread —".to_string(), |s| format!("spread {s:.2}"));
    lines.push(Line::from(spread).dim());
    for level in &view.bids {
        lines.push(Line::from(level_line(level)).green());
    }
    frame.render_widget(
        Paragraph::new(lines).block(super::panel_block(format!("Book {}", view.symbol), focused)),
        area,
    );
}
