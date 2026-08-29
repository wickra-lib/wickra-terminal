//! The bars widget: each alternative chart as a row of rising and falling marks.

use ratatui::layout::Rect;
use ratatui::style::Stylize;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;
use wickra_terminal_core::view::{BarStreamView, BarsView};

/// One stream: a header naming it, then its bars as coloured marks.
///
/// A mark per bar rather than a candle per bar, because these charts have no
/// time axis: the interesting thing is the sequence of ups and downs and how
/// long each run is, which a row of marks shows in a terminal and a column of
/// candles does not.
fn rows(stream: &BarStreamView) -> Vec<Line<'static>> {
    let mut lines = Vec::with_capacity(3);
    lines.push(Line::from(stream.label.clone()).bold());
    if stream.bars.is_empty() {
        lines.push(Line::from("  (no bars completed yet)").dim());
        return lines;
    }
    let marks: Vec<Span> = stream
        .bars
        .iter()
        .map(|bar| {
            if bar.direction >= 0 {
                Span::from("▲").green()
            } else {
                Span::from("▼").red()
            }
        })
        .collect();
    lines.push(Line::from(marks));
    // The last bar in figures, since the marks say direction and nothing else.
    if let Some(last) = stream.bars.last() {
        let volume = last
            .volume
            .map(|v| format!("  vol {v:.3}"))
            .unwrap_or_default();
        lines.push(
            Line::from(format!(
                "  {:.2} → {:.2}  [{:.2} {:.2}]{volume}",
                last.open, last.close, last.low, last.high
            ))
            .dim(),
        );
    }
    lines
}

/// Render the bars panel.
pub(crate) fn render(frame: &mut Frame, area: Rect, view: &BarsView, focused: bool) {
    let lines: Vec<Line> = view.streams.iter().flat_map(rows).collect();
    frame.render_widget(
        Paragraph::new(lines).block(super::panel_block(format!("Bars {}", view.symbol), focused)),
        area,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;
    use wickra_terminal_core::registry::AltBar;

    fn brick(open: f64, close: f64, direction: i8, volume: Option<f64>) -> AltBar {
        AltBar {
            open,
            high: open.max(close),
            low: open.min(close),
            close,
            direction,
            volume,
        }
    }

    fn stream(label: &str, bars: Vec<AltBar>) -> BarStreamView {
        BarStreamView {
            label: label.to_owned(),
            bars,
        }
    }

    fn drawn(view: &BarsView, width: u16, height: u16) -> String {
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        terminal
            .draw(|frame| render(frame, frame.area(), view, false))
            .unwrap();
        terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(ratatui::buffer::Cell::symbol)
            .collect()
    }

    #[test]
    fn a_stream_with_no_bars_says_so_rather_than_showing_a_bare_label() {
        // These charts complete zero bars on most candles, so "nothing yet" is
        // the normal state and has to be distinguishable from a broken panel.
        let lines = rows(&stream("RenkoBars(3)", Vec::new()));
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[1].to_string(), "  (no bars completed yet)");
    }

    #[test]
    fn each_bar_becomes_a_mark_in_its_direction() {
        let lines = rows(&stream(
            "RenkoBars(3)",
            vec![
                brick(10.0, 11.0, 1, None),
                brick(11.0, 10.0, -1, None),
                brick(10.0, 11.0, 1, None),
            ],
        ));
        assert_eq!(lines[1].to_string(), "▲▼▲");
    }

    #[test]
    fn a_bar_with_no_direction_reads_as_rising() {
        // `direction` is zero only for the builders that do not carry one, and
        // the mark has to be one thing or the other.
        assert_eq!(
            rows(&stream("B", vec![brick(10.0, 10.0, 0, None)]))[1].to_string(),
            "▲"
        );
    }

    #[test]
    fn the_last_bar_is_spelled_out_because_the_marks_only_say_direction() {
        let lines = rows(&stream(
            "RenkoBars(3)",
            vec![brick(10.0, 11.0, 1, None), brick(11.0, 12.5, 1, None)],
        ));
        assert_eq!(lines[2].to_string(), "  11.00 → 12.50  [11.00 12.50]");
    }

    #[test]
    fn volume_is_shown_only_by_the_bar_types_that_carry_it() {
        // A Renko brick is a price move, not a period; printing "vol 0" would
        // read as "no volume traded" rather than "this chart does not measure
        // volume".
        let with = rows(&stream(
            "VolumeBars(500)",
            vec![brick(1.0, 2.0, 1, Some(7.5))],
        ));
        assert!(with[2].to_string().ends_with("vol 7.500"), "{}", with[2]);
        let without = rows(&stream("RenkoBars(3)", vec![brick(1.0, 2.0, 1, None)]));
        assert!(!without[2].to_string().contains("vol"), "{}", without[2]);
    }

    #[test]
    fn rendering_draws_the_symbol_and_every_stream() {
        let view = BarsView {
            symbol: "BTC/USDT".to_owned(),
            streams: vec![
                stream("RenkoBars(3)", vec![brick(10.0, 11.0, 1, None)]),
                stream("KagiBars(2)", Vec::new()),
            ],
        };
        let text = drawn(&view, 48, 8);
        assert!(text.contains("Bars BTC/USDT"), "{text}");
        assert!(text.contains("RenkoBars(3)"), "{text}");
        assert!(text.contains("KagiBars(2)"), "{text}");
        assert!(text.contains('▲'), "{text}");
    }

    #[test]
    fn rendering_an_empty_panel_does_not_panic() {
        let view = BarsView {
            symbol: "S".to_owned(),
            streams: Vec::new(),
        };
        assert!(drawn(&view, 12, 4).contains("Bars S"));
    }
}
