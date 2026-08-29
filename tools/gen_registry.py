#!/usr/bin/env python3
"""Generate crates/wickra-terminal-core/src/registry.rs.

Single source of truth: the wickra-core indicator sources themselves
(crates/wickra-core/src/indicators/*.rs). For every type that implements the
`Indicator` trait we read, directly from the source:

  - the associated `type Input` and `type Output`
  - the `pub [const] fn new(...) -> Result<Self> | Self` constructor signature
  - for multi-output indicators, the `f64` field names of the Output struct

What gets registered, and why only this much:

  Input = f64      fed the last traded price, tick by tick.
  Input = Candle   fed each bar as it closes, from the CandleBuilder. Only closed
                   bars: feeding the bar in progress would make every reading
                   repaint as the bar fills.

Everything else is skipped and reported. `(f64, f64)` needs a reference symbol,
`OrderBook` and `Trade` need the terminal's book and tape converted to the core's
types, and `DerivativesTick`, `CrossSection` and `TradeQuote` need feeds this
repository does not have at all. Those are separate steps, not silent omissions:
the run prints what it skipped and why.

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
}

SUPPORTED_INPUTS = set(WRAPPERS)

# Indicators whose `Input = f64` is a per-period RETURN, not a price.
#
# wickra-core says so in their own docs -- "Input is treated as a per-period
# return", "over the trailing window of `period` returns" -- and the terminal has
# only a price to give them. Fed a price, every input looks like a gain, the
# denominator is zero and they return `inf` for every reading: measured across
# 400 varied prices, finite=0 and non-finite=1161.
#
# So they are skipped rather than registered, the same call P4.3d made for
# `Footprint`: an indicator that cannot produce a meaningful value from what this
# terminal can feed it does not belong in the catalogue. Reaching them properly
# needs a returns input family, which is a feature rather than a fix.
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
}

# Extra state a wrapper carries beyond the indicator itself. Only the pairwise
# family needs any: it has to remember which market its second input comes from,
# because that is a property of the spec rather than of the tick.
EXTRA_FIELDS = {
    "(f64,f64)": (("reference", "String"),),
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
    "(f64,f64)": (
        "input"
        + chr(10)
        + "            .reference(&self.reference)"
        + chr(10)
        + "            .and_then(|other| self.inner.update((input.price, other)))"
    ),
}


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
//! when its input is one of the four families this terminal can feed and its
//! output is a scalar `f64` or a struct of `f64` fields:
//!
//! | `Input`     | Fed with                                  | Advances     |
//! |-------------|-------------------------------------------|--------------|
//! | `f64`       | the last trade price                      | every trade  |
//! | `Candle`    | the bar the tick just closed              | every bar    |
//! | `Trade`     | the print, with size and aggressor side   | every trade  |
//! | `OrderBook` | the locally maintained L2 book            | every trade  |
//!
//! Multi-output indicators expose their fields by name.

use std::collections::BTreeMap;

use wickra_core::{self as wc, Candle, Indicator};

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
    "(f64,f64)": (
        "Wraps a pairwise (`Input = (f64, f64)`) single-output indicator: this",
        "market's price against a reference market's. Ticks on which the reference",
        "has not printed yet yield `None` without advancing it.",
    ),
}

# Prose for the struct-output wrappers, mirroring WRAPPER_DOC.
FIELD_WRAPPER_DOC = {
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
        self.inner.warmup_period()
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
        self.inner.warmup_period()
    }}{wants_book(family)}{wants_reference(family)}
}}
"""
        )
    return "".join(out)


PARAMS = '''
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
    skipped = Counter()
    skipped_names: dict[str, list[str]] = {}

    for path in sorted(indicators.glob("*.rs")):
        text = path.read_text(encoding="utf-8")
        for m in re.finditer(r"impl\s+Indicator\s+for\s+(\w+)", text):
            ty = m.group(1)
            inp, out = assoc_types(text, ty)
            if inp is None or out is None:
                skipped["no associated types"] += 1
                skipped_names.setdefault("no associated types", []).append(ty)
                continue
            if ty in RETURN_INPUT_ONLY:
                skipped["input is a return, not a price"] += 1
                skipped_names.setdefault("input is a return, not a price", []).append(ty)
                continue
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
            if out != "f64":
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
            else ""
            for name, _ in EXTRA_FIELDS.get(inp, ())
        )
        if fields:
            body = f"Ok(Box::new({field_wrapper} {{ inner: {made}, last: None{extra} }}))"
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

    alias_rows = chr(10).join(
        f'    ("{alias}", "{canonical}"),'
        for alias, canonical in sorted(ALIASES.items())
        if any(e[0] == canonical for e in entries)
    )
    alias_count = alias_rows.count(chr(10)) + 1 if alias_rows else 0

    build_fn = f"""
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
    text = (
        HEAD
        + emit_scalar_wrappers()
        + emit_field_structs(field_families)
        + emit_field_impls(structs)
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
