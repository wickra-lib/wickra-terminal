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

/// Render one panel's view-model into `area`, scrolled by `scroll` rows.
///
/// The scroll offset is a renderer concern and stays one: the core sends what a
/// panel carries, and each front-end decides how much of that it can show. A
/// browser scrolls a div for free; a terminal has to be told, which is why this
/// is the only place the number appears.
///
/// The chart ignores it. It is a plot rather than a list of rows, and scrolling
/// a plot would mean panning its axes -- a different gesture with a different
/// meaning, not this one.
pub(crate) fn render_panel(
    frame: &mut Frame,
    area: Rect,
    panel: &PanelView,
    focused: bool,
    scroll: u16,
) {
    match panel {
        PanelView::Chart(view) => chart::render(frame, area, view, focused),
        PanelView::Book(view) => book::render(frame, area, view, focused, scroll),
        PanelView::Tape(view) => tape::render(frame, area, view, focused, scroll),
        PanelView::Watchlist(view) => watchlist::render(frame, area, view, focused, scroll),
        PanelView::Footprint(view) => footprint::render(frame, area, view, focused, scroll),
        PanelView::Profile(view) => profile::render(frame, area, view, focused, scroll),
        PanelView::Bars(view) => bars::render(frame, area, view, focused, scroll),
    }
}

/// Drawing a widget into a buffer, for the tests in this module's children.
///
/// A widget's job ends at the buffer, so that is what a test should read: the
/// text a reader would see and the colour it is in. Colour is not decoration
/// here -- it is which side of the book a level is on, and which way a print
/// went -- so a test that reads only text checks half of what was drawn.
#[cfg(test)]
pub(crate) mod harness {
    use ratatui::backend::TestBackend;
    use ratatui::buffer::{Buffer, Cell};
    use ratatui::layout::{Position, Rect};
    use ratatui::style::Color;
    use ratatui::{Frame, Terminal};

    /// The characters the surrounding block draws, which no widget put there.
    const CHROME: [&str; 7] = [" ", "│", "─", "┌", "┐", "└", "┘"];

    /// Draw `render` into a fresh buffer of this size.
    pub(crate) fn draw(width: u16, height: u16, render: impl FnOnce(&mut Frame, Rect)) -> Buffer {
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        terminal.draw(|frame| render(frame, frame.area())).unwrap();
        terminal.backend().buffer().clone()
    }

    /// Row `y` as text, with the block's border and padding trimmed off.
    pub(crate) fn row(buffer: &Buffer, y: u16) -> String {
        (0..buffer.area.width)
            .filter_map(|x| buffer.cell(Position::new(x, y)))
            .map(Cell::symbol)
            .collect::<String>()
            .trim_matches(|c: char| c == '│' || c == ' ')
            .to_owned()
    }

    /// The whole buffer as text, one row per line.
    pub(crate) fn text(buffer: &Buffer) -> String {
        (0..buffer.area.height)
            .map(|y| row(buffer, y))
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// The foreground colour of the first cell in row `y` the widget drew.
    pub(crate) fn row_colour(buffer: &Buffer, y: u16) -> Color {
        (0..buffer.area.width)
            .filter_map(|x| buffer.cell(Position::new(x, y)))
            .find(|cell| !CHROME.contains(&cell.symbol()))
            .map_or(Color::Reset, |cell| cell.fg)
    }
}

#[cfg(test)]
mod tests {
    use super::harness;
    use super::*;
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
            depth: None,
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

        for panel in &frame.panels {
            let buffer = harness::draw(100, 10, |f, area| render_panel(f, area, panel, true, 0));
            assert!(
                harness::text(&buffer).trim().chars().any(|c| c != ' '),
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
