//! Frame rendering: place each panel's widget on its configured grid rect.

use ratatui::layout::Rect;
use ratatui::text::Line;
use ratatui::widgets::Paragraph;
use ratatui::Frame as TuiFrame;
use wickra_terminal_core::{Config, Frame, RectSpec};

use crate::widgets;

/// Map a percent-of-screen [`RectSpec`] onto a concrete area.
#[must_use]
pub(crate) fn rect_from_spec(area: Rect, spec: RectSpec) -> Rect {
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
///
/// `focused_panel` indexes `config.layout.panels`; that panel is drawn with a
/// highlighted border. An index past the end simply highlights nothing, which is
/// what an empty layout should look like.
pub(crate) fn draw(
    frame: &mut TuiFrame,
    view: &Frame,
    config: &Config,
    footer: &str,
    focused_panel: usize,
) {
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
        for (index, (spec, panel)) in config.layout.panels.iter().zip(&view.panels).enumerate() {
            let rect = rect_from_spec(area, spec.rect);
            widgets::render_panel(frame, rect, panel, index == focused_panel);
        }
    }

    frame.render_widget(Paragraph::new(footer.to_string()), footer_area);
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::widgets::harness;
    use wickra_terminal_core::config::{PanelSpec, RectSpec as Spec};
    use wickra_terminal_core::panels::PanelKind;
    use wickra_terminal_core::{SourceSpec, Symbol, Terminal};

    /// The character a panel's border is drawn with.
    const BORDER: char = '│';

    /// A frame of two panels side by side, and the config that placed them.
    fn two_panels() -> (Frame, Config) {
        let mut config = Config::default_layout();
        config.sources = vec![SourceSpec::Synth { seed: 3 }];
        config.layout.panels = vec![
            PanelSpec {
                kind: PanelKind::Chart,
                rect: Spec::new(0, 0, 50, 100),
            },
            PanelSpec {
                kind: PanelKind::Tape,
                rect: Spec::new(50, 0, 50, 100),
            },
        ];
        let mut terminal = Terminal::new(&config).unwrap();
        let symbol = Symbol::new("BTC", "USDT");
        terminal.subscribe(0, &symbol).unwrap();
        terminal.set_focus(0, &symbol);
        for _ in 0..32 {
            terminal.tick();
        }
        (terminal.frame(), config)
    }

    #[test]
    fn an_unsubscribed_terminal_says_so_instead_of_drawing_empty_panels() {
        // A screen of empty bordered boxes reads as a broken terminal. The hint
        // says which key adds a market instead.
        let config = Config::default_layout();
        let empty = Frame { panels: Vec::new() };
        let buffer = harness::draw(70, 6, |frame, _| draw(frame, &empty, &config, "", 0));
        let text = harness::text(&buffer);
        assert!(text.contains("no market subscribed"), "{text}");
        assert!(!text.contains(BORDER), "panels were drawn anyway: {text}");
    }

    #[test]
    fn each_panel_lands_on_its_configured_rect() {
        // The layout is percent-of-screen, so the second panel of a 50/50 split
        // starts at the midpoint. Drawn from a common origin they overlap and
        // the last one wins, which reads as a panel that went missing.
        let (view, config) = two_panels();
        let buffer = harness::draw(80, 10, |frame, _| draw(frame, &view, &config, "", 0));
        let top = harness::row(&buffer, 0);
        let chart = top
            .find("Chart")
            .unwrap_or_else(|| panic!("no chart in {top:?}"));
        let tape = top
            .find("Tape")
            .unwrap_or_else(|| panic!("no tape in {top:?}"));
        assert!(chart < 40 && tape >= 40, "chart at {chart}, tape at {tape}");
    }

    #[test]
    fn the_footer_takes_the_last_row_and_the_panels_stop_above_it() {
        let (view, config) = two_panels();
        let buffer = harness::draw(80, 10, |frame, _| {
            draw(frame, &view, &config, "source added", 0);
        });
        let footer = harness::row(&buffer, 9);
        assert_eq!(footer, "source added");
        assert!(!footer.contains(BORDER));
    }

    #[test]
    fn the_focused_panel_is_drawn_differently_from_the_others() {
        let (view, config) = two_panels();
        let first = harness::draw(80, 10, |frame, _| draw(frame, &view, &config, "", 0));
        let second = harness::draw(80, 10, |frame, _| draw(frame, &view, &config, "", 1));
        assert_ne!(first, second, "moving the focus changed nothing on screen");
    }

    #[test]
    fn a_focus_index_past_the_end_highlights_nothing() {
        // Documented behaviour, and what an empty layout has to look like: the
        // index is not clamped back onto the first panel.
        let (view, config) = two_panels();
        let none = harness::draw(80, 10, |frame, _| draw(frame, &view, &config, "", 99));
        let first = harness::draw(80, 10, |frame, _| draw(frame, &view, &config, "", 0));
        assert_ne!(none, first);
    }

    #[test]
    fn a_screen_with_no_room_for_a_footer_does_not_panic() {
        // One row: the panel area saturates to nothing and the footer takes it.
        let (view, config) = two_panels();
        let buffer = harness::draw(20, 1, |frame, _| draw(frame, &view, &config, "x", 0));
        assert!(!buffer.content().is_empty());
    }

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
