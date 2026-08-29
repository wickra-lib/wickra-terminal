//! Every registered indicator constructs and runs.
//!
//! `registry.rs` is generated, which moves the risk: a hand-written dispatch
//! fails to compile when it is wrong, but a generated one compiles happily with
//! an arm that constructs the wrong thing, or one that no input can ever satisfy.
//! Nothing else in the suite touches more than a couple of the registry's arms,
//! so this drives all of them.
//!
//! Parameters come from `DEFAULTS`, which the generator joins in from wickra's
//! own golden manifest — the values the library pins its reference outputs with,
//! rather than a guessed count that would make half the failures spurious.

use std::collections::BTreeSet;

use wickra_core::{
    Candle, CrossSection, DerivativesTick, Level, Member, OrderBook, Side, Trade, TradeQuote,
};
use wickra_terminal_core::registry::{build, build_paired, DEFAULTS, KINDS, PAIRWISE};
use wickra_terminal_core::{CandleBuilder, TickInput, Timeframe};

/// The market a pairwise indicator is compared against in this suite.
const REFERENCE: &str = "ETH/USDT";

/// Construct any registered kind, pairing the ones that need a second market.
///
/// `build` deliberately refuses a pairwise kind rather than defaulting its
/// reference, so a suite that drives every arm has to say which market it means.
fn build_any(kind: &str, params: &[f64]) -> Box<dyn wickra_terminal_core::TickIndicator> {
    let built = if PAIRWISE.contains(&kind) {
        build_paired(kind, params, REFERENCE)
    } else {
        build(kind, params)
    };
    built.unwrap_or_else(|err| panic!("{kind}: {err}"))
}

/// The reference market's price path.
///
/// Deliberately not a scaled copy of `price_at`: two series in lockstep have a
/// correlation of exactly one and a spread of exactly zero, which is a
/// degenerate input for the whole pairwise family -- cointegration, beta and the
/// spread bands would all report a constant, and a mis-wired indicator would be
/// indistinguishable from a correct one. A different level, amplitude and phase
/// gives a real relationship to measure.
fn reference_at(step: i64) -> f64 {
    let t = step as f64;
    1500.0 + 200.0 * (t * 0.05 + 0.9).sin() + 25.0 * (t * 0.55).sin()
}

/// The mid a print arrived against, alternating which side the print crosses.
///
/// The book fixture is centred on the same price the trade prints at, so
/// taking its mid literally would make the effective spread exactly zero on
/// every tick -- and a family that measures how far a print landed from the
/// mid has nothing to measure. Offsetting by one tick, alternating, is what a
/// real tape looks like: the mid stands, and the print takes the ask or hits
/// the bid.
fn mid_at(price: f64, step: i64) -> f64 {
    let tick = 0.01;
    if step % 2 == 0 {
        price - tick
    } else {
        price + tick
    }
}

/// A derivatives tick that moves, for the perpetual-futures family.
///
/// Every field varies, and each because a constant one silences something:
/// a flat funding rate leaves `FundingRateZScore` dividing a zero variance,
/// a mark price pinned to the index leaves `PerpetualPremiumIndex` reporting
/// zero forever, and equal taker volumes make `TakerBuySellRatio` a constant
/// one. The prices sit near the price path so a basis is a plausible size
/// rather than an arbitrage nobody would believe.
fn derivatives_at(step: i64) -> DerivativesTick {
    let t = step as f64;
    let index = price_at(step);
    // A premium that changes sign, so the basis family sees both.
    let mark = index * (1.0 + 0.0008 * (t * 0.07).sin());
    let futures = index * (1.0 + 0.0015 * (t * 0.03).cos());
    DerivativesTick::new(
        0.0001 * (t * 0.11).sin(),
        mark,
        index,
        futures,
        1_000_000.0 + 50_000.0 * (t * 0.05).sin(),
        600_000.0 + 40_000.0 * (t * 0.09).sin(),
        400_000.0 + 40_000.0 * (t * 0.09).cos(),
        900.0 + 300.0 * (t * 0.13).sin(),
        900.0 + 300.0 * (t * 0.13).cos(),
        // Liquidations are a flow, and mostly zero: a venue does not force
        // one every tick, and a family reading them should survive the gaps.
        if step % 17 == 0 { 25_000.0 } else { 0.0 },
        if step % 23 == 0 { 18_000.0 } else { 0.0 },
        step,
    )
    .expect("finite funding and positive mark/index/futures prices")
}

