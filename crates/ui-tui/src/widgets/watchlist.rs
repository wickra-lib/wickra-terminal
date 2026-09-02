//! The watchlist widget: every tracked market, its price, spread and move.

use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;
use wickra_terminal_core::view::WatchlistView;

/// A rolling volume as a person reads it.
///
/// A venue's base-asset volume runs to seven figures on a liquid market and to
/// three decimals on an illiquid one, and a column wide enough for both is a
/// column that fits nothing else.
fn compact(volume: f64) -> String {
    let magnitude = volume.abs();
    if magnitude >= 1e9 {
        format!("{:.2}B", volume / 1e9)
    } else if magnitude >= 1e6 {
        format!("{:.2}M", volume / 1e6)
    } else if magnitude >= 1e3 {
        format!("{:.2}K", volume / 1e3)
    } else {
        format!("{volume:.2}")
    }
}

/// Render the watchlist panel.
pub(crate) fn render(
    frame: &mut Frame,
    area: Rect,
    view: &WatchlistView,
    focused: bool,
    scroll: u16,
) {
    let lines: Vec<Line> = view
        .rows
        .iter()
        .map(|row| {
            // The change is the one column worth colouring: it is the only one
            // whose sign carries meaning, and colouring the price as well would
            // make the row a wall of green that says nothing.
            let tint = if row.change > 0.0 {
                Color::Green
            } else if row.change < 0.0 {
                Color::Red
            } else {
                Color::Gray
            };
            // A market with no ticker yet reports a zero bid and ask, and a
            // spread of "0.00" there would be a claim rather than a blank.
            let spread = if row.bid > 0.0 && row.ask > 0.0 {
                format!("{:>10.2}", row.ask - row.bid)
            } else {
                format!("{:>10}", "-")
            };
            Line::from(vec![
                Span::raw(format!(
                    "[{}] {:<12} {:>12.2} ",
                    row.source, row.symbol, row.last
                )),
                Span::styled(format!("{:>8.2}%", row.change), Style::default().fg(tint)),
                Span::raw(format!("{spread} {:>10}", compact(row.volume))),
            ])
        })
        .collect();
    frame.render_widget(
        Paragraph::new(lines)
            .scroll((scroll, 0))
            .block(super::panel_block("Watchlist".to_string(), focused)),
        area,
    );
}

#[cfg(test)]
mod tests {
    use super::{compact, render};
    use crate::widgets::harness;
    use ratatui::style::Color;
    use wickra_terminal_core::view::{WatchRow, WatchlistView};

    fn row(symbol: &str, last: f64, change: f64, bid: f64, ask: f64, volume: f64) -> WatchRow {
        WatchRow {
            source: 0,
            symbol: symbol.to_owned(),
            last,
            bid,
            ask,
            volume,
            change,
        }
    }

    /// The colour of the change column, which is not the first thing in the row.
    ///
    /// `harness::row_colour` answers with the first cell a widget drew, and here
    /// that is the untinted price prefix -- so the assertion has to look for the
    /// cell carrying the percentage.
    fn change_colour(buffer: &ratatui::buffer::Buffer, y: u16) -> Color {
        use ratatui::layout::Position;
        let at = (0..buffer.area.width)
            .find(|x| {
                buffer
                    .cell(Position::new(*x, y))
                    .is_some_and(|cell| cell.symbol() == "%")
            })
            .expect("the row draws a percentage");
        buffer
            .cell(Position::new(at, y))
            .map_or(Color::Reset, |cell| cell.fg)
    }

    /// Only the movers carry colour.
    ///
    /// The change is the one column whose sign means something; tinting the
    /// price as well would make the panel a wall of green that says nothing, and
    /// leaving an unmoved market tinted would say it had moved.
    #[test]
    fn the_change_column_is_tinted_by_its_sign_and_nothing_else_is() {
        let view = WatchlistView {
            rows: vec![
                row("BTC/USDT", 110.0, 10.0, 109.0, 111.0, 5_000.0),
                row("ETH/USDT", 90.0, -10.0, 89.0, 91.0, 2_000.0),
                row("SOL/USDT", 100.0, 0.0, 99.0, 101.0, 1_000.0),
            ],
        };
        let buffer = harness::draw(70, 6, |frame, area| render(frame, area, &view, false, 0));
        assert_eq!(change_colour(&buffer, 1), Color::Green);
        assert_eq!(change_colour(&buffer, 2), Color::Red);
        assert_eq!(change_colour(&buffer, 3), Color::Gray);
        // The price ahead of it is untinted, so the row reads at a glance.
        assert_eq!(harness::row_colour(&buffer, 1), Color::Reset);
    }

    /// A market with no ticker shows a dash, not a spread of nothing.
    ///
    /// On a watchlist that is the difference between a market locked at the
    /// touch and one the terminal simply has no quote for.
    #[test]
    fn a_row_without_a_quote_draws_a_dash_rather_than_a_zero_spread() {
        let view = WatchlistView {
            rows: vec![
                row("BTC/USDT", 100.0, 0.0, 0.0, 0.0, 0.0),
                row("ETH/USDT", 100.0, 0.0, 99.5, 100.5, 3_500_000.0),
            ],
        };
        let buffer = harness::draw(70, 5, |frame, area| render(frame, area, &view, false, 0));
        let quoteless = harness::row(&buffer, 1);
        assert!(quoteless.contains('-'), "{quoteless}");
        let quoted = harness::row(&buffer, 2);
        assert!(quoted.contains("1.00"), "the spread is missing: {quoted}");
        assert!(
            quoted.contains("3.50M"),
            "the volume is not abbreviated: {quoted}"
        );
    }

    #[test]
    fn an_empty_watchlist_still_draws_its_panel() {
        let view = WatchlistView { rows: Vec::new() };
        let buffer = harness::draw(30, 4, |frame, area| render(frame, area, &view, true, 0));
        assert!(harness::text(&buffer).contains("Watchlist"));
    }

    /// The same thresholds the browser's `compactVolume` uses.
    ///
    /// The two renderers show one watchlist, and a column that abbreviates in
    /// one and not in the other reads as two different numbers for the same
    /// market -- so the pair is pinned here and in `web/src/__tests__`.
    #[test]
    fn a_volume_abbreviates_at_each_threshold_and_not_below_one() {
        assert_eq!(compact(999.5), "999.50");
        assert_eq!(compact(1_500.0), "1.50K");
        assert_eq!(compact(2_500_000.0), "2.50M");
        assert_eq!(compact(3_250_000_000.0), "3.25B");
    }

    /// A negative turnover is a feed fault, and hiding the sign hides it.
    #[test]
    fn a_negative_volume_keeps_its_sign() {
        assert_eq!(compact(-2_000_000.0), "-2.00M");
    }
}
