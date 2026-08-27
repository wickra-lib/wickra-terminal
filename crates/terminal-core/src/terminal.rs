//! The [`Terminal`] handle — the single entry point every renderer and every
//! language binding drives.
//!
//! A `Terminal` owns the [`AppState`], the built [`Panel`]s and the source-id
//! counter. Renderers call [`Terminal::tick`] to pump sources and get the next
//! [`Frame`]; bindings cross the C ABI through [`Terminal::command_json`], the
//! data-driven boundary that takes a command as JSON and returns the resulting
//! frame as JSON. There are no callbacks and no renderer-specific methods.

use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::candle::Timeframe;
use crate::config::{Config, IndicatorSpec, SourceSpec};
use crate::error::{Error, Result};
use crate::panels::{build_panel, Panel};
use crate::registry;
use crate::source::{build_source, event_symbol, Event, SourceId, Symbol};
use crate::state::{AppState, IndicatorSet, SymbolState};
use crate::view::Frame;

/// A command applied through the data-driven boundary.
#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
enum Command {
    /// Pump every source and rebuild the frame.
    Tick,
    /// Subscribe a market on a source.
    Subscribe {
        /// The source id.
        source: SourceId,
        /// The market in `BASE/QUOTE` form.
        symbol: String,
    },
    /// Unsubscribe a market from a source.
    Unsubscribe {
        /// The source id.
        source: SourceId,
        /// The market in `BASE/QUOTE` form.
        symbol: String,
    },
    /// Focus a market.
    SetFocus {
        /// The source id.
        source: SourceId,
        /// The market in `BASE/QUOTE` form.
        symbol: String,
    },
    /// Add a source at runtime.
    AddSource {
        /// The source to open.
        spec: SourceSpec,
    },
    /// Remove a source at runtime.
    RemoveSource {
        /// The source id.
        id: SourceId,
    },
    /// Rewind a replayable source to a recorded position and re-fold state — the
    /// time-machine.
    Seek {
        /// The source id.
        source: SourceId,
        /// The recorded position to rewind to (clamped to the feed length).
        index: usize,
    },
    /// Track one more indicator on every market. It starts cold and warms up
    /// from the next tick.
    AddIndicator {
        /// The indicator to add.
        spec: IndicatorSpec,
    },
    /// Stop tracking the indicator with this label (`Sma(20)`, `Rsi(14)`).
    RemoveIndicator {
        /// The label as the chart panel shows it.
        label: String,
    },
    /// Change the bar size the candle-input indicators are fed at. Restarts the
    /// bar-derived state; the price history, tape, book and footprint are kept.
    SetTimeframe {
        /// The new bar size, in the compact venue notation (`1m`, `4h`).
        timeframe: Timeframe,
    },
    /// Answer with the registry catalogue instead of a frame: every indicator
    /// name this build accepts, with the parameters wickra itself uses.
    ListIndicators,
    /// Push an externally sourced market event into a host-fed (`Manual`) source,
    /// to be folded on the next tick. The event carries its own market.
    Feed {
        /// The source id.
        source: SourceId,
        /// The market event to fold (a trade, ticker, book snapshot or diff).
        event: Event,
    },
}

/// The registry catalogue: what `ListIndicators` answers with.
///
/// Carrying the parameters alongside each name means a caller can construct any
/// entry without a second lookup — the values are the ones wickra pins its own
/// reference outputs with.
#[derive(Debug, Serialize)]
pub struct Catalogue {
    /// Every indicator this build accepts.
    pub indicators: Vec<CatalogueEntry>,
}

/// One catalogue row.
#[derive(Debug, Serialize)]
pub struct CatalogueEntry {
    /// The registry name, as `IndicatorSpec::kind`.
    pub kind: String,
    /// The parameters wickra uses for it.
    pub params: Vec<f64>,
    /// Whether this indicator compares two markets, so a spec must name a
    /// `reference` symbol for it. False for all but the pairwise family.
    ///
    /// Reported rather than left for a caller to discover by being refused: the
    /// catalogue is how a binding tells a user what it can build, and "this one
    /// needs a second market" is part of that.
    pub needs_reference: bool,
}

