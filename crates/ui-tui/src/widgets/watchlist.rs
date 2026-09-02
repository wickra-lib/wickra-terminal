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
    use super::compact;

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
