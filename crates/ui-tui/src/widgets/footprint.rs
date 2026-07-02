//! The footprint widget: buy/sell volume per price level.

use ratatui::layout::Rect;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::widgets::{Block, Paragraph};
use ratatui::Frame;
use terminal_core::view::FootprintView;

/// Render the footprint panel.
pub fn render(frame: &mut Frame, area: Rect, view: &FootprintView) {
    let lines: Vec<Line> = view
        .levels
        .iter()
        .map(|level| {
            let text = format!(
                "{:>10.2} {:>8.3} x {:<8.3}",
                level.price, level.buy, level.sell
            );
            // Colour by the dominant side at this price.
            if level.buy >= level.sell {
                Line::from(text).green()
            } else {
                Line::from(text).red()
            }
        })
        .collect();
    frame.render_widget(
        Paragraph::new(lines).block(Block::bordered().title(format!("Footprint {}", view.symbol))),
        area,
    );
}