impl Catalogue {
    /// The catalogue of this build.
    #[must_use]
    pub fn current() -> Self {
        Self {
            indicators: registry::DEFAULTS
                .iter()
                .map(|(kind, params)| CatalogueEntry {
                    kind: (*kind).to_string(),
                    params: params.to_vec(),
                    needs_reference: registry::PAIRWISE.contains(kind),
                })
                .collect(),
        }
    }
}

/// The trading terminal: state, panels and the data-driven command boundary.
pub struct Terminal {
    state: AppState,
    config: Config,
    panels: Vec<Box<dyn Panel>>,
    next_source_id: SourceId,
}

impl Terminal {
    /// Build a terminal from a config: open its sources, auto-subscribe each
    /// `Live` source's configured market, and build the panel layout.
    ///
    /// # Errors
    ///
    /// Returns an error if a source cannot be built or a configured live market
    /// cannot be subscribed.
    pub fn new(config: &Config) -> Result<Self> {
        // Build the set once up front: an unknown indicator or a rejected
        // parameter must fail here, naming itself, rather than when the first
        // trade arrives.
        IndicatorSet::from_specs(&config.indicators)?;
        let state = AppState {
            indicators: config.indicators.clone(),
            timeframe: config.timeframe,
            ..AppState::default()
        };
        let mut terminal = Self {
            state,
            config: config.clone(),
            panels: config.layout.panels.iter().map(build_panel).collect(),
            next_source_id: 0,
        };
        for spec in &config.sources {
            let id = terminal.add_source(spec)?;
            if let SourceSpec::Live { symbol, .. } = spec {
                let sym = Symbol::from_str(symbol).map_err(|e| Error::Source(e.to_string()))?;
                terminal.subscribe(id, &sym)?;
            }
        }
        Ok(terminal)
    }

    /// Build a terminal from a JSON config string (the binding entry point).
    ///
    /// # Errors
    ///
    /// Returns [`Error::Config`] if the JSON is invalid, or a build/subscribe
    /// error as [`Terminal::new`].
    pub fn from_json(config_json: &str) -> Result<Self> {
        Self::new(&Config::from_json(config_json)?)
    }

    /// Open a source at runtime, returning its assigned id.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Source`] if the source cannot be built.
    pub fn add_source(&mut self, spec: &SourceSpec) -> Result<SourceId> {
        let id = self.next_source_id;
        self.next_source_id += 1;
        let source = build_source(id, spec)?;
        self.state.sources.push(source);
        Ok(id)
    }

    /// Remove a source and every market it owned.
    pub fn remove_source(&mut self, id: SourceId) {
        self.state.remove_source(id);
    }

    /// Rewind a replayable source to recorded position `index` and re-fold its
    /// markets' state from the start — the time-machine. The state for every other
    /// source is untouched, and replay resumes forward from `index` on the next
    /// tick.
    ///
    /// # Errors
    ///
    /// Returns [`Error::UnknownSource`] if `id` is not open, or
    /// [`Error::Command`] if the source cannot be replayed (a live or synthetic
    /// feed has no recorded history to seek).
    pub fn seek(&mut self, id: SourceId, index: usize) -> Result<()> {
        let history = self
            .state
            .source_mut(id)
            .ok_or(Error::UnknownSource(id))?
            .seek(index);
        let Some(history) = history else {
            return Err(Error::Command(format!(
                "source {id} is not replayable and cannot be seeked"
            )));
        };
        // Reset this source's per-market state, then re-fold deterministically.
        // Other sources keep their state; subscribed markets with no events yet
        // keep a fresh entry so the layout still renders them.
        //
        // The specs are cloned out first because `fresh_market` borrows the state
        // that the loop below is already borrowing mutably.
        let specs = self.state.indicators.clone();
        let timeframe = self.state.timeframe;
        for (key, symbol_state) in &mut self.state.symbols {
            if key.0 == id {
                *symbol_state = SymbolState::new(&specs, timeframe)
                    .expect("indicator specs are validated before they reach the state");
            }
        }
        for (sym, ev) in history {
            self.state.fold(id, &sym, &ev);
        }
        Ok(())
    }

