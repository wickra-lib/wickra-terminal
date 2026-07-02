//! Frame rendering: place each panel's widget on its configured grid rect.

use ratatui::layout::Rect;
use ratatui::text::Line;
use ratatui::widgets::Paragraph;
use ratatui::Frame as TuiFrame;
use terminal_core::{Config, Frame, RectSpec};

use crate::widgets;

/// Map a percent-of-screen [`RectSpec`] onto a concrete area.
#[must_use]
pub fn rect_from_spec(area: Rect, spec: RectSpec) -> Rect {
    let pct =
        |dim: u16, percent: u16| -> u16 { (u32::from(dim) * u32::from(percent) / 100) as u16 };
    Rect {
        x: area.x + pct(area.width, spec.x),
        y: area.y + pct(area.height, spec.y),
        width: pct(area.width, spec.w),
        height: pct(area.height, spec.h),
    }
}

/// Draw a frame of view-models plus a one-line footer (the open prompt or the
/// last status message). With no subscription (an empty frame) it draws a short
/// hint instead of panels.
pub fn draw(frame: &mut TuiFrame, view: &Frame, config: &Config, footer: &str) {
    let full = frame.area();
    let footer_height = 1;
    let area = Rect {
        height: full.height.saturating_sub(footer_height),
        ..full
    };
    let footer_area = Rect {
        y: full.y + full.height.saturating_sub(footer_height),
        height: footer_height.min(full.height),
        ..full
    };

    if view.panels.is_empty() {
        let hint = Paragraph::new(vec![
            Line::from("wickra-terminal"),
            Line::from("no market subscribed — press s to add a source, or pass --source"),
        ]);
        frame.render_widget(hint, area);
    } else {
        for (spec, panel) in config.layout.panels.iter().zip(&view.panels) {
            let rect = rect_from_spec(area, spec.rect);
            widgets::render_panel(frame, rect, panel);
        }
    }

    frame.render_widget(Paragraph::new(footer.to_string()), footer_area);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rect_from_spec_maps_percentages() {
        let area = Rect::new(0, 0, 100, 100);
        let r = rect_from_spec(area, RectSpec::new(10, 20, 50, 30));
        assert_eq!((r.x, r.y, r.width, r.height), (10, 20, 50, 30));
    }

    #[test]
    fn rect_from_spec_respects_area_offset() {
        let area = Rect::new(10, 5, 200, 40);
        let r = rect_from_spec(area, RectSpec::new(0, 0, 50, 50));
        assert_eq!((r.x, r.y, r.width, r.height), (10, 5, 100, 20));
    }
}
