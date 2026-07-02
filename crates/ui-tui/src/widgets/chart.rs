//! The chart widget: a compact price sparkline with indicator overlays.

use ratatui::layout::Rect;
use ratatui::text::Line;
use ratatui::widgets::{Block, Paragraph};
use ratatui::Frame;
use terminal_core::view::ChartView;

/// Block glyphs from empty to full, for the text sparkline.
const LEVELS: [char; 8] = [
    ' ', '\u{2581}', '\u{2582}', '\u{2583}', '\u{2584}', '\u{2585}', '\u{2586}', '\u{2587}',
];

/// Render a price series as a single-line block sparkline, using at most `width`
/// of the most recent points.
#[must_use]
pub fn sparkline(series: &[f64], width: usize) -> String {
    if series.is_empty() || width == 0 {
        return String::new();
    }
    let recent = &series[series.len().saturating_sub(width)..];
    let min = recent.iter().copied().fold(f64::INFINITY, f64::min);
    let max = recent.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let range = (max - min).max(f64::EPSILON);
    recent
        .iter()
        .map(|&v| {
            let level = ((v - min) / range * (LEVELS.len() - 1) as f64).round() as usize;
            LEVELS[level.min(LEVELS.len() - 1)]
        })
        .collect()
}

/// Render the chart panel.
pub fn render(frame: &mut Frame, area: Rect, view: &ChartView) {
    let inner_width = usize::from(area.width.saturating_sub(2));
    let spark = sparkline(&view.series, inner_width);
    let indicators = view
        .indicators
        .iter()
        .map(|indicator| {
            let value = indicator
                .value
                .map_or_else(|| "\u{2026}".to_string(), |v| format!("{v:.2}"));
            format!("{}={}", indicator.name, value)
        })
        .collect::<Vec<_>>()
        .join("  ");
    let lines = vec![Line::from(spark), Line::from(indicators)];
    let title = format!("Chart {} last={:.2}", view.symbol, view.last);
    frame.render_widget(
        Paragraph::new(lines).block(Block::bordered().title(title)),
        area,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sparkline_maps_min_and_max_to_endpoints() {
        let s = sparkline(&[1.0, 2.0, 3.0], 3);
        let chars: Vec<char> = s.chars().collect();
        assert_eq!(chars.len(), 3);
        assert_eq!(chars[0], ' '); // min -> empty
        assert_eq!(chars[2], LEVELS[LEVELS.len() - 1]); // max -> full
    }

    #[test]
    fn sparkline_empty_or_zero_width_is_empty() {
        assert_eq!(sparkline(&[], 10), "");
        assert_eq!(sparkline(&[1.0, 2.0], 0), "");
    }

    #[test]
    fn sparkline_truncates_to_width() {
        assert_eq!(sparkline(&[1.0, 2.0, 3.0, 4.0, 5.0], 2).chars().count(), 2);
    }

    #[test]
    fn sparkline_flat_series_does_not_panic() {
        let s = sparkline(&[5.0, 5.0, 5.0], 3);
        assert_eq!(s.chars().count(), 3);
    }
}