    /// Push an externally sourced market event into a host-fed (`Manual`) source;
    /// it is folded on the next tick. The event carries its own market, which
    /// must be subscribed on the source.
    ///
    /// # Errors
    ///
    /// Returns [`Error::UnknownSource`] if `id` is not open, or [`Error::Command`]
    /// if the event has no market or the source does not accept fed events (it is
    /// not a manual source, or the market is not subscribed on it).
    pub fn feed(&mut self, id: SourceId, event: Event) -> Result<()> {
        let Some(sym) = event_symbol(&event) else {
            return Err(Error::Command(
                "a fed event must carry a market symbol".to_string(),
            ));
        };
        let source = self.state.source_mut(id).ok_or(Error::UnknownSource(id))?;
        if !source.feed(sym, event) {
            return Err(Error::Command(format!(
                "source {id} does not accept fed events (subscribe the market on a manual source first)"
            )));
        }
        Ok(())
    }

    /// Subscribe a market on a source, tracking it and focusing it if nothing is
    /// focused yet.
    ///
    /// # Errors
    ///
    /// Returns [`Error::UnknownSource`] if `id` is not open, or an error from the
    /// underlying source.
    pub fn subscribe(&mut self, id: SourceId, sym: &Symbol) -> Result<()> {
        let source = self.state.source_mut(id).ok_or(Error::UnknownSource(id))?;
        source.subscribe(sym)?;
        let key = (id, sym.clone());
        if !self.state.watchlist.contains(&key) {
            self.state.watchlist.push(key.clone());
        }
        if !self.state.symbols.contains_key(&key) {
            let fresh = self.state.fresh_market();
            self.state.symbols.insert(key.clone(), fresh);
        }
        if self.state.focus.is_none() {
            self.state.focus = Some(key);
        }
        Ok(())
    }

    /// Unsubscribe a market, dropping its state and repairing focus.
    pub fn unsubscribe(&mut self, id: SourceId, sym: &Symbol) {
        if let Some(source) = self.state.source_mut(id) {
            source.unsubscribe(sym);
        }
        let key = (id, sym.clone());
        self.state.watchlist.retain(|k| k != &key);
        self.state.symbols.remove(&key);
        if self.state.focus.as_ref() == Some(&key) {
            self.state.focus = self.state.watchlist.first().cloned();
        }
    }

    /// Focus a market.
    pub fn set_focus(&mut self, id: SourceId, sym: &Symbol) {
        self.state.focus = Some((id, sym.clone()));
    }

    /// Pump every source and build the next frame.
    pub fn tick(&mut self) -> Frame {
        self.state.pump();
        self.frame()
    }

    /// Build the current frame without pumping (every active panel's view-model).
    #[must_use]
    pub fn frame(&self) -> Frame {
        match &self.state.focus {
            Some((sid, sym)) => Frame {
                panels: self
                    .panels
                    .iter()
                    .map(|panel| panel.view(&self.state, (*sid, sym)))
                    .collect(),
            },
            None => Frame { panels: Vec::new() },
        }
    }

    /// Apply a command given as JSON and return the resulting frame as JSON —
    /// the data-driven FFI boundary.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Command`] if the JSON is not a known command, or a
    /// build/subscribe error from the applied command.
    pub fn command_json(&mut self, cmd_json: &str) -> Result<String> {
        let cmd: Command =
            serde_json::from_str(cmd_json).map_err(|e| Error::Command(e.to_string()))?;
        match cmd {
            Command::Tick => {
                self.state.pump();
            }
            Command::Subscribe { source, symbol } => {
                self.subscribe(source, &parse_symbol(&symbol)?)?;
            }
            Command::Unsubscribe { source, symbol } => {
                self.unsubscribe(source, &parse_symbol(&symbol)?);
            }
            Command::SetFocus { source, symbol } => {
                self.set_focus(source, &parse_symbol(&symbol)?);
            }
            Command::AddSource { spec } => {
                self.add_source(&spec)?;
            }
            Command::RemoveSource { id } => {
                self.remove_source(id);
            }
            Command::Seek { source, index } => {
                self.seek(source, index)?;
            }
            Command::Feed { source, event } => {
                self.feed(source, event)?;
            }
            Command::AddIndicator { spec } => {
                self.state.add_indicator(&spec)?;
                self.config.indicators.push(spec);
            }
            Command::RemoveIndicator { label } => {
                if !self.state.remove_indicator(&label) {
                    return Err(Error::Command(format!("no such indicator: {label}")));
                }
                self.config.indicators.retain(|s| s.label() != label);
            }
            Command::SetTimeframe { timeframe } => {
                self.state.set_timeframe(timeframe)?;
                self.config.timeframe = timeframe;
            }
            // The one command that answers rather than renders: every other
            // command changes state and gets the new frame back, so returning a
            // frame here would mean the catalogue had nowhere to go.
            Command::ListIndicators => {
                return Ok(serde_json::to_string(&Catalogue::current())?);
            }
        }
        Ok(serde_json::to_string(&self.frame())?)
    }

