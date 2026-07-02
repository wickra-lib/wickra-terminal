//! The terminal configuration — the data-driven description of what to show.
//!
//! A [`Config`] is the whole terminal as data: which [`SourceSpec`]s to open and
//! a [`Layout`] of panels plus keybinds. It round-trips through both TOML (the
//! on-disk `--config` form) and JSON (the form the bindings pass to
//! `Terminal::new`), so every renderer and every language configures the terminal
//! the same way — no renderer-specific setup.

use crate::panels::PanelKind;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::error::{Error, Result};

/// One data source to open on startup.
///
/// `Live` streams from a venue through the exchange layer; `Replay` drives a
/// recorded feed with a time-machine seek; `Synth` is a deterministic synthetic
/// feed for demos and tests.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SourceSpec {
    /// A live venue feed (e.g. `binance` / `BTC/USDT`). `testnet` selects the
    /// venue's sandbox host.
    Live {
        /// Canonical venue key (`binance`, `bybit`, `okx`, …).
        venue: String,
        /// The market to open, in `BASE/QUOTE` form.
        symbol: String,
        /// Use the venue testnet/sandbox host.
        #[serde(default)]
        testnet: bool,
    },
    /// A recorded feed replayed from a named dataset (a JSON array of events).
    Replay {
        /// The dataset name/path to load.
        dataset: String,
    },
    /// A deterministic synthetic feed seeded by `seed`.
    Synth {
        /// The RNG-free deterministic seed.
        seed: u64,
    },
    /// A host-fed source: the core opens no connection; the host pushes events in
    /// through the `Feed` command. This is how the browser renderer bridges an
    /// exchange WebSocket into the WASM core (which cannot open native sockets),
    /// and how any embedder drives the terminal from its own feed.
    Manual,
}

/// A rectangle in grid units (percent of the screen), `0..=100` on each axis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct RectSpec {
    /// Left edge, percent of width.
    pub x: u16,
    /// Top edge, percent of height.
    pub y: u16,
    /// Width, percent of width.
    pub w: u16,
    /// Height, percent of height.
    pub h: u16,
}

impl RectSpec {
    /// Construct a rectangle.
    #[must_use]
    pub fn new(x: u16, y: u16, w: u16, h: u16) -> Self {
        Self { x, y, w, h }
    }
}

/// One panel placed on the layout: its kind and where it sits.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PanelSpec {
    /// Which panel to build.
    pub kind: PanelKind,
    /// Where it sits on the grid.
    pub rect: RectSpec,
}

/// Action → key bindings, data-driven so both renderers share one keymap.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Keybinds {
    /// The action-name → key-name map (e.g. `"quit" -> "q"`).
    pub bindings: HashMap<String, String>,
}

impl Default for Keybinds {
    fn default() -> Self {
        let bindings = [
            ("quit", "q"),
            ("next_panel", "tab"),
            ("prev_panel", "backtab"),
            ("source_menu", "s"),
            ("add_symbol", "a"),
            ("remove_symbol", "d"),
            ("remove_source", "x"),
            ("next_symbol", "right"),
            ("prev_symbol", "left"),
        ]
        .into_iter()
        .map(|(a, k)| (a.to_string(), k.to_string()))
        .collect();
        Self { bindings }
    }
}

/// A panel layout plus the shared keymap.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Layout {
    /// The panels to render.
    pub panels: Vec<PanelSpec>,
    /// The action → key map.
    #[serde(default)]
    pub keybinds: Keybinds,
}

/// The whole terminal as data: sources to open and a layout to render.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Config {
    /// Sources to open on startup.
    #[serde(default)]
    pub sources: Vec<SourceSpec>,
    /// The panel layout.
    pub layout: Layout,
}

impl Config {
    /// Parse a config from its TOML form.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Config`] if the TOML is malformed or does not match the
    /// config schema.
    pub fn from_toml(s: &str) -> Result<Self> {
        toml::from_str(s).map_err(|e| Error::Config(e.to_string()))
    }

    /// Parse a config from its JSON form (the shape the bindings pass in).
    ///
    /// # Errors
    ///
    /// Returns [`Error::Config`] if the JSON is malformed or does not match the
    /// config schema.
    pub fn from_json(s: &str) -> Result<Self> {
        serde_json::from_str(s).map_err(|e| Error::Config(e.to_string()))
    }

    /// A four-panel default layout (chart, book, tape, watchlist) with no
    /// sources — the starting point a renderer overlays sources onto.
    #[must_use]
    pub fn default_layout() -> Self {
        let panels = vec![
            PanelSpec {
                kind: PanelKind::Chart,
                rect: RectSpec::new(0, 0, 70, 70),
            },
            PanelSpec {
                kind: PanelKind::Book,
                rect: RectSpec::new(70, 0, 30, 35),
            },
            PanelSpec {
                kind: PanelKind::Footprint,
                rect: RectSpec::new(70, 35, 30, 35),
            },
            PanelSpec {
                kind: PanelKind::Tape,
                rect: RectSpec::new(70, 70, 30, 30),
            },
            PanelSpec {
                kind: PanelKind::Watchlist,
                rect: RectSpec::new(0, 70, 70, 30),
            },
        ];
        Self {
            sources: Vec::new(),
            layout: Layout {
                panels,
                keybinds: Keybinds::default(),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_layout_has_five_panels_and_default_keybinds() {
        let cfg = Config::default_layout();
        assert_eq!(cfg.layout.panels.len(), 5);
        assert!(cfg.sources.is_empty());
        assert_eq!(cfg.layout.keybinds.bindings.get("quit").unwrap(), "q");
    }

    #[test]
    fn config_round_trips_through_json() {
        let cfg = Config::default_layout();
        let json = serde_json::to_string(&cfg).unwrap();
        let back = Config::from_json(&json).unwrap();
        assert_eq!(cfg, back);
    }

    #[test]
    fn source_spec_synth_parses_from_json() {
        let cfg = Config::from_json(r#"{"sources":[{"Synth":{"seed":7}}],"layout":{"panels":[]}}"#)
            .unwrap();
        assert_eq!(cfg.sources, vec![SourceSpec::Synth { seed: 7 }]);
    }

    #[test]
    fn source_spec_manual_parses_from_json() {
        // The host-fed source is a unit variant: a bare string in the array.
        let cfg = Config::from_json(r#"{"sources":["Manual"],"layout":{"panels":[]}}"#).unwrap();
        assert_eq!(cfg.sources, vec![SourceSpec::Manual]);
    }

    #[test]
    fn malformed_toml_is_a_config_error() {
        let err = Config::from_toml("not = = valid").unwrap_err();
        assert!(matches!(err, Error::Config(_)));
    }
}
