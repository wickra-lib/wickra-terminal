//! The tape widget: recent prints, coloured by aggressor side.

use ratatui::layout::Rect;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::widgets::Paragraph;
use ratatui::Frame;
use wickra_terminal_core::view::TapeView;

/// Render the tape panel.
pub(crate) fn render(frame: &mut Frame, area: Rect, view: &TapeView, focused: bool) {
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
        Paragraph::new(lines).block(super::panel_block(format!("Tape {}", view.symbol), focused)),
        area,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::widgets::harness;
    use ratatui::style::Color;
    use wickra_terminal_core::view::TapePrint;

    fn print(price: f64, side: &str) -> TapePrint {
        TapePrint {
            price,
            quantity: 1.5,
            side: side.to_owned(),
            timestamp: 0,
        }
    }

    #[test]
    fn a_print_is_coloured_by_the_side_that_crossed() {
        // The side is the only thing separating two otherwise identical rows,
        // and it is carried by colour rather than by position.
        let view = TapeView {
            symbol: "BTC/USDT".to_owned(),
            prints: vec![print(100.0, "buy"), print(99.0, "sell")],
        };
        let buffer = harness::draw(40, 6, |frame, area| render(frame, area, &view, false));
        assert_eq!(harness::row_colour(&buffer, 1), Color::Green);
        assert_eq!(harness::row_colour(&buffer, 2), Color::Red);
        assert!(harness::row(&buffer, 1).contains("buy"));
    }

    #[test]
    fn an_unknown_side_is_not_drawn_as_a_buy() {
        // Anything that is not "buy" is a sell for colouring purposes, so a
        // side the core did not produce reads as the more cautious of the two
        // rather than as a purchase that never happened.
        let view = TapeView {
            symbol: "S".to_owned(),
            prints: vec![print(100.0, "")],
        };
        let buffer = harness::draw(40, 4, |frame, area| render(frame, area, &view, false));
        assert_eq!(harness::row_colour(&buffer, 1), Color::Red);
    }

    #[test]
    fn an_empty_tape_still_draws_its_panel() {
        let view = TapeView {
            symbol: "BTC/USDT".to_owned(),
            prints: Vec::new(),
        };
        let buffer = harness::draw(30, 4, |frame, area| render(frame, area, &view, true));
        assert!(harness::text(&buffer).contains("Tape BTC/USDT"));
    }
}