    /// The config this terminal was built from (renderers read the keymap).
    #[must_use]
    pub fn config(&self) -> &Config {
        &self.config
    }

    /// Read-only access to the folded application state (renderers may inspect
    /// it directly instead of going through frames).
    #[must_use]
    pub fn state(&self) -> &AppState {
        &self.state
    }

    /// The crate version.
    #[must_use]
    pub fn version() -> &'static str {
        env!("CARGO_PKG_VERSION")
    }
}

/// Parse a `BASE/QUOTE` symbol, mapping a bad symbol to a command error.
fn parse_symbol(s: &str) -> Result<Symbol> {
    Symbol::from_str(s).map_err(|e| Error::Command(e.to_string()))
}

#[cfg(test)]
mod tests {
    const TICK: &str = r#"{"type":"Tick"}"#;
    use super::*;
    use crate::view::PanelView;
    use rust_decimal::Decimal;
    use rust_decimal_macros::dec;
    use wickra_exchange_core::{Event, OrderSide, TradePrint};

    fn synth_config() -> Config {
        let mut cfg = Config::default_layout();
        cfg.sources = vec![SourceSpec::Synth { seed: 1 }];
        cfg
    }

    /// A three-trade BTC/USDT replay feed and its focused symbol.
    fn replay_config() -> (Symbol, Config) {
        let sym = Symbol::new("BTC", "USDT");
        let trade = |price, ts| {
            Event::Trade(TradePrint {
                symbol: sym.clone(),
                price,
                quantity: dec!(1),
                aggressor: OrderSide::Buy,
                timestamp: ts,
            })
        };
        let feed = vec![
            trade(dec!(100), 1),
            trade(dec!(101), 2),
            trade(dec!(102), 3),
        ];
        let dataset = serde_json::to_string(&feed).unwrap();
        let mut cfg = Config::default_layout();
        cfg.sources = vec![SourceSpec::Replay { dataset }];
        (sym, cfg)
    }

    #[test]
    fn new_with_synth_source_has_no_focus_until_subscribed() {
        let mut term = Terminal::new(&synth_config()).unwrap();
        // Nothing subscribed yet: an empty frame.
        assert!(term.tick().panels.is_empty());
        term.subscribe(0, &Symbol::new("BTC", "USDT")).unwrap();
        // Now the default layout's panels render.
        let frame = term.tick();
        assert_eq!(frame.panels.len(), 5);
    }

    #[test]
    fn tick_folds_synth_trades_into_the_chart() {
        let mut term = Terminal::new(&synth_config()).unwrap();
        term.subscribe(0, &Symbol::new("BTC", "USDT")).unwrap();
        for _ in 0..30 {
            term.tick();
        }
        let frame = term.frame();
        let chart = frame
            .panels
            .iter()
            .find_map(|p| match p {
                PanelView::Chart(c) => Some(c),
                _ => None,
            })
            .unwrap();
        assert!(chart.last > 0.0);
        assert!(!chart.series.is_empty());
    }

