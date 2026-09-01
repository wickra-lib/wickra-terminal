#![no_main]
//! Fuzz the generated registry: construct a registered kind with arbitrary
//! parameters and drive it across all nine input families.
//!
//! The registry is 7,400 generated lines holding one construction arm per
//! indicator, and no fuzz target reached it. `view_model.rs` drives
//! `command_json`, which does accept `AddIndicator` — but only after the fuzzer
//! happens to produce valid JSON naming a real kind, which is never. Picking the
//! kind out of `KINDS` by index gets past that door on the first iteration.
//!
//! Two properties, and they are different properties:
//!
//! 1. `build` must **refuse** rather than panic. Parameters here are arbitrary,
//!    so they include zero periods, negative periods, NaN and enormous
//!    magnitudes — exactly the arms that `registry_completeness` cannot reach,
//!    because it drives `DEFAULTS`, which is by definition the parameters that
//!    work.
//! 2. Once built, `update` must tolerate any *structurally valid* tick. The
//!    values are arbitrary; the shapes are not, because a `Candle` with a low
//!    above its high is rejected by `Candle::new` and never reaches an
//!    indicator in production either. The parsers are fuzzed by their own
//!    targets; this one is about the indicators.

use libfuzzer_sys::fuzz_target;
use wickra_core::{
    Candle, CrossSection, DerivativesTick, Level, Member, OrderBook, Side, Trade, TradeQuote,
};
use wickra_terminal_core::registry::{
    build, build_bars, build_profile, BAR_TYPES, KINDS, PROFILES,
};
use wickra_terminal_core::TickInput;

/// How many ticks one iteration drives. Bounded so a long input does not turn
/// into a timeout the fuzzer reports as a crash.
const MAX_TICKS: usize = 64;

/// Read a `f64` from four bytes, spread across the range that actually finds
/// bugs: small integers (the periods every constructor validates), zero,
/// negatives, and the occasional extreme.
fn param_from(bytes: &[u8]) -> f64 {
    let raw = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
    match raw % 8 {
        0 => 0.0,
        1 => -(f64::from(raw % 512)),
        2 => f64::from(raw % 512),
        3 => f64::NAN,
        4 => f64::INFINITY,
        5 => f64::from(raw) * 1e12,
        6 => f64::from(raw % 64) + 0.5,
        _ => f64::from(raw % 100) + 1.0,
    }
}

/// A price that moves, derived from the input but kept finite and positive.
///
/// Finite and positive on purpose: a price is not what this target varies. Every
/// constructor below rejects a non-finite input, so feeding NaN here would test
/// the constructors' guards over and over and never reach an indicator.
fn price_from(byte: u8, step: usize) -> f64 {
    100.0 + f64::from(byte) + (step as f64) * 0.25
}

/// Build the tick this step feeds, populating every family the terminal can
/// carry. A family whose value fails its own validation is left absent, which is
/// what a real tick looks like when a feed has not spoken yet.
fn tick_at(price: f64, size: f64, step: usize, closed: Option<Candle>) -> TickInput {
    let ts = step as i64;
    let mut input = TickInput::price(price);
    input.candle = closed;

    let side = if step.is_multiple_of(3) {
        Side::Buy
    } else {
        Side::Sell
    };
    input.trade = Trade::new(price, size, side, ts).ok();

    let tick = 0.01;
    let level = |sign: f64, lean: f64| -> Vec<Level> {
        (1..=5)
            .filter_map(|depth| {
                let depth = f64::from(depth);
                Level::new(price + sign * tick * depth, 1.0 + lean + depth).ok()
            })
            .collect()
    };
    let skew = (step % 5) as f64;
    input.book = OrderBook::new(level(-1.0, skew), level(1.0, 4.0 - skew)).ok();

    // A reference market for the pairwise family. `build` refuses a pairwise
    // kind without one, so these are exercised through `build_paired` in the
    // completeness suite; the entry is here so a kind that reads it while not
    // declaring itself pairwise is not silently starved.
    input
        .references
        .insert("ETH/USDT".to_string(), price * 15.0);

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
    input.cross_section = CrossSection::new(members, ts).ok();

    input.derivatives = DerivativesTick::new(
        0.0001 * ((step % 20) as f64 - 10.0),
        price * 1.0008,
        price,
        price * 1.0015,
        1_000_000.0 + (step as f64) * 1_000.0,
        600_000.0,
        400_000.0,
        900.0 + (step as f64),
        900.0,
        if step.is_multiple_of(17) {
            25_000.0
        } else {
            0.0
        },
        if step.is_multiple_of(23) {
            18_000.0
        } else {
            0.0
        },
        ts,
    )
    .ok();

    // The mid the print arrived against, one tick off the trade price so the
    // effective spread is not identically zero.
    let mid = if step.is_multiple_of(2) {
        price - tick
    } else {
        price + tick
    };
    input.trade_quote = input
        .trade
        .and_then(|print| TradeQuote::new(print, mid).ok());

    input
}

fuzz_target!(|data: &[u8]| {
    // Two bytes select the kind, four give a parameter, and the rest is the
    // tick stream. Anything shorter cannot say what to build.
    if data.len() < 8 {
        return;
    }
    let selector = usize::from(u16::from_le_bytes([data[0], data[1]]));
    let params: Vec<f64> = data[2..].chunks_exact(4).take(4).map(param_from).collect();
    let stream = &data[2 + params.len() * 4..];
    if stream.is_empty() {
        return;
    }

    // One selector reaches all three surfaces, so a single corpus entry can land
    // on an indicator, a profile or a bar builder.
    let total = KINDS.len() + PROFILES.len() + BAR_TYPES.len();
    let index = selector % total;

    let mut indicator = if index < KINDS.len() {
        // `build` refuses a pairwise kind outright -- that refusal is the
        // documented behaviour and is asserted in the completeness suite, so an
        // Err here is a pass, not a miss.
        match build(KINDS[index], &params) {
            Ok(built) => Some(built),
            Err(_) => return,
        }
    } else if index < KINDS.len() + PROFILES.len() {
        let (kind, _) = PROFILES[index - KINDS.len()];
        // Profiles and bar builders answer with a histogram and a bar rather
        // than a number, so they are driven for panics alone: constructing them
        // with arbitrary parameters is the half that has never been fuzzed.
        let _ = build_profile(kind, &params);
        None
    } else {
        let (kind, _) = BAR_TYPES[index - KINDS.len() - PROFILES.len()];
        let _ = build_bars(kind, &params);
        None
    };

    let Some(indicator) = indicator.as_mut() else {
        return;
    };

    // Candles are built from the stream rather than from a bar builder: the
    // aggregation path has its own coverage, and driving it here would mean most
    // iterations closed no bar at all and never reached a candle indicator.
    for (step, window) in stream.chunks(6).take(MAX_TICKS).enumerate() {
        let price = price_from(window[0], step);
        let size = f64::from(window.get(1).copied().unwrap_or(1)) + 1.0;
        let closed = if window.len() == 6 {
            let base = price_from(window[2], step);
            let span = f64::from(window[3]) * 0.1;
            Candle::new(
                base,
                base + span,
                base - span,
                base + span * 0.5,
                f64::from(window[4]) + 1.0,
                step as i64,
            )
            .ok()
        } else {
            None
        };
        let input = tick_at(price, size, step, closed);
        let _ = indicator.update(&input);
        let _ = indicator.fields();
        let _ = indicator.warmup();
    }
});
