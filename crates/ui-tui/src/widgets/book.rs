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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::widgets::harness;
    use ratatui::style::Color;
    use wickra_terminal_core::view::Level;

    fn level(price: f64, quantity: f64) -> Level {
        Level { price, quantity }
    }

    fn view(spread: Option<f64>) -> BookView {
        BookView {
            symbol: "BTC/USDT".to_owned(),
            bids: vec![level(99.0, 1.0), level(98.0, 2.0)],
            asks: vec![level(101.0, 3.0), level(102.0, 4.0)],
            spread,
        }
    }

    #[test]
    fn the_best_ask_sits_directly_above_the_spread() {
        // Asks arrive best-first and are drawn worst-first, so the two sides
        // meet at the spread the way a book reads. Drawn in arrival order the
        // ladder is inside out and the panel is silently wrong.
        let buffer = harness::draw(40, 8, |frame, area| {
            render(frame, area, &view(Some(2.0)), false);
        });
        let rows: Vec<String> = (1..5).map(|y| harness::row(&buffer, y)).collect();
        assert!(rows[0].starts_with("102.00"), "{rows:?}");
        assert!(rows[1].starts_with("101.00"), "{rows:?}");
        assert!(rows[2].starts_with("spread 2.00"), "{rows:?}");
        assert!(rows[3].starts_with("99.00"), "{rows:?}");
    }

    #[test]
    fn each_side_is_drawn_in_its_own_colour() {
        let buffer = harness::draw(40, 8, |frame, area| {
            render(frame, area, &view(Some(2.0)), false);
        });
        assert_eq!(harness::row_colour(&buffer, 1), Color::Red);
        assert_eq!(harness::row_colour(&buffer, 4), Color::Green);
    }

    #[test]
    fn a_book_with_no_spread_says_so_rather_than_showing_a_number() {
        // A one-sided or crossed book has no spread, and printing a zero there
        // would be a claim about the market rather than an absence.
        let buffer = harness::draw(40, 8, |frame, area| {
            render(frame, area, &view(None), false);
        });
        assert!(
            harness::text(&buffer).contains("spread —"),
            "{}",
            harness::text(&buffer)
        );
    }
}
