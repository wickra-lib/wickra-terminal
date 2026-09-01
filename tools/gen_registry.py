#!/usr/bin/env python3
"""Generate crates/wickra-terminal-core/src/registry.rs.

Single source of truth: the wickra-core indicator sources themselves
(crates/wickra-core/src/indicators/*.rs). For every type that implements the
`Indicator` trait we read, directly from the source:

  - the associated `type Input` and `type Output`
  - the `pub [const] fn new(...) -> Result<Self> | Self` constructor signature
  - for multi-output indicators, the `f64` field names of the Output struct

What gets registered, and why only this much:

  Input = f64              fed the last traded price, tick by tick.
  Input = Candle           fed each bar as it closes, from the CandleBuilder.
                           Only closed bars: feeding the bar in progress would
                           make every reading repaint as the bar fills.
  Input = Trade            fed the print with its size and aggressor side, from
                           the terminal's own tape.
  Input = OrderBook        fed the book, from the terminal's own book.
  Input = (f64,f64)        fed this market's price against a reference market's,
                           named in the spec.
  Input = CrossSection     fed the breadth of a named universe of markets, from
                           the members the terminal already tracks.
  Input = DerivativesTick  fed funding, open interest and the taker flow the
                           terminal folds out of its own tape.
  Input = TradeQuote       fed the print together with the mid it arrived
                           against, so a print can be placed against the book.

One more family is reached by name rather than by declared input: a handful of
indicators take an `Input = f64` that wickra-core documents as a per-period
RETURN. They are routed to the `returns` family, which differences closed bars
and feeds the close-to-close return. See RETURN_INPUT_ONLY.

Two shapes of answer do not fit a registry entry and get surfaces of their own
rather than being flattened into one: indicators whose output is a
variable-length histogram become PROFILES, and the bar builders -- which do not
implement `Indicator` at all, and complete zero, one or several bars per candle
-- become BAR_TYPES.

What is left is skipped and reported, never dropped in silence. The run prints
what it skipped and why.

The backtester has a script of the same name doing the same job for a different
shape. Its `BarInput` carries every feed a strategy may consult on one bar; a
terminal has no bars and no strategy, so the input here is a tick and the wrapper
set is correspondingly smaller.

Usage (with a sibling wickra checkout):
    python tools/gen_registry.py --wickra ../wickra \
        --out crates/wickra-terminal-core/src/registry.rs
    cargo fmt --all
"""

from __future__ import annotations

import argparse
import json
import re
from collections import Counter
from pathlib import Path

# Friendly aliases kept for ergonomics, mapping a short name to the canonical
# wickra-core type. The same two the backtester carries.
ALIASES = {
    "Macd": "MacdIndicator",
    "Bollinger": "BollingerBands",
}

# Constructor argument types we know how to read out of a `[f64]` parameter list.
# Anything else means the indicator is skipped rather than mis-constructed.
ARG_READER = {
    "usize": "usize_param(params, {i}, kind)?",
    "f64": "float_param(params, {i}, kind)?",
    "u32": "u32_param(params, {i}, kind)?",
    "i32": "i32_param(params, {i}, kind)?",
    # A moving-average selector, carried as its TA-Lib code the way the core
    # already accepts it, so a JSON config stays a list of numbers rather than
    # growing a second parameter type. `from_code` rejects anything outside
    # 0..=5, so a bad code is an error from the constructor like any other.
    "MaType": "map_new(kind, wc::MaType::from_code(u32_param(params, {i}, kind)?))?",
}

# The input families this terminal can feed today, mapped to the wrapper that
# adapts each to `TickIndicator`: (single-output, struct-output).
#
# The terminal holds a book and a tape of its own, so `Trade` and `OrderBook`
# need only a conversion to the core's types, done once per tick in `state.rs`.
WRAPPERS = {
    "f64": ("ScalarPrice", "ScalarPriceFields"),
    "Candle": ("CandleIn", "CandleInFields"),
    "Trade": ("TradeIn", "TradeInFields"),
    "OrderBook": ("BookIn", "BookInFields"),
    "(f64,f64)": ("PairIn", "PairInFields"),
    # Not an `Input` any indicator declares -- a routing target. See
    # RETURN_INPUT_ONLY below.
    "CrossSection": ("CrossIn", "CrossInFields"),
    "DerivativesTick": ("DerivIn", "DerivInFields"),
    "TradeQuote": ("QuoteIn", "QuoteInFields"),
    "returns": ("ReturnsIn", "ReturnsInFields"),
}

# The families this terminal can feed. `returns` is in here because the routing
# above assigns it before this check, and no indicator declares it as an
# `Input`, so it cannot be reached by accident.
SUPPORTED_INPUTS = set(WRAPPERS)

# Indicators whose `Input = f64` is a per-period RETURN, not a price.
#
# wickra-core says so in their own docs -- "Input is treated as a per-period
# return", "over the trailing window of `period` returns" -- and the terminal has
# only a price to give them. Fed a price, every input looks like a gain, the
# denominator is zero and they return `inf` for every reading: measured across
# 400 varied prices, finite=0 and non-finite=1161.
#
# They used to be skipped for that reason. They are now routed to the `returns`
# family instead, which is the feature that comment asked for: the terminal has
# no return to feed directly, but it builds candles, and the close-to-close
# return of a closed bar is exactly the per-period return these ratios are
# defined over. The set below is therefore a ROUTING list, not an exclusion.
#
# Only these three are excluded because only these three are provably broken --
# driving every indicator the terminal can feed, exactly these produce no finite
# value.
# Other return-documented indicators compute their own returns internally and do
# work on a price.
RETURN_INPUT_ONLY = {
    "GainLossRatio",
    "OmegaRatio",
    "ProfitFactor",
}

# The core type each family names, for the `Indicator<Input = ...>` bound.
INPUT_TY = {
    "f64": "f64",
    "Candle": "Candle",
    "Trade": "wc::Trade",
    "OrderBook": "wc::OrderBook",
    "(f64,f64)": "(f64, f64)",
    "CrossSection": "wc::CrossSection",
    "DerivativesTick": "wc::DerivativesTick",
    "TradeQuote": "wc::TradeQuote",
    "returns": "f64",
}

# Extra state a wrapper carries beyond the indicator itself. Only the pairwise
# family needs any: it has to remember which market its second input comes from,
# because that is a property of the spec rather than of the tick.
EXTRA_FIELDS = {
    "(f64,f64)": (("reference", "String"),),
    "returns": (("previous_close", "Option<f64>"),),
}

# How each family reaches its value out of a `&TickInput` and feeds it. A tick
# that carries nothing this family consumes yields `None` without advancing the
# indicator, which is what keeps a bar indicator on bars and a book indicator on
# book updates while they all share one tick.
UPDATE_EXPR = {
    "f64": "self.inner.update(input.price)",
    "Candle": "input.candle.and_then(|c| self.inner.update(c))",
    "Trade": "input.trade.and_then(|t| self.inner.update(t))",
    "OrderBook": "input.book.clone().and_then(|b| self.inner.update(b))",
    "TradeQuote": (
        "input"
        + chr(10)
        + "            .trade_quote"
        + chr(10)
        + "            .and_then(|quote| self.inner.update(quote))"
    ),
    "DerivativesTick": (
        "input"
        + chr(10)
        + "            .derivatives"
        + chr(10)
        + "            .and_then(|derivatives| self.inner.update(derivatives))"
    ),
    "CrossSection": (
        "input"
        + chr(10)
        + "            .cross_section"
        + chr(10)
        + "            .clone()"
        + chr(10)
        + "            .and_then(|universe| self.inner.update(universe))"
    ),
    "returns": (
        "input.candle.and_then(|candle| {"
        + chr(10)
        + "                let close = candle.close;"
        + chr(10)
        + "                self.previous_close"
        + chr(10)
        + "                    .replace(close)"
        + chr(10)
        + "                    .filter(|previous| previous.is_normal())"
        + chr(10)
        + "                    .and_then(|previous| self.inner.update(close / previous - 1.0))"
        + chr(10)
        + "            })"
    ),
    "(f64,f64)": (
        "input"
        + chr(10)
        + "            .reference(&self.reference)"
        + chr(10)
        + "            .and_then(|other| self.inner.update((input.price, other)))"
    ),
}


