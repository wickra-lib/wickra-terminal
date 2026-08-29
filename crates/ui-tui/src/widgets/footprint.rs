//! The footprint widget: buy/sell volume per price level.

use ratatui::layout::Rect;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::widgets::Paragraph;
use ratatui::Frame;
use wickra_terminal_core::view::FootprintView;

/// Render the footprint panel.
pub(crate) fn render(frame: &mut Frame, area: Rect, view: &FootprintView, focused: bool) {
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
        Paragraph::new(lines).block(super::panel_block(
            format!("Footprint {}", view.symbol),
            focused,
        )),
        area,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::widgets::harness;
    use ratatui::style::Color;
    use wickra_terminal_core::view::FootprintLevel;

    fn level(price: f64, buy: f64, sell: f64) -> FootprintLevel {
        FootprintLevel { price, buy, sell }
    }

    #[test]
    fn a_level_is_coloured_by_the_side_that_traded_more() {
        // The point of the panel is where the aggression was, so the colour has
        // to follow the dominant side and not the price.
        let view = FootprintView {
            symbol: "BTC/USDT".to_owned(),
            levels: vec![level(100.0, 9.0, 1.0), level(99.0, 1.0, 9.0)],
        };
        let buffer = harness::draw(40, 6, |frame, area| render(frame, area, &view, false));
        assert_eq!(harness::row_colour(&buffer, 1), Color::Green);
        assert_eq!(harness::row_colour(&buffer, 2), Color::Red);
    }

    #[test]
    fn a_level_that_traded_evenly_goes_to_the_buy_colour() {
        // `>=` rather than `>`, so an even level is drawn rather than left to
        // fall through to the side that did not win it.
        let view = FootprintView {
            symbol: "S".to_owned(),
            levels: vec![level(100.0, 4.0, 4.0)],
        };
        let buffer = harness::draw(40, 4, |frame, area| render(frame, area, &view, false));
        assert_eq!(harness::row_colour(&buffer, 1), Color::Green);
    }

    #[test]
    fn both_volumes_are_shown_at_each_price() {
        let view = FootprintView {
            symbol: "BTC/USDT".to_owned(),
            levels: vec![level(100.5, 2.25, 0.75)],
        };
        let buffer = harness::draw(44, 4, |frame, area| render(frame, area, &view, true));
        let row = harness::row(&buffer, 1);
        assert!(row.contains("100.50"), "{row}");
        assert!(row.contains("2.250"), "{row}");
        assert!(row.contains("0.750"), "{row}");
    }
}
