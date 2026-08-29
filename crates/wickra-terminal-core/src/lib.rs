//! # wickra-terminal-core
//!
//! The data-driven core of the Wickra trading terminal. It folds market events
//! into an O(1) [`AppState`] and turns [`panels`] into [`view`]-models — plain
//! data (values, series, sides), never renderer commands. Two reference
//! renderers consume those view-models (a native TUI and a Web app), and the
//! same core is exposed as a JSON data API in ten languages.
//!
//! ## Shape
//!
//! - [`DataSource`] unifies the feed modules ([`Live`](source::LiveSource),
//!   [`Replay`](source::ReplaySource), [`Synth`](source::SynthSource)) behind one
//!   symbol-tagged `poll()`.
//! - [`AppState::fold`] applies one event incrementally; nothing is recomputed
//!   over history.
//! - [`Panel`] maps state to a [`PanelView`]; a [`Frame`] is the set of them.
//! - [`Terminal`] ties it together, with [`Terminal::command_json`] as the
//!   data-driven FFI boundary the bindings drive.
//!
//! ```
//! use wickra_terminal_core::{Config, SourceSpec, Terminal, Symbol};
//!
//! let mut cfg = Config::default_layout();
//! cfg.sources = vec![SourceSpec::Synth { seed: 1 }];
//! let mut term = Terminal::new(&cfg).unwrap();
//! term.subscribe(0, &Symbol::new("BTC", "USDT")).unwrap();
//! let frame = term.tick();
//! assert_eq!(frame.panels.len(), 5);
//! ```

pub mod candle;
pub mod config;
pub mod error;
pub mod panels;
pub mod registry;
pub mod source;
pub mod state;
pub mod terminal;
pub mod view;

pub use candle::{CandleBuilder, Timeframe};
pub use config::{
    default_indicators, Config, IndicatorSpec, Keybinds, Layout, PanelSpec, RectSpec, SourceSpec,
};
pub use error::{Error, Result};
pub use panels::{build_panel, Panel, PanelKind};
pub use registry::{build as build_indicator, TickIndicator, TickInput, KINDS};
pub use source::{build_source, DataSource, Event, SourceId, SourceKind, Symbol};
pub use state::{AppState, IndicatorReading, IndicatorSet};
pub use terminal::Terminal;
pub use view::{Frame, IndicatorField, IndicatorValue, PanelView};

/// The crate version (the same string [`Terminal::version`] returns).
#[must_use]
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}