/// A synthetic universe for the breadth family.
///
/// Five markets whose direction and flags both turn over as the step
/// advances. Every property here was chosen because a simpler universe makes
/// some member of the family degenerate rather than wrong, which is the
/// harder failure to notice:
///
/// A universe where every market advances leaves `Trin` dividing declining
/// volume that is always zero, and `AdvanceDecline` monotone. Both directions
/// have to occur.
///
/// Flags that never turn off leave `HighLowIndex` and `PercentAboveMa`
/// reporting a constant, which a mis-wired reading is indistinguishable from.
///
/// Volume varies with the step so the volume-weighted members of the family
/// (`AdVolumeLine`, `UpDownVolumeRatio`, `CumulativeVolumeIndex`) measure
/// something rather than counting markets a second time.
fn cross_section_at(step: i64) -> CrossSection {
    let members = (0..5)
        .map(|market| {
            let phase = (step + market) % 4;
            let magnitude = 1.0 + market as f64;
            Member::with_signals(
                if phase < 2 { magnitude } else { -magnitude },
                100.0 + (step % 7) as f64 * 13.0 + market as f64,
                phase == 0,
                phase == 2,
                phase < 3,
                phase == 1,
            )
        })
        .collect();
    CrossSection::new(members, step).expect("five members, all finite and non-negative")
}

/// A price path with genuine variation, on two timescales.
///
/// Three properties matter, and each was found by an indicator going silent:
///
/// A geometric path has constant log-returns, which drives variance to zero and
/// makes a whole family report a degenerate value.
///
/// The slow component gives long directional runs. Tom DeMark setups need nine
/// consecutive closes on the same side of the close four bars earlier; a single
/// fast oscillation reverses long before that, so `TdLines` and `TdRiskLevel`
/// never completed a setup and looked broken.
///
/// The fast component gives local extremes. Fractal indicators need a bar whose
/// high tops its neighbours on both sides; a smooth curve sampled coarsely has
/// too few, so `AndrewsPitchfork` and `FractalChaosBands` found no pivots.
fn price_at(step: i64) -> f64 {
    let t = step as f64;
    100.0 + 30.0 * (t * 0.05).sin() + 3.0 * (t * 0.7).sin()
}

/// Trades per bar. One trade per bar makes every candle a doji — open, high, low
/// and close all identical, so the bar has no range at all. Indicators built on
/// the open-to-close relationship (RVI, and Inertia on top of it) then divide by
/// a zero range, and fractal indicators (Andrews Pitchfork, Fractal Chaos Bands)
/// find no pivots because there are no highs or lows to pivot on. Four trades per
/// bar gives a real body and real wicks.
const TRADES_PER_BAR: i64 = 4;

/// Bars are spaced a day apart. A calendar indicator — average daily range, the
/// overnight gap, turn-of-month — only fires when the input actually crosses a
/// day boundary; second-spaced bars keep the whole run inside one day and those
/// indicators correctly report nothing.
const BAR_SPACING: &str = "1d";

/// Trade sizes are scaled by this, not offset by it.
///
/// VPIN closes a bucket on cumulative volume — 5000 per bucket across 10 buckets
/// at its manifest defaults, so 50,000 of volume before its first value. At one
/// to seven units a trade the whole run traded about 6400 and VPIN was silent,
/// which reads exactly like a dead arm.
///
/// Scaling keeps the seven-fold spread between the smallest and largest trade
/// that the volume oscillators need; adding a constant would have cleared the
/// same threshold while flattening every size into a narrow band.
const VOLUME_SCALE: f64 = 10.0;
const BAR_MS: i64 = 86_400_000;

/// Feed `bars` bars, each built from several trades, and count what came back.
fn drive(kind: &str, params: &[f64], bars: i64) -> (usize, usize) {
    let mut indicator = build_any(kind, params);
    let mut builder = CandleBuilder::new(Timeframe::parse(BAR_SPACING).unwrap());
    let mut values = 0;
    let mut fields = 0;

    for bar in 0..bars {
        for trade in 0..TRADES_PER_BAR {
            let step = bar * TRADES_PER_BAR + trade;
            // Spread the trades across the bar so it has a genuine high and low.
            let price = price_at(step) + intrabar_offset(trade);
            let ts = bar * BAR_MS + trade * (BAR_MS / TRADES_PER_BAR);
            let size = VOLUME_SCALE * (1.0 + (step % 7) as f64);
            let closed = builder.update(price, size, ts);
            let mut input = TickInput::price(price);
            input.candle = closed;
            input.trade = Some(trade_at(price, size, step, ts));
            input.book = Some(book_at(price, step));
            input
                .references
                .insert(REFERENCE.to_string(), reference_at(step));
            input.cross_section = Some(cross_section_at(step));
            input.derivatives = Some(derivatives_at(step));
            input.trade_quote = input
                .trade
                .and_then(|print| TradeQuote::new(print, mid_at(price, step)).ok());
            if indicator.update(&input).is_some() {
                values += 1;
            }
            if !indicator.fields().is_empty() {
                fields += 1;
            }
        }
    }
    (values, fields)
}