def bar_builders(text: str) -> list[tuple[str, str]]:
    """Every `impl BarBuilder for X` in `text`, with the bar type it emits.

    Discovered rather than listed, for the same reason the indicators are: a
    builder added upstream should show up here on the next regeneration, not
    when someone notices it missing.
    """
    found = []
    for match in re.finditer(r"impl\s+BarBuilder\s+for\s+([A-Za-z0-9]+)\s*\{", text):
        name = match.group(1)
        segment = text[match.end() : match.end() + 400]
        bar = re.search(r"type\s+Bar\s*=\s*([A-Za-z0-9]+)\s*;", segment)
        if bar:
            found.append((name, bar.group(1)))
    return found


def assoc_types(text: str, ty: str) -> tuple[str | None, str | None]:
    """The `type Input` / `type Output` of `impl Indicator for ty`."""
    m = re.search(r"impl\s+Indicator\s+for\s+" + re.escape(ty) + r"\b", text)
    if not m:
        return None, None
    seg = text[m.end() : m.end() + 2000]
    mi = re.search(r"type\s+Input\s*=\s*([^;]+);", seg)
    mo = re.search(r"type\s+Output\s*=\s*([^;]+);", seg)
    inp = re.sub(r"\s+", "", mi.group(1)) if mi else None
    out = re.sub(r"\s+", "", mo.group(1)) if mo else None
    return inp, out


def find_new(text: str, ty: str) -> tuple[list[str], bool] | None:
    """Return (argument types, returns_result) for `pub [const] fn new`."""
    for m in re.finditer(r"impl\s+" + re.escape(ty) + r"\s*\{", text):
        seg = text[m.end() : m.end() + 3000]
        mn = re.search(
            r"pub\s+(?:const\s+)?fn\s+new\s*\(([^)]*)\)\s*->\s*(Result<Self>|Self)\s*\{",
            seg,
            re.S,
        )
        if mn:
            argstr = mn.group(1).strip()
            argtypes = [p.split(":", 1)[1].strip() for p in argstr.split(",") if ":" in p]
            return argtypes, mn.group(2).strip() == "Result<Self>"
    return None


# How each Output field type is read as the `f64` the terminal's boundary carries.
#
# `i64` is exact here: these are lags and counts, orders of magnitude below the
# 2^53 at which an integer stops surviving the round trip.
FIELD_READERS = {
    "f64": "last.{name}",
    "i64": "last.{name} as f64",
    # A line that has not formed yet. Ichimoku publishes five of these and
    # only reports some of them for the first `kijun` bars; the terminal
    # already carries a reading as optional, so the honest rendering is to
    # omit the field on the ticks where the core has no value, not to invent
    # one. Kept out of the vector rather than reported as NaN.
    "Option<f64>": "last.{name}",
}

# How each alternative bar maps onto the one shape a renderer can draw.
#
# The ten builders emit ten different bar types, and they fall into two shapes: a
# two-point bar that records where a move started and ended, and an OHLC bar that
# records a range. A renderer cannot hold ten shapes and should not have to, so
# each is mapped to one `AltBar` here -- the mapping is the honest part of this
# table, and each entry says what it does with the fields the bar does not have.
#
# Keyed by the BAR type rather than the builder, because that is what the impl
# names and what the mapping is actually about.
#
# Each value is (open, high, low, close, direction, volume) as expressions over
# `bar`. A `None` volume means the bar carries none -- a Renko brick is a price
# move, not a period, and inventing a zero would read as "no volume traded".
BAR_SHAPES = {
    # Two-point bars: a start, an end, and which way it went. High and low are
    # derived because the bar has no wick -- that is the point of the chart.
    "RenkoBrick": (
        "bar.open",
        "bar.open.max(bar.close)",
        "bar.open.min(bar.close)",
        "bar.close",
        "bar.direction",
        None,
    ),
    "KagiBar": (
        "bar.start",
        "bar.start.max(bar.end)",
        "bar.start.min(bar.end)",
        "bar.end",
        "bar.direction",
        None,
    ),
    "LineBreakBar": (
        "bar.open",
        "bar.open.max(bar.close)",
        "bar.open.min(bar.close)",
        "bar.close",
        "bar.direction",
        None,
    ),
    "RangeBar": (
        "bar.open",
        "bar.open.max(bar.close)",
        "bar.open.min(bar.close)",
        "bar.close",
        "bar.direction",
        None,
    ),
    # A P&F column is a range with a direction and no endpoints of its own: a
    # rising column opens at its low and closes at its high, and a falling one
    # the other way round.
    "PnfColumn": (
        "if bar.direction >= 0 { bar.low } else { bar.high }",
        "bar.high",
        "bar.low",
        "if bar.direction >= 0 { bar.high } else { bar.low }",
        "bar.direction",
        None,
    ),
    # OHLC bars: the range is recorded, so only the direction has to be derived
    # for the ones that do not carry it.
    "TickBar": (
        "bar.open",
        "bar.high",
        "bar.low",
        "bar.close",
        "if bar.close >= bar.open { 1 } else { -1 }",
        "bar.volume",
    ),
    "VolumeBar": (
        "bar.open",
        "bar.high",
        "bar.low",
        "bar.close",
        "if bar.close >= bar.open { 1 } else { -1 }",
        "bar.volume",
    ),
    "DollarBar": (
        "bar.open",
        "bar.high",
        "bar.low",
        "bar.close",
        "if bar.close >= bar.open { 1 } else { -1 }",
        "bar.volume",
    ),
    "ImbalanceBar": (
        "bar.open",
        "bar.high",
        "bar.low",
        "bar.close",
        "bar.direction",
        None,
    ),
    "RunBar": (
        "bar.open",
        "bar.high",
        "bar.low",
        "bar.close",
        "bar.direction",
        None,
    ),
}

# Indicators whose output is a variable-length histogram, mapped to the field
# that carries it and whether it also carries a price range.
#
# These are the ones the registry deliberately does not carry. A registry entry
# promises one name, one number and a fixed set of named fields; a distribution
# over price levels or times of day is none of those, and its length changes as
# the session runs. Squeezing it in means reporting one bin under the whole
# indicator's name -- which is exactly what `VolumeProfile` did before P12.1
# removed it: it reported `price_low`, a price, under a profile's name.
#
# So they get a surface of their own, alongside the registry rather than inside
# it: `ProfileIndicator` returns the histogram whole.
#
# `Footprint` is not here and does not belong here. Its output is a list of
# price LEVELS, each with its own bid and ask volume, which is a different shape
# again -- and the terminal already renders it from its own footprint state, as
# the `footprint` panel.
PROFILE_OUTPUTS = {
    "VolumeProfileOutput": ("bins", True),
    "TpoProfileOutput": ("counts", True),
    "DayOfWeekProfileOutput": ("bins", False),
    "IntradayVolatilityProfileOutput": ("bins", False),
    "TimeOfDayReturnProfileOutput": ("bins", False),
    "VolumeByTimeProfileOutput": ("bins", False),
}

