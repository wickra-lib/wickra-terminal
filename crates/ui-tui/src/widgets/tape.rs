//! The tape widget: recent prints, coloured by aggressor side.

use ratatui::layout::Rect;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::widgets::{Block, Paragraph};
use ratatui::Frame;
use terminal_core::view::TapeView;

/// Render the tape panel.
pub fn render(frame: &mut Frame, area: Rect, view: &TapeView) {
    let lines: Vec<Line> = view
        .prints
        .iter()
        .map(|print| {
            let text = format!(
                "{:>12.2} {:>12.4} {}",
                print.price, print.quantity, print.side
            );
            if print.side == "buy" {
                Line::from(text).green()
            } else {
                Line::from(text).red()
            }
        })
        .collect();
    frame.render_widget(
        Paragraph::new(lines).block(Block::bordered().title(format!("Tape {}", view.symbol))),
        area,
    );
}
