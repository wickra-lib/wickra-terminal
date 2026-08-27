#!/usr/bin/env python3
"""Generate crates/terminal-core/src/registry.rs.

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
        --out crates/terminal-core/src/registry.rs
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
}

# The input families this terminal can feed today.
SUPPORTED_INPUTS = {"f64", "Candle"}


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


def out_fields(text: str, out: str) -> list[str] | None:
    """The `pub <name>: f64` field names of an Output struct."""
    m = re.search(r"pub\s+struct\s+" + re.escape(out) + r"\s*\{(.*?)\n\}", text, re.S)
    if not m:
        return None
    return re.findall(r"pub\s+(\w+)\s*:\s*f64\b", m.group(1))


def readers(argtypes: list[str]) -> str:
    return ", ".join(ARG_READER[t].format(i=i) for i, t in enumerate(argtypes))


HEAD = '''//! Indicator registry: constructs `wickra-core` indicators by name and wraps
//! them behind a uniform, object-safe [`TickIndicator`] the terminal can drive
//! from one tick.
//!
//! GENERATED FILE — do not edit by hand. Regenerate with:
//!
//! ```text
//! python tools/gen_registry.py --wickra ../wickra --out crates/terminal-core/src/registry.rs
//! cargo fmt --all
//! ```
//!
//! Source of truth: the wickra-core indicator sources — the `Indicator` impls,
//! their `new` signatures and their Output structs. Every indicator whose input
//! is a price (`Input = f64`, fed the last trade) or a bar (`Input = Candle`, fed
//! each bar as it closes) is registered, with a scalar `f64` or an all-`f64`-field
//! struct output. Multi-output indicators expose their fields by name.

use wickra_core::{self as wc, Candle, Indicator};

use crate::error::{Error, Result};

/// What an indicator may consume on one tick.
///
/// `price` is always present — it is the last trade or ticker price. `candle` is
/// `Some` only on the tick that closed a bar, which is why bar indicators advance
/// once per bar rather than once per trade.
#[derive(Debug, Clone, Copy)]
pub struct TickInput {
    /// The last traded price.
    pub price: f64,
    /// The bar that just closed, if this tick closed one.
    pub candle: Option<Candle>,
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
}

/// Wraps a price (`Input = f64`) single-output indicator.
struct ScalarPrice<I>(I);

impl<I> TickIndicator for ScalarPrice<I>
where
    I: Indicator<Input = f64, Output = f64> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        self.0.update(input.price)
    }
    fn fields(&self) -> Vec<(&'static str, f64)> {
        Vec::new()
    }
    fn warmup(&self) -> usize {
        self.0.warmup_period()
    }
}

/// Wraps a bar (`Input = Candle`) single-output indicator. Ticks that did not
/// close a bar yield `None` without advancing it.
struct CandleIn<I>(I);

impl<I> TickIndicator for CandleIn<I>
where
    I: Indicator<Input = Candle, Output = f64> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        input.candle.and_then(|c| self.0.update(c))
    }
    fn fields(&self) -> Vec<(&'static str, f64)> {
        Vec::new()
    }
    fn warmup(&self) -> usize {
        self.0.warmup_period()
    }
}
'''

FIELD_WRAPPER = '''
/// Wraps a price indicator whose output is a struct of `f64` fields. The primary
/// value is the first field; every field is reachable by name.
struct ScalarPriceFields<I, O> {{
    inner: I,
    last: Option<O>,
}}

/// Wraps a bar indicator whose output is a struct of `f64` fields.
struct CandleInFields<I, O> {{
    inner: I,
    last: Option<O>,
}}
'''


def emit_field_impls(structs: dict[str, list[str]]) -> str:
    """One `TickIndicator` impl per multi-output Output struct.

    A blanket impl cannot reach the fields: they are named differently on every
    struct and there is no trait exposing them, so the impls are generated.
    """
    out = []
    for struct, fields in sorted(structs.items()):
        pairs = ", ".join(f'("{f}", last.{f})' for f in fields)
        primary = fields[0]
        for wrapper, input_expr in (
            ("ScalarPriceFields", "self.inner.update(input.price)"),
            ("CandleInFields", "input.candle.and_then(|c| self.inner.update(c))"),
        ):
            out.append(
                f"""
impl<I> TickIndicator for {wrapper}<I, wc::{struct}>
where
    I: Indicator<Input = {"f64" if wrapper.startswith("Scalar") else "Candle"}, Output = wc::{struct}> + Send,
{{
    fn update(&mut self, input: &TickInput) -> Option<f64> {{
        let out = {input_expr};
        self.last = out;
        self.last.as_ref().map(|last| last.{primary})
    }}
    fn fields(&self) -> Vec<(&'static str, f64)> {{
        self.last
            .as_ref()
            .map(|last| vec![{pairs}])
            .unwrap_or_default()
    }}
    fn warmup(&self) -> usize {{
        self.inner.warmup_period()
    }}
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
            fields: list[str] = []
            if out != "f64":
                got = out_fields(bigtext, out)
                if not got:
                    skipped[f"output {out}"] += 1
                    skipped_names.setdefault(f"output {out}", []).append(ty)
                    continue
                fields = got
            entries.append((ty, inp, out, argtypes, returns_result, fields))

    entries.sort(key=lambda e: e[0])

    # Output structs that need a generated impl.
    structs: dict[str, list[str]] = {}
    for _, _, out, _, _, fields in entries:
        if fields:
            structs[out] = fields

    arms = []
    for ty, inp, out, argtypes, returns_result, fields in entries:
        if fields:
            wrapper = "ScalarPriceFields" if inp == "f64" else "CandleInFields"
            ctor = f"wc::{ty}::new({readers(argtypes)})" if argtypes else f"wc::{ty}::new()"
            made = f"map_new(kind, {ctor})?" if returns_result else ctor
            body = f"Ok(Box::new({wrapper} {{ inner: {made}, last: None }}))"
        else:
            wrapper = "ScalarPrice" if inp == "f64" else "CandleIn"
            ctor = f"wc::{ty}::new({readers(argtypes)})" if argtypes else f"wc::{ty}::new()"
            made = f"map_new(kind, {ctor})?" if returns_result else ctor
            body = f"Ok(Box::new({wrapper}({made})))"
        arms.append(f'        "{ty}" => {body},')

    for alias, canonical in sorted(ALIASES.items()):
        if any(e[0] == canonical for e in entries):
            arms.append(f'        "{alias}" => build("{canonical}", params),')

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

/// Construct an indicator by name with positional parameters.
///
/// # Errors
///
/// Returns [`Error::Config`] if the name is unknown, a parameter is missing or
/// out of range, or wickra-core rejects the parameters.
pub fn build(kind: &str, params: &[f64]) -> Result<Box<dyn TickIndicator>> {{
    match kind {{
{chr(10).join(arms)}
        _ => Err(Error::Config(format!("unknown indicator: {{kind}}"))),
    }}
}}
"""

    text = HEAD + FIELD_WRAPPER.format() + emit_field_impls(structs) + PARAMS + build_fn
    out_path = Path(args.out)
    out_path.write_text(text, encoding="utf-8")

    by_input = Counter(e[1] for e in entries)
    print(f"registered {len(entries)} indicators (+{len(names) - len(entries)} aliases) -> {out_path}")
    for k, v in sorted(by_input.items()):
        print(f"  input {k:8} {v}")
    print(f"  multi-output structs: {len(structs)}")
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