# Scalar outputs that are a whole number rather than a float.
#
# `DrawdownDuration` answers "how many bars has this drawdown lasted", which is a
# count and typed as one. It is not a struct, so `out_fields` has nothing to
# read, and it is not `f64`, so the scalar wrapper's `Output = f64` bound does
# not admit it -- it fell between the two and was reported as an unreadable
# output shape. These get a wrapper of their own; the conversion is exact, since
# a bar count is orders of magnitude below the 2^53 where an integer stops
# surviving a round trip through `f64`.
SCALAR_INT_OUTPUTS = {"u32", "u64", "i64", "usize"}

# The field types whose value is absent on some ticks.
OPTIONAL_FIELDS = {"Option<f64>"}

# The field types that can carry a non-finite value and so need checking.
FLOAT_FIELDS = {"f64", "Option<f64>"}


def out_fields(text: str, out: str) -> list[tuple[str, str, str]] | None:
    """An Output struct's fields as `(name, expression, needs a finite check)`.

    `None` when the terminal cannot carry the struct -- when ANY field has a type
    outside `FIELD_READERS`, not merely when no field is an `f64`. The weaker test
    was the bug: it skipped only structs that were entirely unrepresentable, so a
    struct mixing carryable and uncarryable fields registered with a partial set
    and silently dropped the rest. `VolumeProfile` and `TpoProfile` reported
    `price_low` -- a price, under a profile's name -- and lost the bins that ARE
    the profile. Skipping them is the call P4.3d already made for `Footprint`,
    whose sole field is the same shape; this only states the criterion that skip
    was always about.

    The field list is read line by line rather than with a pattern, because a
    declaration is one per line and the types here are worth reading verbatim.
    """
    m = re.search(r"pub\s+struct\s+" + re.escape(out) + r"\s*\{(.*?)\n\}", text, re.S)
    if not m:
        return None
    declared = []
    for line in m.group(1).splitlines():
        text_line = line.strip().rstrip(",")
        if not text_line.startswith("pub "):
            continue
        name, _, ty = text_line[4:].partition(":")
        declared.append((name.strip(), ty.strip()))
    if not declared or any(ty not in FIELD_READERS for _, ty in declared):
        return None
    return [
        (name, FIELD_READERS[ty].format(name=name), ty)
        for name, ty in declared
    ]


def readers(argtypes: list[str]) -> str:
    return ", ".join(ARG_READER[t].format(i=i) for i, t in enumerate(argtypes))


