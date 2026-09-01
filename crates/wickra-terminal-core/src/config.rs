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

use crate::candle::Timeframe;
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
            ("add_indicator", "i"),
            ("remove_indicator", "k"),
            ("set_timeframe", "t"),
            ("list_indicators", "l"),
            ("seek_back", ","),
            ("seek_forward", "."),
        ]
        .into_iter()
        .map(|(a, k)| (a.to_string(), k.to_string()))
        .collect();
        Self { bindings }
    }
}

/// One indicator to track on a chart: a registry name and its parameters.
///
/// The name is a `wickra-core` type name (`Sma`, `Rsi`, `MacdIndicator`) and the
/// parameters are positional, in the order that type's constructor takes them.
/// `registry::KINDS` lists every accepted name and `registry::DEFAULTS` the
/// parameters the library itself uses.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IndicatorSpec {
    /// The registry name.
    pub kind: String,
    /// Positional constructor parameters.
    #[serde(default)]
    pub params: Vec<f64>,
    /// The market this indicator compares against, for the pairwise family.
    ///
    /// Written as a symbol, `ETH/USDT`. Required by the kinds in
    /// `registry::PAIRWISE` and ignored by every other, so a spec that carries
    /// one where it means nothing is accepted rather than rejected: it is a
    /// harmless leftover, not a mistake worth failing a config over.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reference: Option<String>,
}

impl IndicatorSpec {
    /// A spec for `kind` with `params`.
    #[must_use]
    pub fn new(kind: impl Into<String>, params: Vec<f64>) -> Self {
        Self {
            kind: kind.into(),
            params,
            reference: None,
        }
    }

    /// A spec for a pairwise `kind`, comparing this market against `reference`.
    #[must_use]
    pub fn paired(kind: impl Into<String>, params: Vec<f64>, reference: impl Into<String>) -> Self {
        Self {
            kind: kind.into(),
            params,
            reference: Some(reference.into()),
        }
    }

    /// The label a renderer shows: `Sma(20)`, or just `Sma` with no parameters,
    /// or `Beta(20) vs ETH/USDT` for a pairwise one.
    ///
    /// A whole-number parameter prints without a trailing `.0`, so a period reads
    /// as the count it is rather than as a float that happens to be round.
    #[must_use]
    pub fn label(&self) -> String {
        let base = if self.params.is_empty() {
            self.kind.clone()
        } else {
            let params: Vec<String> = self
                .params
                .iter()
                .map(|p| {
                    if p.fract() == 0.0 {
                        format!("{p:.0}")
                    } else {
                        p.to_string()
                    }
                })
                .collect();
            format!("{}({})", self.kind, params.join(","))
        };
        // The reference belongs in the label: `Beta(20)` against BTC and the same
        // against ETH are different readings, and the label is what identifies a
        // row to the chart panel and to RemoveIndicator.
        match &self.reference {
            Some(reference) => format!("{base} vs {reference}"),
            None => base,
        }
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

/// The default chart overlay: a short and a long moving average.
#[must_use]
pub fn default_indicators() -> Vec<IndicatorSpec> {
    vec![
        IndicatorSpec::new("Sma", vec![20.0]),
        IndicatorSpec::new("Ema", vec![50.0]),
    ]
}

/// The whole terminal as data: sources to open, indicators to track, and a
/// layout to render.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Config {
    /// Sources to open on startup.
    #[serde(default)]
    pub sources: Vec<SourceSpec>,
    /// The panel layout. Omitting it means the standard five-panel layout, so a
    /// minimal config is a list of sources and nothing else.
    #[serde(default)]
    pub layout: Layout,
    /// Indicators tracked for every market. Omitting it means the default overlay.
    #[serde(default = "default_indicators")]
    pub indicators: Vec<IndicatorSpec>,
    /// The bar size the candle-input indicators are fed at.
    #[serde(default)]
    pub timeframe: Timeframe,
    /// Profiles tracked for every market, for the `Profile` panel.
    ///
    /// Separate from `indicators` because a profile answers with a histogram
    /// rather than a reading, and the two are consumed by different panels.
    /// Empty by default: a profile walks a distribution on every closed bar,
    /// and a configuration with no profile panel should not pay for one.
    #[serde(default)]
    pub profiles: Vec<IndicatorSpec>,
    /// Alternative bar types built from the same closed candles, for the
    /// `Bars` panel.
    ///
    /// Renko, Kagi, point-and-figure and the rest are not indicators and not
    /// profiles: one closed candle completes zero, one or several of them, and
    /// that unevenness is the character of the chart rather than a defect.
    #[serde(default)]
    pub bars: Vec<IndicatorSpec>,
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

    /// The standard layout with no sources: the starting point a renderer
    /// overlays sources onto.
    #[must_use]
    pub fn default_layout() -> Self {
        Self::default()
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            sources: Vec::new(),
            layout: Layout::default(),
            indicators: default_indicators(),
            timeframe: Timeframe::default(),
            profiles: Vec::new(),
            bars: Vec::new(),
        }
    }
}