/// A print at this price, with a side that follows the price rather than simply
/// alternating.
///
/// A strictly alternating side makes the sign sequence perfectly anti-correlated,
/// which is a degenerate input for `TradeSignAutocorrelation` and hides a wiring
/// fault behind a value that looks real. Following the price gives runs of the
/// same side, which is what a tape actually looks like.
fn trade_at(price: f64, size: f64, step: i64, ts: i64) -> Trade {
    let side = if price >= price_at(step - 1) {
        Side::Buy
    } else {
        Side::Sell
    };
    Trade::new(price, size, side, ts).expect("synthetic trade is valid by construction")
}

/// A five-deep book around this price, with sizes that vary by step.
///
/// The sizes have to move: the imbalance family divides one side's depth by the
/// other, so a book with the same shape on every tick reports one constant and a
/// mis-wired indicator would be indistinguishable from a correct one.
fn book_at(price: f64, step: i64) -> OrderBook {
    let tick = 0.01;
    let skew = (step % 5) as f64;
    let side = |sign: f64, lean: f64| -> Vec<Level> {
        (1..=5)
            .map(|depth| {
                let level = f64::from(depth);
                Level::new(price + sign * tick * level, 1.0 + lean + level)
                    .expect("synthetic level is valid by construction")
            })
            .collect()
    };
    OrderBook::new(side(-1.0, skew), side(1.0, 4.0 - skew))
        .expect("synthetic book is valid by construction")
}

/// Move the price around within a bar so open, high, low and close differ.
fn intrabar_offset(trade: i64) -> f64 {
    match trade {
        0 => 0.0,
        1 => 1.5,
        2 => -1.5,
        _ => 0.5,
    }
}

#[test]
fn every_registered_kind_has_default_parameters() {
    let defaults: std::collections::HashSet<&str> = DEFAULTS.iter().map(|(k, _)| *k).collect();
    // KINDS carries the two friendly aliases as well; those resolve to a
    // canonical entry rather than carrying their own defaults.
    let aliases = ["Macd", "Bollinger"];
    let missing: Vec<&str> = KINDS
        .iter()
        .copied()
        .filter(|k| !defaults.contains(k) && !aliases.contains(k))
        .collect();
    assert!(missing.is_empty(), "no default parameters for: {missing:?}");
}

#[test]
fn every_registered_indicator_constructs() {
    let mut failures = Vec::new();
    for (kind, params) in DEFAULTS {
        let built = if PAIRWISE.contains(&kind) {
            build_paired(kind, params, REFERENCE)
        } else {
            build(kind, params)
        };
        if let Err(err) = built {
            failures.push(format!("{kind}: {err}"));
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} indicators failed to construct:\n  {}",
        failures.len(),
        DEFAULTS.len(),
        failures.join("\n  ")
    );
}

#[test]
fn every_registered_indicator_produces_a_value() {
    // 400 bars clears every warmup in the set with room to spare.
    let mut silent = Vec::new();
    for (kind, params) in DEFAULTS {
        let (values, _) = drive(kind, params, 400);
        if values == 0 {
            silent.push(kind);
        }
    }
    assert!(
        silent.is_empty(),
        "{} indicators never produced a value, so their wiring is dead:\n  {}",
        silent.len(),
        silent.join(", ")
    );
}

#[test]
fn no_indicator_ever_reports_a_non_finite_value() {
    // `serde_json` writes inf and NaN as `null`. Inside `Option<f64>` that is
    // indistinguishable from "warming up"; inside the series `Vec<f64>` it is
    // JSON that the frame's own `Deserialize` rejects, and that a Go
    // `[]float64` or a C# `double[]` cannot hold at all. The frame is the
    // cross-language contract, so a non-finite reading must never reach it.
    //
    // The wrappers filter it, and this drives every arm to keep them filtering.
    // The golden corpus cannot cover this: it exercises three indicators.
    let mut leaked = Vec::new();
    for (kind, params) in DEFAULTS {
        let mut indicator = build_any(kind, params);
        let mut builder = CandleBuilder::new(Timeframe::parse(BAR_SPACING).unwrap());
        for bar in 0..200_i64 {
            for trade in 0..TRADES_PER_BAR {
                let step = bar * TRADES_PER_BAR + trade;
                let price = price_at(step) + intrabar_offset(trade);
                let size = VOLUME_SCALE * (1.0 + (step % 7) as f64);
                let ts = bar * BAR_MS + trade * (BAR_MS / TRADES_PER_BAR);
                let mut input = TickInput::price(price);
                input.candle = builder.update(price, size, ts);
                input.trade = Some(trade_at(price, size, step, ts));
                input.book = Some(book_at(price, step));
                input
                    .references
                    .insert(REFERENCE.to_string(), reference_at(step));

                if let Some(value) = indicator.update(&input) {
                    if !value.is_finite() {
                        leaked.push(format!("{kind} value={value}"));
                    }
                }
                for (name, value) in indicator.fields() {
                    if !value.is_finite() {
                        leaked.push(format!("{kind}.{name}={value}"));
                    }
                }
            }
        }
    }
    leaked.sort();
    leaked.dedup();
    assert!(
        leaked.is_empty(),
        "{} non-finite readings reached the frame:
  {}",
        leaked.len(),
        leaked.join(
            "
  "
        )
    );
}