HEAD = '''//! Indicator registry: constructs `wickra-core` indicators by name and wraps
//! them behind a uniform, object-safe [`TickIndicator`] the terminal can drive
//! from one tick.
//!
//! GENERATED FILE — do not edit by hand. Regenerate with:
//!
//! ```text
//! python tools/gen_registry.py --wickra ../wickra --out crates/wickra-terminal-core/src/registry.rs
//! cargo fmt --all
//! ```
//!
//! Source of truth: the wickra-core indicator sources — the `Indicator` impls,
//! their `new` signatures and their Output structs. An indicator is registered
//! when its input is one of the nine families this terminal can feed and its
//! output is a number or a struct of named numbers:
//!
//! | `Input`           | Fed with                                     | Advances    |
//! |-------------------|----------------------------------------------|-------------|
//! | `f64`             | the last trade price                         | every trade |
//! | `Candle`          | the bar the tick just closed                 | every bar   |
//! | `Trade`           | the print, with size and aggressor side      | every trade |
//! | `OrderBook`       | the locally maintained L2 book               | every trade |
//! | `(f64, f64)`      | this price against a reference market's      | every trade |
//! | returns           | the close-to-close return of the closed bar  | every bar   |
//! | `CrossSection`    | the breadth of a named universe of markets   | every bar   |
//! | `DerivativesTick` | funding, open interest and taker flow        | every trade |
//! | `TradeQuote`      | the print, with the mid it arrived against   | every trade |
//!
//! Multi-output indicators expose their fields by name.
//!
//! Two answers do not fit that contract and have surfaces of their own here:
//! [`ProfileIndicator`], for the indicators whose output is a histogram, and
//! [`BarStream`], for the bar builders, which are not `Indicator`s at all.

use std::collections::BTreeMap;
use std::marker::PhantomData;

use serde::{Deserialize, Serialize};

use wickra_core::{self as wc, BarBuilder, Candle, Indicator};

use crate::error::{Error, Result};

/// What an indicator may consume on one tick.
///
/// `price` is always present — it is the last trade or ticker price. The rest are
/// optional because a tick does not carry all of them: `candle` is `Some` only on
/// the tick that closed a bar, which is why bar indicators advance once per bar
/// rather than once per trade; `trade` and `book` are `Some` when the tick came
/// from a print and the book has two sides to show.
///
/// It is built once per tick and shared by reference across the whole indicator
/// set, so the conversion from the terminal's own book and tape into the core's
/// types is paid once rather than once per indicator.
#[derive(Debug, Clone)]
pub struct TickInput {
    /// The last traded price.
    pub price: f64,
    /// The bar that just closed, if this tick closed one.
    pub candle: Option<Candle>,
    /// The print this tick came from, with its size and aggressor side.
    pub trade: Option<wc::Trade>,
    /// The order book as of this tick, if it has both a bid and an ask side.
    pub book: Option<wc::OrderBook>,
    /// The last price of every other market this terminal tracks, by symbol.
    ///
    /// Populated only when some indicator in the set reads another market, so a
    /// configuration with no pairwise indicator — the usual one — never builds
    /// it. Keyed by the symbol as it is written in a config, `BTC/USDT`.
    pub references: BTreeMap<String, f64>,
    /// Every tracked market as one cross-section, for the breadth family.
    ///
    /// Populated only when some indicator in the set reads the universe, for the
    /// same reason `references` is: assembling it walks every market, and the
    /// usual configuration has no breadth indicator in it. Absent until at least
    /// one market has closed a bar -- a breadth reading compares closes, and a
    /// universe of markets that have not produced one is not a reading.
    pub cross_section: Option<wc::CrossSection>,
    /// This market's derivatives microstructure, if the host has fed any.
    ///
    /// Absent until the venue's mark, index and futures prices have all arrived:
    /// `DerivativesTick::new` rejects a non-positive price, so a tick before
    /// them would not be a tick.
    pub derivatives: Option<wc::DerivativesTick>,
    /// This print paired with the mid that was standing when it arrived.
    ///
    /// Absent when the book is one-sided, since there is no mid to measure
    /// against, and on any tick that is not a print.
    pub trade_quote: Option<wc::TradeQuote>,
}

impl TickInput {
    /// A tick carrying a price and nothing else.
    ///
    /// The builders below add what a given tick actually has. Constructing
    /// through them rather than with a struct literal is what keeps a call site
    /// working when a further input family is registered: the new field defaults
    /// to absent, which is what every existing caller means.
    #[must_use]
    pub fn price(price: f64) -> Self {
        Self {
            price,
            candle: None,
            trade: None,
            book: None,
            references: BTreeMap::new(),
            cross_section: None,
            derivatives: None,
            trade_quote: None,
        }
    }

    /// This tick, having closed `candle`.
    #[must_use]
    pub fn with_candle(mut self, candle: Candle) -> Self {
        self.candle = Some(candle);
        self
    }

    /// This tick, carrying the print it came from.
    #[must_use]
    pub fn with_trade(mut self, trade: wc::Trade) -> Self {
        self.trade = Some(trade);
        self
    }

    /// This tick, carrying the book as of now.
    #[must_use]
    pub fn with_book(mut self, book: wc::OrderBook) -> Self {
        self.book = Some(book);
        self
    }

    /// This tick, with `symbol` last trading at `price`.
    #[must_use]
    pub fn with_reference(mut self, symbol: impl Into<String>, price: f64) -> Self {
        self.references.insert(symbol.into(), price);
        self
    }

    /// The last price of another market, or `None` if it has not printed yet.
    ///
    /// A reference that is absent rather than stale is the point: a pairwise
    /// indicator fed a placeholder would produce a number that looks like a
    /// reading, so it is given nothing and does not advance.
    #[must_use]
    pub fn reference(&self, symbol: &str) -> Option<f64> {
        self.references.get(symbol).copied()
    }
}

/// A uniform, object-safe indicator the terminal drives one tick at a time.
/// One reading of a profile: a histogram, and the price range it spans when it
/// has one.
///
/// Two of the six are distributions over PRICE and carry the range their bins
/// cover; the other four are over TIME -- day of week, minute of session -- and
/// have no price range to report. The bounds are optional rather than zeroed, so
/// a consumer can tell "spans no price" from "spans zero to zero".
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProfileReading {
    /// The histogram, in bin order.
    pub bins: Vec<f64>,
    /// The lowest price the bins cover, for a price profile.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub price_low: Option<f64>,
    /// The highest price the bins cover, for a price profile.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub price_high: Option<f64>,
}

/// One bar from an alternative chart, in the single shape a renderer draws.
///
/// The ten builders emit ten bar types in two shapes -- a two-point bar that
/// records where a move started and ended, and an OHLC bar that records a range.
/// A renderer should not have to hold ten, so each is mapped onto this one, and
/// the mapping is written down per bar type in the generator rather than guessed
/// here.
///
/// `volume` is optional because half of them do not have one: a Renko brick is a
/// price move, not a period, and reporting zero would read as "no volume traded".
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct AltBar {
    /// Where the bar opened.
    pub open: f64,
    /// The highest price it reached.
    pub high: f64,
    /// The lowest price it reached.
    pub low: f64,
    /// Where it closed.
    pub close: f64,
    /// `1` rising, `-1` falling.
    pub direction: i8,
    /// Volume, for the bar types that measure one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub volume: Option<f64>,
}

/// A stream of alternative bars, driven by the same closed candles everything
/// else here reads.
///
/// Separate from [`TickIndicator`] and [`ProfileIndicator`] because it answers
/// with neither a reading nor a distribution: one closed candle can complete
/// zero, one or several bars, and that is the whole character of these charts --
/// a quiet hour produces none and a fast one produces many.
pub trait BarStream: Send {
    /// Feed one tick; returns every bar completed on it, which is usually none.
    fn update(&mut self, input: &TickInput) -> Vec<AltBar>;
}

/// Wraps a bar builder as a [`BarStream`].
///
/// Parameterised by the bar type as well as the builder, which is what keeps one
/// impl per bar from overlapping another.
struct CandleBars<I, B> {
    inner: I,
    /// Only to make the bar type part of this wrapper's identity.
    bar: PhantomData<B>,
}

/// An indicator whose output is a histogram rather than a reading.
///
/// Deliberately not [`TickIndicator`]: that trait promises one `f64` and a fixed
/// set of named fields, and a distribution is neither. Kept apart rather than
/// widening the other, so nothing that consumes a reading has to learn what an
/// absent histogram means.
pub trait ProfileIndicator: Send {
    /// Feed one tick; returns the histogram, or `None` while warming up or when
    /// this tick carries nothing this profile consumes.
    fn update(&mut self, input: &TickInput) -> Option<ProfileReading>;
    /// Number of inputs required before the first histogram.
    fn warmup(&self) -> usize;
}

/// Wraps a bar-input indicator whose output is a histogram.
///
/// Parameterised by the output struct as well as the indicator, which is what
/// keeps one impl per output from overlapping another.
struct CandleProfile<I, O> {
    inner: I,
    /// Only to make the output type part of this wrapper's identity.
    output: PhantomData<O>,
}

pub trait TickIndicator: Send {
    /// Feed one tick; returns the primary value, or `None` while warming up or
    /// when this tick carries nothing this indicator consumes.
    fn update(&mut self, input: &TickInput) -> Option<f64>;
    /// Named output fields of the most recent update (empty for single-output).
    fn fields(&self) -> Vec<(&'static str, f64)>;
    /// Number of inputs required before the first value.
    fn warmup(&self) -> usize;
    /// Whether this indicator reads another market's price.
    ///
    /// Asked for the same reason as [`TickIndicator::wants_book`]: collecting
    /// every tracked market's last price is work no ordinary configuration
    /// needs.
    fn wants_reference(&self) -> bool {
        false
    }
    /// Whether this indicator reads the order book.
    ///
    /// The terminal asks the set before converting its book into the core's
    /// type, so a session whose indicators are all price and bar ones — the
    /// default — never pays for a conversion nothing would read.
    fn wants_book(&self) -> bool {
        false
    }
}

'''

# Per-family prose for the generated wrappers, so each says what it actually
# does rather than carrying one comment stretched over four meanings. Each entry
# is one line per doc-comment line.
WRAPPER_DOC = {
    "f64": ("Wraps a price (`Input = f64`) single-output indicator.",),
    "Candle": (
        "Wraps a bar (`Input = Candle`) single-output indicator. Ticks that did",
        "not close a bar yield `None` without advancing it.",
    ),
    "Trade": (
        "Wraps a tape (`Input = Trade`) single-output indicator, fed the print",
        "with its size and aggressor side rather than the price alone.",
    ),
    "OrderBook": (
        "Wraps a book (`Input = OrderBook`) single-output indicator. Ticks whose",
        "book is one-sided yield `None` without advancing it.",
    ),
    "TradeQuote": (
        "Wraps a microstructure (`Input = TradeQuote`) single-output indicator: one",
        "print paired with the mid that was standing when it arrived. Ticks with a",
        "one-sided book yield `None` without advancing it -- there is no mid to",
        "measure the print against.",
    ),
    "DerivativesTick": (
        "Wraps a derivatives (`Input = DerivativesTick`) single-output indicator:",
        "funding, open interest, positioning and the mark/index/futures prices of",
        "one perpetual market. Ticks before the host has fed those prices yield",
        "`None` without advancing it.",
    ),
    "CrossSection": (
        "Wraps a breadth (`Input = CrossSection`) single-output indicator: the",
        "whole tracked universe on one tick, not one market. Ticks before any",
        "market has closed a bar yield `None` without advancing it.",
    ),
    "returns": (
        "Wraps an indicator whose `Input = f64` is a per-period RETURN rather",
        "than a price, feeding it the close-to-close return of each closed bar.",
        "The first bar establishes the close to difference against and yields",
        "`None`; a previous close that is not a normal number is not divided by.",
    ),
    "(f64,f64)": (
        "Wraps a pairwise (`Input = (f64, f64)`) single-output indicator: this",
        "market's price against a reference market's. Ticks on which the reference",
        "has not printed yet yield `None` without advancing it.",
    ),
}

