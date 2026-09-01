//! The renderer's application loop state.
//!
//! [`App`] owns a [`Terminal`] and reduces user [`Action`]s onto it. It ticks the
//! core synchronously each frame (the pull-based sources are drained per tick);
//! the core owns the feed, so the renderer stays a thin driver.
//!
//! A small modal input layer drives everything the core's command surface
//! offers: `s` adds a source, `a` subscribes a symbol, `d` / `x` remove the
//! focused symbol / source, `i` / `k` add and remove an indicator, `t` changes
//! the timeframe, `l` searches the registry catalogue, and `,` / `.` scrub a
//! recording.
//!
//! That list used to stop at the sources. Five of the core's commands --
//! `AddIndicator`, `RemoveIndicator`, `SetTimeframe`, `ListIndicators` and
//! `Seek` -- were
//! reachable from no renderer at all, which meant the registry could only be
//! configured from a file and the time-machine had no key anywhere. The
//! keymap is data, so both renderers read the same names from the config.

use std::str::FromStr;

use wickra_terminal_core::registry::KINDS;
use wickra_terminal_core::{Frame, Symbol, Terminal, Timeframe};

/// How many catalogue names the status line shows before it just reports the
/// count. A terminal row holds about a dozen of them.
const CATALOGUE_SHOWN: usize = 12;

/// How many steps one traversal of a recording takes, so a keypress moves a
/// twentieth of the feed however long it is.
const SEEK_STEPS: usize = 20;

use crate::input::Action;
use crate::spec;

/// How many rows a panel view carries, for clamping a scroll offset.
///
/// The panels with no row list -- the chart, which is a plot, and the profile,
/// whose rows are bins inside each distribution -- report one, so scrolling them
/// is a no-op rather than an offset that silently blanks them.
fn panel_rows(view: &wickra_terminal_core::PanelView) -> usize {
    use wickra_terminal_core::PanelView as V;
    match view {
        V::Book(book) => book.bids.len().max(book.asks.len()),
        V::Tape(tape) => tape.prints.len(),
        V::Watchlist(list) => list.rows.len(),
        V::Footprint(fp) => fp.levels.len(),
        V::Bars(bars) => bars.streams.iter().map(|s| s.bars.len()).max().unwrap_or(0),
        V::Chart(_) | V::Profile(_) => 1,
    }
}

/// A pending text prompt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum InputKind {
    /// Add a source from a shorthand (`synth:2`, `live:binance:ETH/USDT`, …).
    AddSource,
    /// Subscribe a symbol on the focused source.
    AddSymbol,
    /// Add an indicator from a shorthand (`Sma 20`, `Beta 20 vs ETH/USDT`).
    AddIndicator,
    /// Remove the indicator with this label (`Sma(20)`).
    RemoveIndicator,
    /// Change the bar size (`1m`, `4h`).
    SetTimeframe,
    /// Filter the registry catalogue.
    ListIndicators,
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
    /// How far each panel is scrolled, by layout index.
    ///
    /// Renderer state, not core state, and it has to be: the core sends what a
    /// panel carries and every front-end decides for itself how much of that it
    /// can show. A browser scrolls a div; a terminal has to be told.
    pub scroll: Vec<usize>,
}

impl App {
    /// Wrap a terminal core in a fresh app.
    #[must_use]
    pub(crate) fn new(terminal: Terminal) -> Self {
        let panels = terminal.config().layout.panels.len();
        Self {
            terminal,
            should_quit: false,
            frame: Frame { panels: Vec::new() },
            mode: Mode::Normal,
            status: "s source · a symbol · d unsub · x drop source · i/k indicator · t timeframe · l catalogue · ,/. seek · ↑/↓ scroll · ←/→ symbol · tab panel · q quit"
                .to_string(),
            focused_panel: 0,
            scroll: vec![0; panels],
        }
    }