/// Every registered indicator whose output is a struct, with the field names that
/// struct declares in wickra-core.
///
/// Read from the library sources, not from the registry this checks, which is the
/// whole point: the registry is generated, so asking it what fields it has and
/// then asserting it has them proves nothing. `VolumeProfile` and `TpoProfile`
/// shipped for four phases reporting `price_low` -- a price -- under a profile's
/// name, because the only multi-output assertion in this suite drove MACD and
/// asked whether the field list was non-empty.
const STRUCT_OUTPUT_FIELDS: [(&str, &[&str]); 92] = [
    ("AccelerationBands", &["upper", "middle", "lower"]),
    ("Adx", &["plus_di", "minus_di", "adx"]),
    ("Alligator", &["jaw", "teeth", "lips"]),
    ("AndrewsPitchfork", &["median", "upper", "lower"]),
    ("Aroon", &["up", "down"]),
    ("AtrBands", &["upper", "middle", "lower"]),
    ("AtrRatchet", &["value", "direction"]),
    (
        "AutoFib",
        &[
            "level_0",
            "level_236",
            "level_382",
            "level_500",
            "level_618",
            "level_786",
            "level_1000",
        ],
    ),
    ("BollingerBands", &["upper", "middle", "lower", "stddev"]),
    ("BomarBands", &["upper", "middle", "lower"]),
    (
        "Camarilla",
        &["pp", "r1", "r2", "r3", "r4", "s1", "s2", "s3", "s4"],
    ),
    ("CandleVolume", &["body", "width"]),
    ("CentralPivotRange", &["pivot", "tc", "bc"]),
    ("ChandeKrollStop", &["stop_long", "stop_short"]),
    ("ChandelierExit", &["long_stop", "short_stop"]),
    ("ClassicPivots", &["pp", "r1", "r2", "r3", "s1", "s2", "s3"]),
    ("Cointegration", &["hedge_ratio", "spread", "adf_stat"]),
    ("CompositeProfile", &["poc", "vah", "val"]),
    ("DemarkPivots", &["pp", "r1", "s1"]),
    ("Donchian", &["upper", "middle", "lower"]),
    ("DonchianStop", &["stop_long", "stop_short"]),
    (
        "DoubleBollinger",
        &[
            "upper_outer",
            "upper_inner",
            "middle",
            "lower_inner",
            "lower_outer",
        ],
    ),
    ("ElderRay", &["bull_power", "bear_power"]),
    ("ElderSafeZone", &["value", "direction"]),
    ("Equivolume", &["height", "width"]),
    ("FibArcs", &["arc_382", "arc_500", "arc_618"]),
    (
        "FibChannel",
        &["base", "level_618", "level_1000", "level_1618"],
    ),
    ("FibConfluence", &["price", "strength"]),
    (
        "FibExtension",
        &[
            "level_1272",
            "level_1414",
            "level_1618",
            "level_2000",
            "level_2618",
        ],
    ),
    ("FibFan", &["fan_382", "fan_500", "fan_618"]),
    (
        "FibProjection",
        &["level_618", "level_1000", "level_1618", "level_2618"],
    ),
    (
        "FibRetracement",
        &[
            "level_0",
            "level_236",
            "level_382",
            "level_500",
            "level_618",
            "level_786",
            "level_1000",
        ],
    ),
    ("FibTimeZones", &["on_zone", "bars_to_next"]),
    (
        "FibonacciPivots",
        &["pp", "r1", "r2", "r3", "s1", "s2", "s3"],
    ),
    ("FractalChaosBands", &["upper", "lower"]),
    ("GatorOscillator", &["upper", "lower"]),
    ("GoldenPocket", &["low", "mid", "high"]),
    ("HeikinAshi", &["open", "high", "low", "close"]),
    ("HighLowVolumeNodes", &["hvn", "lvn"]),
    ("HtPhasor", &["inphase", "quadrature"]),
    ("HurstChannel", &["upper", "middle", "lower"]),
    (
        "Ichimoku",
        &["tenkan", "kijun", "senkou_a", "senkou_b", "chikou"],
    ),
    ("InitialBalance", &["high", "low"]),
    ("KalmanHedgeRatio", &["hedge_ratio", "intercept", "spread"]),
    ("KaseDevStop", &["value", "direction"]),
    ("KasePermissionStochastic", &["fast", "slow"]),
    ("Keltner", &["upper", "middle", "lower"]),
    ("Kst", &["kst", "signal"]),
    ("LeadLagCrossCorrelation", &["lag", "correlation"]),
    ("LinRegChannel", &["upper", "middle", "lower"]),
    (
        "LiquidationFeatures",
        &["long", "short", "net", "total", "imbalance"],
    ),
    ("MaEnvelope", &["upper", "middle", "lower"]),
    ("MacdExt", &["macd", "signal", "histogram"]),
    ("MacdFix", &["macd", "signal", "histogram"]),
    ("MacdIndicator", &["macd", "signal", "histogram"]),
    ("Mama", &["mama", "fama"]),
    ("MedianChannel", &["upper", "middle", "lower"]),
    ("ModifiedMaStop", &["value", "direction"]),
    (
        "MurreyMathLines",
        &[
            "mm8_8", "mm7_8", "mm6_8", "mm5_8", "mm4_8", "mm3_8", "mm2_8", "mm1_8", "mm0_8",
        ],
    ),
    ("Nrtr", &["value", "direction"]),
    ("OpeningRange", &["high", "low", "breakout_distance"]),
    ("OvernightIntradayReturn", &["overnight", "intraday"]),
    ("ProjectionBands", &["upper", "middle", "lower"]),
    ("Qqe", &["rsi_ma", "trailing_line"]),
    ("QuartileBands", &["upper", "middle", "lower"]),
    ("RelativeStrengthAB", &["ratio", "ratio_ma", "ratio_rsi"]),
    ("Rwi", &["high", "low"]),
    ("SessionHighLow", &["high", "low"]),
    ("SessionRange", &["asia", "eu", "us"]),
    ("SmoothedHeikinAshi", &["open", "high", "low", "close"]),
    (
        "SpreadBollingerBands",
        &["middle", "upper", "lower", "percent_b"],
    ),
    ("StandardErrorBands", &["upper", "middle", "lower"]),
    ("StarcBands", &["upper", "middle", "lower"]),
    ("Stochastic", &["k", "d"]),
    ("SuperTrend", &["value", "direction"]),
    ("TdLines", &["resistance", "support"]),
    ("TdMovingAverage", &["st1", "st2"]),
    ("TdRangeProjection", &["high", "low"]),
    ("TdRiskLevel", &["buy_risk", "sell_risk"]),
    ("TdSequential", &["setup", "countdown", "direction"]),
    ("TtmSqueeze", &["squeeze", "momentum"]),
    ("ValueArea", &["poc", "vah", "val"]),
    (
        "VolatilityCone",
        &["current", "min", "median", "max", "percentile"],
    ),
    ("VolumeWeightedMacd", &["macd", "signal", "histogram"]),
    ("VolumeWeightedSr", &["support", "resistance"]),
    ("Vortex", &["plus", "minus"]),
    ("VwapStdDevBands", &["upper", "middle", "lower", "stddev"]),
    ("WaveTrend", &["wt1", "wt2"]),
    ("WilliamsFractals", &["up", "down"]),
    ("WoodiePivots", &["pp", "r1", "r2", "s1", "s2"]),
    ("ZeroLagMacd", &["macd", "signal", "histogram"]),
    ("ZigZag", &["swing", "direction"]),
];