# Prose for the struct-output wrappers, mirroring WRAPPER_DOC.
FIELD_WRAPPER_DOC = {
    "TradeQuote": (
        "Wraps a microstructure indicator whose output is a struct of fields. The",
        "primary value is the first field; every field is reachable by name.",
    ),
    "DerivativesTick": (
        "Wraps a derivatives indicator whose output is a struct of fields. The",
        "primary value is the first field; every field is reachable by name.",
    ),
    "CrossSection": (
        "Wraps a breadth indicator whose output is a struct of fields. The",
        "primary value is the first field; every field is reachable by name.",
    ),
    "f64": (
        "Wraps a price indicator whose output is a struct of `f64` fields. The",
        "primary value is the first field; every field is reachable by name.",
    ),
    "Candle": ("Wraps a bar indicator whose output is a struct of `f64` fields.",),
    "Trade": ("Wraps a tape indicator whose output is a struct of `f64` fields.",),
    "OrderBook": ("Wraps a book indicator whose output is a struct of `f64` fields.",),
    "(f64,f64)": (
        "Wraps a pairwise indicator whose output is a struct of `f64` fields.",
    ),
}


def doc(lines: tuple[str, ...]) -> str:
    """Render doc lines as the body of a `///` comment block."""
    return (chr(10) + "/// ").join(lines)


def wants_book(family: str) -> str:
    """The `wants_book` override, for the family that reads the book."""
    if family != "OrderBook":
        return ""
    return (
        chr(10)
        + "    fn wants_book(&self) -> bool {"
        + chr(10)
        + "        true"
        + chr(10)
        + "    }"
    )


# How each family reports its warmup. The returns family spends one bar
# establishing the close it differences against, so it needs one more than the
# indicator itself asks for.
WARMUP_EXPR = {
    "returns": "self.inner.warmup_period() + 1",
}


def warmup_expr(family: str) -> str:
    return WARMUP_EXPR.get(family, "self.inner.warmup_period()")


def extra_decls(family: str) -> str:
    """Struct fields this family's wrapper carries beyond `inner`."""
    return "".join(
        chr(10) + f"    {name}: {ty}," for name, ty in EXTRA_FIELDS.get(family, ())
    )


def wants_reference(family: str) -> str:
    """The `wants_reference` override, for the family that reads another market."""
    if family != "(f64,f64)":
        return ""
    return (
        chr(10)
        + "    fn wants_reference(&self) -> bool {"
        + chr(10)
        + "        true"
        + chr(10)
        + "    }"
    )


def emit_bars(bars: list, defaults: dict) -> str:
    """The alternative-bar surface: the bar, the trait, the wrappers and the builder.

    Every builder takes `Input = Candle`, so there is one wrapper. It is
    parameterised by the BAR type as well as the builder, which keeps one impl
    per bar from overlapping another.
    """
    if not bars:
        return ""
    impls = []
    for name, bar_ty, _, _ in sorted(bars):
        shape = BAR_SHAPES.get(bar_ty)
        if shape is None:
            raise SystemExit(
                f"error: {name} emits {bar_ty}, which has no entry in BAR_SHAPES -- "
                "add one saying how it maps onto AltBar rather than guessing"
            )
        open_, high, low, close, direction, volume = shape
        vol = f"Some({volume})" if volume else "None"
        impls.append(
            f"""
impl<I> BarStream for CandleBars<I, wc::{bar_ty}>
where
    I: BarBuilder<Bar = wc::{bar_ty}> + Send,
{{
    fn update(&mut self, input: &TickInput) -> Vec<AltBar> {{
        let Some(candle) = input.candle else {{
            return Vec::new();
        }};
        self.inner
            .update(candle)
            .into_iter()
            .map(|bar| AltBar {{
                open: {open_},
                high: {high},
                low: {low},
                close: {close},
                direction: {direction},
                volume: {vol},
            }})
            .collect()
    }}
}}
"""
        )

    arms = []
    for name, bar_ty, argtypes, returns_result in sorted(bars):
        ctor = f"wc::{name}::new({readers(argtypes)})" if argtypes else f"wc::{name}::new()"
        made = f"map_new(kind, {ctor})?" if returns_result else ctor
        arms.append(
            f'        "{name}" => Ok(Box::new(CandleBars::<_, wc::{bar_ty}> {{'
            f" inner: {made}, bar: PhantomData }})),"
        )

    rows = []
    for name, _, _, _ in sorted(bars):
        params = defaults.get(name)
        if params is None:
            raise SystemExit(f"error: no manifest defaults for the bar type {name}")
        values = ", ".join(repr(float(v)) for v in params)
        rows.append(f'    ("{name}", &[{values}]),')

    names = " | ".join(f'"{name}"' for name, _, _, _ in sorted(bars))
    return f"""
{"".join(impls)}
/// Every alternative bar type this terminal can build, with the parameters the
/// wickra golden manifest pins them at.
pub const BAR_TYPES: [(&str, &[f64]); {len(rows)}] = [
{chr(10).join(rows)}
];

/// Whether `kind` names an alternative bar type rather than an indicator.
#[must_use]
pub fn is_bar_type(kind: &str) -> bool {{
    matches!(kind, {names})
}}

/// Build an alternative bar stream by name.
///
/// # Errors
///
/// Returns [`Error::Config`] if `kind` is not a bar type, or if its parameters
/// are missing or rejected by the constructor.
pub fn build_bars(kind: &str, params: &[f64]) -> Result<Box<dyn BarStream>> {{
    match kind {{
{chr(10).join(arms)}
        _ => Err(Error::Config(format!("unknown bar type: {{kind}}"))),
    }}
}}
"""


def emit_profiles(profiles: list, defaults: dict) -> str:
    """The profile surface: the reading, the trait, the wrappers and the builder.

    All six take `Input = Candle`, so there is one wrapper rather than one per
    family. It is parameterised by the OUTPUT struct as well as the indicator,
    which is what keeps one impl per output from overlapping another -- the same
    reason the struct-output wrapper carries its output type.
    """
    if not profiles:
        return ""
    impls = []
    for name, out, _, _ in sorted(profiles):
        field, priced = PROFILE_OUTPUTS[out]
        low = "Some(reading.price_low)" if priced else "None"
        high = "Some(reading.price_high)" if priced else "None"
        impls.append(
            f"""
impl<I> ProfileIndicator for CandleProfile<I, wc::{out}>
where
    I: Indicator<Input = Candle, Output = wc::{out}> + Send,
{{
    fn update(&mut self, input: &TickInput) -> Option<ProfileReading> {{
        input
            .candle
            .and_then(|candle| self.inner.update(candle))
            .map(|reading| ProfileReading {{
                bins: reading.{field},
                price_low: {low},
                price_high: {high},
            }})
    }}
    fn warmup(&self) -> usize {{
        self.inner.warmup_period()
    }}
}}
"""
        )

    arms = []
    for name, out, argtypes, returns_result in sorted(profiles):
        ctor = f"wc::{name}::new({readers(argtypes)})" if argtypes else f"wc::{name}::new()"
        made = f"map_new(kind, {ctor})?" if returns_result else ctor
        arms.append(
            f'        "{name}" => Ok(Box::new(CandleProfile::<_, wc::{out}> {{'
            f" inner: {made}, output: PhantomData }})),"
        )

    rows = []
    for name, _, _, _ in sorted(profiles):
        params = defaults.get(name)
        if params is None:
            raise SystemExit(f"error: no manifest defaults for the profile {name}")
        values = ", ".join(repr(float(v)) for v in params)
        rows.append(f'    ("{name}", &[{values}]),')

    names = " | ".join(f'"{name}"' for name, _, _, _ in sorted(profiles))
    return f"""
{"".join(impls)}
/// Every profile this terminal can build, with the parameters the wickra golden
/// manifest pins them at.
///
/// Kept apart from [`DEFAULTS`] rather than merged into it: a caller asking the
/// catalogue what it can *read* wants indicators, and a caller laying out a
/// panel wants profiles. One list holding both would make every consumer filter.
pub const PROFILES: [(&str, &[f64]); {len(rows)}] = [
{chr(10).join(rows)}
];

/// Whether `kind` names a profile rather than an indicator.
#[must_use]
pub fn is_profile(kind: &str) -> bool {{
    matches!(kind, {names})
}}

/// Build a profile by name.
///
/// # Errors
///
/// Returns [`Error::Config`] if `kind` is not a profile, or if its parameters
/// are missing or rejected by the constructor.
pub fn build_profile(kind: &str, params: &[f64]) -> Result<Box<dyn ProfileIndicator>> {{
    match kind {{
{chr(10).join(arms)}
        _ => Err(Error::Config(format!("unknown profile: {{kind}}"))),
    }}
}}
"""


