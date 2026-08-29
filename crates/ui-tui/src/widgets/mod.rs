//! ratatui widgets — one per [`PanelView`] variant.
//!
//! Each widget is a pure function from a view-model to a rendered area. The
//! renderer never inspects state; it only maps the core's view-models to
//! ratatui, which is exactly what makes the TUI one interchangeable renderer.

pub(crate) mod bars;
pub(crate) mod book;
pub(crate) mod chart;
mod footprint;
mod profile;
pub(crate) mod tape;
pub(crate) mod watchlist;

use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::widgets::Block;
use ratatui::Frame;
use wickra_terminal_core::PanelView;

/// The bordered block every panel draws itself into.
///
/// The focused one is highlighted. Focus has to be visible somewhere or the
/// keybinds that move it are indistinguishable from keybinds that do nothing —
/// which is what `tab` and `backtab` were until now.
#[must_use]
pub(crate) fn panel_block(title: String, focused: bool) -> Block<'static> {
    let block = Block::bordered().title(title);
    if focused {
        block.border_style(Style::new().cyan().bold())
    } else {
        block
    }
}

/// Render one panel's view-model into `area`.
pub(crate) fn render_panel(frame: &mut Frame, area: Rect, panel: &PanelView, focused: bool) {
    match panel {
        PanelView::Chart(view) => chart::render(frame, area, view, focused),
        PanelView::Book(view) => book::render(frame, area, view, focused),
        PanelView::Tape(view) => tape::render(frame, area, view, focused),
        PanelView::Watchlist(view) => watchlist::render(frame, area, view, focused),
        PanelView::Footprint(view) => footprint::render(frame, area, view, focused),
        PanelView::Profile(view) => profile::render(frame, area, view, focused),
        PanelView::Bars(view) => bars::render(frame, area, view, focused),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal as TuiTerminal;
    use wickra_terminal_core::config::{PanelSpec, RectSpec};
    use wickra_terminal_core::panels::PanelKind;
    use wickra_terminal_core::{Config, IndicatorSpec, SourceSpec, Symbol, Terminal};

    /// A terminal laid out with every panel this renderer can draw.
    fn every_panel() -> Terminal {
        let mut config = Config::default_layout();
        config.sources = vec![SourceSpec::Synth { seed: 7 }];
        config.profiles = vec![IndicatorSpec::new("VolumeProfile", vec![4.0, 8.0])];
        config.bars = vec![IndicatorSpec::new("RenkoBars", vec![3.0])];
        config.layout.panels = [
            PanelKind::Chart,
            PanelKind::Book,
            PanelKind::Tape,
            PanelKind::Watchlist,
            PanelKind::Footprint,
            PanelKind::Profile,
            PanelKind::Bars,
        ]
        .into_iter()
        .enumerate()
        .map(|(index, kind)| PanelSpec {
            kind,
            rect: RectSpec {
                x: 0,
                #[allow(clippy::cast_possible_truncation)]
                y: index as u16 * 10,
                w: 100,
                h: 10,
            },
        })
        .collect();
        let mut terminal = Terminal::new(&config).unwrap();
        let symbol = Symbol::new("BTC", "USDT");
        terminal.subscribe(0, &symbol).unwrap();
        terminal.set_focus(0, &symbol);
        terminal
    }

    #[test]
    fn every_panel_kind_reaches_a_widget() {
        // The dispatch is a match over the view-model, so a panel added to the
        // core without a widget here is a compile error -- but a panel whose
        // arm draws nothing is not, and neither is one no test ever renders.
        // This drives all seven through the real dispatch.
        let mut terminal = every_panel();
        for _ in 0..64 {
            terminal.tick();
        }
        let frame = terminal.frame();
        assert_eq!(frame.panels.len(), 7);

        let mut tui = TuiTerminal::new(TestBackend::new(100, 10)).unwrap();
        for panel in &frame.panels {
            tui.draw(|f| render_panel(f, f.area(), panel, true))
                .unwrap();
            let drawn: String = tui
                .backend()
                .buffer()
                .content()
                .iter()
                .map(ratatui::buffer::Cell::symbol)
                .collect();
            assert!(
                drawn.trim().chars().any(|c| c != ' '),
                "a panel rendered blank: {panel:?}"
            );
        }
    }

    #[test]
    fn a_focused_block_is_styled_and_an_unfocused_one_is_not() {
        assert_ne!(
            panel_block("t".to_owned(), true),
            panel_block("t".to_owned(), false)
        );
    }
}
