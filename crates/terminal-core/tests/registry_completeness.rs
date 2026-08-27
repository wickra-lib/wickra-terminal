//! Every registered indicator constructs and runs.
//!
//! `registry.rs` is generated, which moves the risk: a hand-written dispatch
//! fails to compile when it is wrong, but a generated one compiles happily with
//! an arm that constructs the wrong thing, or one that no input can ever satisfy.
//! Nothing else in the suite touches more than a couple of the 460 arms, so this
//! drives all of them.
//!
//! Parameters come from `DEFAULTS`, which the generator joins in from wickra's
//! own golden manifest — the values the library pins its reference outputs with,
//! rather than a guessed count that would make half the failures spurious.

use terminal_core::registry::{build, build_paired, DEFAULTS, KINDS, PAIRWISE};
use terminal_core::{CandleBuilder, TickInput, Timeframe};
use wickra_core::{Candle, Level, OrderBook, Side, Trade};

/// The market a pairwise indicator is compared against in this suite.
const REFERENCE: &str = "ETH/USDT";

/// Construct any registered kind, pairing the ones that need a second market.
///
/// `build` deliberately refuses a pairwise kind rather than defaulting its
/// reference, so a suite that drives every arm has to say which market it means.
fn build_any(kind: &str, params: &[f64]) -> Box<dyn terminal_core::TickIndicator> {
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
fn multi_output_indicators_expose_their_fields() {
    // MACD is the canonical multi-output shape: a line, a signal and a histogram.
    let (values, fields) = drive("MacdIndicator", &[12.0, 26.0, 9.0], 400);
    assert!(values > 0, "MACD produced no value");
    assert!(fields > 0, "MACD exposed no named fields");

    let mut macd = build("MacdIndicator", &[12.0, 26.0, 9.0]).unwrap();
    let mut builder = CandleBuilder::new(Timeframe::parse(BAR_SPACING).unwrap());
    for bar in 0..400 {
        for trade in 0..TRADES_PER_BAR {
            let step = bar * TRADES_PER_BAR + trade;
            let price = price_at(step) + intrabar_offset(trade);
            let ts = bar * BAR_MS + trade * (BAR_MS / TRADES_PER_BAR);
            let closed = builder.update(price, 1.0, ts);
            let mut tick = TickInput::price(price);
            tick.candle = closed;
            macd.update(&tick);
        }
    }
    let names: Vec<&str> = macd.fields().iter().map(|(n, _)| *n).collect();
    assert!(!names.is_empty(), "MACD exposed no fields after warmup");
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
    let _: &dyn terminal_core::TickIndicator = mixed[0].as_ref();
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
const REGISTERED_FLOOR: usize = 460;

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
