//! The renderer's application loop state.
//!
//! [`App`] owns a [`Terminal`] and reduces user [`Action`]s onto it. It ticks the
//! core synchronously each frame (the pull-based sources are drained per tick);
//! the core owns the feed, so the renderer stays a thin driver.

use terminal_core::{Frame, Terminal};

use crate::input::Action;

/// The renderer state driven by the event loop.
pub struct App {
    /// The terminal core this renderer drives.
    pub terminal: Terminal,
    /// Set once the user asks to quit.
    pub should_quit: bool,
    /// The most recent frame of view-models.
    pub frame: Frame,
}

impl App {
    /// Wrap a terminal core in a fresh app.
    #[must_use]
    pub fn new(terminal: Terminal) -> Self {
        Self {
            terminal,
            should_quit: false,
            frame: Frame { panels: Vec::new() },
        }
    }

    /// Pump the core and capture the next frame.
    pub fn update(&mut self) {
        self.frame = self.terminal.tick();
    }

    /// Reduce a user action onto the terminal.
    pub fn on_action(&mut self, action: Action) {
        match action {
            Action::Quit => self.should_quit = true,
            Action::NextSymbol => self.cycle_symbol(true),
            Action::PrevSymbol => self.cycle_symbol(false),
            // Panel focus and the source menu land in later phases; the keys are
            // recognised now but do not yet change state.
            Action::NextPanel | Action::PrevPanel | Action::SourceMenu | Action::None => {}
        }
    }

    /// Move focus to the next/previous watched market.
    fn cycle_symbol(&mut self, forward: bool) {
        let watchlist = self.terminal.state().watchlist.clone();
        let len = watchlist.len();
        if len == 0 {
            return;
        }
        let current = self.terminal.state().focus.clone();
        let idx = current
            .and_then(|focus| watchlist.iter().position(|key| *key == focus))
            .unwrap_or(0);
        let next = if forward {
            (idx + 1) % len
        } else {
            (idx + len - 1) % len
        };
        let (source, symbol) = watchlist[next].clone();
        self.terminal.set_focus(source, &symbol);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use terminal_core::{Config, SourceSpec, Symbol};

    fn synth_app() -> App {
        let mut cfg = Config::default_layout();
        cfg.sources = vec![SourceSpec::Synth { seed: 1 }];
        App::new(Terminal::new(&cfg).unwrap())
    }

    #[test]
    fn quit_action_sets_should_quit() {
        let mut app = synth_app();
        assert!(!app.should_quit);
        app.on_action(Action::Quit);
        assert!(app.should_quit);
    }

    #[test]
    fn update_captures_a_frame_after_subscribe() {
        let mut app = synth_app();
        app.terminal
            .subscribe(0, &Symbol::new("BTC", "USDT"))
            .unwrap();
        app.update();
        assert_eq!(app.frame.panels.len(), 4);
    }

    #[test]
    fn cycle_symbol_moves_focus_across_the_watchlist() {
        let mut app = synth_app();
        let btc = Symbol::new("BTC", "USDT");
        let eth = Symbol::new("ETH", "USDT");
        app.terminal.subscribe(0, &btc).unwrap();
        app.terminal.subscribe(0, &eth).unwrap();
        app.terminal.set_focus(0, &btc);
        app.on_action(Action::NextSymbol);
        assert_eq!(app.terminal.state().focus, Some((0, eth)));
        app.on_action(Action::PrevSymbol);
        assert_eq!(app.terminal.state().focus, Some((0, btc)));
    }

    #[test]
    fn cycle_symbol_on_empty_watchlist_is_a_noop() {
        let mut app = synth_app();
        app.on_action(Action::NextSymbol);
        assert!(app.terminal.state().focus.is_none());
    }
}
