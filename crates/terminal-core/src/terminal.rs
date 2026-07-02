//! The [`Terminal`] handle — the single entry point every renderer and every
//! language binding drives.
//!
//! A `Terminal` owns the [`AppState`], the built [`Panel`]s and the source-id
//! counter. Renderers call [`Terminal::tick`] to pump sources and get the next
//! [`Frame`]; bindings cross the C ABI through [`Terminal::command_json`], the
//! data-driven boundary that takes a command as JSON and returns the resulting
//! frame as JSON. There are no callbacks and no renderer-specific methods.

use std::str::FromStr;

use serde::Deserialize;

use crate::config::{Config, SourceSpec};
use crate::error::{Error, Result};
use crate::panels::{build_panel, Panel};
use crate::source::{build_source, SourceId, Symbol};
use crate::state::AppState;
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
        let mut terminal = Self {
            state: AppState::default(),
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
        self.state.symbols.entry(key.clone()).or_default();
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
    use super::*;
    use crate::view::PanelView;

    fn synth_config() -> Config {
        let mut cfg = Config::default_layout();
        cfg.sources = vec![SourceSpec::Synth { seed: 1 }];
        cfg
    }

    #[test]
    fn new_with_synth_source_has_no_focus_until_subscribed() {
        let mut term = Terminal::new(&synth_config()).unwrap();
        // Nothing subscribed yet: an empty frame.
        assert!(term.tick().panels.is_empty());
        term.subscribe(0, &Symbol::new("BTC", "USDT")).unwrap();
        // Now the four panels render.
        let frame = term.tick();
        assert_eq!(frame.panels.len(), 4);
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
}
