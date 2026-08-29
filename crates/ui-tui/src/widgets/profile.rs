//! The profile widget: each configured distribution as a horizontal histogram.

use ratatui::layout::Rect;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::widgets::Paragraph;
use ratatui::Frame;
use wickra_terminal_core::view::{ProfileRow, ProfileView};

/// The block characters a bar is drawn from, in eighths.
///
/// A histogram in a terminal is one row per bin, so the resolution has to come
/// from within the cell rather than from more rows.
const EIGHTHS: [char; 8] = ['▏', '▎', '▍', '▌', '▋', '▊', '▉', '█'];

/// Draw one bin as a bar, scaled to the widest bin in its profile.
///
/// Scaled per profile rather than globally: two distributions on one panel
/// measure different things — volume and a count of time slots — and a shared
/// scale would flatten whichever has the smaller units into nothing.
fn bar(value: f64, peak: f64, width: usize) -> String {
    if peak <= 0.0 || !value.is_finite() || value <= 0.0 {
        return String::new();
    }
    let filled = (value / peak) * width as f64 * 8.0;
    let whole = (filled / 8.0) as usize;
    let remainder = (filled as usize) % 8;
    let mut out: String = "█".repeat(whole.min(width));
    if whole < width && remainder > 0 {
        out.push(EIGHTHS[remainder - 1]);
    }
    out
}

/// One profile: a header naming it and its range, then a row per bin.
fn rows(profile: &ProfileRow, width: usize) -> Vec<Line<'static>> {
    let mut lines = Vec::with_capacity(profile.bins.len() + 1);
    let header = match (profile.price_low, profile.price_high) {
        (Some(low), Some(high)) => format!("{} [{low:.2} – {high:.2}]", profile.label),
        _ => profile.label.clone(),
    };
    lines.push(Line::from(header).bold());
    if profile.bins.is_empty() {
        lines.push(Line::from("  (warming up)").dim());
        return lines;
    }
    let peak = profile
        .bins
        .iter()
        .copied()
        .filter(|bin| bin.is_finite())
        .fold(0.0_f64, f64::max);
    for bin in &profile.bins {
        lines.push(Line::from(format!("  {}", bar(*bin, peak, width))).cyan());
    }
    lines
}

/// Render the profile panel.
pub(crate) fn render(frame: &mut Frame, area: Rect, view: &ProfileView, focused: bool) {
    // Two columns of chrome from the block, two from the row indent.
    let width = usize::from(area.width.saturating_sub(4)).max(1);
    let lines: Vec<Line> = view
        .profiles
        .iter()
        .flat_map(|profile| rows(profile, width))
        .collect();
    frame.render_widget(
        Paragraph::new(lines).block(super::panel_block(
            format!("Profiles {}", view.symbol),
            focused,
        )),
        area,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::widgets::harness;

    fn row(label: &str, bins: Vec<f64>, priced: bool) -> ProfileRow {
        ProfileRow {
            label: label.to_owned(),
            bins,
            price_low: priced.then_some(100.0),
            price_high: priced.then_some(110.0),
        }
    }

    /// The whole buffer as text, so a test can assert on what a reader sees
    /// rather than on the widget tree that produced it.
    fn drawn(view: &ProfileView, width: u16, height: u16) -> String {
        harness::text(&harness::draw(width, height, |frame, area| {
            render(frame, area, view, true);
        }))
    }

    #[test]
    fn a_bar_fills_in_proportion_to_the_peak() {
        assert_eq!(bar(10.0, 10.0, 4).chars().count(), 4);
        assert_eq!(bar(5.0, 10.0, 4).chars().count(), 2);
        assert!(bar(10.0, 10.0, 4).chars().all(|c| c == '█'));
    }

    #[test]
    fn a_partial_bar_ends_in_an_eighth() {
        // Five sixteenths of four cells is two cells and two eighths.
        let drawn = bar(5.0, 16.0, 4);
        assert_eq!(drawn.chars().next_back(), Some(EIGHTHS[1]));
    }

    #[test]
    fn a_bar_with_nothing_to_show_is_empty() {
        assert_eq!(bar(1.0, 0.0, 4), "");
        assert_eq!(bar(0.0, 10.0, 4), "");
        assert_eq!(bar(-1.0, 10.0, 4), "");
        assert_eq!(bar(f64::NAN, 10.0, 4), "");
        assert_eq!(bar(f64::INFINITY, 10.0, 4), "");
    }

    #[test]
    fn a_priced_profile_names_its_range_and_a_timed_one_does_not() {
        let priced = rows(&row("VolumeProfile(4,8)", vec![1.0], true), 8);
        assert_eq!(
            priced[0].to_string(),
            "VolumeProfile(4,8) [100.00 – 110.00]"
        );
        let timed = rows(&row("DayOfWeekProfile", vec![1.0], false), 8);
        assert_eq!(timed[0].to_string(), "DayOfWeekProfile");
    }

    #[test]
    fn a_profile_with_no_bins_says_so_rather_than_drawing_nothing() {
        // An empty panel and a warming-up panel look identical otherwise, and
        // the difference is the whole question a reader has.
        let lines = rows(&row("VolumeProfile(4,8)", Vec::new(), true), 8);
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[1].to_string(), "  (warming up)");
    }

    #[test]
    fn one_line_per_bin_follows_the_header() {
        let lines = rows(&row("P", vec![1.0, 2.0, 3.0], false), 8);
        assert_eq!(lines.len(), 4);
    }

    #[test]
    fn an_infinite_bin_does_not_set_the_peak() {
        // Infinity is what the finite filter is actually for: `f64::max`
        // already ignores NaN, but folding an infinity in makes the peak
        // infinite, and every real bin is then scaled to nothing.
        let lines = rows(&row("P", vec![f64::INFINITY, 4.0], false), 4);
        assert_eq!(lines[1].to_string().trim(), "");
        assert_eq!(lines[2].to_string().trim(), "████");
    }

    #[test]
    fn rendering_draws_the_symbol_and_the_bins() {
        let view = ProfileView {
            symbol: "BTC/USDT".to_owned(),
            profiles: vec![row("VolumeProfile(4,8)", vec![1.0, 2.0], true)],
        };
        let text = drawn(&view, 44, 6);
        assert!(text.contains("Profiles BTC/USDT"), "{text}");
        assert!(text.contains("VolumeProfile(4,8)"), "{text}");
        assert!(text.contains('█'), "{text}");
    }

    #[test]
    fn rendering_into_a_pane_too_narrow_for_its_chrome_does_not_panic() {
        // The width the bars are scaled to is the pane less four columns of
        // chrome, which underflows on a pane narrower than that.
        let view = ProfileView {
            symbol: "S".to_owned(),
            profiles: vec![row("P", vec![1.0], false)],
        };
        assert!(!drawn(&view, 3, 4).is_empty());
    }
}