def emit_int_wrappers(families: set[str]) -> str:
    """One wrapper per family in use, for whole-number scalar outputs.

    A separate type rather than a second impl on the scalar wrapper: two impls of
    the same trait on one struct, differing only in the bound on `I::Output`,
    overlap as far as the compiler is concerned.
    """
    out = []
    for family, (wrapper, _) in WRAPPERS.items():
        if family not in families:
            continue
        out.append(
            f"""
/// {doc(WRAPPER_DOC[family])}
///
/// This one carries an indicator whose output is a whole number -- a count of
/// bars, not a price -- converted to the `f64` the boundary speaks.
struct {wrapper}Int<I> {{
    inner: I,{extra_decls(family)}
}}

impl<I, O> TickIndicator for {wrapper}Int<I>
where
    I: Indicator<Input = {INPUT_TY[family]}, Output = O> + Send,
    O: Into<f64> + Send,
{{
    fn update(&mut self, input: &TickInput) -> Option<f64> {{
        {UPDATE_EXPR[family]}.map(Into::into)
    }}
    fn fields(&self) -> Vec<(&'static str, f64)> {{
        Vec::new()
    }}
    fn warmup(&self) -> usize {{
        {warmup_expr(family)}
    }}{wants_book(family)}{wants_reference(family)}
}}
"""
        )
    return "".join(out)


def emit_scalar_wrappers() -> str:
    """One wrapper per input family, for `Output = f64` indicators."""
    out = []
    for family, (wrapper, _) in WRAPPERS.items():
        out.append(
            f"""
/// {doc(WRAPPER_DOC[family])}
struct {wrapper}<I> {{
    inner: I,{extra_decls(family)}
}}

impl<I> TickIndicator for {wrapper}<I>
where
    I: Indicator<Input = {INPUT_TY[family]}, Output = f64> + Send,
{{
    fn update(&mut self, input: &TickInput) -> Option<f64> {{
        {UPDATE_EXPR[family]}.filter(|value| value.is_finite())
    }}
    fn fields(&self) -> Vec<(&'static str, f64)> {{
        Vec::new()
    }}
    fn warmup(&self) -> usize {{
        {warmup_expr(family)}
    }}{wants_book(family)}{wants_reference(family)}
}}
"""
        )
    return "".join(out)


def emit_field_structs(families: set[str]) -> str:
    """The struct-output wrapper types, for the families that have one.

    Only the families in use are emitted. A wrapper for a family whose every
    indicator is single-output would be a type nothing names.
    """
    out = []
    for family, (_, wrapper) in WRAPPERS.items():
        if family not in families:
            continue
        out.append(
            f"""
/// {doc(FIELD_WRAPPER_DOC[family])}
struct {wrapper}<I, O> {{
    inner: I,
    last: Option<O>,{extra_decls(family)}
}}
"""
        )
    return "".join(out)


def nl_join(parts) -> str:
    """Join generated lines with a real newline, kept out of the f-strings."""
    return chr(10).join(parts)


def emit_field_impls(
    structs: dict[tuple[str, str], list[tuple[str, str, str]]],
) -> str:
    """One `TickIndicator` impl per (input family, Output struct) pair in use.

    A blanket impl cannot reach the fields: they are named differently on every
    struct and there is no trait exposing them, so the impls are generated. They
    are keyed by family as well as by struct so that only the pairs some
    indicator actually needs are emitted, rather than every struct crossed with
    every family.
    """
    out = []
    for (family, struct), fields in sorted(structs.items()):
        wrapper = WRAPPERS[family][1]
        optional = any(ty in OPTIONAL_FIELDS for _, _, ty in fields)
        # Only a float can be non-finite; an integer field is finite by its
        # type, and an absent optional has nothing to check.
        finite_check = " && ".join(
            (
                f"{expr}.is_none_or(f64::is_finite)"
                if ty in OPTIONAL_FIELDS
                else f"{expr}.is_finite()"
            )
            for _, expr, ty in fields
            if ty in FLOAT_FIELDS
        )
        # The first field is the scalar reading. When it is optional the
        # reading is simply absent on the ticks where the core has no value.
        first_name, first_expr, first_ty = fields[0]
        primary = (
            f"self.last.as_ref().and_then(|last| {first_expr})"
            if first_ty in OPTIONAL_FIELDS
            else f"self.last.as_ref().map(|last| {first_expr})"
        )
        if optional:
            # Built by pushing, because an absent field is left out entirely
            # rather than reported as some stand-in number.
            pushes = nl_join(
                (
                    f"        if let Some(value) = {expr} {{"
                    + chr(10)
                    + f'            out.push(("{name}", value));'
                    + chr(10)
                    + "        }"
                )
                if ty in OPTIONAL_FIELDS
                else f'        out.push(("{name}", {expr}));'
                for name, expr, ty in fields
            )
            fields_body = (
                "        let Some(last) = self.last.as_ref() else {"
                + chr(10)
                + "            return Vec::new();"
                + chr(10)
                + "        };"
                + chr(10)
                + "        let mut out = Vec::new();"
                + chr(10)
                + pushes
                + chr(10)
                + "        out"
            )
        else:
            pairs = ", ".join(f'("{name}", {expr})' for name, expr, _ in fields)
            fields_body = (
                "        self.last"
                + chr(10)
                + "            .as_ref()"
                + chr(10)
                + f"            .map(|last| vec![{pairs}])"
                + chr(10)
                + "            .unwrap_or_default()"
            )
        out.append(
            f"""
impl<I> TickIndicator for {wrapper}<I, wc::{struct}>
where
    I: Indicator<Input = {INPUT_TY[family]}, Output = wc::{struct}> + Send,
{{
    fn update(&mut self, input: &TickInput) -> Option<f64> {{
        let out = {UPDATE_EXPR[family]}
            .filter(|last| {finite_check});
        self.last = out;
        {primary}
    }}
    fn fields(&self) -> Vec<(&'static str, f64)> {{
{fields_body}
    }}
    fn warmup(&self) -> usize {{
        {warmup_expr(family)}
    }}{wants_book(family)}{wants_reference(family)}
}}
"""
        )
    return "".join(out)


