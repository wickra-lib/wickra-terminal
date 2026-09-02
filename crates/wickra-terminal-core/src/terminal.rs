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
use crate::config::{Config, IndicatorSpec, PanelSpec, RectSpec, SourceSpec};
use crate::error::{Error, Result};
use crate::panels::{build_panel, Panel};
use crate::registry;
use crate::source::manual::MAX_PENDING_EVENTS;
use crate::source::{build_source, event_symbol, Event, Fed, SourceId, Symbol};
use crate::state::{AppState, BarSet, DerivativesUpdate, IndicatorSet, ProfileSet, SymbolState};
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
    /// Answer with the recorded events instead of a frame, in exactly the shape
    /// `Replay` takes — so a session can be saved and played back.
    ///
    /// The core is filesystem-free, because it has to be to run in a browser, so
    /// it records into a bounded ring and hands the events over; writing them
    /// anywhere is the host's job.
    ExportRecording,
    /// Turn recording on with a capacity, or off.
    ///
    /// Clears what is already held either way: a capacity change that kept the
    /// old events would leave a recording that is part one size and part
    /// another.
    SetRecording {
        /// How many recent events to keep, or `null` to stop recording.
        capacity: Option<usize>,
    },
    /// Answer with a replayable source's position instead of a frame, so a
    /// renderer can draw a time-machine scrubber.
    ///
    /// Its own command rather than a field on the frame: the position belongs to
    /// a source, not to a panel, and putting it in every frame would put it in
    /// front of every consumer that has no replay at all.
    ReplayPosition {
        /// The source id.
        source: SourceId,
    },
    /// Push an externally sourced market event into a host-fed (`Manual`) source,
    /// to be folded on the next tick. The event carries its own market.
    Feed {
        /// The source id.
        source: SourceId,
        /// The market event to fold (a trade, ticker, book snapshot or diff).
        event: Event,
    },
    /// Fold a derivatives update -- funding, open interest, positioning,
    /// mark/index/futures prices -- into a market's microstructure.
    ///
    /// Its own command rather than a variant of `Feed`, because these do not
    /// travel as `Event`s: the exchange layer models them as a separate feed
    /// with its own cadences, and no venue publishes them on the trade stream.
    /// Every field of the update is optional, so a host sends whichever channel
    /// just arrived and the terminal folds it into what it already holds.
    FeedDerivatives {
        /// The source id the market is tracked on.
        source: SourceId,
        /// The market, as written in a config: `BTC/USDT`.
        symbol: String,
        /// The channels that arrived.
        update: DerivativesUpdate,
    },
    /// Place a new panel on the layout, appended after the ones already there.
    ///
    /// The layout was read once when the terminal was built and never again, so
    /// a terminal opened with the wrong panels had to be restarted with a
    /// different config -- which is not something a person does while watching a
    /// market move.
    AddPanel {
        /// The panel to build, and where to put it.
        spec: PanelSpec,
    },
    /// Take the panel at `index` off the layout.
    RemovePanel {
        /// Its position in the layout, counting from zero.
        index: usize,
    },
    /// Move or resize the panel at `index`.
    ///
    /// The rectangle only. A panel's depth is what it was built with, so
    /// changing that means building a different panel -- a remove and an add,
    /// which says plainly that the old one's state goes with it.
    MovePanel {
        /// Its position in the layout, counting from zero.
        index: usize,
        /// Where it should sit now.
        rect: RectSpec,
    },
}

/// A replayable source's position: what `ReplayPosition` answers with.
///
/// A source that cannot be replayed answers `0/0` rather than an error. That is
/// the honest reading -- a live feed has no recorded length -- and it lets a
/// renderer ask about whatever is focused without first knowing what it is.
#[derive(Debug, Serialize)]
pub struct ReplayPosition {
    /// How many recorded events have been folded.
    pub cursor: usize,
    /// How many events the recording holds. Zero when it is not a recording.
    pub length: usize,
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
    /// Every profile this build accepts, for `config.profiles`.
    ///
    /// A separate list rather than more indicator rows: a profile answers with a
    /// histogram and an indicator with a number, and one list holding both would
    /// make every consumer filter before it could use either. Listed at all
    /// because this is the discovery surface every binding reads -- omitted, a
    /// caller outside Rust has no way to learn these exist or what they take.
    pub profiles: Vec<SurfaceEntry>,
    /// Every alternative bar type this build accepts, for `config.bars`.
    ///
    /// Apart from the profiles for the same reason they are apart from the
    /// indicators: these complete zero or more bars per candle rather than
    /// answering with anything.
    pub bar_types: Vec<SurfaceEntry>,
}

/// One row of a catalogue surface that is not an indicator.
///
/// A profile and a bar type are constructible by name with parameters, and
/// nothing else about a `CatalogueEntry` applies to them: neither reads a second
/// market, and neither has an alias. Reporting them through that row would mean
/// two fields that are always false and always absent, which reads as "we did
/// not think about it" rather than "these do not apply".
#[derive(Debug, Serialize)]
pub struct SurfaceEntry {
    /// The name, as `IndicatorSpec::kind` in `config.profiles` or `config.bars`.
    pub kind: String,
    /// The parameters the wickra golden manifest pins it at.
    pub params: Vec<f64>,
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
    /// The canonical kind this row is a friendly alias for, if it is one.
    ///
    /// Absent from a row that is its own canonical name, which is all but two of
    /// them, so the field costs nothing on the rest. Present so a caller
    /// listing the catalogue can tell that `Macd` and `MacdIndicator` are one
    /// indicator rather than two.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alias_of: Option<String>,
}