/// The last reading, the field list from that tick, and every field name the
/// indicator exposed at any point in the run.
///
/// The two lists differ once an output field is optional. `Ichimoku` publishes
/// five lines and `WilliamsFractals` two, and neither shows all of them on every
/// bar -- a bar carries an up fractal or a down one, rarely both. A field with no
/// value is left out rather than reported as some stand-in number, so the set on
/// the final bar is a subset of what the struct declares, while the union over
/// the run is the whole of it.
fn last_reading(
    kind: &str,
    params: &[f64],
    bars: i64,
) -> (Option<f64>, Vec<&'static str>, BTreeSet<&'static str>) {
    let mut indicator = build_any(kind, params);
    let mut builder = CandleBuilder::new(Timeframe::parse(BAR_SPACING).unwrap());
    let mut value = None;
    let mut names = Vec::new();
    let mut seen = BTreeSet::new();

    for bar in 0..bars {
        for trade in 0..TRADES_PER_BAR {
            let step = bar * TRADES_PER_BAR + trade;
            let price = price_at(step) + intrabar_offset(trade);
            let ts = bar * BAR_MS + trade * (BAR_MS / TRADES_PER_BAR);
            let size = VOLUME_SCALE * (1.0 + (step % 7) as f64);
            let closed = builder.update(price, size, ts);
            let mut input = TickInput::price(price);
            input.candle = closed;
            input.trade = Some(trade_at(price, size, step, ts));
            input.book = Some(book_at(price, step));
            input
                .references
                .insert(REFERENCE.to_string(), reference_at(step));
            input.cross_section = Some(cross_section_at(step));
            input.derivatives = Some(derivatives_at(step));
            input.trade_quote = input
                .trade
                .and_then(|print| TradeQuote::new(print, mid_at(price, step)).ok());
            // Fields are read on every tick, not only on the ticks that produce a
            // reading. The reading is the FIRST field, so when that one field is
            // optional and absent the indicator returns None while still holding
            // the others -- `WilliamsFractals` on a bar carrying a down fractal
            // and no up one. Gating the read on the reading hid exactly the case
            // an optional field exists for.
            if let Some(latest) = indicator.update(&input) {
                value = Some(latest);
            }
            let current: Vec<&'static str> =
                indicator.fields().iter().map(|(name, _)| *name).collect();
            if !current.is_empty() {
                seen.extend(current.iter().copied());
                // The richest tick, not the last one. Order can only be checked
                // where more than one field is present, and the final bar of an
                // indicator with optional fields often carries just one.
                if current.len() > names.len() {
                    names = current;
                }
            }
        }
    }
    (value, names, seen)
}