PARAMS = '''
/// The largest window an indicator may be asked for.
///
/// A period is a length and an indicator allocates it, so an unbounded one is an
/// allocation the caller chooses. Past a million there is nothing to measure --
/// a million one-minute bars is two years -- and below it the allocation stays
/// in the megabytes.
pub const MAX_PERIOD: usize = 1_000_000;

/// Read a positional parameter as a count.
fn usize_param(params: &[f64], idx: usize, kind: &str) -> Result<usize> {
    let value = params.get(idx).copied().ok_or_else(|| {
        Error::Config(format!("{kind}: missing parameter {idx}"))
    })?;
    if value < 0.0 || value.fract() != 0.0 {
        return Err(Error::Config(format!(
            "{kind}: parameter {idx} must be a non-negative whole number, got {value}"
        )));
    }
    // An upper bound, because a period is a length and an indicator allocates
    // it. Without this a config could name a window of 10^20: the cast to usize
    // succeeds, the indicator asks for a Vec that size, and the process aborts
    // on a capacity overflow it cannot catch. A fuzz run found it in under a
    // minute, and the new indicator prompt made it a thing a user could type.
    //
    // A million is past any real window -- a million one-minute bars is two
    // years -- and bounds the allocation at a few megabytes.
    if value > MAX_PERIOD as f64 {
        return Err(Error::Config(format!(
            "{kind}: parameter {idx} is {value}, larger than the {MAX_PERIOD} maximum"
        )));
    }
    Ok(value as usize)
}

/// Read a positional parameter as a float.
fn float_param(params: &[f64], idx: usize, kind: &str) -> Result<f64> {
    params
        .get(idx)
        .copied()
        .ok_or_else(|| Error::Config(format!("{kind}: missing parameter {idx}")))
}

/// Read a positional parameter as an unsigned 32-bit integer.
fn u32_param(params: &[f64], idx: usize, kind: &str) -> Result<u32> {
    let value = usize_param(params, idx, kind)?;
    u32::try_from(value)
        .map_err(|_| Error::Config(format!("{kind}: parameter {idx} is out of range")))
}

/// Read a positional parameter as a signed 32-bit integer.
fn i32_param(params: &[f64], idx: usize, kind: &str) -> Result<i32> {
    let value = float_param(params, idx, kind)?;
    if value.fract() != 0.0 {
        return Err(Error::Config(format!(
            "{kind}: parameter {idx} must be a whole number, got {value}"
        )));
    }
    if value < f64::from(i32::MIN) || value > f64::from(i32::MAX) {
        return Err(Error::Config(format!(
            "{kind}: parameter {idx} is out of range"
        )));
    }
    Ok(value as i32)
}

/// Turn a wickra-core construction error into a config error naming the kind.
fn map_new<T>(kind: &str, made: core::result::Result<T, wc::Error>) -> Result<T> {
    made.map_err(|err| Error::Config(format!("{kind}: {err}")))
}
'''


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--wickra", required=True, help="path to a wickra checkout")
    ap.add_argument("--out", required=True, help="path to write registry.rs")
    args = ap.parse_args()

    src = Path(args.wickra) / "crates" / "wickra-core" / "src"
    indicators = src / "indicators"
    if not indicators.is_dir():
        raise SystemExit(f"error: {indicators} not found — is --wickra a wickra checkout?")

    # The whole crate, for Output struct definitions that may live outside
    # indicators/ (the OHLCV and market types do).
    bigtext = "\n".join(p.read_text(encoding="utf-8") for p in sorted(src.rglob("*.rs")))

    entries = []          # (name, input, output, argtypes, returns_result, fields)
    profiles = []         # (name, output struct, argtypes, returns_result)
    bars = []             # (name, bar type, argtypes, returns_result)
    skipped = Counter()
    skipped_names: dict[str, list[str]] = {}

    for path in sorted(indicators.glob("*.rs")):
        text = path.read_text(encoding="utf-8")
        # Alternative bar builders, which are a different trait entirely: they
        # answer with bars rather than readings, so they never enter `entries`.
        for name, bar_ty in bar_builders(text):
            found = find_new(text, name)
            if found is None:
                skipped["bar builder with no pub fn new"] += 1
                skipped_names.setdefault("bar builder with no pub fn new", []).append(name)
                continue
            argtypes, returns_result = found
            if any(a not in ARG_READER for a in argtypes):
                skipped["bar builder with an unreadable argument"] += 1
                skipped_names.setdefault("bar builder with an unreadable argument", []).append(name)
                continue
            bars.append((name, bar_ty, argtypes, returns_result))
        for m in re.finditer(r"impl\s+Indicator\s+for\s+(\w+)", text):
            ty = m.group(1)
            inp, out = assoc_types(text, ty)
            if inp is None or out is None:
                skipped["no associated types"] += 1
                skipped_names.setdefault("no associated types", []).append(ty)
                continue
            if ty in RETURN_INPUT_ONLY:
                # Routed to the returns family rather than skipped: the terminal
                # has no return to feed directly, but it builds candles, and a
                # close-to-close return is exactly the per-period return these
                # ratios are defined over.
                inp = "returns"
            if inp not in SUPPORTED_INPUTS:
                skipped[f"input {inp}"] += 1
                skipped_names.setdefault(f"input {inp}", []).append(ty)
                continue
            found = find_new(text, ty)
            if found is None:
                skipped["no pub fn new"] += 1
                skipped_names.setdefault("no pub fn new", []).append(ty)
                continue
            argtypes, returns_result = found
            if any(a not in ARG_READER for a in argtypes):
                skipped["unreadable constructor argument"] += 1
                skipped_names.setdefault("unreadable constructor argument", []).append(ty)
                continue
            fields: list[tuple[str, str, str]] = []
            if out in PROFILE_OUTPUTS:
                # Not registered and not skipped: carried by the profile surface
                # below, which returns the histogram whole.
                profiles.append((ty, out, argtypes, returns_result))
                continue
            if out in SCALAR_INT_OUTPUTS:
                pass
            elif out != "f64":
                got = out_fields(bigtext, out)
                if not got:
                    skipped[f"output {out}"] += 1
                    skipped_names.setdefault(f"output {out}", []).append(ty)
                    continue
                fields = got
            entries.append((ty, inp, out, argtypes, returns_result, fields))

    entries.sort(key=lambda e: e[0])

    # (input family, Output struct) pairs that need a generated impl.
    structs: dict[tuple[str, str], list[tuple[str, str, str]]] = {}
    for _, inp, out, _, _, fields in entries:
        if fields:
            structs[(inp, out)] = fields

    arms = []
    for ty, inp, out, argtypes, returns_result, fields in entries:
        scalar_wrapper, field_wrapper = WRAPPERS[inp]
        ctor = f"wc::{ty}::new({readers(argtypes)})" if argtypes else f"wc::{ty}::new()"
        made = f"map_new(kind, {ctor})?" if returns_result else ctor
        extra = "".join(
            f", {name}: pair_reference(kind, reference)?.to_string()"
            if name == "reference"
            else f", {name}: None"
            for name, _ in EXTRA_FIELDS.get(inp, ())
        )
        if fields:
            body = f"Ok(Box::new({field_wrapper} {{ inner: {made}, last: None{extra} }}))"
        elif out in SCALAR_INT_OUTPUTS:
            body = f"Ok(Box::new({scalar_wrapper}Int {{ inner: {made}{extra} }}))"
        else:
            body = f"Ok(Box::new({scalar_wrapper} {{ inner: {made}{extra} }}))"
        arms.append(f'        "{ty}" => {body},')

    for alias, canonical in sorted(ALIASES.items()):
        if any(e[0] == canonical for e in entries):
            arms.append(
                f'        "{alias}" => build_inner("{canonical}", params, reference),'
            )

    names = sorted([e[0] for e in entries] + [a for a in ALIASES if any(e[0] == ALIASES[a] for e in entries)])

    # Default constructor parameters, joined by canonical name from the wickra
    # golden manifest. These are the parameters the library itself pins its
    # reference values with, so the build-all test constructs every indicator the
    # way wickra does rather than with a guessed number.
    manifest_path = Path(args.wickra) / "testdata" / "golden" / "golden_manifest.json"
    defaults: dict[str, list[float]] = {}
    if manifest_path.is_file():
        for row in json.loads(manifest_path.read_text(encoding="utf-8")):
            defaults[row["canonical"]] = [float(v) for v in row.get("params", [])]
    else:
        raise SystemExit(f"error: {manifest_path} not found — it carries the default parameters")

    registered = {e[0] for e in entries}
    missing_defaults = sorted(registered - set(defaults))
    default_rows = []
    for name in sorted(registered & set(defaults)):
        vals = ", ".join(f"{v:?}".replace("?", "") if False else repr(float(v)) for v in defaults[name])
        default_rows.append(f'    ("{name}", &[{vals}]),')

    pairwise = sorted(e[0] for e in entries if e[1] == "(f64,f64)")
    breadth = sorted(e[0] for e in entries if e[1] == "CrossSection")
    int_families = {e[1] for e in entries if e[2] in SCALAR_INT_OUTPUTS}
    families = sorted({e[1] for e in entries})

    alias_rows = chr(10).join(
        f'    ("{alias}", "{canonical}"),'
        for alias, canonical in sorted(ALIASES.items())
        if any(e[0] == canonical for e in entries)
    )
    alias_count = alias_rows.count(chr(10)) + 1 if alias_rows else 0

    build_fn = f"""
/// Every input family this terminal can feed, sorted.
///
/// Named rather than counted: a document that lists the families can be checked
/// against this, and a family added to the registry without a row in that list
/// fails the check instead of leaving the list quietly short.
pub const INPUT_FAMILIES: [&str; {len(families)}] = [
{chr(10).join(f'    "{f}",' for f in families)}
];

/// Every registered indicator name, sorted.
pub const KINDS: [&str; {len(names)}] = [
{chr(10).join(f'    "{n}",' for n in names)}
];

/// Default constructor parameters, taken from the wickra golden manifest — the
/// same values the library pins its own reference outputs with. Used by the
/// build-all test so every registered indicator is constructed the way wickra
/// constructs it, rather than with a guessed parameter count.
pub const DEFAULTS: [(&str, &[f64]); {len(default_rows)}] = [
{chr(10).join(default_rows)}
];

/// The friendly aliases, each paired with the canonical kind it builds.
///
/// Emitted rather than kept only in the generator, because a consumer needs
/// them: `Catalogue::current` used to walk `DEFAULTS`, which holds canonical
/// names only, so both aliases were constructible and invisible to
/// `ListIndicators` -- the discovery surface every binding reads.
pub const ALIASES: [(&str, &str); {alias_count}] = [
{alias_rows}
];

/// Every registered indicator that reads a second market, sorted.
///
/// These are the kinds [`build`] refuses: they need a reference symbol, which is
/// a property of the spec rather than of the parameters. Exposed so a caller can
/// tell a user which indicators need one before they try.
pub const PAIRWISE: [&str; {len(pairwise)}] = [
{chr(10).join(f'    "{n}",' for n in pairwise)}
];

/// Every registered indicator that reads the whole tracked universe, sorted.
pub const CROSS_SECTION: [&str; {len(breadth)}] = [
{chr(10).join(f'    "{n}",' for n in breadth)}
];

/// Whether `kind` reads the universe rather than one market.
///
/// Asked by the state before it borrows a market, because assembling the
/// universe walks every market and so cannot happen while one is borrowed.
/// Unlike a pairwise reference -- which is a field on the spec, and readable
/// without the registry -- nothing in a spec says a kind reads breadth. The
/// registry is the only thing that knows, so it says so.
#[must_use]
pub fn is_cross_section(kind: &str) -> bool {{
    CROSS_SECTION.binary_search(&kind).is_ok()
}}

/// The reference symbol a pairwise indicator was configured with.
fn pair_reference<'a>(kind: &str, reference: Option<&'a str>) -> Result<&'a str> {{
    reference.ok_or_else(|| {{
        Error::Config(format!(
            "{{kind}} compares two markets, so it needs a reference symbol"
        ))
    }})
}}

/// Construct an indicator by name with positional parameters.
///
/// A pairwise indicator is rejected here rather than given a default reference:
/// which market it compares against changes what it measures, so guessing one
/// would produce a plausible number about the wrong thing. Use [`build_paired`].
///
/// # Errors
///
/// Returns [`Error::Config`] if the name is unknown, a parameter is missing or
/// out of range, wickra-core rejects the parameters, or the kind is one of
/// [`PAIRWISE`].
pub fn build(kind: &str, params: &[f64]) -> Result<Box<dyn TickIndicator>> {{
    build_inner(kind, params, None)
}}

/// Construct an indicator that compares this market against `reference`.
///
/// # Errors
///
/// As [`build`]. A kind that is not pairwise ignores the reference rather than
/// failing, so a caller may pass one uniformly.
pub fn build_paired(
    kind: &str,
    params: &[f64],
    reference: &str,
) -> Result<Box<dyn TickIndicator>> {{
    build_inner(kind, params, Some(reference))
}}

fn build_inner(
    kind: &str,
    params: &[f64],
    reference: Option<&str>,
) -> Result<Box<dyn TickIndicator>> {{
    match kind {{
{chr(10).join(arms)}
        _ => Err(Error::Config(format!("unknown indicator: {{kind}}"))),
    }}
}}
"""

    field_families = {family for family, _ in structs}
    # The header carries a table of the families this terminal feeds, and prose
    # around it that counts them. Both were written when there were four, and
    # both were still saying four after the fifth, and then the ninth, were
    # wired -- a generated file describing itself wrongly. The header is prose
    # and cannot be derived, so it is checked instead: one table row per family,
    # or the run stops and says which family has none.
    documented = {
        row.split("|")[1].strip().strip("`")
        for row in HEAD.splitlines()
        if row.startswith("//! | ") and "Fed with" not in row and "---" not in row
    }
    undocumented = {f.replace(" ", "") for f in families} - {
        d.replace(" ", "") for d in documented
    }
    if undocumented:
        raise SystemExit(
            "error: the registry header's input table does not mention "
            + ", ".join(sorted(undocumented))
            + " -- add a row saying what feeds it and when it advances, rather "
            "than shipping a header that under-describes the file"
        )

    text = (
        HEAD
        + emit_scalar_wrappers()
        + emit_int_wrappers(int_families)
        + emit_field_structs(field_families)
        + emit_field_impls(structs)
        + emit_profiles(profiles, defaults)
        + emit_bars(bars, defaults)
        + PARAMS
        + build_fn
    )
    out_path = Path(args.out)
    out_path.write_text(text, encoding="utf-8")

    by_input = Counter(e[1] for e in entries)
    print(f"registered {len(entries)} indicators (+{len(names) - len(entries)} aliases) -> {out_path}")
    for k, v in sorted(by_input.items()):
        print(f"  input {k:8} {v}")
    print(f"  multi-output (family, struct) pairs: {len(structs)}")
    if missing_defaults:
        print(f"  WARNING: {len(missing_defaults)} registered indicators have no manifest defaults: "
              + ", ".join(missing_defaults[:6]))
    if skipped:
        print("\nskipped (not silently — these need work this terminal has not done):")
        for reason, count in skipped.most_common():
            sample = ", ".join(sorted(skipped_names[reason])[:4])
            more = "" if count <= 4 else f", +{count - 4} more"
            print(f"  {count:3}  {reason:32} {sample}{more}")


if __name__ == "__main__":
    main()