impl Catalogue {
    /// The catalogue of this build.
    ///
    /// Walks `KINDS`, which is every name `build` accepts, rather than
    /// `DEFAULTS`, which holds only canonical ones. Walking `DEFAULTS` left the
    /// two friendly aliases constructible and invisible: a caller could build a
    /// `Macd`, but nothing in the discovery surface every binding reads said so.
    #[must_use]
    pub fn current() -> Self {
        Self {
            indicators: registry::KINDS
                .iter()
                .map(|kind| {
                    let canonical = registry::ALIASES
                        .iter()
                        .find(|(alias, _)| alias == kind)
                        .map(|(_, canonical)| *canonical);
                    // An alias builds its canonical kind, so it takes its
                    // parameters too -- the point of carrying them is that a row
                    // is constructible as it stands.
                    let lookup = canonical.unwrap_or(kind);
                    let params = registry::DEFAULTS
                        .iter()
                        .find(|(name, _)| name == &lookup)
                        .map_or_else(Vec::new, |(_, params)| params.to_vec());
                    CatalogueEntry {
                        kind: (*kind).to_string(),
                        params,
                        needs_reference: registry::PAIRWISE.contains(&lookup),
                        alias_of: canonical.map(ToString::to_string),
                    }
                })
                .collect(),
            profiles: registry::PROFILES
                .iter()
                .map(|(kind, params)| SurfaceEntry {
                    kind: (*kind).to_string(),
                    params: params.to_vec(),
                })
                .collect(),
            bar_types: registry::BAR_TYPES
                .iter()
                .map(|(kind, params)| SurfaceEntry {
                    kind: (*kind).to_string(),
                    params: params.to_vec(),
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

impl std::fmt::Debug for Terminal {
    /// `panels` is a vector of trait objects, so it is reported by count.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Terminal")
            .field("state", &self.state)
            .field("config", &self.config)
            .field("panels", &self.panels.len())
            .field("next_source_id", &self.next_source_id)
            .finish()
    }
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
        // Same for the profiles, and for the same reason: a config naming a
        // profile that is not one should say so here, not on the first bar.
        ProfileSet::from_specs(&config.profiles)?;
        BarSet::from_specs(&config.bars)?;
        let mut state = AppState {
            indicators: config.indicators.clone(),
            profiles: config.profiles.clone(),
            bars: config.bars.clone(),
            timeframe: config.timeframe,
            ..AppState::default()
        };
        state.set_recording(config.record);
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
        let profiles = self.state.profiles.clone();
        let bars = self.state.bars.clone();
        let timeframe = self.state.timeframe;
        for (key, symbol_state) in &mut self.state.symbols {
            if key.0 == id {
                *symbol_state = SymbolState::new(&specs, &profiles, &bars, timeframe)
                    .expect("indicator specs are validated before they reach the state");
            }
        }
        // Scoped to this source: the re-fold has reset and is replaying only its
        // markets, so a reference from another source would be a present-day
        // price paired with a historical tick.
        for (sym, ev) in history {
            self.state.fold_scoped(id, &sym, &ev, Some(id));
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
    /// Fold a derivatives update into a tracked market's microstructure.
    ///
    /// Unlike [`feed`](Self::feed), this does not queue anything on a source:
    /// the update is not an event and does not need to be replayed in order.
    /// It lands directly on the market's state, and the derivatives indicators
    /// read it on the next print -- which is the cadence they have, since a
    /// funding print alone does not move a price.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Command`] if the market is not tracked on that source.
    /// A silent no-op would let a host feed a misspelled symbol forever and
    /// wonder why the readings never arrive.
    pub fn feed_derivatives(
        &mut self,
        id: SourceId,
        symbol: &str,
        update: &DerivativesUpdate,
    ) -> Result<()> {
        let market = parse_symbol(symbol)?;
        let state = self.state.symbols.get_mut(&(id, market)).ok_or_else(|| {
            Error::Command(format!(
                "{symbol} is not tracked on source {id}: subscribe it first"
            ))
        })?;
        state.apply_derivatives(update);
        Ok(())
    }

    pub fn feed(&mut self, id: SourceId, event: Event) -> Result<()> {
        let Some(sym) = event_symbol(&event) else {
            return Err(Error::Command(
                "a fed event must carry a market symbol".to_string(),
            ));
        };
        let source = self.state.source_mut(id).ok_or(Error::UnknownSource(id))?;
        match source.feed(sym, event) {
            Fed::Accepted => Ok(()),
            Fed::Refused => Err(Error::Command(format!(
                "source {id} does not accept fed events (subscribe the market on a manual source first)"
            ))),
            Fed::Full => Err(Error::Command(format!(
                "source {id} has {MAX_PENDING_EVENTS} events waiting: tick to drain them before feeding more"
            ))),
        }
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
            // Fetched after the market exists and before anything else looks at
            // it, so the first frame already carries a history. A source with
            // none to offer returns an empty list and this costs a call.
            //
            // Only on a market the terminal did not already hold: re-subscribing
            // one that is already open must not replay its history into state
            // that has moved on since.
            let bars = self.backfill(id, sym);
            if !bars.is_empty() {
                if let Some(state) = self.state.symbols.get_mut(&key) {
                    state.seed_bars(&bars);
                }
            }
        }
        if self.state.focus.is_none() {
            self.state.focus = Some(key);
        }
        Ok(())
    }

    /// The historical bars a source offers for a market, or an empty list.
    ///
    /// Split out so the borrow of the source ends before the state is written:
    /// both live on `self`, and seeding while the source is borrowed would not
    /// compile.
    fn backfill(&mut self, id: SourceId, sym: &Symbol) -> Vec<wickra_core::Candle> {
        if self.config.backfill == 0 {
            return Vec::new();
        }
        let interval = self.config.timeframe.label();
        let limit = self.config.backfill;
        self.state
            .source_mut(id)
            .map(|source| source.backfill(sym, &interval, limit))
            .unwrap_or_default()
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

    /// Place a new panel on the layout, and answer with its index.
    ///
    /// The layout was read once in [`new`](Self::new) and never again, so a
    /// terminal opened with the wrong panels had to be restarted with a
    /// different config -- which is not something a person does while watching a
    /// market move.
    ///
    /// Appended rather than inserted at a position: the index is how every other
    /// panel command names its target, and inserting would renumber the ones a
    /// caller is already holding.
    pub fn add_panel(&mut self, spec: &PanelSpec) -> usize {
        self.panels.push(build_panel(spec));
        self.config.layout.panels.push(spec.clone());
        self.panels.len() - 1
    }

    /// Take the panel at `index` off the layout.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Command`] if there is no panel at `index`.
    pub fn remove_panel(&mut self, index: usize) -> Result<()> {
        if index >= self.panels.len() {
            return Err(Error::Command(format!(
                "no panel at {index}: the layout has {}",
                self.panels.len()
            )));
        }
        self.panels.remove(index);
        self.config.layout.panels.remove(index);
        Ok(())
    }

    /// Move or resize the panel at `index`.
    ///
    /// The rectangle only. A panel's depth is what it was built with, so
    /// changing that means building a different panel -- a remove and an add,
    /// which says plainly that the old one's state goes with it.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Command`] if there is no panel at `index`.
    pub fn move_panel(&mut self, index: usize, rect: RectSpec) -> Result<()> {
        let count = self.panels.len();
        let spec = self.config.layout.panels.get_mut(index).ok_or_else(|| {
            Error::Command(format!("no panel at {index}: the layout has {count}"))
        })?;
        spec.rect = rect;
        Ok(())
    }

    /// Track one more indicator on every market, now and on markets opened
    /// later. It starts cold and warms up from the next tick.
    ///
    /// The typed twin of the `AddIndicator` command, and what that command calls
    /// -- so the config stays in step either way. Without it a Rust embedder had
    /// to assemble JSON to reach its own registry, which is why neither renderer
    /// ever did.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Config`] if the spec names no registered kind, if its
    /// parameters are refused, or if a pairwise kind carries no reference.
    pub fn add_indicator(&mut self, spec: &IndicatorSpec) -> Result<()> {
        self.state.add_indicator(spec)?;
        self.config.indicators.push(spec.clone());
        Ok(())
    }

    /// Stop tracking the indicator with this label (`Sma(20)`, `Rsi(14)`).
    ///
    /// # Errors
    ///
    /// Returns [`Error::Command`] if no tracked indicator carries that label.
    pub fn remove_indicator(&mut self, label: &str) -> Result<()> {
        if !self.state.remove_indicator(label) {
            return Err(Error::Command(format!("no such indicator: {label}")));
        }
        self.config.indicators.retain(|s| s.label() != label);
        Ok(())
    }

    /// Change the bar size the candle-input indicators are fed at.
    ///
    /// Restarts the bar-derived state: each market opens a new bar, the kept
    /// bars are dropped and the indicator set is rebuilt. The price history,
    /// tape, book and footprint are untouched, since none comes from bars.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Config`] if an indicator cannot be rebuilt.
    pub fn set_timeframe(&mut self, timeframe: Timeframe) -> Result<()> {
        self.state.set_timeframe(timeframe)?;
        self.config.timeframe = timeframe;
        Ok(())
    }

    /// The recorded events, oldest first, in the shape `Replay` takes.
    ///
    /// Empty unless recording is on. The typed twin of `ExportRecording`.
    #[must_use]
    pub fn recording(&self) -> Vec<Event> {
        self.state.recording()
    }

    /// How many events the recording holds. Cheaper than counting
    /// [`recording`](Self::recording), which clones the whole ring.
    #[must_use]
    pub fn recording_len(&self) -> usize {
        self.state.recorded.len()
    }

    /// Turn recording on with a capacity, or off.
    ///
    /// Clears what is already held either way, so a capacity change never leaves
    /// a recording that is part one size and part another.
    pub fn set_recording(&mut self, capacity: Option<usize>) {
        self.state.set_recording(capacity);
        self.config.record = capacity;
    }

    /// A replayable source's position and recorded length, for a time-machine
    /// scrubber. `None` for a source that cannot be replayed.
    ///
    /// The trait has carried `cursor` and `event_count` from the start and
    /// nothing read them, so neither renderer could show where in a recording it
    /// was -- or offer to move.
    #[must_use]
    pub fn replay_position(&self, id: SourceId) -> Option<(usize, usize)> {
        let source = self.state.sources.iter().find(|s| s.id() == id)?;
        let length = source.event_count();
        (length > 0).then(|| (source.cursor(), length))
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
            Command::FeedDerivatives {
                source,
                symbol,
                update,
            } => {
                self.feed_derivatives(source, &symbol, &update)?;
            }
            Command::AddPanel { spec } => {
                self.add_panel(&spec);
            }
            Command::RemovePanel { index } => {
                self.remove_panel(index)?;
            }
            Command::MovePanel { index, rect } => {
                self.move_panel(index, rect)?;
            }
            Command::AddIndicator { spec } => {
                self.add_indicator(&spec)?;
            }
            Command::RemoveIndicator { label } => {
                self.remove_indicator(&label)?;
            }
            Command::SetTimeframe { timeframe } => {
                self.set_timeframe(timeframe)?;
            }
            // The one command that answers rather than renders: every other
            // command changes state and gets the new frame back, so returning a
            // frame here would mean the catalogue had nowhere to go.
            Command::ListIndicators => {
                return Ok(serde_json::to_string(&Catalogue::current())?);
            }
            Command::ExportRecording => {
                return Ok(serde_json::to_string(&self.state.recording())?);
            }
            Command::SetRecording { capacity } => {
                self.state.set_recording(capacity);
                self.config.record = capacity;
            }
            Command::ReplayPosition { source } => {
                let (cursor, length) = self.replay_position(source).unwrap_or((0, 0));
                return Ok(serde_json::to_string(&ReplayPosition { cursor, length })?);
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
    use crate::config::{PanelSpec, RectSpec};
    use crate::panels::PanelKind;
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

    /// A source that offers history and nothing else, so the seeding can be
    /// tested without a venue.
    ///
    /// `LiveSource` is the only source that has a backfill and the only one that
    /// needs a socket to produce it, which would otherwise leave the seeding
    /// path untested — and the seeding, not the HTTP call, is what has the
    /// interesting failure modes.
    #[derive(Debug)]
    struct HistorySource {
        id: SourceId,
        bars: Vec<wickra_core::Candle>,
        asked: usize,
    }

    impl crate::source::DataSource for HistorySource {
        fn id(&self) -> SourceId {
            self.id
        }
        fn kind(&self) -> crate::source::SourceKind {
            crate::source::SourceKind::Synth
        }
        fn subscribe(&mut self, _sym: &Symbol) -> Result<()> {
            Ok(())
        }
        fn unsubscribe(&mut self, _sym: &Symbol) {}
        fn poll(&mut self) -> Vec<(Symbol, Event)> {
            Vec::new()
        }
        fn backfill(
            &mut self,
            _sym: &Symbol,
            _interval: &str,
            limit: usize,
        ) -> Vec<wickra_core::Candle> {
            self.asked = limit;
            self.bars.clone()
        }
    }

    fn rising_bars(n: usize) -> Vec<wickra_core::Candle> {
        (0..n)
            .map(|i| {
                let open = 100.0 + i as f64;
                let ts = i64::try_from(i).expect("a small test index");
                wickra_core::Candle::new(open, open + 2.0, open - 1.0, open + 1.0, 5.0, ts)
                    .expect("a rising bar is valid")
            })
            .collect()
    }

    /// A terminal with one history-bearing source already open.
    fn seeded(bars: Vec<wickra_core::Candle>, backfill: usize) -> Terminal {
        let mut config = Config::default_layout();
        config.indicators = vec![IndicatorSpec::new("Atr", vec![5.0])];
        config.backfill = backfill;
        let mut terminal = Terminal::new(&config).expect("the default config builds");
        terminal.state.sources.push(Box::new(HistorySource {
            id: 0,
            bars,
            asked: 0,
        }));
        terminal
    }

    /// The chart panel's view-model out of the current frame.
    ///
    /// `find_map` rather than `find` plus a refutable `let`: the `let` needed an
    /// `else` arm that the `find` had already made unreachable, so three copies
    /// of an `unreachable!()` sat in the tests waiting to be read as a real
    /// branch. Here the arm that skips a panel is the one every other panel in
    /// the default layout takes.
    fn chart_of(terminal: &Terminal) -> crate::view::ChartView {
        terminal
            .frame()
            .panels
            .into_iter()
            .find_map(|panel| match panel {
                PanelView::Chart(chart) => Some(chart),
                _ => None,
            })
            .expect("the default layout has a chart")
    }

    /// A source that backfills is still an ordinary source.
    ///
    /// `poll`, `kind` and `unsubscribe` are the rest of the trait, and a history
    /// source that answered `backfill` correctly and nothing else would seed a
    /// chart once and then sit there: the tick path would take whatever `poll`
    /// gave it, and a market dropped from it would keep being folded.
    #[test]
    fn a_backfilling_source_still_polls_and_unsubscribes() {
        let mut terminal = seeded(rising_bars(4), 200);
        let sym = Symbol::new("BTC", "USDT");
        terminal.subscribe(0, &sym).expect("the source accepts");

        // `tick` polls the source; it has no live events, so the frame is the
        // seeded history and nothing more.
        let frame = terminal.tick();
        assert!(frame
            .panels
            .iter()
            .any(|p| matches!(p, PanelView::Chart(_))));
        // `kind` says it is not a recording, so the scrubber has nothing to show.
        assert_eq!(terminal.replay_position(0), None);

        terminal.unsubscribe(0, &sym);
        assert!(
            terminal.state().watchlist.is_empty(),
            "the market was not dropped"
        );
    }

    #[test]
    fn a_fresh_subscription_is_seeded_from_history() {
        // Without this every bar came from ticks the terminal saw itself, so
        // Atr(5) at an hourly timeframe was silent for five hours and the chart
        // opened empty on a market that has traded for years.
        let mut terminal = seeded(rising_bars(30), 200);
        terminal.subscribe(0, &Symbol::new("BTC", "USDT")).unwrap();

        let chart = chart_of(&terminal);
        assert_eq!(chart.bars.len(), 30, "the history did not reach the chart");
        assert!(chart.last > 0.0, "the last price was not seeded");
        assert!(
            chart.indicators.iter().any(|i| i.value.is_some()),
            "no indicator warmed up on the history"
        );
    }

    #[test]
    fn backfill_zero_fetches_nothing() {
        let mut terminal = seeded(rising_bars(30), 0);
        terminal.subscribe(0, &Symbol::new("BTC", "USDT")).unwrap();
        let chart = chart_of(&terminal);
        assert!(chart.bars.is_empty(), "history was fetched with backfill 0");
    }

    #[test]
    fn re_subscribing_does_not_replay_the_history() {
        // State that has moved on since must not have its past folded into it a
        // second time.
        let mut terminal = seeded(rising_bars(30), 200);
        let sym = Symbol::new("BTC", "USDT");
        terminal.subscribe(0, &sym).unwrap();
        terminal.subscribe(0, &sym).unwrap();
        let chart = chart_of(&terminal);
        assert_eq!(chart.bars.len(), 30, "the history was seeded twice");
    }

    /// A replay config over a short recorded feed.
    fn recording_config(record: Option<usize>) -> Config {
        let feed = (0..6)
            .map(|i| {
                format!(
                    r#"{{"type":"trade","symbol":{{"base":"BTC","quote":"USDT"}},"price":"{}","quantity":"1","aggressor":"Buy","timestamp":{}}}"#,
                    100 + i,
                    i + 1
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        let mut config = Config::default_layout();
        config.sources = vec![SourceSpec::Replay {
            dataset: format!("[{feed}]"),
        }];
        config.record = record;
        config
    }

    #[test]
    fn nothing_is_recorded_unless_recording_is_on() {
        // Off by default, and it has to be: a terminal left running overnight
        // must not fill memory with a feed nobody asked it to keep.
        let mut terminal = Terminal::new(&recording_config(None)).unwrap();
        terminal.subscribe(0, &Symbol::new("BTC", "USDT")).unwrap();
        for _ in 0..6 {
            terminal.tick();
        }
        assert_eq!(terminal.recording_len(), 0);
    }

    #[test]
    fn a_recording_comes_back_in_the_shape_replay_takes() {
        // The whole point: the terminal could rewind a recording and had no way
        // to make one, because Replay takes the feed as JSON rather than a path.
        // So what comes out has to go straight back in.
        let mut terminal = Terminal::new(&recording_config(Some(64))).unwrap();
        terminal.subscribe(0, &Symbol::new("BTC", "USDT")).unwrap();
        for _ in 0..6 {
            terminal.tick();
        }
        assert_eq!(terminal.recording_len(), 6);

        // The typed twin answers the same recording the command serialises. A
        // host embedding the core in Rust reaches for this one, and nothing read
        // it -- so the two could have drifted apart unnoticed.
        let typed = terminal.recording();
        assert_eq!(typed.len(), terminal.recording_len());

        let exported = terminal
            .command_json(r#"{"type":"ExportRecording"}"#)
            .unwrap();
        assert_eq!(
            serde_json::to_string(&typed).expect("the events serialise"),
            exported,
            "the typed recording and the exported one are not the same events"
        );
        let mut replayed = Config::default_layout();
        replayed.sources = vec![SourceSpec::Replay { dataset: exported }];
        let mut second = Terminal::new(&replayed).unwrap();
        second.subscribe(0, &Symbol::new("BTC", "USDT")).unwrap();
        for _ in 0..6 {
            second.tick();
        }
        assert!(
            second
                .state()
                .get(&(0, Symbol::new("BTC", "USDT")))
                .is_some(),
            "the exported recording did not replay"
        );
    }

    #[test]
    fn a_seek_does_not_double_the_recording() {
        // The trap this design exists around: `fold` is also how a seek re-folds
        // a recording, so recording there would append the replayed events back
        // onto the recording and every rewind would double it.
        let mut terminal = Terminal::new(&recording_config(Some(64))).unwrap();
        terminal.subscribe(0, &Symbol::new("BTC", "USDT")).unwrap();
        for _ in 0..6 {
            terminal.tick();
        }
        let before = terminal.recording_len();
        terminal.seek(0, 2).unwrap();
        assert_eq!(
            terminal.recording_len(),
            before,
            "a seek grew the recording"
        );
    }

    #[test]
    fn the_recording_is_bounded_and_keeps_the_newest() {
        let mut terminal = Terminal::new(&recording_config(Some(2))).unwrap();
        terminal.subscribe(0, &Symbol::new("BTC", "USDT")).unwrap();
        for _ in 0..6 {
            terminal.tick();
        }
        assert_eq!(terminal.recording_len(), 2);
    }

    #[test]
    fn turning_recording_on_or_off_clears_what_is_held() {
        // A capacity change that kept the old events would leave a recording
        // that is part one size and part another.
        let mut terminal = Terminal::new(&recording_config(Some(64))).unwrap();
        terminal.subscribe(0, &Symbol::new("BTC", "USDT")).unwrap();
        for _ in 0..6 {
            terminal.tick();
        }
        assert_eq!(terminal.recording_len(), 6);
        terminal.set_recording(Some(10));
        assert_eq!(terminal.recording_len(), 0);
        assert_eq!(terminal.config().record, Some(10));
        terminal.set_recording(None);
        assert_eq!(terminal.config().record, None);
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
        assert_eq!(rows.len(), registry::KINDS.len());
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
    fn the_catalogue_lists_every_profile_and_bar_type() {
        // `ListIndicators` is how a caller outside Rust finds out what this
        // build can do. It carried the indicators only, so the six profiles and
        // ten bar types were configurable by name and undiscoverable: a Python
        // or Go user had no way to learn that `VolumeProfile` exists, let alone
        // what it takes, short of reading the Rust source.
        let catalogue = Catalogue::current();
        for (kind, params) in registry::PROFILES {
            let row = catalogue
                .profiles
                .iter()
                .find(|row| row.kind == kind)
                .unwrap_or_else(|| panic!("the catalogue does not list the profile {kind}"));
            assert_eq!(row.params, params, "{kind} is listed with other parameters");
        }
        for (kind, params) in registry::BAR_TYPES {
            let row = catalogue
                .bar_types
                .iter()
                .find(|row| row.kind == kind)
                .unwrap_or_else(|| panic!("the catalogue does not list the bar type {kind}"));
            assert_eq!(row.params, params, "{kind} is listed with other parameters");
        }
        assert_eq!(catalogue.profiles.len(), registry::PROFILES.len());
        assert_eq!(catalogue.bar_types.len(), registry::BAR_TYPES.len());
    }

    #[test]
    fn a_catalogue_row_is_constructible_as_it_stands() {
        // The reason the parameters are carried at all: a caller should be able
        // to take a row and build it without a second lookup. That was true of
        // the indicators and is now claimed for the other two surfaces.
        let catalogue = Catalogue::current();
        for row in &catalogue.profiles {
            registry::build_profile(&row.kind, &row.params)
                .unwrap_or_else(|err| panic!("{}: {err}", row.kind));
        }
        for row in &catalogue.bar_types {
            registry::build_bars(&row.kind, &row.params)
                .unwrap_or_else(|err| panic!("{}: {err}", row.kind));
        }
    }

    #[test]
    fn the_catalogue_lists_every_name_that_can_be_built() {
        // It used to walk `DEFAULTS`, which holds canonical names only, so both
        // friendly aliases were constructible and absent from the one surface a
        // caller has for finding out what exists.
        let mut terminal = synth_terminal();
        let json = terminal
            .command_json(r#"{"type":"ListIndicators"}"#)
            .unwrap();
        let value: serde_json::Value = serde_json::from_str(&json).unwrap();
        let rows = value["indicators"].as_array().expect("an indicators array");

        for kind in registry::KINDS {
            assert!(
                rows.iter().any(|row| row["kind"] == kind),
                "{kind} can be built but is not in the catalogue"
            );
        }

        // An alias carries its canonical kind's parameters, so the row is
        // constructible as it stands, and says which kind it is.
        let alias = rows
            .iter()
            .find(|row| row["kind"] == "Macd")
            .expect("Macd in the catalogue");
        let canonical = rows
            .iter()
            .find(|row| row["kind"] == "MacdIndicator")
            .expect("MacdIndicator in the catalogue");
        assert_eq!(alias["params"], canonical["params"]);
        assert_eq!(alias["alias_of"], "MacdIndicator");
        assert!(
            canonical.get("alias_of").is_none(),
            "a canonical row carries no alias_of"
        );
    }

    #[test]
    fn every_catalogue_row_builds() {
        // The promise the catalogue makes: a row is enough to construct with.
        let rows = Catalogue::current();
        for row in &rows.indicators {
            let built = if row.needs_reference {
                registry::build_paired(&row.kind, &row.params, "ETH/USDT").is_ok()
            } else {
                registry::build(&row.kind, &row.params).is_ok()
            };
            assert!(
                built,
                "{} does not build from its own catalogue row",
                row.kind
            );
        }
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

    /// A terminal on a synth source tracking one derivatives indicator.
    fn derivatives_terminal(kind: &str) -> Terminal {
        let mut cfg = synth_config();
        cfg.indicators = vec![IndicatorSpec {
            kind: kind.to_string(),
            params: Vec::new(),
            reference: None,
        }];
        let mut term = Terminal::new(&cfg).expect("a synth terminal with one indicator");
        term.subscribe(0, &Symbol::new("BTC", "USDT"))
            .expect("subscribe");
        term
    }

    /// A derivatives update carrying the three prices a tick needs, plus funding.
    fn priced_update(step: i64) -> DerivativesUpdate {
        DerivativesUpdate {
            funding_rate: Some(0.0001 * f64::from(i32::try_from(step % 7).unwrap_or(0))),
            mark_price: Some(20_000.0 + step as f64),
            index_price: Some(20_000.0),
            futures_price: Some(20_050.0),
            open_interest: Some(1_000_000.0 + step as f64 * 10.0),
            ..DerivativesUpdate::default()
        }
    }

    #[test]
    fn a_derivatives_indicator_reads_what_the_host_fed() {
        let mut term = derivatives_terminal("FundingRate");
        for step in 0..30 {
            term.feed_derivatives(0, "BTC/USDT", &priced_update(step))
                .expect("BTC/USDT is subscribed");
            term.command_json(TICK).expect("tick");
        }
        let reading = term
            .state
            .get(&(0, Symbol::new("BTC", "USDT")))
            .expect("BTC is tracked")
            .indicators
            .values()
            .first()
            .and_then(|(_, reading)| *reading);
        assert!(
            reading.is_some(),
            "FundingRate produced no reading after 30 fed updates"
        );
    }

    #[test]
    fn a_derivatives_update_for_an_untracked_market_is_an_error() {
        let mut term = derivatives_terminal("FundingRate");
        // A silent no-op here would let a host feed a misspelled symbol forever
        // and wonder why the readings never arrive.
        let err = term
            .feed_derivatives(0, "ETH/USDT", &priced_update(0))
            .expect_err("ETH/USDT was never subscribed");
        assert!(
            err.to_string().contains("not tracked"),
            "the error should say the market is not tracked, got: {err}"
        );
    }

    #[test]
    fn a_malformed_symbol_is_refused_before_it_reaches_the_state() {
        let mut term = derivatives_terminal("FundingRate");
        assert!(term
            .feed_derivatives(0, "not a symbol", &priced_update(0))
            .is_err());
    }

    #[test]
    fn the_command_boundary_carries_a_derivatives_update() {
        // The whole point of the command: a host in any of the ten languages
        // drives this through JSON, not through the Rust API.
        let mut term = derivatives_terminal("FundingRate");
        let command = r#"{"type":"FeedDerivatives","source":0,"symbol":"BTC/USDT",
            "update":{"funding_rate":0.0002,"mark_price":20100.0,"index_price":20000.0,
            "futures_price":20050.0,"timestamp":7}}"#;
        term.command_json(command).expect("a well-formed command");
        // And an unknown field is refused rather than silently dropped, so a
        // typo in a host's payload is an error rather than a missing channel.
        let typo = r#"{"type":"FeedDerivatives","source":0,"symbol":"BTC/USDT",
            "update":{"funding_rat":0.0002,"timestamp":7}}"#;
        assert!(term.command_json(typo).is_err());
    }

    /// Every profile and every alternative bar type, configured by name, reaches
    /// the frame.
    ///
    /// The two tests above prove one of each. That is enough to show the panels
    /// exist and nothing more: a profile whose name the config layer rejects, or
    /// whose panel row never gets built, would be invisible here while the
    /// registry-level suite -- which calls `build_profile` directly and skips
    /// `Terminal::new` entirely -- stayed green.
    ///
    /// What is asserted is reachability, not warmth. Four of the six profiles are
    /// distributions over the CLOCK -- day of week, minute of session -- and
    /// clearing their warmup would need days of synthetic bars. That they produce
    /// a histogram at all is `every_profile_builds_and_produces_a_histogram`'s
    /// job; that they are reachable by name from a config is this one's.
    #[test]
    fn every_profile_and_bar_type_reaches_the_frame() {
        let mut cfg = synth_config();
        cfg.timeframe = Timeframe::parse("1s").expect("1s is a timeframe");
        cfg.layout.panels = vec![
            PanelSpec {
                kind: PanelKind::Profile,
                rect: RectSpec {
                    x: 0,
                    y: 0,
                    w: 100,
                    h: 50,
                },
                depth: None,
            },
            PanelSpec {
                kind: PanelKind::Bars,
                rect: RectSpec {
                    x: 0,
                    y: 50,
                    w: 100,
                    h: 50,
                },
                depth: None,
            },
        ];
        cfg.profiles = registry::PROFILES
            .iter()
            .map(|(kind, params)| IndicatorSpec {
                kind: (*kind).to_string(),
                params: params.to_vec(),
                reference: None,
            })
            .collect();
        cfg.bars = registry::BAR_TYPES
            .iter()
            .map(|(kind, params)| IndicatorSpec {
                kind: (*kind).to_string(),
                params: params.to_vec(),
                reference: None,
            })
            .collect();

        let mut term = Terminal::new(&cfg).expect("a terminal carrying every profile and bar type");
        term.subscribe(0, &Symbol::new("BTC", "USDT"))
            .expect("subscribe");
        let mut frame = String::new();
        for _ in 0..400 {
            frame = term.command_json(TICK).expect("tick");
        }

        let missing: Vec<&str> = registry::PROFILES
            .iter()
            .chain(registry::BAR_TYPES.iter())
            .map(|(kind, _)| *kind)
            .filter(|kind| !frame.contains(kind))
            .collect();
        assert!(
            missing.is_empty(),
            "{} of the configured surfaces never reached the frame: {missing:?}",
            missing.len()
        );
    }

    #[test]
    fn a_configured_profile_reaches_the_frame() {
        let mut cfg = synth_config();
        cfg.layout.panels = vec![PanelSpec {
            kind: PanelKind::Profile,
            rect: RectSpec {
                x: 0,
                y: 0,
                w: 100,
                h: 100,
            },
            depth: None,
        }];
        // One-second bars, so a tick closes a bar. The synth source advances
        // its clock a second per poll, and VolumeProfile needs twenty closed
        // bars -- at the default minute that is 1200 ticks to prove a wiring.
        cfg.timeframe = Timeframe::parse("1s").expect("1s is a timeframe");
        cfg.profiles = vec![IndicatorSpec {
            kind: "VolumeProfile".to_string(),
            params: vec![20.0, 50.0],
            reference: None,
        }];
        let mut term = Terminal::new(&cfg).expect("a terminal with a profile panel");
        term.subscribe(0, &Symbol::new("BTC", "USDT"))
            .expect("subscribe");
        let mut frame = String::new();
        for _ in 0..400 {
            frame = term.command_json(TICK).expect("tick");
        }
        assert!(
            frame.contains(r#""panel":"profile""#),
            "the frame carries no profile panel: {frame}"
        );
        assert!(
            frame.contains("VolumeProfile"),
            "the profile panel does not name the configured profile: {frame}"
        );
        // The histogram itself, not just the row: a panel that reports an
        // empty `bins` for four hundred bars is a panel that is not wired.
        let bins = frame
            .split(r#""bins":["#)
            .nth(1)
            .expect("the row carries a bins array");
        assert!(
            !bins.starts_with(']'),
            "the profile produced an empty histogram after 400 bars"
        );
    }

    #[test]
    fn a_config_naming_an_indicator_as_a_profile_is_refused() {
        // `Sma` is a perfectly good indicator and not a profile at all. The
        // config should say so when it is built, not when the first bar closes.
        let mut cfg = synth_config();
        cfg.profiles = vec![IndicatorSpec {
            kind: "Sma".to_string(),
            params: vec![14.0],
            reference: None,
        }];
        let err = Terminal::new(&cfg).expect_err("Sma is not a profile");
        assert!(
            err.to_string().contains("Sma"),
            "the error should name it: {err}"
        );
    }

    #[test]
    fn a_configured_bar_stream_reaches_the_frame() {
        let mut cfg = synth_config();
        cfg.layout.panels = vec![PanelSpec {
            kind: PanelKind::Bars,
            rect: RectSpec {
                x: 0,
                y: 0,
                w: 100,
                h: 100,
            },
            depth: None,
        }];
        // One-second bars, as the profile test does and for the same reason:
        // a Renko brick needs the price to move a whole box, and at the
        // default minute that is a great many ticks before anything completes.
        cfg.timeframe = Timeframe::parse("1s").expect("1s is a timeframe");
        cfg.bars = vec![IndicatorSpec {
            kind: "RenkoBars".to_string(),
            params: vec![2.0],
            reference: None,
        }];
        let mut term = Terminal::new(&cfg).expect("a terminal with a bars panel");
        term.subscribe(0, &Symbol::new("BTC", "USDT"))
            .expect("subscribe");
        let mut frame = String::new();
        for _ in 0..2000 {
            frame = term.command_json(TICK).expect("tick");
        }
        assert!(
            frame.contains(r#""panel":"bars""#),
            "the frame carries no bars panel: {frame}"
        );
        assert!(
            frame.contains("RenkoBars"),
            "the panel does not name the stream"
        );
        let bars = frame
            .split(r#""bars":["#)
            .nth(1)
            .expect("the stream carries a bars array");
        assert!(
            !bars.starts_with(']'),
            "RenkoBars completed nothing in 2000 ticks, so the stream is not wired"
        );
    }

    #[test]
    fn a_config_naming_an_indicator_as_a_bar_type_is_refused() {
        let mut cfg = synth_config();
        cfg.bars = vec![IndicatorSpec {
            kind: "Sma".to_string(),
            params: vec![14.0],
            reference: None,
        }];
        let err = Terminal::new(&cfg).expect_err("Sma is not a bar type");
        assert!(
            err.to_string().contains("Sma"),
            "the error should name it: {err}"
        );
    }

    /// The layout can be changed while the terminal runs.
    ///
    /// It was read once in `new` and never again, so a terminal opened with the
    /// wrong panels had to be restarted with a different config -- which is not
    /// something a person does while watching a market move.
    #[test]
    fn a_panel_can_be_added_removed_and_moved_at_run_time() {
        let mut config = synth_config();
        config.layout.panels = vec![PanelSpec::new(
            PanelKind::Chart,
            RectSpec::new(0, 0, 100, 50),
        )];
        let mut terminal = Terminal::new(&config).expect("the config builds");
        terminal
            .subscribe(0, &Symbol::new("BTC", "USDT"))
            .expect("the synth source accepts");
        assert_eq!(terminal.frame().panels.len(), 1);

        let at = terminal.add_panel(&PanelSpec::new(
            PanelKind::Book,
            RectSpec::new(0, 50, 100, 50),
        ));
        assert_eq!(at, 1, "a panel is appended, not inserted");
        let panels = terminal.frame().panels;
        assert_eq!(panels.len(), 2);
        assert!(matches!(panels[1], PanelView::Book(_)), "{:?}", panels[1]);

        // The config moves with it, because the config is what a renderer reads
        // to place the panels a frame carries.
        assert_eq!(terminal.config().layout.panels.len(), 2);

        terminal
            .move_panel(1, RectSpec::new(50, 0, 50, 100))
            .expect("panel 1 exists");
        assert_eq!(
            terminal.config().layout.panels[1].rect,
            RectSpec::new(50, 0, 50, 100)
        );

        terminal.remove_panel(0).expect("panel 0 exists");
        let panels = terminal.frame().panels;
        assert_eq!(panels.len(), 1);
        assert!(
            matches!(panels[0], PanelView::Book(_)),
            "the wrong panel went"
        );
        assert_eq!(terminal.config().layout.panels.len(), 1);
    }

    /// An index past the end is refused, and says how many panels there are.
    ///
    /// A renderer holds indices between frames, and a layout that shrank under
    /// it must get an error rather than silently act on the wrong panel.
    #[test]
    fn a_panel_command_past_the_end_is_refused() {
        let mut config = synth_config();
        config.layout.panels = vec![PanelSpec::new(
            PanelKind::Chart,
            RectSpec::new(0, 0, 100, 100),
        )];
        let mut terminal = Terminal::new(&config).expect("the config builds");

        let err = terminal.remove_panel(1).expect_err("there is no panel 1");
        assert!(err.to_string().contains("no panel at 1"), "{err}");
        assert!(err.to_string().contains("has 1"), "{err}");

        let err = terminal
            .move_panel(9, RectSpec::new(0, 0, 10, 10))
            .expect_err("there is no panel 9");
        assert!(err.to_string().contains("no panel at 9"), "{err}");

        // And the layout is untouched by either refusal.
        assert_eq!(terminal.config().layout.panels.len(), 1);
    }

    /// The three panel commands reach the same code over the JSON boundary.
    #[test]
    fn the_panel_commands_cross_the_boundary() {
        let mut config = synth_config();
        config.layout.panels = vec![PanelSpec::new(
            PanelKind::Chart,
            RectSpec::new(0, 0, 100, 100),
        )];
        let mut terminal = Terminal::new(&config).expect("the config builds");
        terminal
            .subscribe(0, &Symbol::new("BTC", "USDT"))
            .expect("the synth source accepts");

        let frame = terminal
            .command_json(
                r#"{"type":"AddPanel","spec":{"kind":"Tape","rect":{"x":0,"y":0,"w":50,"h":50}}}"#,
            )
            .expect("AddPanel is accepted");
        assert!(frame.contains(r#""panel":"tape""#), "{frame}");

        terminal
            .command_json(r#"{"type":"MovePanel","index":1,"rect":{"x":50,"y":0,"w":50,"h":100}}"#)
            .expect("MovePanel is accepted");
        assert_eq!(
            terminal.config().layout.panels[1].rect,
            RectSpec::new(50, 0, 50, 100)
        );

        let frame = terminal
            .command_json(r#"{"type":"RemovePanel","index":1}"#)
            .expect("RemovePanel is accepted");
        assert!(!frame.contains(r#""panel":"tape""#), "{frame}");

        let err = terminal
            .command_json(r#"{"type":"RemovePanel","index":7}"#)
            .expect_err("there is no panel 7");
        assert!(err.to_string().contains("no panel at 7"), "{err}");
    }

    /// A panel added with a depth carries it, the way a configured one does.
    #[test]
    fn an_added_panel_honours_the_depth_it_was_given() {
        let (sym, mut config) = replay_config();
        config.layout.panels = vec![PanelSpec::new(
            PanelKind::Chart,
            RectSpec::new(0, 0, 100, 100),
        )];
        let mut terminal = Terminal::new(&config).expect("the config builds");
        terminal.subscribe(0, &sym).expect("the replay accepts");
        for _ in 0..3 {
            terminal.tick();
        }

        let mut deep = PanelSpec::new(PanelKind::Tape, RectSpec::new(0, 0, 100, 100));
        deep.depth = Some(1);
        terminal.add_panel(&deep);

        let panels = terminal.frame().panels;
        let PanelView::Tape(tape) = &panels[1] else {
            panic!("the added panel is a tape")
        };
        assert_eq!(tape.prints.len(), 1, "the depth was ignored");
    }
}
