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

/// Write a recording into `dir`, returning the status line to show.
///
/// Takes the directory rather than reading the current one, so a test can point
/// it at a temporary path instead of changing the process's working directory --
/// which is shared by every test thread and would make this one race the rest.
///
/// Named by the wall clock rather than overwriting one path: what a person saves
/// is a moment they want to keep, and the next keypress must not take it away.
fn write_recording(dir: &std::path::Path, json: &str, count: usize) -> String {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |since| since.as_secs());
    let path = dir.join(format!("wickra-recording-{stamp}.json"));
    match std::fs::write(&path, json) {
        Ok(()) => format!("wrote {count} events to {}", path.display()),
        Err(err) => format!("could not write {}: {err}", path.display()),
    }
}

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
            status: "s source · a symbol · d unsub · x drop source · i/k indicator · t timeframe · l catalogue · ,/. seek · w save · ↑/↓ scroll · ←/→ symbol · tab panel · q quit"
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
            Action::SaveRecording => self.save_recording(),
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
        // The core rebuilds the indicator set from the specs it is already
        // holding, and those were validated when they were added -- the same
        // reasoning `Terminal::seek` uses when it rebuilds a market. An arm for
        // a failure nothing can produce is a branch no test can reach.
        self.terminal
            .set_timeframe(timeframe)
            .expect("the indicator specs were validated when they were added");
        self.status = format!("timeframe {}", timeframe.label());
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
        // `replay_position` answered above, so the source is open and has a
        // recording to seek; both of `seek`'s errors are already excluded.
        self.terminal
            .seek(source, target)
            .expect("replay_position answered, so this source is open and replayable");
        self.status = format!("replay {target}/{length}");
    }

    /// Write the recorded events beside the terminal, ready to be replayed.
    ///
    /// The core records into a bounded ring and is deliberately
    /// filesystem-free -- it has to be, to run in a browser -- so writing is
    /// this renderer's job. The file is exactly what `Replay { dataset }` takes,
    /// which is the whole point: the terminal could rewind a recording and had
    /// no way to make one.
    ///
    /// Named by the wall clock rather than overwriting one path, because the
    /// thing a person saves is a moment they want to keep and the next keypress
    /// must not take it away.
    fn save_recording(&mut self) {
        if self.terminal.config().record.is_none() {
            self.status = "recording is off; set `record` in the config to keep events".to_string();
            return;
        }
        let count = self.terminal.recording_len();
        if count == 0 {
            self.status = "nothing recorded yet".to_string();
            return;
        }
        // Through the command boundary rather than serialising here: the format
        // is the core's to decide, and this way the file is byte-for-byte what
        // `ExportRecording` hands any other binding.
        let json = self
            .terminal
            .command_json(r#"{"type":"ExportRecording"}"#)
            .expect("ExportRecording is a constant command with no failing path");
        self.status = write_recording(std::path::Path::new("."), &json, count);
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
        // `scroll` is built with one entry per panel and neither can grow after
        // that, so the lookup above having succeeded is proof this one does too.
        let kind = self.terminal.config().layout.panels[self.focused_panel].kind;
        let clamped = self.clamp_scroll();
        self.status = format!("{kind:?} row {clamped}");
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
    use wickra_terminal_core::{Config, IndicatorSpec, PanelSpec, SourceSpec};

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
        // `find_map` over the frame rather than a match with a catch-all: the
        // arm for "not a tape" is taken by every other panel in the layout, so
        // it is live, where a `panic!` catch-all on the indexed panel never is.
        let rows = app
            .frame
            .panels
            .iter()
            .find_map(|panel| match panel {
                wickra_terminal_core::PanelView::Tape(view) => Some(view.prints.len()),
                _ => None,
            })
            .expect("the default layout has a tape");
        let reached = app.scroll[tape];
        assert!(
            reached < rows.max(1),
            "scrolled to {reached} of {rows} rows"
        );
    }

    #[test]
    fn every_prompt_has_a_label() {
        // The footer is the only place a user reads what a prompt is asking for.
        // A prompt with no label of its own is a blinking cursor over nothing.
        let mut app = synth_app();
        for (action, expected) in [
            (Action::SourceMenu, "add source"),
            (Action::AddSymbol, "add symbol"),
            (Action::AddIndicator, "add indicator"),
            (Action::RemoveIndicator, "remove indicator"),
            (Action::SetTimeframe, "timeframe"),
            (Action::ListIndicators, "catalogue"),
        ] {
            app.on_action(action);
            let footer = app.footer();
            assert!(
                footer.contains(expected),
                "{action:?} prompts with {footer:?}"
            );
            app.input_cancel();
        }
    }

    #[test]
    fn a_catalogue_search_that_matches_many_reports_the_count() {
        // Five hundred names do not fit a status line, so a broad filter has to
        // say how many it found rather than printing what fits and stopping.
        let mut app = synth_app();
        app.on_action(Action::ListIndicators);
        type_and_submit(&mut app, "");
        assert!(
            app.status.contains("match, first"),
            "status: {}",
            app.status
        );
    }

    #[test]
    fn removing_an_indicator_that_is_not_tracked_says_so() {
        let mut app = synth_app();
        app.on_action(Action::RemoveIndicator);
        type_and_submit(&mut app, "NotTracked(3)");
        assert!(
            app.status.contains("remove failed"),
            "status: {}",
            app.status
        );
    }

    #[test]
    fn seeking_with_nothing_focused_says_so() {
        // A terminal with a source and no subscription: the keypress has to
        // report that rather than looking like a seek that did nothing.
        let mut app = synth_app();
        app.on_action(Action::SeekBack);
        assert!(
            app.status.contains("nothing focused"),
            "status: {}",
            app.status
        );
    }

    #[test]
    fn saving_without_recording_says_how_to_turn_it_on() {
        // Recording is off by default, so this is the message a user meets
        // first -- and "nothing happened" would send them looking for a bug.
        let mut app = synth_app();
        app.on_action(Action::SaveRecording);
        assert!(
            app.status.contains("recording is off"),
            "status: {}",
            app.status
        );
        assert!(
            app.status.contains("record"),
            "it does not say what to set: {}",
            app.status
        );
    }

    #[test]
    fn saving_with_recording_on_but_nothing_yet_says_so() {
        let mut config = Config::default_layout();
        config.sources = vec![SourceSpec::Synth { seed: 1 }];
        config.record = Some(64);
        let mut app = App::new(Terminal::new(&config).unwrap());
        app.on_action(Action::SaveRecording);
        assert!(
            app.status.contains("nothing recorded"),
            "status: {}",
            app.status
        );
    }

    #[test]
    fn a_recording_is_written_where_it_can_be_replayed() {
        // The file has to be exactly what Replay { dataset } takes, because that
        // is the whole point: the terminal could rewind a recording and had no
        // way to make one.
        let mut config = Config::default_layout();
        config.sources = vec![SourceSpec::Synth { seed: 1 }];
        config.record = Some(256);
        let mut terminal = Terminal::new(&config).unwrap();
        terminal.subscribe(0, &Symbol::new("BTC", "USDT")).unwrap();
        for _ in 0..5 {
            terminal.tick();
        }
        let json = terminal
            .command_json(r#"{"type":"ExportRecording"}"#)
            .expect("export");
        assert!(terminal.recording_len() > 0, "nothing was recorded");

        let dir = std::env::temp_dir().join("wickra-terminal-recording-test");
        std::fs::create_dir_all(&dir).expect("a writable temp directory");
        let status = write_recording(&dir, &json, terminal.recording_len());
        assert!(status.starts_with("wrote "), "status: {status}");

        let path = status
            .split_once(" to ")
            .map(|(_, path)| std::path::PathBuf::from(path))
            .expect("the status names the file");
        let body = std::fs::read_to_string(&path).expect("read the recording back");
        let _ = std::fs::remove_file(&path);
        let head = &body[..body.len().min(60)];
        assert!(body.starts_with('['), "not a JSON array: {head}");

        // And it replays: the file goes straight back in as a dataset.
        let mut replay = Config::default_layout();
        replay.sources = vec![SourceSpec::Replay { dataset: body }];
        let mut second = Terminal::new(&replay).expect("the recording is a valid feed");
        second.subscribe(0, &Symbol::new("BTC", "USDT")).unwrap();
        second.tick();
        assert!(
            second
                .state()
                .get(&(0, Symbol::new("BTC", "USDT")))
                .is_some(),
            "the written recording did not replay"
        );
    }

    #[test]
    fn a_recording_that_cannot_be_written_reports_the_reason() {
        // A directory that is not one: the status has to name the path and the
        // error rather than claiming a write that did not happen.
        let missing = std::env::temp_dir().join("wickra-no-such-directory-here");
        let _ = std::fs::remove_dir_all(&missing);
        let status = write_recording(&missing, "[]", 0);
        assert!(status.starts_with("could not write"), "status: {status}");
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

    /// Scrolling with no panels at all does nothing rather than panicking.
    ///
    /// An empty layout is a config a user can write, and `scroll` is then empty
    /// while `focused_panel` is still 0 -- so the lookup misses and the only
    /// correct answer is to leave the status alone.
    #[test]
    fn scrolling_an_empty_layout_is_a_no_op() {
        let mut config = Config::default_layout();
        config.layout.panels.clear();
        config.sources = vec![SourceSpec::Synth { seed: 1 }];
        let mut app = App::new(Terminal::new(&config).unwrap());
        let before = app.status.clone();
        app.on_action(Action::ScrollDown);
        app.on_action(Action::ScrollUp);
        assert_eq!(app.status, before, "an empty layout reported a scroll");
        assert!(app.scroll.is_empty());
    }

    /// A bars panel reports the longest of its streams, so it scrolls.
    ///
    /// Every other panel kind is one list; this one is several at once, and a
    /// row count taken from the first stream would stop the scroll short of the
    /// longest. The chart and the profile deliberately report one row -- they
    /// are not lists -- which is why this is the arm worth pinning.
    #[test]
    fn a_bars_panel_scrolls_by_its_longest_stream() {
        let mut config = Config::default_layout();
        config.sources = vec![SourceSpec::Synth { seed: 1 }];
        config.bars = vec![
            IndicatorSpec::new("RenkoBars", vec![1.0]),
            IndicatorSpec::new("TickBars", vec![4.0]),
        ];
        config.layout.panels = vec![PanelSpec {
            kind: wickra_terminal_core::PanelKind::Bars,
            rect: config.layout.panels[0].rect,
            depth: None,
        }];
        let mut app = App::new(Terminal::new(&config).unwrap());
        app.terminal
            .subscribe(0, &Symbol::new("BTC", "USDT"))
            .unwrap();
        for _ in 0..400 {
            app.update();
        }
        let rows = app
            .frame
            .panels
            .iter()
            .find_map(|panel| match panel {
                wickra_terminal_core::PanelView::Bars(view) => {
                    Some(view.streams.iter().map(|s| s.bars.len()).max().unwrap_or(0))
                }
                _ => None,
            })
            .expect("the layout is one bars panel");

        for _ in 0..50 {
            app.on_action(Action::ScrollDown);
        }
        assert!(app.status.starts_with("Bars row"), "status: {}", app.status);
        assert!(
            app.scroll[0] < rows.max(1),
            "scrolled to {} of {rows} rows",
            app.scroll[0]
        );
    }

    /// An indicator prompt that cannot be parsed says so and changes nothing.
    #[test]
    fn an_unparseable_indicator_is_reported_and_adds_nothing() {
        let mut app = synth_app();
        let before = app.terminal.config().indicators.len();
        app.on_action(Action::AddIndicator);
        type_and_submit(&mut app, "Sma not-a-number");
        assert!(
            app.status.starts_with("bad indicator:"),
            "status: {}",
            app.status
        );
        assert_eq!(app.terminal.config().indicators.len(), before);
    }

    /// The save key writes a file, and says so, without a config change first.
    ///
    /// Two answers before that one, both of which used to be silence: a
    /// terminal that is not recording, and one that is recording but has seen
    /// nothing yet.
    #[test]
    fn the_save_key_reports_the_recorder_state_and_then_writes() {
        let mut app = synth_app();
        app.on_action(Action::SaveRecording);
        assert!(
            app.status.contains("recording is off"),
            "status: {}",
            app.status
        );

        let mut config = Config::default_layout();
        config.sources = vec![SourceSpec::Synth { seed: 1 }];
        config.record = Some(256);
        let mut app = App::new(Terminal::new(&config).unwrap());
        app.on_action(Action::SaveRecording);
        assert!(
            app.status.contains("nothing recorded"),
            "status: {}",
            app.status
        );

        app.terminal
            .subscribe(0, &Symbol::new("BTC", "USDT"))
            .unwrap();
        for _ in 0..5 {
            app.update();
        }
        app.on_action(Action::SaveRecording);
        assert!(app.status.starts_with("wrote "), "status: {}", app.status);
        let path = app
            .status
            .split_once(" to ")
            .map(|(_, path)| std::path::PathBuf::from(path))
            .expect("the status names the file");
        let body = std::fs::read_to_string(&path).expect("read the recording back");
        let _ = std::fs::remove_file(&path);
        assert!(body.starts_with('['), "not a JSON array");
    }
}
