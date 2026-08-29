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
