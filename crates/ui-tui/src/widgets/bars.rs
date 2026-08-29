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