    /// Pump the core and capture the next frame.
    pub(crate) fn update(&mut self) {
        self.frame = self.terminal.tick();
        // The layout can grow a panel at run time, and a frame says how many
        // rows each panel now has -- both of which can strand an offset.
        self.scroll
            .resize(self.terminal.config().layout.panels.len(), 0);
        self.clamp_scroll();
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
            Action::AddIndicator => self.begin_input(InputKind::AddIndicator),
            Action::RemoveIndicator => self.begin_input(InputKind::RemoveIndicator),
            Action::SetTimeframe => self.begin_input(InputKind::SetTimeframe),
            Action::ListIndicators => self.begin_input(InputKind::ListIndicators),
            Action::SeekBack => self.seek_by(-1),
            Action::SeekForward => self.seek_by(1),
            Action::ScrollUp => self.scroll_by(-1),
            Action::ScrollDown => self.scroll_by(1),
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
            InputKind::AddIndicator => self.add_indicator(&buffer),
            InputKind::RemoveIndicator => self.remove_indicator(&buffer),
            InputKind::SetTimeframe => self.set_timeframe(&buffer),
            InputKind::ListIndicators => self.list_indicators(&buffer),
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
                    InputKind::AddIndicator => "add indicator (Sma 20 | Beta 20 vs ETH/USDT)",
                    InputKind::RemoveIndicator => "remove indicator (the label, e.g. Sma(20))",
                    InputKind::SetTimeframe => "timeframe (1m | 5m | 1h | 4h)",
                    InputKind::ListIndicators => {
                        "search the catalogue (a substring, blank for all)"
                    }
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

    /// Add an indicator to every market from a shorthand.
    fn add_indicator(&mut self, shorthand: &str) {
        let spec = match spec::parse_indicator(shorthand) {
            Ok(spec) => spec,
            Err(err) => {
                self.status = format!("bad indicator: {err}");
                return;
            }
        };
        let label = spec.label();
        match self.terminal.add_indicator(&spec) {
            Ok(()) => self.status = format!("tracking {label}"),
            Err(err) => self.status = format!("add failed: {err}"),
        }
    }

    /// Stop tracking the indicator with this label.
    fn remove_indicator(&mut self, label: &str) {
        let label = label.trim();
        match self.terminal.remove_indicator(label) {
            Ok(()) => self.status = format!("removed {label}"),
            Err(err) => self.status = format!("remove failed: {err}"),
        }
    }

    /// Change the bar size the candle indicators are fed at.
    fn set_timeframe(&mut self, text: &str) {
        let timeframe = match Timeframe::parse(text.trim()) {
            Ok(timeframe) => timeframe,
            Err(err) => {
                self.status = format!("bad timeframe: {err}");
                return;
            }
        };
        match self.terminal.set_timeframe(timeframe) {
            Ok(()) => self.status = format!("timeframe {}", timeframe.label()),
            Err(err) => self.status = format!("timeframe failed: {err}"),
        }
    }

    /// Search the registry catalogue and report the matches in the status line.
    ///
    /// A status line rather than a scrollable overlay, and the reason is the
    /// number: the catalogue is five hundred entries, so a list is only useful
    /// once it is filtered, and once it is filtered it fits on a line. What a
    /// user needs from it is the exact spelling of a name they half remember,
    /// which is what this answers.
    fn list_indicators(&mut self, filter: &str) {
        let filter = filter.trim().to_ascii_lowercase();
        let matches: Vec<&str> = KINDS
            .iter()
            .copied()
            .filter(|kind| filter.is_empty() || kind.to_ascii_lowercase().contains(&filter))
            .collect();
        self.status = match matches.len() {
            0 => format!("no indicator matches {filter:?}"),
            n if n <= CATALOGUE_SHOWN => format!("{n}: {}", matches.join(" ")),
            n => format!(
                "{n} match, first {CATALOGUE_SHOWN}: {}",
                matches[..CATALOGUE_SHOWN].join(" ")
            ),
        };
    }

    /// Step the focused source's replay cursor -- the time-machine.
    ///
    /// A proportional step rather than a fixed number of events: a recording is
    /// whatever length it is, and stepping one event at a time through a feed of
    /// fifty thousand is not scrubbing. Sources that cannot be replayed report
    /// so rather than doing nothing silently.
    fn seek_by(&mut self, direction: i64) {
        let Some((source, _)) = self.terminal.state().focus.clone() else {
            self.status = "nothing focused to seek".to_string();
            return;
        };
        let Some((cursor, length)) = self.terminal.replay_position(source) else {
            self.status = format!("source {source} is not replayable");
            return;
        };
        let step = (length / SEEK_STEPS).max(1);
        let offset = direction * i64::try_from(step).unwrap_or(i64::MAX);
        let target = i64::try_from(cursor)
            .unwrap_or(i64::MAX)
            .saturating_add(offset);
        let target = usize::try_from(target.max(0)).unwrap_or(0).min(length);
        match self.terminal.seek(source, target) {
            Ok(()) => self.status = format!("replay {target}/{length}"),
            Err(err) => self.status = format!("seek failed: {err}"),
        }
    }

    /// Scroll the focused panel, clamped to what that panel carries.
    ///
    /// Panel focus was drawn and acted on nothing: tab moved a border and no key
    /// did anything with it. This is the first thing it means. The bound is the
    /// panel's own row count, so a book carrying twelve levels does not scroll
    /// at all and one configured to carry fifty scrolls through thirty-eight --
    /// which is why the depth is configurable in the first place.
    fn scroll_by(&mut self, direction: i64) {
        let Some(offset) = self.scroll.get_mut(self.focused_panel) else {
            return;
        };
        let moved = i64::try_from(*offset)
            .unwrap_or(i64::MAX)
            .saturating_add(direction);
        *offset = usize::try_from(moved.max(0)).unwrap_or(0);
        let kind = self
            .terminal
            .config()
            .layout
            .panels
            .get(self.focused_panel)
            .map(|spec| spec.kind);
        let clamped = self.clamp_scroll();
        self.status = match kind {
            Some(kind) => format!("{kind:?} row {clamped}"),
            None => String::new(),
        };
    }

    /// Hold every offset inside the rows its panel actually carries.
    ///
    /// Called after a scroll and after each frame, because the frame is what
    /// says how many rows there are -- a tape that has only printed twice must
    /// not be scrollable to row forty just because the last one was.
    fn clamp_scroll(&mut self) -> usize {
        for (index, offset) in self.scroll.iter_mut().enumerate() {
            let rows = self.frame.panels.get(index).map_or(0, panel_rows);
            *offset = (*offset).min(rows.saturating_sub(1));
        }
        self.scroll.get(self.focused_panel).copied().unwrap_or(0)
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

    /// Type into an open prompt and submit it.
    fn type_and_submit(app: &mut App, text: &str) {
        for ch in text.chars() {
            app.input_push(ch);
        }
        app.input_submit();
    }

    #[test]
    fn an_indicator_can_be_added_and_removed_from_the_keyboard() {
        // The registry is the headline feature and was reachable only from a
        // config file: neither renderer bound AddIndicator to anything.
        let mut app = synth_app();
        app.terminal
            .subscribe(0, &Symbol::new("BTC", "USDT"))
            .unwrap();
        let before = app.terminal.config().indicators.len();

        app.on_action(Action::AddIndicator);
        assert!(app.is_input());
        type_and_submit(&mut app, "Rsi 14");
        assert_eq!(app.terminal.config().indicators.len(), before + 1);
        assert!(app.status.contains("Rsi(14)"), "status: {}", app.status);

        app.on_action(Action::RemoveIndicator);
        type_and_submit(&mut app, "Rsi(14)");
        assert_eq!(app.terminal.config().indicators.len(), before);
    }

    #[test]
    fn a_pairwise_indicator_takes_its_reference_from_the_prompt() {
        // `vs` is split off before the parameters, so a market with digits in it
        // is never read as one.
        let mut app = synth_app();
        app.terminal
            .subscribe(0, &Symbol::new("BTC", "USDT"))
            .unwrap();
        app.on_action(Action::AddIndicator);
        type_and_submit(&mut app, "Beta 20 vs ETH/USDT");
        let spec = app.terminal.config().indicators.last().unwrap();
        assert_eq!(spec.kind, "Beta");
        assert_eq!(spec.params, vec![20.0]);
        assert_eq!(spec.reference.as_deref(), Some("ETH/USDT"));
    }

    #[test]
    fn a_bad_indicator_reports_rather_than_being_dropped() {
        let mut app = synth_app();
        app.on_action(Action::AddIndicator);
        type_and_submit(&mut app, "NoSuchIndicator 3");
        assert!(app.status.contains("failed"), "status: {}", app.status);
    }

    #[test]
    fn the_timeframe_can_be_changed_from_the_keyboard() {
        let mut app = synth_app();
        app.on_action(Action::SetTimeframe);
        type_and_submit(&mut app, "5m");
        assert_eq!(app.terminal.config().timeframe.label(), "5m");
        assert!(app.status.contains("5m"), "status: {}", app.status);

        app.on_action(Action::SetTimeframe);
        type_and_submit(&mut app, "not a timeframe");
        assert!(
            app.status.contains("bad timeframe"),
            "status: {}",
            app.status
        );
        assert_eq!(
            app.terminal.config().timeframe.label(),
            "5m",
            "kept the old"
        );
    }

    #[test]
    fn the_catalogue_can_be_searched() {
        // What a user needs from five hundred names is the exact spelling of one
        // they half remember, which is a filter rather than a list.
        let mut app = synth_app();
        app.on_action(Action::ListIndicators);
        type_and_submit(&mut app, "bollinger");
        assert!(
            app.status.contains("BollingerBands"),
            "status: {}",
            app.status
        );

        app.on_action(Action::ListIndicators);
        type_and_submit(&mut app, "zzzz");
        assert!(
            app.status.contains("no indicator"),
            "status: {}",
            app.status
        );
    }

    /// A recorded feed of `n` trades, as the JSON a `Replay` source takes.
    ///
    /// Written out rather than serialised: the shape is the one the golden
    /// corpus already pins, and building it here would mean three dev
    /// dependencies (`serde_json`, `rust_decimal`, the exchange types) for one
    /// test.
    fn replay_feed(n: u32) -> String {
        let events: Vec<String> = (0..n)
            .map(|i| {
                format!(
                    r#"{{"type":"trade","symbol":{{"base":"BTC","quote":"USDT"}},"price":"{}","quantity":"1","aggressor":"Buy","timestamp":{}}}"#,
                    20_000 + i,
                    i64::from(i) * 1000
                )
            })
            .collect();
        format!("[{}]", events.join(","))
    }

    #[test]
    fn seeking_moves_a_replay_and_says_where_it_is() {
        // The time-machine had no key in either renderer, so the one thing that
        // makes a recording more than a slow synthetic feed was unreachable.
        let mut config = Config::default_layout();
        config.sources = vec![SourceSpec::Replay {
            dataset: replay_feed(40),
        }];
        let mut app = App::new(Terminal::new(&config).unwrap());
        app.terminal
            .subscribe(0, &Symbol::new("BTC", "USDT"))
            .unwrap();
        app.update();
        app.update();

        let (_, length) = app.terminal.replay_position(0).unwrap();
        assert_eq!(length, 40);

        app.on_action(Action::SeekBack);
        assert!(app.status.starts_with("replay "), "status: {}", app.status);
        let (rewound, _) = app.terminal.replay_position(0).unwrap();

        app.on_action(Action::SeekForward);
        let (moved, _) = app.terminal.replay_position(0).unwrap();
        assert!(
            moved >= rewound,
            "forward went backwards: {rewound} -> {moved}"
        );
    }

    #[test]
    fn scrolling_moves_the_focused_panel_and_nothing_else() {
        // Panel focus was drawn and acted on nothing: tab moved a border and no
        // key did anything with it. This is the first thing it means.
        let mut config = Config::default_layout();
        config.sources = vec![SourceSpec::Synth { seed: 1 }];
        // A book deep enough to scroll: with the default twelve levels there is
        // nothing underneath them to scroll to, which is why the depth is
        // configurable at all.
        for panel in &mut config.layout.panels {
            panel.depth = Some(40);
        }
        let mut app = App::new(Terminal::new(&config).unwrap());
        app.terminal
            .subscribe(0, &Symbol::new("BTC", "USDT"))
            .unwrap();
        for _ in 0..60 {
            app.update();
        }

        // Focus the tape, which by then carries more rows than one screen.
        let tape = config
            .layout
            .panels
            .iter()
            .position(|p| p.kind == wickra_terminal_core::PanelKind::Tape)
            .expect("the default layout has a tape");
        app.focused_panel = tape;

        app.on_action(Action::ScrollDown);
        app.on_action(Action::ScrollDown);
        assert_eq!(app.scroll[tape], 2, "the focused panel did not scroll");
        assert!(
            app.scroll
                .iter()
                .enumerate()
                .all(|(i, o)| i == tape || *o == 0),
            "scrolling moved a panel that was not focused: {:?}",
            app.scroll
        );

        app.on_action(Action::ScrollUp);
        assert_eq!(app.scroll[tape], 1);
    }

    #[test]
    fn scrolling_stops_at_the_rows_a_panel_actually_carries() {
        // A tape that has printed twice must not be scrollable to row forty just
        // because the last one was -- the frame says how many rows there are.
        let mut config = Config::default_layout();
        config.sources = vec![SourceSpec::Synth { seed: 1 }];
        let mut app = App::new(Terminal::new(&config).unwrap());
        app.terminal
            .subscribe(0, &Symbol::new("BTC", "USDT"))
            .unwrap();
        app.update();

        let tape = config
            .layout
            .panels
            .iter()
            .position(|p| p.kind == wickra_terminal_core::PanelKind::Tape)
            .expect("the default layout has a tape");
        app.focused_panel = tape;
        for _ in 0..50 {
            app.on_action(Action::ScrollDown);
        }
        let rows = match &app.frame.panels[tape] {
            wickra_terminal_core::PanelView::Tape(view) => view.prints.len(),
            other => panic!("expected a tape, got {other:?}"),
        };
        assert!(
            app.scroll[tape] < rows.max(1),
            "scrolled to {} of {rows} rows",
            app.scroll[tape]
        );
    }

    #[test]
    fn scrolling_up_never_goes_below_the_first_row() {
        let mut app = synth_app();
        app.terminal
            .subscribe(0, &Symbol::new("BTC", "USDT"))
            .unwrap();
        app.update();
        for _ in 0..5 {
            app.on_action(Action::ScrollUp);
        }
        assert_eq!(app.scroll[app.focused_panel], 0);
    }

    #[test]
    fn seeking_a_source_that_cannot_replay_says_so() {
        // Silently doing nothing is the failure this reports: a synth feed looks
        // identical to a recording until you try to scrub it.
        let mut app = synth_app();
        app.terminal
            .subscribe(0, &Symbol::new("BTC", "USDT"))
            .unwrap();
        app.on_action(Action::SeekBack);
        assert!(
            app.status.contains("not replayable"),
            "status: {}",
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