#[test]
fn every_multi_output_indicator_exposes_every_field_its_output_declares() {
    let mut checked = 0;
    for (kind, expected) in STRUCT_OUTPUT_FIELDS {
        let Some((_, params)) = DEFAULTS.iter().find(|(k, _)| *k == kind) else {
            panic!("{kind} has a struct output but is not registered");
        };
        let (value, names, seen) = last_reading(kind, params, 400);
        assert!(value.is_some(), "{kind} produced no reading in 400 bars");
        let declared: BTreeSet<&str> = expected.iter().copied().collect();
        assert_eq!(
            seen, declared,
            "{kind} never exposed some field its Output struct declares"
        );
        // And in the declared order: the richest tick's field list must be a
        // subsequence of it, which is what a reordering generator would break.
        // A one-field list is a subsequence of any order, so this only bites
        // where a tick carried several -- which is every indicator here whose
        // fields are not mutually exclusive.
        let positions: Vec<Option<usize>> = names
            .iter()
            .map(|name| expected.iter().position(|e| e == name))
            .collect();
        assert!(
            positions.iter().all(Option::is_some),
            "{kind} exposes {names:?}, which is not all declared in {expected:?}"
        );
        assert!(
            positions.windows(2).all(|pair| pair[0] < pair[1]),
            "{kind} exposes {names:?}, which is not in the declared order {expected:?}"
        );
        checked += 1;
    }
    assert_eq!(
        checked,
        STRUCT_OUTPUT_FIELDS.len(),
        "not every struct-output indicator was checked"
    );

    // And the converse, or the table would be a list of what already passes: an
    // indicator that starts exposing fields without being added here would slip
    // through the check above by simply not being in it.
    let listed: BTreeSet<&str> = STRUCT_OUTPUT_FIELDS.iter().map(|(kind, _)| *kind).collect();
    for (kind, params) in DEFAULTS {
        let (_, names, _) = last_reading(kind, params, 400);
        assert!(
            names.is_empty() || listed.contains(kind),
            "{kind} exposes fields but is not in STRUCT_OUTPUT_FIELDS; add it with the           field names its wickra-core Output struct declares"
        );
    }
}

#[test]
fn a_multi_output_reading_is_its_first_field() {
    // The documented contract: the value a frame shows for a multi-output
    // indicator is the first field of its output, and `fields` carries the rest.
    // Nothing asserted it, so a generator that reordered them would be silent.
    for (kind, _) in STRUCT_OUTPUT_FIELDS {
        let Some((_, params)) = DEFAULTS.iter().find(|(k, _)| *k == kind) else {
            panic!("{kind} is not registered");
        };
        let mut indicator = build_any(kind, params);
        let mut builder = CandleBuilder::new(Timeframe::parse(BAR_SPACING).unwrap());
        for bar in 0..400 {
            for trade in 0..TRADES_PER_BAR {
                let step = bar * TRADES_PER_BAR + trade;
                let price = price_at(step) + intrabar_offset(trade);
                let ts = bar * BAR_MS + trade * (BAR_MS / TRADES_PER_BAR);
                let size = VOLUME_SCALE * (1.0 + (step % 7) as f64);
                let closed = builder.update(price, size, ts);
                let mut input = TickInput::price(price);
                input.candle = closed;
                input.trade = Some(trade_at(price, size, step, ts));
                input.book = Some(book_at(price, step));
                input
                    .references
                    .insert(REFERENCE.to_string(), reference_at(step));
                if let Some(value) = indicator.update(&input) {
                    let fields = indicator.fields();
                    // Bit equality, not a tolerance: the reading and the first
                    // field are the same expression, so anything but the identical
                    // value means they stopped being the same field.
                    assert_eq!(
                        fields[0].1.to_bits(),
                        value.to_bits(),
                        "{kind} does not report its first field"
                    );
                }
            }
        }
    }
}

#[test]
fn aliases_resolve_to_their_canonical_indicator() {
    for (alias, canonical) in [("Macd", "MacdIndicator"), ("Bollinger", "BollingerBands")] {
        let Some((_, params)) = DEFAULTS.iter().find(|(k, _)| *k == canonical) else {
            panic!("{canonical} is not registered");
        };
        let via_alias = build(alias, params);
        assert!(via_alias.is_ok(), "{alias} did not resolve");
        assert_eq!(
            via_alias.unwrap().warmup(),
            build(canonical, params).unwrap().warmup(),
            "{alias} and {canonical} disagree"
        );
    }
}