    #[test]
    fn command_json_tick_returns_a_frame() {
        let mut term = Terminal::from_json(
            r#"{"sources":[{"Synth":{"seed":1}}],"layout":{"panels":[{"kind":"Chart","rect":{"x":0,"y":0,"w":100,"h":100}}]}}"#,
        )
        .unwrap();
        term.command_json(r#"{"type":"Subscribe","source":0,"symbol":"BTC/USDT"}"#)
            .unwrap();
        let frame_json = term.command_json(r#"{"type":"Tick"}"#).unwrap();
        assert!(frame_json.contains("\"panel\":\"chart\""));
    }

    #[test]
    fn command_json_rejects_unknown_command() {
        let mut term = Terminal::new(&synth_config()).unwrap();
        let err = term.command_json(r#"{"type":"Nope"}"#).unwrap_err();
        assert!(matches!(err, Error::Command(_)));
    }

    #[test]
    fn add_and_remove_source_at_runtime() {
        let mut term = Terminal::new(&Config::default_layout()).unwrap();
        let id = term.add_source(&SourceSpec::Synth { seed: 2 }).unwrap();
        term.subscribe(id, &Symbol::new("ETH", "USDT")).unwrap();
        assert_eq!(term.state().watchlist.len(), 1);
        term.remove_source(id);
        assert!(term.state().watchlist.is_empty());
        assert!(term.state().focus.is_none());
    }

    #[test]
    fn unsubscribe_repairs_focus() {
        let mut term = Terminal::new(&synth_config()).unwrap();
        let btc = Symbol::new("BTC", "USDT");
        let eth = Symbol::new("ETH", "USDT");
        term.subscribe(0, &btc).unwrap();
        term.subscribe(0, &eth).unwrap();
        term.unsubscribe(0, &btc);
        // Focus falls back to the remaining subscription.
        assert_eq!(term.state().focus, Some((0, eth)));
    }

    #[test]
    fn version_is_the_crate_version() {
        assert_eq!(Terminal::version(), env!("CARGO_PKG_VERSION"));
    }

    #[test]
    fn seek_rewinds_replay_state_and_resumes_forward() {
        let (sym, cfg) = replay_config();
        let mut term = Terminal::new(&cfg).unwrap();
        term.subscribe(0, &sym).unwrap();
        for _ in 0..3 {
            term.tick();
        }
        assert_eq!(term.state().get(&(0, sym.clone())).unwrap().last, dec!(102));

        // Rewind to position 2: only the first two trades are folded.
        term.seek(0, 2).unwrap();
        let st = term.state().get(&(0, sym.clone())).unwrap();
        assert_eq!(st.last, dec!(101));
        assert_eq!(st.series(10), vec![100.0, 101.0]);

        // Replay resumes forward: the next tick folds the third trade again.
        term.tick();
        assert_eq!(term.state().get(&(0, sym.clone())).unwrap().last, dec!(102));
    }

    #[test]
    fn seek_to_zero_clears_market_state() {
        let (sym, cfg) = replay_config();
        let mut term = Terminal::new(&cfg).unwrap();
        term.subscribe(0, &sym).unwrap();
        for _ in 0..3 {
            term.tick();
        }
        term.seek(0, 0).unwrap();
        let st = term.state().get(&(0, sym.clone())).unwrap();
        assert_eq!(st.last, Decimal::ZERO);
        assert!(st.series(10).is_empty());
    }

    #[test]
    fn seek_non_replayable_source_errors() {
        let mut term = Terminal::new(&synth_config()).unwrap();
        term.subscribe(0, &Symbol::new("BTC", "USDT")).unwrap();
        assert!(matches!(term.seek(0, 1).unwrap_err(), Error::Command(_)));
    }

    #[test]
    fn seek_unknown_source_errors() {
        let mut term = Terminal::new(&synth_config()).unwrap();
        assert!(matches!(
            term.seek(99, 0).unwrap_err(),
            Error::UnknownSource(99)
        ));
    }

    #[test]
    fn command_json_seek_rewinds() {
        let (_, cfg) = replay_config();
        let mut term = Terminal::new(&cfg).unwrap();
        term.command_json(r#"{"type":"Subscribe","source":0,"symbol":"BTC/USDT"}"#)
            .unwrap();
        for _ in 0..3 {
            term.command_json(r#"{"type":"Tick"}"#).unwrap();
        }
        // Seek to index 1: only the first trade (price 100) remains folded.
        let frame = term
            .command_json(r#"{"type":"Seek","source":0,"index":1}"#)
            .unwrap();
        assert!(frame.contains("\"last\":100.0"));
    }

    fn manual_config() -> Config {
        let mut cfg = Config::default_layout();
        cfg.sources = vec![SourceSpec::Manual];
        cfg
    }

    fn a_trade(sym: &Symbol, price: Decimal) -> Event {
        Event::Trade(TradePrint {
            symbol: sym.clone(),
            price,
            quantity: dec!(1),
            aggressor: OrderSide::Buy,
            timestamp: 1,
        })
    }

    #[test]
    fn feed_pushes_events_into_a_manual_source_folded_on_tick() {
        let sym = Symbol::new("BTC", "USDT");
        let mut term = Terminal::new(&manual_config()).unwrap();
        term.subscribe(0, &sym).unwrap();
        term.feed(0, a_trade(&sym, dec!(100))).unwrap();
        // Fed events fold on the next tick, not immediately.
        assert_eq!(
            term.state().get(&(0, sym.clone())).unwrap().last,
            Decimal::ZERO
        );
        term.tick();
        assert_eq!(term.state().get(&(0, sym.clone())).unwrap().last, dec!(100));
    }

    #[test]
    fn feed_to_a_non_manual_source_errors() {
        let (sym, cfg) = replay_config();
        let mut term = Terminal::new(&cfg).unwrap();
        term.subscribe(0, &sym).unwrap();
        assert!(matches!(
            term.feed(0, a_trade(&sym, dec!(1))).unwrap_err(),
            Error::Command(_)
        ));
    }

    #[test]
    fn feed_unknown_source_errors() {
        let mut term = Terminal::new(&manual_config()).unwrap();
        assert!(matches!(
            term.feed(99, a_trade(&Symbol::new("BTC", "USDT"), dec!(1)))
                .unwrap_err(),
            Error::UnknownSource(99)
        ));
    }

    #[test]
    fn feed_event_without_a_market_errors() {
        let mut term = Terminal::new(&manual_config()).unwrap();
        assert!(matches!(
            term.feed(0, Event::Disconnected).unwrap_err(),
            Error::Command(_)
        ));
    }

    #[test]
    fn command_json_feed_then_tick_folds() {
        let mut term = Terminal::from_json(
            r#"{"sources":["Manual"],"layout":{"panels":[{"kind":"Chart","rect":{"x":0,"y":0,"w":100,"h":100}}]}}"#,
        )
        .unwrap();
        term.command_json(r#"{"type":"Subscribe","source":0,"symbol":"BTC/USDT"}"#)
            .unwrap();
        term.command_json(
            r#"{"type":"Feed","source":0,"event":{"type":"trade","symbol":{"base":"BTC","quote":"USDT"},"price":"100","quantity":"1","aggressor":"Buy","timestamp":1}}"#,
        )
        .unwrap();
        let frame = term.command_json(r#"{"type":"Tick"}"#).unwrap();
        assert!(frame.contains("\"last\":100.0"));
    }

    fn synth_terminal() -> Terminal {
        let mut config = Config::default_layout();
        config.sources = vec![SourceSpec::Synth { seed: 1 }];
        let mut terminal = Terminal::new(&config).unwrap();
        terminal
            .command_json(r#"{"type":"Subscribe","source":0,"symbol":"BTC/USDT"}"#)
            .unwrap();
        terminal
    }

    fn chart_indicator_labels(frame_json: &str) -> Vec<String> {
        let frame: serde_json::Value = serde_json::from_str(frame_json).unwrap();
        frame["panels"]
            .as_array()
            .unwrap()
            .iter()
            .find(|p| p["panel"] == "chart")
            .expect("a chart panel")["indicators"]
            .as_array()
            .unwrap()
            .iter()
            .map(|i| i["name"].as_str().unwrap().to_string())
            .collect()
    }

    #[test]
    fn add_indicator_reaches_the_chart_panel() {
        let mut terminal = synth_terminal();
        let before = chart_indicator_labels(&terminal.command_json(TICK).unwrap());
        assert!(!before.contains(&"Rsi(14)".to_string()));

        terminal
            .command_json(r#"{"type":"AddIndicator","spec":{"kind":"Rsi","params":[14]}}"#)
            .unwrap();
        let after = chart_indicator_labels(&terminal.command_json(TICK).unwrap());
        assert!(after.contains(&"Rsi(14)".to_string()), "got {after:?}");
        assert_eq!(after.len(), before.len() + 1);
    }

    #[test]
    fn remove_indicator_drops_it_from_the_panel() {
        let mut terminal = synth_terminal();
        terminal
            .command_json(r#"{"type":"RemoveIndicator","label":"Ema(50)"}"#)
            .unwrap();
        let labels = chart_indicator_labels(&terminal.command_json(TICK).unwrap());
        assert!(!labels.contains(&"Ema(50)".to_string()), "got {labels:?}");
        assert!(labels.contains(&"Sma(20)".to_string()));
    }

    #[test]
    fn removing_an_unknown_indicator_is_an_error() {
        let mut terminal = synth_terminal();
        let Err(err) = terminal.command_json(r#"{"type":"RemoveIndicator","label":"Nope(1)"}"#)
        else {
            panic!("removing an untracked indicator should fail");
        };
        assert!(err.to_string().contains("Nope(1)"), "{err}");
    }

    #[test]
    fn adding_the_same_indicator_twice_is_an_error() {
        let mut terminal = synth_terminal();
        let Err(err) =
            terminal.command_json(r#"{"type":"AddIndicator","spec":{"kind":"Sma","params":[20]}}"#)
        else {
            panic!("a duplicate label should be rejected");
        };
        assert!(err.to_string().contains("already tracked"), "{err}");
    }

    #[test]
    fn adding_an_unknown_indicator_is_an_error_and_changes_nothing() {
        let mut terminal = synth_terminal();
        let before = chart_indicator_labels(&terminal.command_json(TICK).unwrap());
        assert!(terminal
            .command_json(r#"{"type":"AddIndicator","spec":{"kind":"NotReal","params":[]}}"#)
            .is_err());
        let after = chart_indicator_labels(&terminal.command_json(TICK).unwrap());
        assert_eq!(
            before, after,
            "a rejected spec must leave the set untouched"
        );
    }

    #[test]
    fn list_indicators_answers_with_the_catalogue() {
        let mut terminal = synth_terminal();
        let json = terminal
            .command_json(r#"{"type":"ListIndicators"}"#)
            .unwrap();
        let value: serde_json::Value = serde_json::from_str(&json).unwrap();
        let rows = value["indicators"].as_array().expect("an indicators array");
        assert_eq!(rows.len(), registry::DEFAULTS.len());
        assert!(
            value.get("panels").is_none(),
            "the catalogue is not a frame"
        );

        // Every row must be directly constructible, which is the point of
        // carrying the parameters alongside the name.
        let sma = rows
            .iter()
            .find(|r| r["kind"] == "Sma")
            .expect("Sma in the catalogue");
        assert_eq!(sma["params"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn an_added_indicator_survives_into_the_config() {
        let mut terminal = synth_terminal();
        terminal
            .command_json(r#"{"type":"AddIndicator","spec":{"kind":"Rsi","params":[14]}}"#)
            .unwrap();
        assert!(terminal.config().indicators.iter().any(|s| s.kind == "Rsi"));
        terminal
            .command_json(r#"{"type":"RemoveIndicator","label":"Rsi(14)"}"#)
            .unwrap();
        assert!(!terminal.config().indicators.iter().any(|s| s.kind == "Rsi"));
    }

    #[test]
    fn an_indicator_added_before_a_market_opens_reaches_it() {
        let mut config = Config::default_layout();
        config.sources = vec![SourceSpec::Synth { seed: 1 }];
        let mut terminal = Terminal::new(&config).unwrap();
        // Added while no market is subscribed, so it can only reach the market
        // through the config set rather than through an existing SymbolState.
        terminal
            .command_json(r#"{"type":"AddIndicator","spec":{"kind":"Rsi","params":[14]}}"#)
            .unwrap();
        terminal
            .command_json(r#"{"type":"Subscribe","source":0,"symbol":"BTC/USDT"}"#)
            .unwrap();
        let labels = chart_indicator_labels(&terminal.command_json(TICK).unwrap());
        assert!(labels.contains(&"Rsi(14)".to_string()), "got {labels:?}");
    }

    #[test]
    fn a_multi_output_indicator_reports_its_named_fields() {
        let mut terminal = synth_terminal();
        terminal
            .command_json(
                r#"{"type":"AddIndicator","spec":{"kind":"MacdIndicator","params":[12,26,9]}}"#,
            )
            .unwrap();
        // Enough ticks for the slow EMA and the signal line to warm up.
        let mut raw = String::new();
        for _ in 0..200 {
            raw = terminal.command_json(TICK).unwrap();
        }
        let frame: serde_json::Value = serde_json::from_str(&raw).unwrap();
        let chart = frame["panels"]
            .as_array()
            .unwrap()
            .iter()
            .find(|p| p["panel"] == "chart")
            .expect("a chart panel");
        let macd = chart["indicators"]
            .as_array()
            .unwrap()
            .iter()
            .find(|i| i["name"] == "MacdIndicator(12,26,9)")
            .expect("MACD in the chart panel");

        let fields = macd["fields"].as_array().expect("named fields");
        assert!(fields.len() > 1, "MACD should report more than one field");
        let names: Vec<&str> = fields.iter().map(|f| f["name"].as_str().unwrap()).collect();
        assert!(names.len() == fields.len(), "every field must be named");
        // The primary value is the first field, so a renderer wanting one line
        // does not have to know which field that is.
        assert_eq!(macd["value"], fields[0]["value"]);
    }

    #[test]
    fn a_single_output_indicator_omits_the_fields_key_entirely() {
        // The wire shape a consumer written before multi-output existed sees.
        let mut terminal = synth_terminal();
        let mut raw = String::new();
        for _ in 0..40 {
            raw = terminal.command_json(TICK).unwrap();
        }
        let frame: serde_json::Value = serde_json::from_str(&raw).unwrap();
        let chart = frame["panels"]
            .as_array()
            .unwrap()
            .iter()
            .find(|p| p["panel"] == "chart")
            .expect("a chart panel");
        let sma = chart["indicators"]
            .as_array()
            .unwrap()
            .iter()
            .find(|i| i["name"] == "Sma(20)")
            .expect("Sma in the chart panel");
        assert!(
            sma.get("fields").is_none(),
            "an empty field list must not appear on the wire: {sma}"
        );
        assert!(sma.get("value").is_some());
    }

    #[test]
    fn set_timeframe_changes_the_bar_size() {
        let mut terminal = synth_terminal();
        terminal
            .command_json(r#"{"type":"SetTimeframe","timeframe":"5m"}"#)
            .unwrap();
        assert_eq!(
            terminal.config().timeframe,
            Timeframe::parse("5m").unwrap(),
            "the config should carry the new bar size"
        );
    }

    #[test]
    fn set_timeframe_restarts_the_bar_derived_state_only() {
        let mut terminal = synth_terminal();
        // Warm something up first: a price series and a price indicator.
        for _ in 0..60 {
            terminal.command_json(TICK).unwrap();
        }
        let before = chart_indicator_labels(&terminal.command_json(TICK).unwrap());
        let series_before = {
            let frame: serde_json::Value =
                serde_json::from_str(&terminal.command_json(TICK).unwrap()).unwrap();
            frame["panels"]
                .as_array()
                .unwrap()
                .iter()
                .find(|p| p["panel"] == "chart")
                .unwrap()["series"]
                .as_array()
                .unwrap()
                .len()
        };
        assert!(series_before > 1, "the price series should have filled");

        terminal
            .command_json(r#"{"type":"SetTimeframe","timeframe":"1h"}"#)
            .unwrap();
        let raw = terminal.command_json(TICK).unwrap();
        let frame: serde_json::Value = serde_json::from_str(&raw).unwrap();
        let chart = frame["panels"]
            .as_array()
            .unwrap()
            .iter()
            .find(|p| p["panel"] == "chart")
            .unwrap();

        // The price series is not derived from bars, so it survives.
        assert!(
            chart["series"].as_array().unwrap().len() > 1,
            "retiming should not clear the price history"
        );
        // The indicator set is rebuilt, so the same indicators are tracked but
        // are warming up again.
        assert_eq!(chart_indicator_labels(&raw), before);
        assert!(
            chart["indicators"].as_array().unwrap()[0]["value"].is_null(),
            "a rebuilt indicator should be warming up again"
        );
    }

    #[test]
    fn an_invalid_timeframe_is_rejected() {
        let mut terminal = synth_terminal();
        let Err(err) = terminal.command_json(r#"{"type":"SetTimeframe","timeframe":"1w"}"#) else {
            panic!("an unknown unit should be rejected");
        };
        assert!(err.to_string().contains("1w"), "{err}");
    }
}