impl Default for Layout {
    /// Five panels (chart, book, footprint, tape, watchlist) and the default keymap.
    fn default() -> Self {
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
            panels,
            keybinds: Keybinds::default(),
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

    #[test]
    fn a_config_with_indicators_and_a_timeframe_round_trips_through_json() {
        let mut cfg = Config::default_layout();
        cfg.indicators = vec![
            IndicatorSpec::new("Rsi", vec![14.0]),
            IndicatorSpec::new("MacdIndicator", vec![12.0, 26.0, 9.0]),
            IndicatorSpec::new("AdaptiveCycle", vec![]),
        ];
        cfg.timeframe = Timeframe::parse("15m").unwrap();
        let json = serde_json::to_string(&cfg).unwrap();
        assert_eq!(Config::from_json(&json).unwrap(), cfg);
    }

    #[test]
    fn a_config_round_trips_through_toml() {
        // TOML is the on-disk `--config` form, and it is a different serialiser
        // with different rules about tables and nesting, so JSON passing says
        // nothing about it.
        let mut cfg = Config::default_layout();
        cfg.sources = vec![SourceSpec::Synth { seed: 3 }];
        cfg.indicators = vec![IndicatorSpec::new("Atr", vec![14.0])];
        cfg.timeframe = Timeframe::parse("4h").unwrap();
        let text = toml::to_string(&cfg).unwrap();
        assert_eq!(Config::from_toml(&text).unwrap(), cfg);
    }

    #[test]
    fn the_timeframe_survives_as_its_label_not_as_a_number() {
        let mut cfg = Config::default_layout();
        cfg.timeframe = Timeframe::parse("15m").unwrap();
        let json = serde_json::to_string(&cfg).unwrap();
        assert!(
            json.contains(r#""timeframe":"15m""#),
            "a config should carry the label a reader recognises: {json}"
        );
    }

    #[test]
    fn omitting_indicators_and_timeframe_yields_the_defaults() {
        let cfg = Config::from_json(r#"{"sources":[],"layout":{"panels":[]}}"#).unwrap();
        assert_eq!(cfg.indicators, default_indicators());
        assert_eq!(cfg.timeframe, Timeframe::default());
    }

    #[test]
    fn omitting_the_layout_yields_the_standard_panels() {
        // The shape the README shows: sources and nothing else.
        let cfg = Config::from_json(r#"{"sources":[{"Synth":{"seed":1}}]}"#).unwrap();
        assert_eq!(cfg.layout.panels.len(), 5);
        assert_eq!(cfg.sources, vec![SourceSpec::Synth { seed: 1 }]);
    }

    #[test]
    fn an_empty_object_is_a_valid_config() {
        let cfg = Config::from_json("{}").unwrap();
        assert_eq!(cfg, Config::default());
    }

    #[test]
    fn an_indicator_spec_may_omit_its_parameters() {
        let cfg = Config::from_json(r#"{"indicators":[{"kind":"AdaptiveCycle"}]}"#).unwrap();
        assert_eq!(
            cfg.indicators,
            vec![IndicatorSpec::new("AdaptiveCycle", vec![])]
        );
    }

    #[test]
    fn an_invalid_timeframe_is_rejected_at_parse_time() {
        // Not at first use: a typo in a config file should say so when the file
        // is read, not when the first bar would have closed.
        let err = Config::from_json(r#"{"timeframe":"1w"}"#).unwrap_err();
        assert!(matches!(err, Error::Config(_)), "{err}");
        assert!(err.to_string().contains("1w"), "{err}");
    }

    #[test]
    fn indicator_labels_render_whole_numbers_without_a_decimal_point() {
        assert_eq!(IndicatorSpec::new("Sma", vec![20.0]).label(), "Sma(20)");
        assert_eq!(
            IndicatorSpec::new("MacdIndicator", vec![12.0, 26.0, 9.0]).label(),
            "MacdIndicator(12,26,9)"
        );
        assert_eq!(
            IndicatorSpec::new("AdaptiveCycle", vec![]).label(),
            "AdaptiveCycle"
        );
        // A genuinely fractional parameter keeps its point.
        assert_eq!(
            IndicatorSpec::new("AccelerationBands", vec![14.0, 2.5]).label(),
            "AccelerationBands(14,2.5)"
        );
    }

    #[test]
    fn every_default_indicator_is_in_the_registry() {
        // The default overlay is built with `expect` in `IndicatorSet::default`,
        // so a rename in the registry would turn into a panic at run time rather
        // than a compile error. This is the guard for that.
        for spec in default_indicators() {
            assert!(
                crate::registry::build(&spec.kind, &spec.params).is_ok(),
                "the default overlay names {}, which the registry does not accept",
                spec.kind
            );
        }
    }
}