#[test]
fn an_unknown_kind_is_a_config_error() {
    // `unwrap_err` would need `Box<dyn TickIndicator>: Debug`; requiring Debug on
    // the trait just to phrase a test would be the tail wagging the dog.
    let Err(err) = build("NotAnIndicator", &[]) else {
        panic!("an unknown kind was accepted");
    };
    assert!(
        err.to_string().contains("unknown indicator"),
        "unexpected error: {err}"
    );
}

#[test]
fn a_missing_parameter_names_the_indicator() {
    // Sma takes a period; with no parameters the error must say which indicator
    // and which position, not just fail.
    let Err(err) = build("Sma", &[]) else {
        panic!("Sma was constructed with no period");
    };
    let err = err.to_string();
    assert!(
        err.contains("Sma"),
        "error does not name the indicator: {err}"
    );
    assert!(err.contains('0'), "error does not name the position: {err}");
}

#[test]
fn bar_indicators_do_not_advance_on_a_tick_without_a_bar() {
    // Atr consumes candles. Ticks that close no bar must leave it untouched,
    // otherwise a busy market would warm it up faster than a quiet one.
    let mut atr = build("Atr", &[14.0]).unwrap();
    for step in 0..1_000_i64 {
        let value = atr.update(&TickInput::price(price_at(step)));
        assert!(value.is_none(), "Atr advanced on a tick that closed no bar");
    }
}

#[test]
fn price_indicators_advance_on_every_tick() {
    let mut sma = build("Sma", &[5.0]).unwrap();
    let mut seen = None;
    for step in 0..20_i64 {
        seen = sma.update(&TickInput::price(price_at(step)));
    }
    assert!(
        seen.is_some(),
        "Sma produced nothing from twenty prices with no bars"
    );
}

#[test]
fn the_registry_is_object_safe_in_a_heterogeneous_collection() {
    let mixed: Vec<_> = ["Sma", "Ema", "Atr", "Rsi", "MacdIndicator"]
        .iter()
        .map(|kind| {
            let params: &[f64] = DEFAULTS
                .iter()
                .find(|(k, _)| k == kind)
                .map_or(&[], |(_, p)| *p);
            build(kind, params).unwrap_or_else(|err| panic!("{kind}: {err}"))
        })
        .collect();
    assert_eq!(mixed.len(), 5);
    let _: &dyn wickra_terminal_core::TickIndicator = mixed[0].as_ref();
}

#[test]
fn a_candle_indicator_reads_the_bar_not_the_price() {
    // Feed a bar whose close is nowhere near the tick price: a candle indicator
    // must report from the bar, proving the wrapper does not quietly fall back to
    // `input.price`.
    let mut atr = build("Atr", &[2.0]).unwrap();
    let mut last = None;
    for step in 0..10_i64 {
        let bar = Candle::new_unchecked(10.0, 12.0, 8.0, 11.0, 1.0, step * 1_000);
        last = atr.update(&TickInput::price(99_999.0).with_candle(bar));
    }
    let value = last.expect("Atr produced no value from ten bars");
    assert!(
        value > 0.0 && value < 100.0,
        "Atr read the tick price instead of the bar: {value}"
    );
}

/// The registry is regenerated from a sibling checkout, which is the one way it
/// can shrink without anyone noticing: point the generator at an older or
/// partial wickra tree and it emits a smaller file that still compiles and whose
/// every entry still passes every test above.
///
/// The number is a floor rather than an equality, so adding indicators upstream
/// does not fail the build; only losing them does. Raise it when a regeneration
/// legitimately grows the set.
///
/// Raised from 455 to 458: the generator learned three shapes it could not read
/// before -- `Option<f64>` output fields (`Ichimoku`, `WilliamsFractals`) and a
/// `MaType` constructor argument (`MacdExt`).
///
/// It was lowered from 457 to 455 before that, on purpose: `VolumeProfile` and
/// `TpoProfile` were registered with only the two prices from an output whose
/// third field is the variable-length bin list the profile exists for, so each
/// reported `price_low` under a profile's name. They stay skipped like
/// `Footprint`, whose output is the same shape. See `docs/INDICATORS.md`.
const REGISTERED_FLOOR: usize = 495;

#[test]
fn the_registry_has_not_silently_shrunk() {
    assert!(
        DEFAULTS.len() >= REGISTERED_FLOOR,
        "the registry has {} indicators, down from {REGISTERED_FLOOR}. If a          regeneration dropped them on purpose, lower the floor in this test and          say why; otherwise the generator was pointed at the wrong tree.",
        DEFAULTS.len()
    );
}

