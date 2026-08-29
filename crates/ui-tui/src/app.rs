//! The renderer's application loop state.
//!
//! [`App`] owns a [`Terminal`] and reduces user [`Action`]s onto it. It ticks the
//! core synchronously each frame (the pull-based sources are drained per tick);
//! the core owns the feed, so the renderer stays a thin driver.
//!
//! A small modal input layer drives the runtime module toggle: `s` opens a
//! prompt to add a source, `a` a prompt to subscribe a symbol, and `d` / `x`
//! remove the focused symbol / source. Sources can be added, removed and
//! hot-swapped while the terminal runs, and multiple sources coexist.

use std::str::FromStr;

use wickra_terminal_core::{Frame, Symbol, Terminal};

use crate::input::Action;
use crate::spec;

/// A pending text prompt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum InputKind {
    /// Add a source from a shorthand (`synth:2`, `live:binance:ETH/USDT`, …).
    AddSource,
    /// Subscribe a symbol on the focused source.
    AddSymbol,
}

/// The current interaction mode.
pub(crate) enum Mode {
    /// Keys map to actions.
    Normal,
    /// Keys edit a text buffer for the given prompt.
    Input { kind: InputKind, buffer: String },
}

/// The renderer state driven by the event loop.
pub(crate) struct App {
    /// The terminal core this renderer drives.
    pub terminal: Terminal,
    /// Set once the user asks to quit.
    pub should_quit: bool,
    /// The most recent frame of view-models.
    pub frame: Frame,
    /// The current interaction mode.
    pub mode: Mode,
    /// The last status/feedback message.
    pub status: String,
    /// Which panel of the configured layout is focused, by index.
    pub focused_panel: usize,
}

impl App {
    /// Wrap a terminal core in a fresh app.
    #[must_use]
    pub(crate) fn new(terminal: Terminal) -> Self {
        Self {
            terminal,
            should_quit: false,
            frame: Frame { panels: Vec::new() },
            mode: Mode::Normal,
            status: "s add source · a add symbol · d unsub · x remove source · ←/→ symbol · tab panel · q quit"
                .to_string(),
            focused_panel: 0,
        }
    }

    /// Pump the core and capture the next frame.
    pub(crate) fn update(&mut self) {
        self.frame = self.terminal.tick();
    }

    /// Reduce a user action onto the terminal.
    pub(crate) fn on_action(&mut self, action: Action) {
        match action {
            Action::Quit => self.should_quit = true,
            Action::NextSymbol => self.cycle_symbol(true),
            Action::PrevSymbol => self.cycle_symbol(false),
            Action::SourceMenu => self.begin_input(InputKind::AddSource),
            Action::AddSymbol => self.begin_input(InputKind::AddSymbol),
            Action::RemoveSymbol => self.remove_focused_symbol(),
            Action::RemoveSource => self.remove_focused_source(),
            Action::NextPanel => self.cycle_panel(true),
            Action::PrevPanel => self.cycle_panel(false),
            Action::None => {}
        }
    }

    /// Enter input mode with an empty buffer.
    fn begin_input(&mut self, kind: InputKind) {
        self.mode = Mode::Input {
            kind,
            buffer: String::new(),
        };
    }

    /// Append a character to the input buffer (no-op outside input mode).
    pub(crate) fn input_push(&mut self, ch: char) {
        if let Mode::Input { buffer, .. } = &mut self.mode {
            buffer.push(ch);
        }
    }

    /// Delete the last character of the input buffer.
    pub(crate) fn input_backspace(&mut self) {
        if let Mode::Input { buffer, .. } = &mut self.mode {
            buffer.pop();
        }
    }

    /// Cancel input mode.
    pub(crate) fn input_cancel(&mut self) {
        self.mode = Mode::Normal;
        self.status = "cancelled".to_string();
    }