#[test]
fn every_input_family_is_represented() {
    // A generator run that silently lost one family would still leave a large,
    // healthy-looking registry: the other families would carry every test above.
    // So each family gets an indicator here, and each must advance on the tick
    // that carries its input and stay put on the one that does not.
    let mut price = build("Sma", &[5.0]).unwrap();
    let mut bar = build("Atr", &[5.0]).unwrap();
    let mut tape = build("SignedVolume", &[]).unwrap();
    let mut book = build("Microprice", &[]).unwrap();
    let mut pair = build_paired("PairwiseBeta", &[14.0], REFERENCE).unwrap();

    // A bare price tick: only the price family may move on it.
    let bare = TickInput::price(100.0);
    for _ in 0..20 {
        price.update(&bare);
        bar.update(&bare);
        tape.update(&bare);
        book.update(&bare);
        pair.update(&bare);
    }
    assert!(
        price.update(&bare).is_some(),
        "no price-input indicator advanced: the f64 family is missing"
    );
    assert!(
        bar.update(&bare).is_none(),
        "a bar indicator advanced without a bar: the Candle family is mis-wired"
    );
    assert!(
        tape.update(&bare).is_none(),
        "a tape indicator advanced without a print: the Trade family is mis-wired"
    );
    assert!(
        book.update(&bare).is_none(),
        "a book indicator advanced without a book: the OrderBook family is mis-wired"
    );
    assert!(
        pair.update(&bare).is_none(),
        "a pairwise indicator advanced with no reference price: the (f64, f64) family is mis-wired"
    );

    // Now a tick that carries a print and a book.
    let full = TickInput::price(100.0)
        .with_trade(trade_at(100.0, 2.0, 1, 0))
        .with_book(book_at(100.0, 0));
    assert!(
        tape.update(&full).is_some(),
        "the Trade family did not advance on a tick carrying a print"
    );
    assert!(
        book.update(&full).is_some(),
        "the OrderBook family did not advance on a tick carrying a book"
    );

    // The pairwise family needs a reference to have moved, so drive it a while.
    let mut advanced = None;
    for step in 0..40_i64 {
        let tick = TickInput::price(price_at(step)).with_reference(REFERENCE, reference_at(step));
        advanced = pair.update(&tick).or(advanced);
    }
    assert!(
        advanced.is_some(),
        "the pairwise family did not advance on ticks carrying a reference price"
    );
}

#[test]
fn a_pairwise_indicator_is_refused_without_a_reference() {
    // Defaulting the reference would produce a plausible number about the wrong
    // pair, which is worse than refusing: the error has to name the indicator
    // and say what is missing.
    assert!(!PAIRWISE.is_empty(), "no pairwise kinds registered");
    let kind = PAIRWISE[0];
    // The kind's real parameters, so the refusal is about the missing reference
    // and not about a parameter list this test happened to get wrong.
    let params: &[f64] = DEFAULTS
        .iter()
        .find(|(k, _)| *k == kind)
        .map_or(&[], |(_, p)| *p);
    let Err(err) = build(kind, params) else {
        panic!("{kind} was built with no reference market");
    };
    let err = err.to_string();
    assert!(
        err.contains(PAIRWISE[0]),
        "error does not name the kind: {err}"
    );
    assert!(
        err.contains("reference"),
        "error does not say a reference is missing: {err}"
    );
}

#[test]
fn a_pairwise_indicator_reads_the_reference_it_was_given() {
    // Two references, same primary price path. If the wrapper ignored its own
    // reference and read whatever the tick happened to carry, both would report
    // the same thing.
    //
    // The second series has its own frequency and phase rather than being the
    // first one scaled: DistanceSsd normalises, so a pure scale factor cancels
    // exactly and two references that differ only in units genuinely do produce
    // the same reading. That would make this test pass for the wrong reason if
    // it were wired correctly, and fail for the wrong reason if it were not.
    let mut against_eth = build_paired("DistanceSsd", &[14.0], "ETH/USDT").unwrap();
    let mut against_sol = build_paired("DistanceSsd", &[14.0], "SOL/USDT").unwrap();
    let mut eth = None;
    let mut sol = None;
    for step in 0..40_i64 {
        let other = 800.0 + 120.0 * ((step as f64) * 0.11 + 2.1).sin();
        let tick = TickInput::price(price_at(step))
            .with_reference("ETH/USDT", reference_at(step))
            .with_reference("SOL/USDT", other);
        eth = against_eth.update(&tick).or(eth);
        sol = against_sol.update(&tick).or(sol);
    }
    let eth = eth.expect("no reading against ETH");
    let sol = sol.expect("no reading against SOL");
    assert!(
        (eth - sol).abs() > 1e-9,
        "both references produced {eth}, so the wrapper is not reading its own"
    );
}

#[test]
fn multi_output_indicators_survived_the_generation() {
    // The field impls are generated per Output struct, so losing them would leave
    // every multi-output indicator reporting a value and no fields at all.
    let with_fields = DEFAULTS
        .iter()
        .filter(|(kind, params)| {
            let (_, fields) = drive(kind, params, 300);
            fields > 0
        })
        .count();
    assert!(
        with_fields >= 60,
        "only {with_fields} indicators reported named fields; the multi-output          wrappers look lost"
    );
}