    /// Apply the current input buffer and return to normal mode.
    pub(crate) fn input_submit(&mut self) {
        let Mode::Input { kind, buffer } = &self.mode else {
            return;
        };
        let kind = *kind;
        let buffer = buffer.clone();
        self.mode = Mode::Normal;
        match kind {
            InputKind::AddSource => self.add_source(&buffer),
            InputKind::AddSymbol => self.add_symbol(&buffer),
        }
    }

    /// Whether a text prompt is open.
    #[must_use]
    pub(crate) fn is_input(&self) -> bool {
        matches!(self.mode, Mode::Input { .. })
    }

    /// The footer line: the open prompt, or the last status message.
    #[must_use]
    pub(crate) fn footer(&self) -> String {
        match &self.mode {
            Mode::Input { kind, buffer } => {
                let label = match kind {
                    InputKind::AddSource => "add source (synth:N | live:venue:SYM | replay:JSON)",
                    InputKind::AddSymbol => "add symbol (BASE/QUOTE)",
                };
                format!("{label}: {buffer}\u{2588}")
            }
            Mode::Normal => self.status.clone(),
        }
    }

    /// Add a source from a shorthand and auto-subscribe an embedded Live symbol.
    fn add_source(&mut self, shorthand: &str) {
        let spec = match spec::parse_source(shorthand) {
            Ok(spec) => spec,
            Err(err) => {
                self.status = format!("bad source: {err}");
                return;
            }
        };
        match self.terminal.add_source(&spec) {
            Ok(id) => {
                if let wickra_terminal_core::SourceSpec::Live { symbol, .. } = &spec {
                    if let Ok(sym) = Symbol::from_str(symbol) {
                        let _ = self.terminal.subscribe(id, &sym);
                    }
                }
                self.status = format!("added source {id}: {shorthand}");
            }
            Err(err) => self.status = format!("add failed: {err}"),
        }
    }

    /// Subscribe a symbol on the focused source (or the most recently added).
    fn add_symbol(&mut self, symbol: &str) {
        let sym = match Symbol::from_str(symbol) {
            Ok(sym) => sym,
            Err(err) => {
                self.status = format!("bad symbol: {err}");
                return;
            }
        };
        let source = self.target_source();
        match self.terminal.subscribe(source, &sym) {
            Ok(()) => self.status = format!("subscribed {sym} on source {source}"),
            Err(err) => self.status = format!("subscribe failed: {err}"),
        }
    }

    /// The source to act on: the focused one, else the most recently added.
    fn target_source(&self) -> wickra_terminal_core::SourceId {
        if let Some((source, _)) = self.terminal.state().focus.as_ref() {
            return *source;
        }
        self.terminal
            .state()
            .sources
            .last()
            .map_or(0, |source| source.id())
    }

    /// Unsubscribe the focused symbol.
    fn remove_focused_symbol(&mut self) {
        if let Some((source, symbol)) = self.terminal.state().focus.clone() {
            self.terminal.unsubscribe(source, &symbol);
            self.status = format!("unsubscribed {symbol}");
        }
    }

    /// Remove the focused source and everything it owns.
    fn remove_focused_source(&mut self) {
        if let Some((source, _)) = self.terminal.state().focus.clone() {
            self.terminal.remove_source(source);
            self.status = format!("removed source {source}");
        }
    }

    /// Move focus to the next/previous panel of the layout.
    ///
    /// Panel focus is a renderer concern, not a core one: the core has no notion
    /// of a focused panel, because every renderer decides for itself what focus
    /// means to it. Here it is the highlighted border, and which panel a future
    /// panel-local key would act on.
    fn cycle_panel(&mut self, forward: bool) {
        let len = self.terminal.config().layout.panels.len();
        if len == 0 {
            return;
        }
        self.focused_panel = if forward {
            (self.focused_panel + 1) % len
        } else {
            (self.focused_panel + len - 1) % len
        };
        let kind = self.terminal.config().layout.panels[self.focused_panel].kind;
        self.status = format!("panel {kind:?}");
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
    use wickra_terminal_core::{Config, SourceSpec};

    fn synth_app() -> App {
        let mut cfg = Config::default_layout();
        cfg.sources = vec![SourceSpec::Synth { seed: 1 }];
        App::new(Terminal::new(&cfg).unwrap())
    }

    #[test]
    fn quit_action_sets_should_quit() {
        let mut app = synth_app();
        app.on_action(Action::Quit);
        assert!(app.should_quit);
    }

    #[test]
    fn source_menu_opens_and_adds_a_source() {
        let mut app = synth_app();
        app.on_action(Action::SourceMenu);
        assert!(app.is_input());
        for ch in "synth:2".chars() {
            app.input_push(ch);
        }
        app.input_submit();
        assert!(!app.is_input());
        // Two sources now: the config's source 0 and the new source 1.
        assert_eq!(app.terminal.state().sources.len(), 2);
    }

    #[test]
    fn add_symbol_subscribes_and_remove_symbol_unsubscribes() {
        let mut app = synth_app();
        app.on_action(Action::AddSymbol);
        for ch in "ETH/USDT".chars() {
            app.input_push(ch);
        }
        app.input_submit();
        assert_eq!(app.terminal.state().watchlist.len(), 1);
        app.on_action(Action::RemoveSymbol);
        assert!(app.terminal.state().watchlist.is_empty());
    }

    #[test]
    fn remove_source_drops_the_focused_source() {
        let mut app = synth_app();
        app.terminal
            .subscribe(0, &Symbol::new("BTC", "USDT"))
            .unwrap();
        assert_eq!(app.terminal.state().sources.len(), 1);
        app.on_action(Action::RemoveSource);
        assert!(app.terminal.state().sources.is_empty());
    }

    #[test]
    fn input_backspace_and_cancel() {
        let mut app = synth_app();
        app.on_action(Action::SourceMenu);
        app.input_push('a');
        app.input_push('b');
        app.input_backspace();
        assert_eq!(
            app.footer(),
            "add source (synth:N | live:venue:SYM | replay:JSON): a\u{2588}"
        );
        app.input_cancel();
        assert!(!app.is_input());
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
    fn tab_cycles_panel_focus_and_wraps() {
        let mut app = synth_app();
        let panels = app.terminal.config().layout.panels.len();
        assert_eq!(panels, 5, "the default layout should have five panels");
        assert_eq!(app.focused_panel, 0);

        for expected in 1..panels {
            app.on_action(Action::NextPanel);
            assert_eq!(app.focused_panel, expected);
        }
        // One more wraps back to the first rather than running off the end.
        app.on_action(Action::NextPanel);
        assert_eq!(app.focused_panel, 0);
    }

    #[test]
    fn backtab_cycles_the_other_way_and_wraps() {
        let mut app = synth_app();
        let panels = app.terminal.config().layout.panels.len();
        app.on_action(Action::PrevPanel);
        assert_eq!(
            app.focused_panel,
            panels - 1,
            "backtab from the first wraps"
        );
        app.on_action(Action::PrevPanel);
        assert_eq!(app.focused_panel, panels - 2);
    }

    #[test]
    fn cycling_panels_names_the_one_now_focused() {
        // The status line is the only place a user reads which panel they are on
        // besides the border, so it has to move with the focus.
        let mut app = synth_app();
        app.on_action(Action::NextPanel);
        let kind = app.terminal.config().layout.panels[app.focused_panel].kind;
        assert!(
            app.status.contains(&format!("{kind:?}")),
            "status {:?} does not name {kind:?}",
            app.status
        );
    }

    #[test]
    fn cycling_panels_is_a_no_op_with_an_empty_layout() {
        let mut config = Config::default_layout();
        config.layout.panels.clear();
        config.sources = vec![SourceSpec::Synth { seed: 1 }];
        let mut app = App::new(Terminal::new(&config).unwrap());
        app.on_action(Action::NextPanel);
        app.on_action(Action::PrevPanel);
        assert_eq!(app.focused_panel, 0, "no panels means nothing to focus");
    }
}
