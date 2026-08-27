//! Indicator registry: constructs `wickra-core` indicators by name and wraps
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

/// Wraps a price indicator whose output is a struct of `f64` fields. The primary
/// value is the first field; every field is reachable by name.
struct ScalarPriceFields<I, O> {
    inner: I,
    last: Option<O>,
}

/// Wraps a bar indicator whose output is a struct of `f64` fields.
struct CandleInFields<I, O> {
    inner: I,
    last: Option<O>,
}

impl<I> TickIndicator for ScalarPriceFields<I, wc::AccelerationBandsOutput>
where
    I: Indicator<Input = f64, Output = wc::AccelerationBandsOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = self.inner.update(input.price);
        self.last = out;
        self.last.as_ref().map(|last| last.upper)
    }
    fn fields(&self) -> Vec<(&'static str, f64)> {
        self.last
            .as_ref()
            .map(|last| {
                vec![
                    ("upper", last.upper),
                    ("middle", last.middle),
                    ("lower", last.lower),
                ]
            })
            .unwrap_or_default()
    }
    fn warmup(&self) -> usize {
        self.inner.warmup_period()
    }
}

impl<I> TickIndicator for CandleInFields<I, wc::AccelerationBandsOutput>
where
    I: Indicator<Input = Candle, Output = wc::AccelerationBandsOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = input.candle.and_then(|c| self.inner.update(c));
        self.last = out;
        self.last.as_ref().map(|last| last.upper)
    }
    fn fields(&self) -> Vec<(&'static str, f64)> {
        self.last
            .as_ref()
            .map(|last| {
                vec![
                    ("upper", last.upper),
                    ("middle", last.middle),
                    ("lower", last.lower),
                ]
            })
            .unwrap_or_default()
    }
    fn warmup(&self) -> usize {
        self.inner.warmup_period()
    }
}

impl<I> TickIndicator for ScalarPriceFields<I, wc::AdxOutput>
where
    I: Indicator<Input = f64, Output = wc::AdxOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = self.inner.update(input.price);
        self.last = out;
        self.last.as_ref().map(|last| last.plus_di)
    }
    fn fields(&self) -> Vec<(&'static str, f64)> {
        self.last
            .as_ref()
            .map(|last| {
                vec![
                    ("plus_di", last.plus_di),
                    ("minus_di", last.minus_di),
                    ("adx", last.adx),
                ]
            })
            .unwrap_or_default()
    }
    fn warmup(&self) -> usize {
        self.inner.warmup_period()
    }
}

impl<I> TickIndicator for CandleInFields<I, wc::AdxOutput>
where
    I: Indicator<Input = Candle, Output = wc::AdxOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = input.candle.and_then(|c| self.inner.update(c));
        self.last = out;
        self.last.as_ref().map(|last| last.plus_di)
    }
    fn fields(&self) -> Vec<(&'static str, f64)> {
        self.last
            .as_ref()
            .map(|last| {
                vec![
                    ("plus_di", last.plus_di),
                    ("minus_di", last.minus_di),
                    ("adx", last.adx),
                ]
            })
            .unwrap_or_default()
    }
    fn warmup(&self) -> usize {
        self.inner.warmup_period()
    }
}

impl<I> TickIndicator for ScalarPriceFields<I, wc::AlligatorOutput>
where
    I: Indicator<Input = f64, Output = wc::AlligatorOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = self.inner.update(input.price);
        self.last = out;
        self.last.as_ref().map(|last| last.jaw)
    }
    fn fields(&self) -> Vec<(&'static str, f64)> {
        self.last
            .as_ref()
            .map(|last| {
                vec![
                    ("jaw", last.jaw),
                    ("teeth", last.teeth),
                    ("lips", last.lips),
                ]
            })
            .unwrap_or_default()
    }
    fn warmup(&self) -> usize {
        self.inner.warmup_period()
    }
}

impl<I> TickIndicator for CandleInFields<I, wc::AlligatorOutput>
where
    I: Indicator<Input = Candle, Output = wc::AlligatorOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = input.candle.and_then(|c| self.inner.update(c));
        self.last = out;
        self.last.as_ref().map(|last| last.jaw)
    }
    fn fields(&self) -> Vec<(&'static str, f64)> {
        self.last
            .as_ref()
            .map(|last| {
                vec![
                    ("jaw", last.jaw),
                    ("teeth", last.teeth),
                    ("lips", last.lips),
                ]
            })
            .unwrap_or_default()
    }
    fn warmup(&self) -> usize {
        self.inner.warmup_period()
    }
}

impl<I> TickIndicator for ScalarPriceFields<I, wc::AndrewsPitchforkOutput>
where
    I: Indicator<Input = f64, Output = wc::AndrewsPitchforkOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = self.inner.update(input.price);
        self.last = out;
        self.last.as_ref().map(|last| last.median)
    }
    fn fields(&self) -> Vec<(&'static str, f64)> {
        self.last
            .as_ref()
            .map(|last| {
                vec![
                    ("median", last.median),
                    ("upper", last.upper),
                    ("lower", last.lower),
                ]
            })
            .unwrap_or_default()
    }
    fn warmup(&self) -> usize {
        self.inner.warmup_period()
    }
}

impl<I> TickIndicator for CandleInFields<I, wc::AndrewsPitchforkOutput>
where
    I: Indicator<Input = Candle, Output = wc::AndrewsPitchforkOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = input.candle.and_then(|c| self.inner.update(c));
        self.last = out;
        self.last.as_ref().map(|last| last.median)
    }
    fn fields(&self) -> Vec<(&'static str, f64)> {
        self.last
            .as_ref()
            .map(|last| {
                vec![
                    ("median", last.median),
                    ("upper", last.upper),
                    ("lower", last.lower),
                ]
            })
            .unwrap_or_default()
    }
    fn warmup(&self) -> usize {
        self.inner.warmup_period()
    }
}

impl<I> TickIndicator for ScalarPriceFields<I, wc::AroonOutput>
where
    I: Indicator<Input = f64, Output = wc::AroonOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = self.inner.update(input.price);
        self.last = out;
        self.last.as_ref().map(|last| last.up)
    }
    fn fields(&self) -> Vec<(&'static str, f64)> {
        self.last
            .as_ref()
            .map(|last| vec![("up", last.up), ("down", last.down)])
            .unwrap_or_default()
    }
    fn warmup(&self) -> usize {
        self.inner.warmup_period()
    }
}

impl<I> TickIndicator for CandleInFields<I, wc::AroonOutput>
where
    I: Indicator<Input = Candle, Output = wc::AroonOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = input.candle.and_then(|c| self.inner.update(c));
        self.last = out;
        self.last.as_ref().map(|last| last.up)
    }
    fn fields(&self) -> Vec<(&'static str, f64)> {
        self.last
            .as_ref()
            .map(|last| vec![("up", last.up), ("down", last.down)])
            .unwrap_or_default()
    }
    fn warmup(&self) -> usize {
        self.inner.warmup_period()
    }
}

impl<I> TickIndicator for ScalarPriceFields<I, wc::AtrBandsOutput>
where
    I: Indicator<Input = f64, Output = wc::AtrBandsOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = self.inner.update(input.price);
        self.last = out;
        self.last.as_ref().map(|last| last.upper)
    }
    fn fields(&self) -> Vec<(&'static str, f64)> {
        self.last
            .as_ref()
            .map(|last| {
                vec![
                    ("upper", last.upper),
                    ("middle", last.middle),
                    ("lower", last.lower),
                ]
            })
            .unwrap_or_default()
    }
    fn warmup(&self) -> usize {
        self.inner.warmup_period()
    }
}

impl<I> TickIndicator for CandleInFields<I, wc::AtrBandsOutput>
where
    I: Indicator<Input = Candle, Output = wc::AtrBandsOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = input.candle.and_then(|c| self.inner.update(c));
        self.last = out;
        self.last.as_ref().map(|last| last.upper)
    }
    fn fields(&self) -> Vec<(&'static str, f64)> {
        self.last
            .as_ref()
            .map(|last| {
                vec![
                    ("upper", last.upper),
                    ("middle", last.middle),
                    ("lower", last.lower),
                ]
            })
            .unwrap_or_default()
    }
    fn warmup(&self) -> usize {
        self.inner.warmup_period()
    }
}

impl<I> TickIndicator for ScalarPriceFields<I, wc::AtrRatchetOutput>
where
    I: Indicator<Input = f64, Output = wc::AtrRatchetOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = self.inner.update(input.price);
        self.last = out;
        self.last.as_ref().map(|last| last.value)
    }
    fn fields(&self) -> Vec<(&'static str, f64)> {
        self.last
            .as_ref()
            .map(|last| vec![("value", last.value), ("direction", last.direction)])
            .unwrap_or_default()
    }
    fn warmup(&self) -> usize {
        self.inner.warmup_period()
    }
}

impl<I> TickIndicator for CandleInFields<I, wc::AtrRatchetOutput>
where
    I: Indicator<Input = Candle, Output = wc::AtrRatchetOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = input.candle.and_then(|c| self.inner.update(c));
        self.last = out;
        self.last.as_ref().map(|last| last.value)
    }
    fn fields(&self) -> Vec<(&'static str, f64)> {
        self.last
            .as_ref()
            .map(|last| vec![("value", last.value), ("direction", last.direction)])
            .unwrap_or_default()
    }
    fn warmup(&self) -> usize {
        self.inner.warmup_period()
    }
}

impl<I> TickIndicator for ScalarPriceFields<I, wc::AutoFibOutput>
where
    I: Indicator<Input = f64, Output = wc::AutoFibOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = self.inner.update(input.price);
        self.last = out;
        self.last.as_ref().map(|last| last.level_0)
    }
    fn fields(&self) -> Vec<(&'static str, f64)> {
        self.last
            .as_ref()
            .map(|last| {
                vec![
                    ("level_0", last.level_0),
                    ("level_236", last.level_236),
                    ("level_382", last.level_382),
                    ("level_500", last.level_500),
                    ("level_618", last.level_618),
                    ("level_786", last.level_786),
                    ("level_1000", last.level_1000),
                ]
            })
            .unwrap_or_default()
    }
    fn warmup(&self) -> usize {
        self.inner.warmup_period()
    }
}

impl<I> TickIndicator for CandleInFields<I, wc::AutoFibOutput>
where
    I: Indicator<Input = Candle, Output = wc::AutoFibOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = input.candle.and_then(|c| self.inner.update(c));
        self.last = out;
        self.last.as_ref().map(|last| last.level_0)
    }
    fn fields(&self) -> Vec<(&'static str, f64)> {
        self.last
            .as_ref()
            .map(|last| {
                vec![
                    ("level_0", last.level_0),
                    ("level_236", last.level_236),
                    ("level_382", last.level_382),
                    ("level_500", last.level_500),
                    ("level_618", last.level_618),
                    ("level_786", last.level_786),
                    ("level_1000", last.level_1000),
                ]
            })
            .unwrap_or_default()
    }
    fn warmup(&self) -> usize {
        self.inner.warmup_period()
    }
}

impl<I> TickIndicator for ScalarPriceFields<I, wc::BollingerOutput>
where
    I: Indicator<Input = f64, Output = wc::BollingerOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = self.inner.update(input.price);
        self.last = out;
        self.last.as_ref().map(|last| last.upper)
    }
    fn fields(&self) -> Vec<(&'static str, f64)> {
        self.last
            .as_ref()
            .map(|last| {
                vec![
                    ("upper", last.upper),
                    ("middle", last.middle),
                    ("lower", last.lower),
                    ("stddev", last.stddev),
                ]
            })
            .unwrap_or_default()
    }
    fn warmup(&self) -> usize {
        self.inner.warmup_period()
    }
}

impl<I> TickIndicator for CandleInFields<I, wc::BollingerOutput>
where
    I: Indicator<Input = Candle, Output = wc::BollingerOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = input.candle.and_then(|c| self.inner.update(c));
        self.last = out;
        self.last.as_ref().map(|last| last.upper)
    }
    fn fields(&self) -> Vec<(&'static str, f64)> {
        self.last
            .as_ref()
            .map(|last| {
                vec![
                    ("upper", last.upper),
                    ("middle", last.middle),
                    ("lower", last.lower),
                    ("stddev", last.stddev),
                ]
            })
            .unwrap_or_default()
    }
    fn warmup(&self) -> usize {
        self.inner.warmup_period()
    }
}

impl<I> TickIndicator for ScalarPriceFields<I, wc::BomarBandsOutput>
where
    I: Indicator<Input = f64, Output = wc::BomarBandsOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = self.inner.update(input.price);
        self.last = out;
        self.last.as_ref().map(|last| last.upper)
    }
    fn fields(&self) -> Vec<(&'static str, f64)> {
        self.last
            .as_ref()
            .map(|last| {
                vec![
                    ("upper", last.upper),
                    ("middle", last.middle),
                    ("lower", last.lower),
                ]
            })
            .unwrap_or_default()
    }
    fn warmup(&self) -> usize {
        self.inner.warmup_period()
    }
}

impl<I> TickIndicator for CandleInFields<I, wc::BomarBandsOutput>
where
    I: Indicator<Input = Candle, Output = wc::BomarBandsOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = input.candle.and_then(|c| self.inner.update(c));
        self.last = out;
        self.last.as_ref().map(|last| last.upper)
    }
    fn fields(&self) -> Vec<(&'static str, f64)> {
        self.last
            .as_ref()
            .map(|last| {
                vec![
                    ("upper", last.upper),
                    ("middle", last.middle),
                    ("lower", last.lower),
                ]
            })
            .unwrap_or_default()
    }
    fn warmup(&self) -> usize {
        self.inner.warmup_period()
    }
}

impl<I> TickIndicator for ScalarPriceFields<I, wc::CamarillaPivotsOutput>
where
    I: Indicator<Input = f64, Output = wc::CamarillaPivotsOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = self.inner.update(input.price);
        self.last = out;
        self.last.as_ref().map(|last| last.pp)
    }
    fn fields(&self) -> Vec<(&'static str, f64)> {
        self.last
            .as_ref()
            .map(|last| {
                vec![
                    ("pp", last.pp),
                    ("r1", last.r1),
                    ("r2", last.r2),
                    ("r3", last.r3),
                    ("r4", last.r4),
                    ("s1", last.s1),
                    ("s2", last.s2),
                    ("s3", last.s3),
                    ("s4", last.s4),
                ]
            })
            .unwrap_or_default()
    }
    fn warmup(&self) -> usize {
        self.inner.warmup_period()
    }
}

impl<I> TickIndicator for CandleInFields<I, wc::CamarillaPivotsOutput>
where
    I: Indicator<Input = Candle, Output = wc::CamarillaPivotsOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = input.candle.and_then(|c| self.inner.update(c));
        self.last = out;
        self.last.as_ref().map(|last| last.pp)
    }
    fn fields(&self) -> Vec<(&'static str, f64)> {
        self.last
            .as_ref()
            .map(|last| {
                vec![
                    ("pp", last.pp),
                    ("r1", last.r1),
                    ("r2", last.r2),
                    ("r3", last.r3),
                    ("r4", last.r4),
                    ("s1", last.s1),
                    ("s2", last.s2),
                    ("s3", last.s3),
                    ("s4", last.s4),
                ]
            })
            .unwrap_or_default()
    }
    fn warmup(&self) -> usize {
        self.inner.warmup_period()
    }
}

impl<I> TickIndicator for ScalarPriceFields<I, wc::CandleVolumeOutput>
where
    I: Indicator<Input = f64, Output = wc::CandleVolumeOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = self.inner.update(input.price);
        self.last = out;
        self.last.as_ref().map(|last| last.body)
    }
    fn fields(&self) -> Vec<(&'static str, f64)> {
        self.last
            .as_ref()
            .map(|last| vec![("body", last.body), ("width", last.width)])
            .unwrap_or_default()
    }
    fn warmup(&self) -> usize {
        self.inner.warmup_period()
    }
}

impl<I> TickIndicator for CandleInFields<I, wc::CandleVolumeOutput>
where
    I: Indicator<Input = Candle, Output = wc::CandleVolumeOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = input.candle.and_then(|c| self.inner.update(c));
        self.last = out;
        self.last.as_ref().map(|last| last.body)
    }
    fn fields(&self) -> Vec<(&'static str, f64)> {
        self.last
            .as_ref()
            .map(|last| vec![("body", last.body), ("width", last.width)])
            .unwrap_or_default()
    }
    fn warmup(&self) -> usize {
        self.inner.warmup_period()
    }
}

impl<I> TickIndicator for ScalarPriceFields<I, wc::CentralPivotRangeOutput>
where
    I: Indicator<Input = f64, Output = wc::CentralPivotRangeOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = self.inner.update(input.price);
        self.last = out;
        self.last.as_ref().map(|last| last.pivot)
    }
    fn fields(&self) -> Vec<(&'static str, f64)> {
        self.last
            .as_ref()
            .map(|last| vec![("pivot", last.pivot), ("tc", last.tc), ("bc", last.bc)])
            .unwrap_or_default()
    }
    fn warmup(&self) -> usize {
        self.inner.warmup_period()
    }
}

impl<I> TickIndicator for CandleInFields<I, wc::CentralPivotRangeOutput>
where
    I: Indicator<Input = Candle, Output = wc::CentralPivotRangeOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = input.candle.and_then(|c| self.inner.update(c));
        self.last = out;
        self.last.as_ref().map(|last| last.pivot)
    }
    fn fields(&self) -> Vec<(&'static str, f64)> {
        self.last
            .as_ref()
            .map(|last| vec![("pivot", last.pivot), ("tc", last.tc), ("bc", last.bc)])
            .unwrap_or_default()
    }
    fn warmup(&self) -> usize {
        self.inner.warmup_period()
    }
}

impl<I> TickIndicator for ScalarPriceFields<I, wc::ChandeKrollStopOutput>
where
    I: Indicator<Input = f64, Output = wc::ChandeKrollStopOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = self.inner.update(input.price);
        self.last = out;
        self.last.as_ref().map(|last| last.stop_long)
    }
    fn fields(&self) -> Vec<(&'static str, f64)> {
        self.last
            .as_ref()
            .map(|last| {
                vec![
                    ("stop_long", last.stop_long),
                    ("stop_short", last.stop_short),
                ]
            })
            .unwrap_or_default()
    }
    fn warmup(&self) -> usize {
        self.inner.warmup_period()
    }
}

impl<I> TickIndicator for CandleInFields<I, wc::ChandeKrollStopOutput>
where
    I: Indicator<Input = Candle, Output = wc::ChandeKrollStopOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = input.candle.and_then(|c| self.inner.update(c));
        self.last = out;
        self.last.as_ref().map(|last| last.stop_long)
    }
    fn fields(&self) -> Vec<(&'static str, f64)> {
        self.last
            .as_ref()
            .map(|last| {
                vec![
                    ("stop_long", last.stop_long),
                    ("stop_short", last.stop_short),
                ]
            })
            .unwrap_or_default()
    }
    fn warmup(&self) -> usize {
        self.inner.warmup_period()
    }
}

impl<I> TickIndicator for ScalarPriceFields<I, wc::ChandelierExitOutput>
where
    I: Indicator<Input = f64, Output = wc::ChandelierExitOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = self.inner.update(input.price);
        self.last = out;
        self.last.as_ref().map(|last| last.long_stop)
    }
    fn fields(&self) -> Vec<(&'static str, f64)> {
        self.last
            .as_ref()
            .map(|last| {
                vec![
                    ("long_stop", last.long_stop),
                    ("short_stop", last.short_stop),
                ]
            })
            .unwrap_or_default()
    }
    fn warmup(&self) -> usize {
        self.inner.warmup_period()
    }
}

impl<I> TickIndicator for CandleInFields<I, wc::ChandelierExitOutput>
where
    I: Indicator<Input = Candle, Output = wc::ChandelierExitOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = input.candle.and_then(|c| self.inner.update(c));
        self.last = out;
        self.last.as_ref().map(|last| last.long_stop)
    }
    fn fields(&self) -> Vec<(&'static str, f64)> {
        self.last
            .as_ref()
            .map(|last| {
                vec![
                    ("long_stop", last.long_stop),
                    ("short_stop", last.short_stop),
                ]
            })
            .unwrap_or_default()
    }
    fn warmup(&self) -> usize {
        self.inner.warmup_period()
    }
}

impl<I> TickIndicator for ScalarPriceFields<I, wc::ClassicPivotsOutput>
where
    I: Indicator<Input = f64, Output = wc::ClassicPivotsOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = self.inner.update(input.price);
        self.last = out;
        self.last.as_ref().map(|last| last.pp)
    }
    fn fields(&self) -> Vec<(&'static str, f64)> {
        self.last
            .as_ref()
            .map(|last| {
                vec![
                    ("pp", last.pp),
                    ("r1", last.r1),
                    ("r2", last.r2),
                    ("r3", last.r3),
                    ("s1", last.s1),
                    ("s2", last.s2),
                    ("s3", last.s3),
                ]
            })
            .unwrap_or_default()
    }
    fn warmup(&self) -> usize {
        self.inner.warmup_period()
    }
}

impl<I> TickIndicator for CandleInFields<I, wc::ClassicPivotsOutput>
where
    I: Indicator<Input = Candle, Output = wc::ClassicPivotsOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = input.candle.and_then(|c| self.inner.update(c));
        self.last = out;
        self.last.as_ref().map(|last| last.pp)
    }
    fn fields(&self) -> Vec<(&'static str, f64)> {
        self.last
            .as_ref()
            .map(|last| {
                vec![
                    ("pp", last.pp),
                    ("r1", last.r1),
                    ("r2", last.r2),
                    ("r3", last.r3),
                    ("s1", last.s1),
                    ("s2", last.s2),
                    ("s3", last.s3),
                ]
            })
            .unwrap_or_default()
    }
    fn warmup(&self) -> usize {
        self.inner.warmup_period()
    }
}

impl<I> TickIndicator for ScalarPriceFields<I, wc::CompositeProfileOutput>
where
    I: Indicator<Input = f64, Output = wc::CompositeProfileOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = self.inner.update(input.price);
        self.last = out;
        self.last.as_ref().map(|last| last.poc)
    }
    fn fields(&self) -> Vec<(&'static str, f64)> {
        self.last
            .as_ref()
            .map(|last| vec![("poc", last.poc), ("vah", last.vah), ("val", last.val)])
            .unwrap_or_default()
    }
    fn warmup(&self) -> usize {
        self.inner.warmup_period()
    }
}

impl<I> TickIndicator for CandleInFields<I, wc::CompositeProfileOutput>
where
    I: Indicator<Input = Candle, Output = wc::CompositeProfileOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = input.candle.and_then(|c| self.inner.update(c));
        self.last = out;
        self.last.as_ref().map(|last| last.poc)
    }
    fn fields(&self) -> Vec<(&'static str, f64)> {
        self.last
            .as_ref()
            .map(|last| vec![("poc", last.poc), ("vah", last.vah), ("val", last.val)])
            .unwrap_or_default()
    }
    fn warmup(&self) -> usize {
        self.inner.warmup_period()
    }
}

impl<I> TickIndicator for ScalarPriceFields<I, wc::DemarkPivotsOutput>
where
    I: Indicator<Input = f64, Output = wc::DemarkPivotsOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = self.inner.update(input.price);
        self.last = out;
        self.last.as_ref().map(|last| last.pp)
    }
    fn fields(&self) -> Vec<(&'static str, f64)> {
        self.last
            .as_ref()
            .map(|last| vec![("pp", last.pp), ("r1", last.r1), ("s1", last.s1)])
            .unwrap_or_default()
    }
    fn warmup(&self) -> usize {
        self.inner.warmup_period()
    }
}

impl<I> TickIndicator for CandleInFields<I, wc::DemarkPivotsOutput>
where
    I: Indicator<Input = Candle, Output = wc::DemarkPivotsOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = input.candle.and_then(|c| self.inner.update(c));
        self.last = out;
        self.last.as_ref().map(|last| last.pp)
    }
    fn fields(&self) -> Vec<(&'static str, f64)> {
        self.last
            .as_ref()
            .map(|last| vec![("pp", last.pp), ("r1", last.r1), ("s1", last.s1)])
            .unwrap_or_default()
    }
    fn warmup(&self) -> usize {
        self.inner.warmup_period()
    }
}

impl<I> TickIndicator for ScalarPriceFields<I, wc::DonchianOutput>
where
    I: Indicator<Input = f64, Output = wc::DonchianOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = self.inner.update(input.price);
        self.last = out;
        self.last.as_ref().map(|last| last.upper)
    }
    fn fields(&self) -> Vec<(&'static str, f64)> {
        self.last
            .as_ref()
            .map(|last| {
                vec![
                    ("upper", last.upper),
                    ("middle", last.middle),
                    ("lower", last.lower),
                ]
            })
            .unwrap_or_default()
    }
    fn warmup(&self) -> usize {
        self.inner.warmup_period()
    }
}

impl<I> TickIndicator for CandleInFields<I, wc::DonchianOutput>
where
    I: Indicator<Input = Candle, Output = wc::DonchianOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = input.candle.and_then(|c| self.inner.update(c));
        self.last = out;
        self.last.as_ref().map(|last| last.upper)
    }
    fn fields(&self) -> Vec<(&'static str, f64)> {
        self.last
            .as_ref()
            .map(|last| {
                vec![
                    ("upper", last.upper),
                    ("middle", last.middle),
                    ("lower", last.lower),
                ]
            })
            .unwrap_or_default()
    }
    fn warmup(&self) -> usize {
        self.inner.warmup_period()
    }
}

impl<I> TickIndicator for ScalarPriceFields<I, wc::DonchianStopOutput>
where
    I: Indicator<Input = f64, Output = wc::DonchianStopOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = self.inner.update(input.price);
        self.last = out;
        self.last.as_ref().map(|last| last.stop_long)
    }
    fn fields(&self) -> Vec<(&'static str, f64)> {
        self.last
            .as_ref()
            .map(|last| {
                vec![
                    ("stop_long", last.stop_long),
                    ("stop_short", last.stop_short),
                ]
            })
            .unwrap_or_default()
    }
    fn warmup(&self) -> usize {
        self.inner.warmup_period()
    }
}

impl<I> TickIndicator for CandleInFields<I, wc::DonchianStopOutput>
where
    I: Indicator<Input = Candle, Output = wc::DonchianStopOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = input.candle.and_then(|c| self.inner.update(c));
        self.last = out;
        self.last.as_ref().map(|last| last.stop_long)
    }
    fn fields(&self) -> Vec<(&'static str, f64)> {
        self.last
            .as_ref()
            .map(|last| {
                vec![
                    ("stop_long", last.stop_long),
                    ("stop_short", last.stop_short),
                ]
            })
            .unwrap_or_default()
    }
    fn warmup(&self) -> usize {
        self.inner.warmup_period()
    }
}

impl<I> TickIndicator for ScalarPriceFields<I, wc::DoubleBollingerOutput>
where
    I: Indicator<Input = f64, Output = wc::DoubleBollingerOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = self.inner.update(input.price);
        self.last = out;
        self.last.as_ref().map(|last| last.upper_outer)
    }
    fn fields(&self) -> Vec<(&'static str, f64)> {
        self.last
            .as_ref()
            .map(|last| {
                vec![
                    ("upper_outer", last.upper_outer),
                    ("upper_inner", last.upper_inner),
                    ("middle", last.middle),
                    ("lower_inner", last.lower_inner),
                    ("lower_outer", last.lower_outer),
                ]
            })
            .unwrap_or_default()
    }
    fn warmup(&self) -> usize {
        self.inner.warmup_period()
    }
}

impl<I> TickIndicator for CandleInFields<I, wc::DoubleBollingerOutput>
where
    I: Indicator<Input = Candle, Output = wc::DoubleBollingerOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = input.candle.and_then(|c| self.inner.update(c));
        self.last = out;
        self.last.as_ref().map(|last| last.upper_outer)
    }
    fn fields(&self) -> Vec<(&'static str, f64)> {
        self.last
            .as_ref()
            .map(|last| {
                vec![
                    ("upper_outer", last.upper_outer),
                    ("upper_inner", last.upper_inner),
                    ("middle", last.middle),
                    ("lower_inner", last.lower_inner),
                    ("lower_outer", last.lower_outer),
                ]
            })
            .unwrap_or_default()
    }
    fn warmup(&self) -> usize {
        self.inner.warmup_period()
    }
}

impl<I> TickIndicator for ScalarPriceFields<I, wc::ElderRayOutput>
where
    I: Indicator<Input = f64, Output = wc::ElderRayOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = self.inner.update(input.price);
        self.last = out;
        self.last.as_ref().map(|last| last.bull_power)
    }
    fn fields(&self) -> Vec<(&'static str, f64)> {
        self.last
            .as_ref()
            .map(|last| {
                vec![
                    ("bull_power", last.bull_power),
                    ("bear_power", last.bear_power),
                ]
            })
            .unwrap_or_default()
    }
    fn warmup(&self) -> usize {
        self.inner.warmup_period()
    }
}

impl<I> TickIndicator for CandleInFields<I, wc::ElderRayOutput>
where
    I: Indicator<Input = Candle, Output = wc::ElderRayOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = input.candle.and_then(|c| self.inner.update(c));
        self.last = out;
        self.last.as_ref().map(|last| last.bull_power)
    }
    fn fields(&self) -> Vec<(&'static str, f64)> {
        self.last
            .as_ref()
            .map(|last| {
                vec![
                    ("bull_power", last.bull_power),
                    ("bear_power", last.bear_power),
                ]
            })
            .unwrap_or_default()
    }
    fn warmup(&self) -> usize {
        self.inner.warmup_period()
    }
}

impl<I> TickIndicator for ScalarPriceFields<I, wc::ElderSafeZoneOutput>
where
    I: Indicator<Input = f64, Output = wc::ElderSafeZoneOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = self.inner.update(input.price);
        self.last = out;
        self.last.as_ref().map(|last| last.value)
    }
    fn fields(&self) -> Vec<(&'static str, f64)> {
        self.last
            .as_ref()
            .map(|last| vec![("value", last.value), ("direction", last.direction)])
            .unwrap_or_default()
    }
    fn warmup(&self) -> usize {
        self.inner.warmup_period()
    }
}

impl<I> TickIndicator for CandleInFields<I, wc::ElderSafeZoneOutput>
where
    I: Indicator<Input = Candle, Output = wc::ElderSafeZoneOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = input.candle.and_then(|c| self.inner.update(c));
        self.last = out;
        self.last.as_ref().map(|last| last.value)
    }
    fn fields(&self) -> Vec<(&'static str, f64)> {
        self.last
            .as_ref()
            .map(|last| vec![("value", last.value), ("direction", last.direction)])
            .unwrap_or_default()
    }
    fn warmup(&self) -> usize {
        self.inner.warmup_period()
    }
}

impl<I> TickIndicator for ScalarPriceFields<I, wc::EquivolumeOutput>
where
    I: Indicator<Input = f64, Output = wc::EquivolumeOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = self.inner.update(input.price);
        self.last = out;
        self.last.as_ref().map(|last| last.height)
    }
    fn fields(&self) -> Vec<(&'static str, f64)> {
        self.last
            .as_ref()
            .map(|last| vec![("height", last.height), ("width", last.width)])
            .unwrap_or_default()
    }
    fn warmup(&self) -> usize {
        self.inner.warmup_period()
    }
}

impl<I> TickIndicator for CandleInFields<I, wc::EquivolumeOutput>
where
    I: Indicator<Input = Candle, Output = wc::EquivolumeOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = input.candle.and_then(|c| self.inner.update(c));
        self.last = out;
        self.last.as_ref().map(|last| last.height)
    }
    fn fields(&self) -> Vec<(&'static str, f64)> {
        self.last
            .as_ref()
            .map(|last| vec![("height", last.height), ("width", last.width)])
            .unwrap_or_default()
    }
    fn warmup(&self) -> usize {
        self.inner.warmup_period()
    }
}

impl<I> TickIndicator for ScalarPriceFields<I, wc::FibArcsOutput>
where
    I: Indicator<Input = f64, Output = wc::FibArcsOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = self.inner.update(input.price);
        self.last = out;
        self.last.as_ref().map(|last| last.arc_382)
    }
    fn fields(&self) -> Vec<(&'static str, f64)> {
        self.last
            .as_ref()
            .map(|last| {
                vec![
                    ("arc_382", last.arc_382),
                    ("arc_500", last.arc_500),
                    ("arc_618", last.arc_618),
                ]
            })
            .unwrap_or_default()
    }
    fn warmup(&self) -> usize {
        self.inner.warmup_period()
    }
}

impl<I> TickIndicator for CandleInFields<I, wc::FibArcsOutput>
where
    I: Indicator<Input = Candle, Output = wc::FibArcsOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = input.candle.and_then(|c| self.inner.update(c));
        self.last = out;
        self.last.as_ref().map(|last| last.arc_382)
    }
    fn fields(&self) -> Vec<(&'static str, f64)> {
        self.last
            .as_ref()
            .map(|last| {
                vec![
                    ("arc_382", last.arc_382),
                    ("arc_500", last.arc_500),
                    ("arc_618", last.arc_618),
                ]
            })
            .unwrap_or_default()
    }
    fn warmup(&self) -> usize {
        self.inner.warmup_period()
    }
}

impl<I> TickIndicator for ScalarPriceFields<I, wc::FibChannelOutput>
where
    I: Indicator<Input = f64, Output = wc::FibChannelOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = self.inner.update(input.price);
        self.last = out;
        self.last.as_ref().map(|last| last.base)
    }
    fn fields(&self) -> Vec<(&'static str, f64)> {
        self.last
            .as_ref()
            .map(|last| {
                vec![
                    ("base", last.base),
                    ("level_618", last.level_618),
                    ("level_1000", last.level_1000),
                    ("level_1618", last.level_1618),
                ]
            })
            .unwrap_or_default()
    }
    fn warmup(&self) -> usize {
        self.inner.warmup_period()
    }
}

impl<I> TickIndicator for CandleInFields<I, wc::FibChannelOutput>
where
    I: Indicator<Input = Candle, Output = wc::FibChannelOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = input.candle.and_then(|c| self.inner.update(c));
        self.last = out;
        self.last.as_ref().map(|last| last.base)
    }
    fn fields(&self) -> Vec<(&'static str, f64)> {
        self.last
            .as_ref()
            .map(|last| {
                vec![
                    ("base", last.base),
                    ("level_618", last.level_618),
                    ("level_1000", last.level_1000),
                    ("level_1618", last.level_1618),
                ]
            })
            .unwrap_or_default()
    }
    fn warmup(&self) -> usize {
        self.inner.warmup_period()
    }
}

impl<I> TickIndicator for ScalarPriceFields<I, wc::FibConfluenceOutput>
where
    I: Indicator<Input = f64, Output = wc::FibConfluenceOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = self.inner.update(input.price);
        self.last = out;
        self.last.as_ref().map(|last| last.price)
    }
    fn fields(&self) -> Vec<(&'static str, f64)> {
        self.last
            .as_ref()
            .map(|last| vec![("price", last.price), ("strength", last.strength)])
            .unwrap_or_default()
    }
    fn warmup(&self) -> usize {
        self.inner.warmup_period()
    }
}

impl<I> TickIndicator for CandleInFields<I, wc::FibConfluenceOutput>
where
    I: Indicator<Input = Candle, Output = wc::FibConfluenceOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = input.candle.and_then(|c| self.inner.update(c));
        self.last = out;
        self.last.as_ref().map(|last| last.price)
    }
    fn fields(&self) -> Vec<(&'static str, f64)> {
        self.last
            .as_ref()
            .map(|last| vec![("price", last.price), ("strength", last.strength)])
            .unwrap_or_default()
    }
    fn warmup(&self) -> usize {
        self.inner.warmup_period()
    }
}

impl<I> TickIndicator for ScalarPriceFields<I, wc::FibExtensionOutput>
where
    I: Indicator<Input = f64, Output = wc::FibExtensionOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = self.inner.update(input.price);
        self.last = out;
        self.last.as_ref().map(|last| last.level_1272)
    }
    fn fields(&self) -> Vec<(&'static str, f64)> {
        self.last
            .as_ref()
            .map(|last| {
                vec![
                    ("level_1272", last.level_1272),
                    ("level_1414", last.level_1414),
                    ("level_1618", last.level_1618),
                    ("level_2000", last.level_2000),
                    ("level_2618", last.level_2618),
                ]
            })
            .unwrap_or_default()
    }
    fn warmup(&self) -> usize {
        self.inner.warmup_period()
    }
}

impl<I> TickIndicator for CandleInFields<I, wc::FibExtensionOutput>
where
    I: Indicator<Input = Candle, Output = wc::FibExtensionOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = input.candle.and_then(|c| self.inner.update(c));
        self.last = out;
        self.last.as_ref().map(|last| last.level_1272)
    }
    fn fields(&self) -> Vec<(&'static str, f64)> {
        self.last
            .as_ref()
            .map(|last| {
                vec![
                    ("level_1272", last.level_1272),
                    ("level_1414", last.level_1414),
                    ("level_1618", last.level_1618),
                    ("level_2000", last.level_2000),
                    ("level_2618", last.level_2618),
                ]
            })
            .unwrap_or_default()
    }
    fn warmup(&self) -> usize {
        self.inner.warmup_period()
    }
}

impl<I> TickIndicator for ScalarPriceFields<I, wc::FibFanOutput>
where
    I: Indicator<Input = f64, Output = wc::FibFanOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = self.inner.update(input.price);
        self.last = out;
        self.last.as_ref().map(|last| last.fan_382)
    }
    fn fields(&self) -> Vec<(&'static str, f64)> {
        self.last
            .as_ref()
            .map(|last| {
                vec![
                    ("fan_382", last.fan_382),
                    ("fan_500", last.fan_500),
                    ("fan_618", last.fan_618),
                ]
            })
            .unwrap_or_default()
    }
    fn warmup(&self) -> usize {
        self.inner.warmup_period()
    }
}

impl<I> TickIndicator for CandleInFields<I, wc::FibFanOutput>
where
    I: Indicator<Input = Candle, Output = wc::FibFanOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = input.candle.and_then(|c| self.inner.update(c));
        self.last = out;
        self.last.as_ref().map(|last| last.fan_382)
    }
    fn fields(&self) -> Vec<(&'static str, f64)> {
        self.last
            .as_ref()
            .map(|last| {
                vec![
                    ("fan_382", last.fan_382),
                    ("fan_500", last.fan_500),
                    ("fan_618", last.fan_618),
                ]
            })
            .unwrap_or_default()
    }
    fn warmup(&self) -> usize {
        self.inner.warmup_period()
    }
}

impl<I> TickIndicator for ScalarPriceFields<I, wc::FibProjectionOutput>
where
    I: Indicator<Input = f64, Output = wc::FibProjectionOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = self.inner.update(input.price);
        self.last = out;
        self.last.as_ref().map(|last| last.level_618)
    }
    fn fields(&self) -> Vec<(&'static str, f64)> {
        self.last
            .as_ref()
            .map(|last| {
                vec![
                    ("level_618", last.level_618),
                    ("level_1000", last.level_1000),
                    ("level_1618", last.level_1618),
                    ("level_2618", last.level_2618),
                ]
            })
            .unwrap_or_default()
    }
    fn warmup(&self) -> usize {
        self.inner.warmup_period()
    }
}

impl<I> TickIndicator for CandleInFields<I, wc::FibProjectionOutput>
where
    I: Indicator<Input = Candle, Output = wc::FibProjectionOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = input.candle.and_then(|c| self.inner.update(c));
        self.last = out;
        self.last.as_ref().map(|last| last.level_618)
    }
    fn fields(&self) -> Vec<(&'static str, f64)> {
        self.last
            .as_ref()
            .map(|last| {
                vec![
                    ("level_618", last.level_618),
                    ("level_1000", last.level_1000),
                    ("level_1618", last.level_1618),
                    ("level_2618", last.level_2618),
                ]
            })
            .unwrap_or_default()
    }
    fn warmup(&self) -> usize {
        self.inner.warmup_period()
    }
}

impl<I> TickIndicator for ScalarPriceFields<I, wc::FibRetracementOutput>
where
    I: Indicator<Input = f64, Output = wc::FibRetracementOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = self.inner.update(input.price);
        self.last = out;
        self.last.as_ref().map(|last| last.level_0)
    }
    fn fields(&self) -> Vec<(&'static str, f64)> {
        self.last
            .as_ref()
            .map(|last| {
                vec![
                    ("level_0", last.level_0),
                    ("level_236", last.level_236),
                    ("level_382", last.level_382),
                    ("level_500", last.level_500),
                    ("level_618", last.level_618),
                    ("level_786", last.level_786),
                    ("level_1000", last.level_1000),
                ]
            })
            .unwrap_or_default()
    }
    fn warmup(&self) -> usize {
        self.inner.warmup_period()
    }
}

impl<I> TickIndicator for CandleInFields<I, wc::FibRetracementOutput>
where
    I: Indicator<Input = Candle, Output = wc::FibRetracementOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = input.candle.and_then(|c| self.inner.update(c));
        self.last = out;
        self.last.as_ref().map(|last| last.level_0)
    }
    fn fields(&self) -> Vec<(&'static str, f64)> {
        self.last
            .as_ref()
            .map(|last| {
                vec![
                    ("level_0", last.level_0),
                    ("level_236", last.level_236),
                    ("level_382", last.level_382),
                    ("level_500", last.level_500),
                    ("level_618", last.level_618),
                    ("level_786", last.level_786),
                    ("level_1000", last.level_1000),
                ]
            })
            .unwrap_or_default()
    }
    fn warmup(&self) -> usize {
        self.inner.warmup_period()
    }
}

impl<I> TickIndicator for ScalarPriceFields<I, wc::FibTimeZonesOutput>
where
    I: Indicator<Input = f64, Output = wc::FibTimeZonesOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = self.inner.update(input.price);
        self.last = out;
        self.last.as_ref().map(|last| last.on_zone)
    }
    fn fields(&self) -> Vec<(&'static str, f64)> {
        self.last
            .as_ref()
            .map(|last| {
                vec![
                    ("on_zone", last.on_zone),
                    ("bars_to_next", last.bars_to_next),
                ]
            })
            .unwrap_or_default()
    }
    fn warmup(&self) -> usize {
        self.inner.warmup_period()
    }
}

impl<I> TickIndicator for CandleInFields<I, wc::FibTimeZonesOutput>
where
    I: Indicator<Input = Candle, Output = wc::FibTimeZonesOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = input.candle.and_then(|c| self.inner.update(c));
        self.last = out;
        self.last.as_ref().map(|last| last.on_zone)
    }
    fn fields(&self) -> Vec<(&'static str, f64)> {
        self.last
            .as_ref()
            .map(|last| {
                vec![
                    ("on_zone", last.on_zone),
                    ("bars_to_next", last.bars_to_next),
                ]
            })
            .unwrap_or_default()
    }
    fn warmup(&self) -> usize {
        self.inner.warmup_period()
    }
}

impl<I> TickIndicator for ScalarPriceFields<I, wc::FibonacciPivotsOutput>
where
    I: Indicator<Input = f64, Output = wc::FibonacciPivotsOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = self.inner.update(input.price);
        self.last = out;
        self.last.as_ref().map(|last| last.pp)
    }
    fn fields(&self) -> Vec<(&'static str, f64)> {
        self.last
            .as_ref()
            .map(|last| {
                vec![
                    ("pp", last.pp),
                    ("r1", last.r1),
                    ("r2", last.r2),
                    ("r3", last.r3),
                    ("s1", last.s1),
                    ("s2", last.s2),
                    ("s3", last.s3),
                ]
            })
            .unwrap_or_default()
    }
    fn warmup(&self) -> usize {
        self.inner.warmup_period()
    }
}

impl<I> TickIndicator for CandleInFields<I, wc::FibonacciPivotsOutput>
where
    I: Indicator<Input = Candle, Output = wc::FibonacciPivotsOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = input.candle.and_then(|c| self.inner.update(c));
        self.last = out;
        self.last.as_ref().map(|last| last.pp)
    }
    fn fields(&self) -> Vec<(&'static str, f64)> {
        self.last
            .as_ref()
            .map(|last| {
                vec![
                    ("pp", last.pp),
                    ("r1", last.r1),
                    ("r2", last.r2),
                    ("r3", last.r3),
                    ("s1", last.s1),
                    ("s2", last.s2),
                    ("s3", last.s3),
                ]
            })
            .unwrap_or_default()
    }
    fn warmup(&self) -> usize {
        self.inner.warmup_period()
    }
}

impl<I> TickIndicator for ScalarPriceFields<I, wc::FractalChaosBandsOutput>
where
    I: Indicator<Input = f64, Output = wc::FractalChaosBandsOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = self.inner.update(input.price);
        self.last = out;
        self.last.as_ref().map(|last| last.upper)
    }
    fn fields(&self) -> Vec<(&'static str, f64)> {
        self.last
            .as_ref()
            .map(|last| vec![("upper", last.upper), ("lower", last.lower)])
            .unwrap_or_default()
    }
    fn warmup(&self) -> usize {
        self.inner.warmup_period()
    }
}

impl<I> TickIndicator for CandleInFields<I, wc::FractalChaosBandsOutput>
where
    I: Indicator<Input = Candle, Output = wc::FractalChaosBandsOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = input.candle.and_then(|c| self.inner.update(c));
        self.last = out;
        self.last.as_ref().map(|last| last.upper)
    }
    fn fields(&self) -> Vec<(&'static str, f64)> {
        self.last
            .as_ref()
            .map(|last| vec![("upper", last.upper), ("lower", last.lower)])
            .unwrap_or_default()
    }
    fn warmup(&self) -> usize {
        self.inner.warmup_period()
    }
}

impl<I> TickIndicator for ScalarPriceFields<I, wc::GatorOscillatorOutput>
where
    I: Indicator<Input = f64, Output = wc::GatorOscillatorOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = self.inner.update(input.price);
        self.last = out;
        self.last.as_ref().map(|last| last.upper)
    }
    fn fields(&self) -> Vec<(&'static str, f64)> {
        self.last
            .as_ref()
            .map(|last| vec![("upper", last.upper), ("lower", last.lower)])
            .unwrap_or_default()
    }
    fn warmup(&self) -> usize {
        self.inner.warmup_period()
    }
}

impl<I> TickIndicator for CandleInFields<I, wc::GatorOscillatorOutput>
where
    I: Indicator<Input = Candle, Output = wc::GatorOscillatorOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = input.candle.and_then(|c| self.inner.update(c));
        self.last = out;
        self.last.as_ref().map(|last| last.upper)
    }
    fn fields(&self) -> Vec<(&'static str, f64)> {
        self.last
            .as_ref()
            .map(|last| vec![("upper", last.upper), ("lower", last.lower)])
            .unwrap_or_default()
    }
    fn warmup(&self) -> usize {
        self.inner.warmup_period()
    }
}

impl<I> TickIndicator for ScalarPriceFields<I, wc::GoldenPocketOutput>
where
    I: Indicator<Input = f64, Output = wc::GoldenPocketOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = self.inner.update(input.price);
        self.last = out;
        self.last.as_ref().map(|last| last.low)
    }
    fn fields(&self) -> Vec<(&'static str, f64)> {
        self.last
            .as_ref()
            .map(|last| vec![("low", last.low), ("mid", last.mid), ("high", last.high)])
            .unwrap_or_default()
    }
    fn warmup(&self) -> usize {
        self.inner.warmup_period()
    }
}

impl<I> TickIndicator for CandleInFields<I, wc::GoldenPocketOutput>
where
    I: Indicator<Input = Candle, Output = wc::GoldenPocketOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = input.candle.and_then(|c| self.inner.update(c));
        self.last = out;
        self.last.as_ref().map(|last| last.low)
    }
    fn fields(&self) -> Vec<(&'static str, f64)> {
        self.last
            .as_ref()
            .map(|last| vec![("low", last.low), ("mid", last.mid), ("high", last.high)])
            .unwrap_or_default()
    }
    fn warmup(&self) -> usize {
        self.inner.warmup_period()
    }
}

impl<I> TickIndicator for ScalarPriceFields<I, wc::HeikinAshiOutput>
where
    I: Indicator<Input = f64, Output = wc::HeikinAshiOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = self.inner.update(input.price);
        self.last = out;
        self.last.as_ref().map(|last| last.open)
    }
    fn fields(&self) -> Vec<(&'static str, f64)> {
        self.last
            .as_ref()
            .map(|last| {
                vec![
                    ("open", last.open),
                    ("high", last.high),
                    ("low", last.low),
                    ("close", last.close),
                ]
            })
            .unwrap_or_default()
    }
    fn warmup(&self) -> usize {
        self.inner.warmup_period()
    }
}

impl<I> TickIndicator for CandleInFields<I, wc::HeikinAshiOutput>
where
    I: Indicator<Input = Candle, Output = wc::HeikinAshiOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = input.candle.and_then(|c| self.inner.update(c));
        self.last = out;
        self.last.as_ref().map(|last| last.open)
    }
    fn fields(&self) -> Vec<(&'static str, f64)> {
        self.last
            .as_ref()
            .map(|last| {
                vec![
                    ("open", last.open),
                    ("high", last.high),
                    ("low", last.low),
                    ("close", last.close),
                ]
            })
            .unwrap_or_default()
    }
    fn warmup(&self) -> usize {
        self.inner.warmup_period()
    }
}

impl<I> TickIndicator for ScalarPriceFields<I, wc::HighLowVolumeNodesOutput>
where
    I: Indicator<Input = f64, Output = wc::HighLowVolumeNodesOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = self.inner.update(input.price);
        self.last = out;
        self.last.as_ref().map(|last| last.hvn)
    }
    fn fields(&self) -> Vec<(&'static str, f64)> {
        self.last
            .as_ref()
            .map(|last| vec![("hvn", last.hvn), ("lvn", last.lvn)])
            .unwrap_or_default()
    }
    fn warmup(&self) -> usize {
        self.inner.warmup_period()
    }
}

impl<I> TickIndicator for CandleInFields<I, wc::HighLowVolumeNodesOutput>
where
    I: Indicator<Input = Candle, Output = wc::HighLowVolumeNodesOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = input.candle.and_then(|c| self.inner.update(c));
        self.last = out;
        self.last.as_ref().map(|last| last.hvn)
    }
    fn fields(&self) -> Vec<(&'static str, f64)> {
        self.last
            .as_ref()
            .map(|last| vec![("hvn", last.hvn), ("lvn", last.lvn)])
            .unwrap_or_default()
    }
    fn warmup(&self) -> usize {
        self.inner.warmup_period()
    }
}

impl<I> TickIndicator for ScalarPriceFields<I, wc::HtPhasorOutput>
where
    I: Indicator<Input = f64, Output = wc::HtPhasorOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = self.inner.update(input.price);
        self.last = out;
        self.last.as_ref().map(|last| last.inphase)
    }
    fn fields(&self) -> Vec<(&'static str, f64)> {
        self.last
            .as_ref()
            .map(|last| vec![("inphase", last.inphase), ("quadrature", last.quadrature)])
            .unwrap_or_default()
    }
    fn warmup(&self) -> usize {
        self.inner.warmup_period()
    }
}

impl<I> TickIndicator for CandleInFields<I, wc::HtPhasorOutput>
where
    I: Indicator<Input = Candle, Output = wc::HtPhasorOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = input.candle.and_then(|c| self.inner.update(c));
        self.last = out;
        self.last.as_ref().map(|last| last.inphase)
    }
    fn fields(&self) -> Vec<(&'static str, f64)> {
        self.last
            .as_ref()
            .map(|last| vec![("inphase", last.inphase), ("quadrature", last.quadrature)])
            .unwrap_or_default()
    }
    fn warmup(&self) -> usize {
        self.inner.warmup_period()
    }
}

impl<I> TickIndicator for ScalarPriceFields<I, wc::HurstChannelOutput>
where
    I: Indicator<Input = f64, Output = wc::HurstChannelOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = self.inner.update(input.price);
        self.last = out;
        self.last.as_ref().map(|last| last.upper)
    }
    fn fields(&self) -> Vec<(&'static str, f64)> {
        self.last
            .as_ref()
            .map(|last| {
                vec![
                    ("upper", last.upper),
                    ("middle", last.middle),
                    ("lower", last.lower),
                ]
            })
            .unwrap_or_default()
    }
    fn warmup(&self) -> usize {
        self.inner.warmup_period()
    }
}

impl<I> TickIndicator for CandleInFields<I, wc::HurstChannelOutput>
where
    I: Indicator<Input = Candle, Output = wc::HurstChannelOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = input.candle.and_then(|c| self.inner.update(c));
        self.last = out;
        self.last.as_ref().map(|last| last.upper)
    }
    fn fields(&self) -> Vec<(&'static str, f64)> {
        self.last
            .as_ref()
            .map(|last| {
                vec![
                    ("upper", last.upper),
                    ("middle", last.middle),
                    ("lower", last.lower),
                ]
            })
            .unwrap_or_default()
    }
    fn warmup(&self) -> usize {
        self.inner.warmup_period()
    }
}

impl<I> TickIndicator for ScalarPriceFields<I, wc::InitialBalanceOutput>
where
    I: Indicator<Input = f64, Output = wc::InitialBalanceOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = self.inner.update(input.price);
        self.last = out;
        self.last.as_ref().map(|last| last.high)
    }
    fn fields(&self) -> Vec<(&'static str, f64)> {
        self.last
            .as_ref()
            .map(|last| vec![("high", last.high), ("low", last.low)])
            .unwrap_or_default()
    }
    fn warmup(&self) -> usize {
        self.inner.warmup_period()
    }
}

impl<I> TickIndicator for CandleInFields<I, wc::InitialBalanceOutput>
where
    I: Indicator<Input = Candle, Output = wc::InitialBalanceOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = input.candle.and_then(|c| self.inner.update(c));
        self.last = out;
        self.last.as_ref().map(|last| last.high)
    }
    fn fields(&self) -> Vec<(&'static str, f64)> {
        self.last
            .as_ref()
            .map(|last| vec![("high", last.high), ("low", last.low)])
            .unwrap_or_default()
    }
    fn warmup(&self) -> usize {
        self.inner.warmup_period()
    }
}

impl<I> TickIndicator for ScalarPriceFields<I, wc::KaseDevStopOutput>
where
    I: Indicator<Input = f64, Output = wc::KaseDevStopOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = self.inner.update(input.price);
        self.last = out;
        self.last.as_ref().map(|last| last.value)
    }
    fn fields(&self) -> Vec<(&'static str, f64)> {
        self.last
            .as_ref()
            .map(|last| vec![("value", last.value), ("direction", last.direction)])
            .unwrap_or_default()
    }
    fn warmup(&self) -> usize {
        self.inner.warmup_period()
    }
}

impl<I> TickIndicator for CandleInFields<I, wc::KaseDevStopOutput>
where
    I: Indicator<Input = Candle, Output = wc::KaseDevStopOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = input.candle.and_then(|c| self.inner.update(c));
        self.last = out;
        self.last.as_ref().map(|last| last.value)
    }
    fn fields(&self) -> Vec<(&'static str, f64)> {
        self.last
            .as_ref()
            .map(|last| vec![("value", last.value), ("direction", last.direction)])
            .unwrap_or_default()
    }
    fn warmup(&self) -> usize {
        self.inner.warmup_period()
    }
}

impl<I> TickIndicator for ScalarPriceFields<I, wc::KasePermissionStochasticOutput>
where
    I: Indicator<Input = f64, Output = wc::KasePermissionStochasticOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = self.inner.update(input.price);
        self.last = out;
        self.last.as_ref().map(|last| last.fast)
    }
    fn fields(&self) -> Vec<(&'static str, f64)> {
        self.last
            .as_ref()
            .map(|last| vec![("fast", last.fast), ("slow", last.slow)])
            .unwrap_or_default()
    }
    fn warmup(&self) -> usize {
        self.inner.warmup_period()
    }
}

impl<I> TickIndicator for CandleInFields<I, wc::KasePermissionStochasticOutput>
where
    I: Indicator<Input = Candle, Output = wc::KasePermissionStochasticOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = input.candle.and_then(|c| self.inner.update(c));
        self.last = out;
        self.last.as_ref().map(|last| last.fast)
    }
    fn fields(&self) -> Vec<(&'static str, f64)> {
        self.last
            .as_ref()
            .map(|last| vec![("fast", last.fast), ("slow", last.slow)])
            .unwrap_or_default()
    }
    fn warmup(&self) -> usize {
        self.inner.warmup_period()
    }
}

impl<I> TickIndicator for ScalarPriceFields<I, wc::KeltnerOutput>
where
    I: Indicator<Input = f64, Output = wc::KeltnerOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = self.inner.update(input.price);
        self.last = out;
        self.last.as_ref().map(|last| last.upper)
    }
    fn fields(&self) -> Vec<(&'static str, f64)> {
        self.last
            .as_ref()
            .map(|last| {
                vec![
                    ("upper", last.upper),
                    ("middle", last.middle),
                    ("lower", last.lower),
                ]
            })
            .unwrap_or_default()
    }
    fn warmup(&self) -> usize {
        self.inner.warmup_period()
    }
}

impl<I> TickIndicator for CandleInFields<I, wc::KeltnerOutput>
where
    I: Indicator<Input = Candle, Output = wc::KeltnerOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = input.candle.and_then(|c| self.inner.update(c));
        self.last = out;
        self.last.as_ref().map(|last| last.upper)
    }
    fn fields(&self) -> Vec<(&'static str, f64)> {
        self.last
            .as_ref()
            .map(|last| {
                vec![
                    ("upper", last.upper),
                    ("middle", last.middle),
                    ("lower", last.lower),
                ]
            })
            .unwrap_or_default()
    }
    fn warmup(&self) -> usize {
        self.inner.warmup_period()
    }
}

impl<I> TickIndicator for ScalarPriceFields<I, wc::KstOutput>
where
    I: Indicator<Input = f64, Output = wc::KstOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = self.inner.update(input.price);
        self.last = out;
        self.last.as_ref().map(|last| last.kst)
    }
    fn fields(&self) -> Vec<(&'static str, f64)> {
        self.last
            .as_ref()
            .map(|last| vec![("kst", last.kst), ("signal", last.signal)])
            .unwrap_or_default()
    }
    fn warmup(&self) -> usize {
        self.inner.warmup_period()
    }
}

impl<I> TickIndicator for CandleInFields<I, wc::KstOutput>
where
    I: Indicator<Input = Candle, Output = wc::KstOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = input.candle.and_then(|c| self.inner.update(c));
        self.last = out;
        self.last.as_ref().map(|last| last.kst)
    }
    fn fields(&self) -> Vec<(&'static str, f64)> {
        self.last
            .as_ref()
            .map(|last| vec![("kst", last.kst), ("signal", last.signal)])
            .unwrap_or_default()
    }
    fn warmup(&self) -> usize {
        self.inner.warmup_period()
    }
}

impl<I> TickIndicator for ScalarPriceFields<I, wc::LinRegChannelOutput>
where
    I: Indicator<Input = f64, Output = wc::LinRegChannelOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = self.inner.update(input.price);
        self.last = out;
        self.last.as_ref().map(|last| last.upper)
    }
    fn fields(&self) -> Vec<(&'static str, f64)> {
        self.last
            .as_ref()
            .map(|last| {
                vec![
                    ("upper", last.upper),
                    ("middle", last.middle),
                    ("lower", last.lower),
                ]
            })
            .unwrap_or_default()
    }
    fn warmup(&self) -> usize {
        self.inner.warmup_period()
    }
}

impl<I> TickIndicator for CandleInFields<I, wc::LinRegChannelOutput>
where
    I: Indicator<Input = Candle, Output = wc::LinRegChannelOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = input.candle.and_then(|c| self.inner.update(c));
        self.last = out;
        self.last.as_ref().map(|last| last.upper)
    }
    fn fields(&self) -> Vec<(&'static str, f64)> {
        self.last
            .as_ref()
            .map(|last| {
                vec![
                    ("upper", last.upper),
                    ("middle", last.middle),
                    ("lower", last.lower),
                ]
            })
            .unwrap_or_default()
    }
    fn warmup(&self) -> usize {
        self.inner.warmup_period()
    }
}

impl<I> TickIndicator for ScalarPriceFields<I, wc::MaEnvelopeOutput>
where
    I: Indicator<Input = f64, Output = wc::MaEnvelopeOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = self.inner.update(input.price);
        self.last = out;
        self.last.as_ref().map(|last| last.upper)
    }
    fn fields(&self) -> Vec<(&'static str, f64)> {
        self.last
            .as_ref()
            .map(|last| {
                vec![
                    ("upper", last.upper),
                    ("middle", last.middle),
                    ("lower", last.lower),
                ]
            })
            .unwrap_or_default()
    }
    fn warmup(&self) -> usize {
        self.inner.warmup_period()
    }
}

impl<I> TickIndicator for CandleInFields<I, wc::MaEnvelopeOutput>
where
    I: Indicator<Input = Candle, Output = wc::MaEnvelopeOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = input.candle.and_then(|c| self.inner.update(c));
        self.last = out;
        self.last.as_ref().map(|last| last.upper)
    }
    fn fields(&self) -> Vec<(&'static str, f64)> {
        self.last
            .as_ref()
            .map(|last| {
                vec![
                    ("upper", last.upper),
                    ("middle", last.middle),
                    ("lower", last.lower),
                ]
            })
            .unwrap_or_default()
    }
    fn warmup(&self) -> usize {
        self.inner.warmup_period()
    }
}

impl<I> TickIndicator for ScalarPriceFields<I, wc::MacdOutput>
where
    I: Indicator<Input = f64, Output = wc::MacdOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = self.inner.update(input.price);
        self.last = out;
        self.last.as_ref().map(|last| last.macd)
    }
    fn fields(&self) -> Vec<(&'static str, f64)> {
        self.last
            .as_ref()
            .map(|last| {
                vec![
                    ("macd", last.macd),
                    ("signal", last.signal),
                    ("histogram", last.histogram),
                ]
            })
            .unwrap_or_default()
    }
    fn warmup(&self) -> usize {
        self.inner.warmup_period()
    }
}

impl<I> TickIndicator for CandleInFields<I, wc::MacdOutput>
where
    I: Indicator<Input = Candle, Output = wc::MacdOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = input.candle.and_then(|c| self.inner.update(c));
        self.last = out;
        self.last.as_ref().map(|last| last.macd)
    }
    fn fields(&self) -> Vec<(&'static str, f64)> {
        self.last
            .as_ref()
            .map(|last| {
                vec![
                    ("macd", last.macd),
                    ("signal", last.signal),
                    ("histogram", last.histogram),
                ]
            })
            .unwrap_or_default()
    }
    fn warmup(&self) -> usize {
        self.inner.warmup_period()
    }
}

impl<I> TickIndicator for ScalarPriceFields<I, wc::MamaOutput>
where
    I: Indicator<Input = f64, Output = wc::MamaOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = self.inner.update(input.price);
        self.last = out;
        self.last.as_ref().map(|last| last.mama)
    }
    fn fields(&self) -> Vec<(&'static str, f64)> {
        self.last
            .as_ref()
            .map(|last| vec![("mama", last.mama), ("fama", last.fama)])
            .unwrap_or_default()
    }
    fn warmup(&self) -> usize {
        self.inner.warmup_period()
    }
}

impl<I> TickIndicator for CandleInFields<I, wc::MamaOutput>
where
    I: Indicator<Input = Candle, Output = wc::MamaOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = input.candle.and_then(|c| self.inner.update(c));
        self.last = out;
        self.last.as_ref().map(|last| last.mama)
    }
    fn fields(&self) -> Vec<(&'static str, f64)> {
        self.last
            .as_ref()
            .map(|last| vec![("mama", last.mama), ("fama", last.fama)])
            .unwrap_or_default()
    }
    fn warmup(&self) -> usize {
        self.inner.warmup_period()
    }
}

impl<I> TickIndicator for ScalarPriceFields<I, wc::MedianChannelOutput>
where
    I: Indicator<Input = f64, Output = wc::MedianChannelOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = self.inner.update(input.price);
        self.last = out;
        self.last.as_ref().map(|last| last.upper)
    }
    fn fields(&self) -> Vec<(&'static str, f64)> {
        self.last
            .as_ref()
            .map(|last| {
                vec![
                    ("upper", last.upper),
                    ("middle", last.middle),
                    ("lower", last.lower),
                ]
            })
            .unwrap_or_default()
    }
    fn warmup(&self) -> usize {
        self.inner.warmup_period()
    }
}

impl<I> TickIndicator for CandleInFields<I, wc::MedianChannelOutput>
where
    I: Indicator<Input = Candle, Output = wc::MedianChannelOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = input.candle.and_then(|c| self.inner.update(c));
        self.last = out;
        self.last.as_ref().map(|last| last.upper)
    }
    fn fields(&self) -> Vec<(&'static str, f64)> {
        self.last
            .as_ref()
            .map(|last| {
                vec![
                    ("upper", last.upper),
                    ("middle", last.middle),
                    ("lower", last.lower),
                ]
            })
            .unwrap_or_default()
    }
    fn warmup(&self) -> usize {
        self.inner.warmup_period()
    }
}

impl<I> TickIndicator for ScalarPriceFields<I, wc::ModifiedMaStopOutput>
where
    I: Indicator<Input = f64, Output = wc::ModifiedMaStopOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = self.inner.update(input.price);
        self.last = out;
        self.last.as_ref().map(|last| last.value)
    }
    fn fields(&self) -> Vec<(&'static str, f64)> {
        self.last
            .as_ref()
            .map(|last| vec![("value", last.value), ("direction", last.direction)])
            .unwrap_or_default()
    }
    fn warmup(&self) -> usize {
        self.inner.warmup_period()
    }
}

impl<I> TickIndicator for CandleInFields<I, wc::ModifiedMaStopOutput>
where
    I: Indicator<Input = Candle, Output = wc::ModifiedMaStopOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = input.candle.and_then(|c| self.inner.update(c));
        self.last = out;
        self.last.as_ref().map(|last| last.value)
    }
    fn fields(&self) -> Vec<(&'static str, f64)> {
        self.last
            .as_ref()
            .map(|last| vec![("value", last.value), ("direction", last.direction)])
            .unwrap_or_default()
    }
    fn warmup(&self) -> usize {
        self.inner.warmup_period()
    }
}

impl<I> TickIndicator for ScalarPriceFields<I, wc::MurreyMathLinesOutput>
where
    I: Indicator<Input = f64, Output = wc::MurreyMathLinesOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = self.inner.update(input.price);
        self.last = out;
        self.last.as_ref().map(|last| last.mm8_8)
    }
    fn fields(&self) -> Vec<(&'static str, f64)> {
        self.last
            .as_ref()
            .map(|last| {
                vec![
                    ("mm8_8", last.mm8_8),
                    ("mm7_8", last.mm7_8),
                    ("mm6_8", last.mm6_8),
                    ("mm5_8", last.mm5_8),
                    ("mm4_8", last.mm4_8),
                    ("mm3_8", last.mm3_8),
                    ("mm2_8", last.mm2_8),
                    ("mm1_8", last.mm1_8),
                    ("mm0_8", last.mm0_8),
                ]
            })
            .unwrap_or_default()
    }
    fn warmup(&self) -> usize {
        self.inner.warmup_period()
    }
}

impl<I> TickIndicator for CandleInFields<I, wc::MurreyMathLinesOutput>
where
    I: Indicator<Input = Candle, Output = wc::MurreyMathLinesOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = input.candle.and_then(|c| self.inner.update(c));
        self.last = out;
        self.last.as_ref().map(|last| last.mm8_8)
    }
    fn fields(&self) -> Vec<(&'static str, f64)> {
        self.last
            .as_ref()
            .map(|last| {
                vec![
                    ("mm8_8", last.mm8_8),
                    ("mm7_8", last.mm7_8),
                    ("mm6_8", last.mm6_8),
                    ("mm5_8", last.mm5_8),
                    ("mm4_8", last.mm4_8),
                    ("mm3_8", last.mm3_8),
                    ("mm2_8", last.mm2_8),
                    ("mm1_8", last.mm1_8),
                    ("mm0_8", last.mm0_8),
                ]
            })
            .unwrap_or_default()
    }
    fn warmup(&self) -> usize {
        self.inner.warmup_period()
    }
}

impl<I> TickIndicator for ScalarPriceFields<I, wc::NrtrOutput>
where
    I: Indicator<Input = f64, Output = wc::NrtrOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = self.inner.update(input.price);
        self.last = out;
        self.last.as_ref().map(|last| last.value)
    }
    fn fields(&self) -> Vec<(&'static str, f64)> {
        self.last
            .as_ref()
            .map(|last| vec![("value", last.value), ("direction", last.direction)])
            .unwrap_or_default()
    }
    fn warmup(&self) -> usize {
        self.inner.warmup_period()
    }
}

impl<I> TickIndicator for CandleInFields<I, wc::NrtrOutput>
where
    I: Indicator<Input = Candle, Output = wc::NrtrOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = input.candle.and_then(|c| self.inner.update(c));
        self.last = out;
        self.last.as_ref().map(|last| last.value)
    }
    fn fields(&self) -> Vec<(&'static str, f64)> {
        self.last
            .as_ref()
            .map(|last| vec![("value", last.value), ("direction", last.direction)])
            .unwrap_or_default()
    }
    fn warmup(&self) -> usize {
        self.inner.warmup_period()
    }
}

impl<I> TickIndicator for ScalarPriceFields<I, wc::OpeningRangeOutput>
where
    I: Indicator<Input = f64, Output = wc::OpeningRangeOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = self.inner.update(input.price);
        self.last = out;
        self.last.as_ref().map(|last| last.high)
    }
    fn fields(&self) -> Vec<(&'static str, f64)> {
        self.last
            .as_ref()
            .map(|last| {
                vec![
                    ("high", last.high),
                    ("low", last.low),
                    ("breakout_distance", last.breakout_distance),
                ]
            })
            .unwrap_or_default()
    }
    fn warmup(&self) -> usize {
        self.inner.warmup_period()
    }
}

impl<I> TickIndicator for CandleInFields<I, wc::OpeningRangeOutput>
where
    I: Indicator<Input = Candle, Output = wc::OpeningRangeOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = input.candle.and_then(|c| self.inner.update(c));
        self.last = out;
        self.last.as_ref().map(|last| last.high)
    }
    fn fields(&self) -> Vec<(&'static str, f64)> {
        self.last
            .as_ref()
            .map(|last| {
                vec![
                    ("high", last.high),
                    ("low", last.low),
                    ("breakout_distance", last.breakout_distance),
                ]
            })
            .unwrap_or_default()
    }
    fn warmup(&self) -> usize {
        self.inner.warmup_period()
    }
}

impl<I> TickIndicator for ScalarPriceFields<I, wc::OvernightIntradayReturnOutput>
where
    I: Indicator<Input = f64, Output = wc::OvernightIntradayReturnOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = self.inner.update(input.price);
        self.last = out;
        self.last.as_ref().map(|last| last.overnight)
    }
    fn fields(&self) -> Vec<(&'static str, f64)> {
        self.last
            .as_ref()
            .map(|last| vec![("overnight", last.overnight), ("intraday", last.intraday)])
            .unwrap_or_default()
    }
    fn warmup(&self) -> usize {
        self.inner.warmup_period()
    }
}

impl<I> TickIndicator for CandleInFields<I, wc::OvernightIntradayReturnOutput>
where
    I: Indicator<Input = Candle, Output = wc::OvernightIntradayReturnOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = input.candle.and_then(|c| self.inner.update(c));
        self.last = out;
        self.last.as_ref().map(|last| last.overnight)
    }
    fn fields(&self) -> Vec<(&'static str, f64)> {
        self.last
            .as_ref()
            .map(|last| vec![("overnight", last.overnight), ("intraday", last.intraday)])
            .unwrap_or_default()
    }
    fn warmup(&self) -> usize {
        self.inner.warmup_period()
    }
}

impl<I> TickIndicator for ScalarPriceFields<I, wc::ProjectionBandsOutput>
where
    I: Indicator<Input = f64, Output = wc::ProjectionBandsOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = self.inner.update(input.price);
        self.last = out;
        self.last.as_ref().map(|last| last.upper)
    }
    fn fields(&self) -> Vec<(&'static str, f64)> {
        self.last
            .as_ref()
            .map(|last| {
                vec![
                    ("upper", last.upper),
                    ("middle", last.middle),
                    ("lower", last.lower),
                ]
            })
            .unwrap_or_default()
    }
    fn warmup(&self) -> usize {
        self.inner.warmup_period()
    }
}

impl<I> TickIndicator for CandleInFields<I, wc::ProjectionBandsOutput>
where
    I: Indicator<Input = Candle, Output = wc::ProjectionBandsOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = input.candle.and_then(|c| self.inner.update(c));
        self.last = out;
        self.last.as_ref().map(|last| last.upper)
    }
    fn fields(&self) -> Vec<(&'static str, f64)> {
        self.last
            .as_ref()
            .map(|last| {
                vec![
                    ("upper", last.upper),
                    ("middle", last.middle),
                    ("lower", last.lower),
                ]
            })
            .unwrap_or_default()
    }
    fn warmup(&self) -> usize {
        self.inner.warmup_period()
    }
}

impl<I> TickIndicator for ScalarPriceFields<I, wc::QqeOutput>
where
    I: Indicator<Input = f64, Output = wc::QqeOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = self.inner.update(input.price);
        self.last = out;
        self.last.as_ref().map(|last| last.rsi_ma)
    }
    fn fields(&self) -> Vec<(&'static str, f64)> {
        self.last
            .as_ref()
            .map(|last| {
                vec![
                    ("rsi_ma", last.rsi_ma),
                    ("trailing_line", last.trailing_line),
                ]
            })
            .unwrap_or_default()
    }
    fn warmup(&self) -> usize {
        self.inner.warmup_period()
    }
}

impl<I> TickIndicator for CandleInFields<I, wc::QqeOutput>
where
    I: Indicator<Input = Candle, Output = wc::QqeOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = input.candle.and_then(|c| self.inner.update(c));
        self.last = out;
        self.last.as_ref().map(|last| last.rsi_ma)
    }
    fn fields(&self) -> Vec<(&'static str, f64)> {
        self.last
            .as_ref()
            .map(|last| {
                vec![
                    ("rsi_ma", last.rsi_ma),
                    ("trailing_line", last.trailing_line),
                ]
            })
            .unwrap_or_default()
    }
    fn warmup(&self) -> usize {
        self.inner.warmup_period()
    }
}

impl<I> TickIndicator for ScalarPriceFields<I, wc::QuartileBandsOutput>
where
    I: Indicator<Input = f64, Output = wc::QuartileBandsOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = self.inner.update(input.price);
        self.last = out;
        self.last.as_ref().map(|last| last.upper)
    }
    fn fields(&self) -> Vec<(&'static str, f64)> {
        self.last
            .as_ref()
            .map(|last| {
                vec![
                    ("upper", last.upper),
                    ("middle", last.middle),
                    ("lower", last.lower),
                ]
            })
            .unwrap_or_default()
    }
    fn warmup(&self) -> usize {
        self.inner.warmup_period()
    }
}

impl<I> TickIndicator for CandleInFields<I, wc::QuartileBandsOutput>
where
    I: Indicator<Input = Candle, Output = wc::QuartileBandsOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = input.candle.and_then(|c| self.inner.update(c));
        self.last = out;
        self.last.as_ref().map(|last| last.upper)
    }
    fn fields(&self) -> Vec<(&'static str, f64)> {
        self.last
            .as_ref()
            .map(|last| {
                vec![
                    ("upper", last.upper),
                    ("middle", last.middle),
                    ("lower", last.lower),
                ]
            })
            .unwrap_or_default()
    }
    fn warmup(&self) -> usize {
        self.inner.warmup_period()
    }
}

impl<I> TickIndicator for ScalarPriceFields<I, wc::RwiOutput>
where
    I: Indicator<Input = f64, Output = wc::RwiOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = self.inner.update(input.price);
        self.last = out;
        self.last.as_ref().map(|last| last.high)
    }
    fn fields(&self) -> Vec<(&'static str, f64)> {
        self.last
            .as_ref()
            .map(|last| vec![("high", last.high), ("low", last.low)])
            .unwrap_or_default()
    }
    fn warmup(&self) -> usize {
        self.inner.warmup_period()
    }
}

impl<I> TickIndicator for CandleInFields<I, wc::RwiOutput>
where
    I: Indicator<Input = Candle, Output = wc::RwiOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = input.candle.and_then(|c| self.inner.update(c));
        self.last = out;
        self.last.as_ref().map(|last| last.high)
    }
    fn fields(&self) -> Vec<(&'static str, f64)> {
        self.last
            .as_ref()
            .map(|last| vec![("high", last.high), ("low", last.low)])
            .unwrap_or_default()
    }
    fn warmup(&self) -> usize {
        self.inner.warmup_period()
    }
}

impl<I> TickIndicator for ScalarPriceFields<I, wc::SessionHighLowOutput>
where
    I: Indicator<Input = f64, Output = wc::SessionHighLowOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = self.inner.update(input.price);
        self.last = out;
        self.last.as_ref().map(|last| last.high)
    }
    fn fields(&self) -> Vec<(&'static str, f64)> {
        self.last
            .as_ref()
            .map(|last| vec![("high", last.high), ("low", last.low)])
            .unwrap_or_default()
    }
    fn warmup(&self) -> usize {
        self.inner.warmup_period()
    }
}

impl<I> TickIndicator for CandleInFields<I, wc::SessionHighLowOutput>
where
    I: Indicator<Input = Candle, Output = wc::SessionHighLowOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = input.candle.and_then(|c| self.inner.update(c));
        self.last = out;
        self.last.as_ref().map(|last| last.high)
    }
    fn fields(&self) -> Vec<(&'static str, f64)> {
        self.last
            .as_ref()
            .map(|last| vec![("high", last.high), ("low", last.low)])
            .unwrap_or_default()
    }
    fn warmup(&self) -> usize {
        self.inner.warmup_period()
    }
}

impl<I> TickIndicator for ScalarPriceFields<I, wc::SessionRangeOutput>
where
    I: Indicator<Input = f64, Output = wc::SessionRangeOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = self.inner.update(input.price);
        self.last = out;
        self.last.as_ref().map(|last| last.asia)
    }
    fn fields(&self) -> Vec<(&'static str, f64)> {
        self.last
            .as_ref()
            .map(|last| vec![("asia", last.asia), ("eu", last.eu), ("us", last.us)])
            .unwrap_or_default()
    }
    fn warmup(&self) -> usize {
        self.inner.warmup_period()
    }
}

impl<I> TickIndicator for CandleInFields<I, wc::SessionRangeOutput>
where
    I: Indicator<Input = Candle, Output = wc::SessionRangeOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = input.candle.and_then(|c| self.inner.update(c));
        self.last = out;
        self.last.as_ref().map(|last| last.asia)
    }
    fn fields(&self) -> Vec<(&'static str, f64)> {
        self.last
            .as_ref()
            .map(|last| vec![("asia", last.asia), ("eu", last.eu), ("us", last.us)])
            .unwrap_or_default()
    }
    fn warmup(&self) -> usize {
        self.inner.warmup_period()
    }
}

impl<I> TickIndicator for ScalarPriceFields<I, wc::SmoothedHeikinAshiOutput>
where
    I: Indicator<Input = f64, Output = wc::SmoothedHeikinAshiOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = self.inner.update(input.price);
        self.last = out;
        self.last.as_ref().map(|last| last.open)
    }
    fn fields(&self) -> Vec<(&'static str, f64)> {
        self.last
            .as_ref()
            .map(|last| {
                vec![
                    ("open", last.open),
                    ("high", last.high),
                    ("low", last.low),
                    ("close", last.close),
                ]
            })
            .unwrap_or_default()
    }
    fn warmup(&self) -> usize {
        self.inner.warmup_period()
    }
}

impl<I> TickIndicator for CandleInFields<I, wc::SmoothedHeikinAshiOutput>
where
    I: Indicator<Input = Candle, Output = wc::SmoothedHeikinAshiOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = input.candle.and_then(|c| self.inner.update(c));
        self.last = out;
        self.last.as_ref().map(|last| last.open)
    }
    fn fields(&self) -> Vec<(&'static str, f64)> {
        self.last
            .as_ref()
            .map(|last| {
                vec![
                    ("open", last.open),
                    ("high", last.high),
                    ("low", last.low),
                    ("close", last.close),
                ]
            })
            .unwrap_or_default()
    }
    fn warmup(&self) -> usize {
        self.inner.warmup_period()
    }
}

impl<I> TickIndicator for ScalarPriceFields<I, wc::StandardErrorBandsOutput>
where
    I: Indicator<Input = f64, Output = wc::StandardErrorBandsOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = self.inner.update(input.price);
        self.last = out;
        self.last.as_ref().map(|last| last.upper)
    }
    fn fields(&self) -> Vec<(&'static str, f64)> {
        self.last
            .as_ref()
            .map(|last| {
                vec![
                    ("upper", last.upper),
                    ("middle", last.middle),
                    ("lower", last.lower),
                ]
            })
            .unwrap_or_default()
    }
    fn warmup(&self) -> usize {
        self.inner.warmup_period()
    }
}

impl<I> TickIndicator for CandleInFields<I, wc::StandardErrorBandsOutput>
where
    I: Indicator<Input = Candle, Output = wc::StandardErrorBandsOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = input.candle.and_then(|c| self.inner.update(c));
        self.last = out;
        self.last.as_ref().map(|last| last.upper)
    }
    fn fields(&self) -> Vec<(&'static str, f64)> {
        self.last
            .as_ref()
            .map(|last| {
                vec![
                    ("upper", last.upper),
                    ("middle", last.middle),
                    ("lower", last.lower),
                ]
            })
            .unwrap_or_default()
    }
    fn warmup(&self) -> usize {
        self.inner.warmup_period()
    }
}

impl<I> TickIndicator for ScalarPriceFields<I, wc::StarcBandsOutput>
where
    I: Indicator<Input = f64, Output = wc::StarcBandsOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = self.inner.update(input.price);
        self.last = out;
        self.last.as_ref().map(|last| last.upper)
    }
    fn fields(&self) -> Vec<(&'static str, f64)> {
        self.last
            .as_ref()
            .map(|last| {
                vec![
                    ("upper", last.upper),
                    ("middle", last.middle),
                    ("lower", last.lower),
                ]
            })
            .unwrap_or_default()
    }
    fn warmup(&self) -> usize {
        self.inner.warmup_period()
    }
}

impl<I> TickIndicator for CandleInFields<I, wc::StarcBandsOutput>
where
    I: Indicator<Input = Candle, Output = wc::StarcBandsOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = input.candle.and_then(|c| self.inner.update(c));
        self.last = out;
        self.last.as_ref().map(|last| last.upper)
    }
    fn fields(&self) -> Vec<(&'static str, f64)> {
        self.last
            .as_ref()
            .map(|last| {
                vec![
                    ("upper", last.upper),
                    ("middle", last.middle),
                    ("lower", last.lower),
                ]
            })
            .unwrap_or_default()
    }
    fn warmup(&self) -> usize {
        self.inner.warmup_period()
    }
}

impl<I> TickIndicator for ScalarPriceFields<I, wc::StochasticOutput>
where
    I: Indicator<Input = f64, Output = wc::StochasticOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = self.inner.update(input.price);
        self.last = out;
        self.last.as_ref().map(|last| last.k)
    }
    fn fields(&self) -> Vec<(&'static str, f64)> {
        self.last
            .as_ref()
            .map(|last| vec![("k", last.k), ("d", last.d)])
            .unwrap_or_default()
    }
    fn warmup(&self) -> usize {
        self.inner.warmup_period()
    }
}

impl<I> TickIndicator for CandleInFields<I, wc::StochasticOutput>
where
    I: Indicator<Input = Candle, Output = wc::StochasticOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = input.candle.and_then(|c| self.inner.update(c));
        self.last = out;
        self.last.as_ref().map(|last| last.k)
    }
    fn fields(&self) -> Vec<(&'static str, f64)> {
        self.last
            .as_ref()
            .map(|last| vec![("k", last.k), ("d", last.d)])
            .unwrap_or_default()
    }
    fn warmup(&self) -> usize {
        self.inner.warmup_period()
    }
}

impl<I> TickIndicator for ScalarPriceFields<I, wc::SuperTrendOutput>
where
    I: Indicator<Input = f64, Output = wc::SuperTrendOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = self.inner.update(input.price);
        self.last = out;
        self.last.as_ref().map(|last| last.value)
    }
    fn fields(&self) -> Vec<(&'static str, f64)> {
        self.last
            .as_ref()
            .map(|last| vec![("value", last.value), ("direction", last.direction)])
            .unwrap_or_default()
    }
    fn warmup(&self) -> usize {
        self.inner.warmup_period()
    }
}

impl<I> TickIndicator for CandleInFields<I, wc::SuperTrendOutput>
where
    I: Indicator<Input = Candle, Output = wc::SuperTrendOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = input.candle.and_then(|c| self.inner.update(c));
        self.last = out;
        self.last.as_ref().map(|last| last.value)
    }
    fn fields(&self) -> Vec<(&'static str, f64)> {
        self.last
            .as_ref()
            .map(|last| vec![("value", last.value), ("direction", last.direction)])
            .unwrap_or_default()
    }
    fn warmup(&self) -> usize {
        self.inner.warmup_period()
    }
}

impl<I> TickIndicator for ScalarPriceFields<I, wc::TdLinesOutput>
where
    I: Indicator<Input = f64, Output = wc::TdLinesOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = self.inner.update(input.price);
        self.last = out;
        self.last.as_ref().map(|last| last.resistance)
    }
    fn fields(&self) -> Vec<(&'static str, f64)> {
        self.last
            .as_ref()
            .map(|last| vec![("resistance", last.resistance), ("support", last.support)])
            .unwrap_or_default()
    }
    fn warmup(&self) -> usize {
        self.inner.warmup_period()
    }
}

impl<I> TickIndicator for CandleInFields<I, wc::TdLinesOutput>
where
    I: Indicator<Input = Candle, Output = wc::TdLinesOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = input.candle.and_then(|c| self.inner.update(c));
        self.last = out;
        self.last.as_ref().map(|last| last.resistance)
    }
    fn fields(&self) -> Vec<(&'static str, f64)> {
        self.last
            .as_ref()
            .map(|last| vec![("resistance", last.resistance), ("support", last.support)])
            .unwrap_or_default()
    }
    fn warmup(&self) -> usize {
        self.inner.warmup_period()
    }
}

impl<I> TickIndicator for ScalarPriceFields<I, wc::TdMovingAverageOutput>
where
    I: Indicator<Input = f64, Output = wc::TdMovingAverageOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = self.inner.update(input.price);
        self.last = out;
        self.last.as_ref().map(|last| last.st1)
    }
    fn fields(&self) -> Vec<(&'static str, f64)> {
        self.last
            .as_ref()
            .map(|last| vec![("st1", last.st1), ("st2", last.st2)])
            .unwrap_or_default()
    }
    fn warmup(&self) -> usize {
        self.inner.warmup_period()
    }
}

impl<I> TickIndicator for CandleInFields<I, wc::TdMovingAverageOutput>
where
    I: Indicator<Input = Candle, Output = wc::TdMovingAverageOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = input.candle.and_then(|c| self.inner.update(c));
        self.last = out;
        self.last.as_ref().map(|last| last.st1)
    }
    fn fields(&self) -> Vec<(&'static str, f64)> {
        self.last
            .as_ref()
            .map(|last| vec![("st1", last.st1), ("st2", last.st2)])
            .unwrap_or_default()
    }
    fn warmup(&self) -> usize {
        self.inner.warmup_period()
    }
}

impl<I> TickIndicator for ScalarPriceFields<I, wc::TdRangeProjectionOutput>
where
    I: Indicator<Input = f64, Output = wc::TdRangeProjectionOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = self.inner.update(input.price);
        self.last = out;
        self.last.as_ref().map(|last| last.high)
    }
    fn fields(&self) -> Vec<(&'static str, f64)> {
        self.last
            .as_ref()
            .map(|last| vec![("high", last.high), ("low", last.low)])
            .unwrap_or_default()
    }
    fn warmup(&self) -> usize {
        self.inner.warmup_period()
    }
}

impl<I> TickIndicator for CandleInFields<I, wc::TdRangeProjectionOutput>
where
    I: Indicator<Input = Candle, Output = wc::TdRangeProjectionOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = input.candle.and_then(|c| self.inner.update(c));
        self.last = out;
        self.last.as_ref().map(|last| last.high)
    }
    fn fields(&self) -> Vec<(&'static str, f64)> {
        self.last
            .as_ref()
            .map(|last| vec![("high", last.high), ("low", last.low)])
            .unwrap_or_default()
    }
    fn warmup(&self) -> usize {
        self.inner.warmup_period()
    }
}

impl<I> TickIndicator for ScalarPriceFields<I, wc::TdRiskLevelOutput>
where
    I: Indicator<Input = f64, Output = wc::TdRiskLevelOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = self.inner.update(input.price);
        self.last = out;
        self.last.as_ref().map(|last| last.buy_risk)
    }
    fn fields(&self) -> Vec<(&'static str, f64)> {
        self.last
            .as_ref()
            .map(|last| vec![("buy_risk", last.buy_risk), ("sell_risk", last.sell_risk)])
            .unwrap_or_default()
    }
    fn warmup(&self) -> usize {
        self.inner.warmup_period()
    }
}

impl<I> TickIndicator for CandleInFields<I, wc::TdRiskLevelOutput>
where
    I: Indicator<Input = Candle, Output = wc::TdRiskLevelOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = input.candle.and_then(|c| self.inner.update(c));
        self.last = out;
        self.last.as_ref().map(|last| last.buy_risk)
    }
    fn fields(&self) -> Vec<(&'static str, f64)> {
        self.last
            .as_ref()
            .map(|last| vec![("buy_risk", last.buy_risk), ("sell_risk", last.sell_risk)])
            .unwrap_or_default()
    }
    fn warmup(&self) -> usize {
        self.inner.warmup_period()
    }
}

impl<I> TickIndicator for ScalarPriceFields<I, wc::TdSequentialOutput>
where
    I: Indicator<Input = f64, Output = wc::TdSequentialOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = self.inner.update(input.price);
        self.last = out;
        self.last.as_ref().map(|last| last.setup)
    }
    fn fields(&self) -> Vec<(&'static str, f64)> {
        self.last
            .as_ref()
            .map(|last| {
                vec![
                    ("setup", last.setup),
                    ("countdown", last.countdown),
                    ("direction", last.direction),
                ]
            })
            .unwrap_or_default()
    }
    fn warmup(&self) -> usize {
        self.inner.warmup_period()
    }
}

impl<I> TickIndicator for CandleInFields<I, wc::TdSequentialOutput>
where
    I: Indicator<Input = Candle, Output = wc::TdSequentialOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = input.candle.and_then(|c| self.inner.update(c));
        self.last = out;
        self.last.as_ref().map(|last| last.setup)
    }
    fn fields(&self) -> Vec<(&'static str, f64)> {
        self.last
            .as_ref()
            .map(|last| {
                vec![
                    ("setup", last.setup),
                    ("countdown", last.countdown),
                    ("direction", last.direction),
                ]
            })
            .unwrap_or_default()
    }
    fn warmup(&self) -> usize {
        self.inner.warmup_period()
    }
}

impl<I> TickIndicator for ScalarPriceFields<I, wc::TpoProfileOutput>
where
    I: Indicator<Input = f64, Output = wc::TpoProfileOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = self.inner.update(input.price);
        self.last = out;
        self.last.as_ref().map(|last| last.price_low)
    }
    fn fields(&self) -> Vec<(&'static str, f64)> {
        self.last
            .as_ref()
            .map(|last| {
                vec![
                    ("price_low", last.price_low),
                    ("price_high", last.price_high),
                ]
            })
            .unwrap_or_default()
    }
    fn warmup(&self) -> usize {
        self.inner.warmup_period()
    }
}

impl<I> TickIndicator for CandleInFields<I, wc::TpoProfileOutput>
where
    I: Indicator<Input = Candle, Output = wc::TpoProfileOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = input.candle.and_then(|c| self.inner.update(c));
        self.last = out;
        self.last.as_ref().map(|last| last.price_low)
    }
    fn fields(&self) -> Vec<(&'static str, f64)> {
        self.last
            .as_ref()
            .map(|last| {
                vec![
                    ("price_low", last.price_low),
                    ("price_high", last.price_high),
                ]
            })
            .unwrap_or_default()
    }
    fn warmup(&self) -> usize {
        self.inner.warmup_period()
    }
}

impl<I> TickIndicator for ScalarPriceFields<I, wc::TtmSqueezeOutput>
where
    I: Indicator<Input = f64, Output = wc::TtmSqueezeOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = self.inner.update(input.price);
        self.last = out;
        self.last.as_ref().map(|last| last.squeeze)
    }
    fn fields(&self) -> Vec<(&'static str, f64)> {
        self.last
            .as_ref()
            .map(|last| vec![("squeeze", last.squeeze), ("momentum", last.momentum)])
            .unwrap_or_default()
    }
    fn warmup(&self) -> usize {
        self.inner.warmup_period()
    }
}

impl<I> TickIndicator for CandleInFields<I, wc::TtmSqueezeOutput>
where
    I: Indicator<Input = Candle, Output = wc::TtmSqueezeOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = input.candle.and_then(|c| self.inner.update(c));
        self.last = out;
        self.last.as_ref().map(|last| last.squeeze)
    }
    fn fields(&self) -> Vec<(&'static str, f64)> {
        self.last
            .as_ref()
            .map(|last| vec![("squeeze", last.squeeze), ("momentum", last.momentum)])
            .unwrap_or_default()
    }
    fn warmup(&self) -> usize {
        self.inner.warmup_period()
    }
}

impl<I> TickIndicator for ScalarPriceFields<I, wc::ValueAreaOutput>
where
    I: Indicator<Input = f64, Output = wc::ValueAreaOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = self.inner.update(input.price);
        self.last = out;
        self.last.as_ref().map(|last| last.poc)
    }
    fn fields(&self) -> Vec<(&'static str, f64)> {
        self.last
            .as_ref()
            .map(|last| vec![("poc", last.poc), ("vah", last.vah), ("val", last.val)])
            .unwrap_or_default()
    }
    fn warmup(&self) -> usize {
        self.inner.warmup_period()
    }
}

impl<I> TickIndicator for CandleInFields<I, wc::ValueAreaOutput>
where
    I: Indicator<Input = Candle, Output = wc::ValueAreaOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = input.candle.and_then(|c| self.inner.update(c));
        self.last = out;
        self.last.as_ref().map(|last| last.poc)
    }
    fn fields(&self) -> Vec<(&'static str, f64)> {
        self.last
            .as_ref()
            .map(|last| vec![("poc", last.poc), ("vah", last.vah), ("val", last.val)])
            .unwrap_or_default()
    }
    fn warmup(&self) -> usize {
        self.inner.warmup_period()
    }
}

impl<I> TickIndicator for ScalarPriceFields<I, wc::VolatilityConeOutput>
where
    I: Indicator<Input = f64, Output = wc::VolatilityConeOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = self.inner.update(input.price);
        self.last = out;
        self.last.as_ref().map(|last| last.current)
    }
    fn fields(&self) -> Vec<(&'static str, f64)> {
        self.last
            .as_ref()
            .map(|last| {
                vec![
                    ("current", last.current),
                    ("min", last.min),
                    ("median", last.median),
                    ("max", last.max),
                    ("percentile", last.percentile),
                ]
            })
            .unwrap_or_default()
    }
    fn warmup(&self) -> usize {
        self.inner.warmup_period()
    }
}

impl<I> TickIndicator for CandleInFields<I, wc::VolatilityConeOutput>
where
    I: Indicator<Input = Candle, Output = wc::VolatilityConeOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = input.candle.and_then(|c| self.inner.update(c));
        self.last = out;
        self.last.as_ref().map(|last| last.current)
    }
    fn fields(&self) -> Vec<(&'static str, f64)> {
        self.last
            .as_ref()
            .map(|last| {
                vec![
                    ("current", last.current),
                    ("min", last.min),
                    ("median", last.median),
                    ("max", last.max),
                    ("percentile", last.percentile),
                ]
            })
            .unwrap_or_default()
    }
    fn warmup(&self) -> usize {
        self.inner.warmup_period()
    }
}

impl<I> TickIndicator for ScalarPriceFields<I, wc::VolumeProfileOutput>
where
    I: Indicator<Input = f64, Output = wc::VolumeProfileOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = self.inner.update(input.price);
        self.last = out;
        self.last.as_ref().map(|last| last.price_low)
    }
    fn fields(&self) -> Vec<(&'static str, f64)> {
        self.last
            .as_ref()
            .map(|last| {
                vec![
                    ("price_low", last.price_low),
                    ("price_high", last.price_high),
                ]
            })
            .unwrap_or_default()
    }
    fn warmup(&self) -> usize {
        self.inner.warmup_period()
    }
}

impl<I> TickIndicator for CandleInFields<I, wc::VolumeProfileOutput>
where
    I: Indicator<Input = Candle, Output = wc::VolumeProfileOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = input.candle.and_then(|c| self.inner.update(c));
        self.last = out;
        self.last.as_ref().map(|last| last.price_low)
    }
    fn fields(&self) -> Vec<(&'static str, f64)> {
        self.last
            .as_ref()
            .map(|last| {
                vec![
                    ("price_low", last.price_low),
                    ("price_high", last.price_high),
                ]
            })
            .unwrap_or_default()
    }
    fn warmup(&self) -> usize {
        self.inner.warmup_period()
    }
}

impl<I> TickIndicator for ScalarPriceFields<I, wc::VolumeWeightedMacdOutput>
where
    I: Indicator<Input = f64, Output = wc::VolumeWeightedMacdOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = self.inner.update(input.price);
        self.last = out;
        self.last.as_ref().map(|last| last.macd)
    }
    fn fields(&self) -> Vec<(&'static str, f64)> {
        self.last
            .as_ref()
            .map(|last| {
                vec![
                    ("macd", last.macd),
                    ("signal", last.signal),
                    ("histogram", last.histogram),
                ]
            })
            .unwrap_or_default()
    }
    fn warmup(&self) -> usize {
        self.inner.warmup_period()
    }
}

impl<I> TickIndicator for CandleInFields<I, wc::VolumeWeightedMacdOutput>
where
    I: Indicator<Input = Candle, Output = wc::VolumeWeightedMacdOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = input.candle.and_then(|c| self.inner.update(c));
        self.last = out;
        self.last.as_ref().map(|last| last.macd)
    }
    fn fields(&self) -> Vec<(&'static str, f64)> {
        self.last
            .as_ref()
            .map(|last| {
                vec![
                    ("macd", last.macd),
                    ("signal", last.signal),
                    ("histogram", last.histogram),
                ]
            })
            .unwrap_or_default()
    }
    fn warmup(&self) -> usize {
        self.inner.warmup_period()
    }
}

impl<I> TickIndicator for ScalarPriceFields<I, wc::VolumeWeightedSrOutput>
where
    I: Indicator<Input = f64, Output = wc::VolumeWeightedSrOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = self.inner.update(input.price);
        self.last = out;
        self.last.as_ref().map(|last| last.support)
    }
    fn fields(&self) -> Vec<(&'static str, f64)> {
        self.last
            .as_ref()
            .map(|last| vec![("support", last.support), ("resistance", last.resistance)])
            .unwrap_or_default()
    }
    fn warmup(&self) -> usize {
        self.inner.warmup_period()
    }
}

impl<I> TickIndicator for CandleInFields<I, wc::VolumeWeightedSrOutput>
where
    I: Indicator<Input = Candle, Output = wc::VolumeWeightedSrOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = input.candle.and_then(|c| self.inner.update(c));
        self.last = out;
        self.last.as_ref().map(|last| last.support)
    }
    fn fields(&self) -> Vec<(&'static str, f64)> {
        self.last
            .as_ref()
            .map(|last| vec![("support", last.support), ("resistance", last.resistance)])
            .unwrap_or_default()
    }
    fn warmup(&self) -> usize {
        self.inner.warmup_period()
    }
}

impl<I> TickIndicator for ScalarPriceFields<I, wc::VortexOutput>
where
    I: Indicator<Input = f64, Output = wc::VortexOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = self.inner.update(input.price);
        self.last = out;
        self.last.as_ref().map(|last| last.plus)
    }
    fn fields(&self) -> Vec<(&'static str, f64)> {
        self.last
            .as_ref()
            .map(|last| vec![("plus", last.plus), ("minus", last.minus)])
            .unwrap_or_default()
    }
    fn warmup(&self) -> usize {
        self.inner.warmup_period()
    }
}

impl<I> TickIndicator for CandleInFields<I, wc::VortexOutput>
where
    I: Indicator<Input = Candle, Output = wc::VortexOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = input.candle.and_then(|c| self.inner.update(c));
        self.last = out;
        self.last.as_ref().map(|last| last.plus)
    }
    fn fields(&self) -> Vec<(&'static str, f64)> {
        self.last
            .as_ref()
            .map(|last| vec![("plus", last.plus), ("minus", last.minus)])
            .unwrap_or_default()
    }
    fn warmup(&self) -> usize {
        self.inner.warmup_period()
    }
}

impl<I> TickIndicator for ScalarPriceFields<I, wc::VwapStdDevBandsOutput>
where
    I: Indicator<Input = f64, Output = wc::VwapStdDevBandsOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = self.inner.update(input.price);
        self.last = out;
        self.last.as_ref().map(|last| last.upper)
    }
    fn fields(&self) -> Vec<(&'static str, f64)> {
        self.last
            .as_ref()
            .map(|last| {
                vec![
                    ("upper", last.upper),
                    ("middle", last.middle),
                    ("lower", last.lower),
                    ("stddev", last.stddev),
                ]
            })
            .unwrap_or_default()
    }
    fn warmup(&self) -> usize {
        self.inner.warmup_period()
    }
}

impl<I> TickIndicator for CandleInFields<I, wc::VwapStdDevBandsOutput>
where
    I: Indicator<Input = Candle, Output = wc::VwapStdDevBandsOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = input.candle.and_then(|c| self.inner.update(c));
        self.last = out;
        self.last.as_ref().map(|last| last.upper)
    }
    fn fields(&self) -> Vec<(&'static str, f64)> {
        self.last
            .as_ref()
            .map(|last| {
                vec![
                    ("upper", last.upper),
                    ("middle", last.middle),
                    ("lower", last.lower),
                    ("stddev", last.stddev),
                ]
            })
            .unwrap_or_default()
    }
    fn warmup(&self) -> usize {
        self.inner.warmup_period()
    }
}

impl<I> TickIndicator for ScalarPriceFields<I, wc::WaveTrendOutput>
where
    I: Indicator<Input = f64, Output = wc::WaveTrendOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = self.inner.update(input.price);
        self.last = out;
        self.last.as_ref().map(|last| last.wt1)
    }
    fn fields(&self) -> Vec<(&'static str, f64)> {
        self.last
            .as_ref()
            .map(|last| vec![("wt1", last.wt1), ("wt2", last.wt2)])
            .unwrap_or_default()
    }
    fn warmup(&self) -> usize {
        self.inner.warmup_period()
    }
}

impl<I> TickIndicator for CandleInFields<I, wc::WaveTrendOutput>
where
    I: Indicator<Input = Candle, Output = wc::WaveTrendOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = input.candle.and_then(|c| self.inner.update(c));
        self.last = out;
        self.last.as_ref().map(|last| last.wt1)
    }
    fn fields(&self) -> Vec<(&'static str, f64)> {
        self.last
            .as_ref()
            .map(|last| vec![("wt1", last.wt1), ("wt2", last.wt2)])
            .unwrap_or_default()
    }
    fn warmup(&self) -> usize {
        self.inner.warmup_period()
    }
}

impl<I> TickIndicator for ScalarPriceFields<I, wc::WoodiePivotsOutput>
where
    I: Indicator<Input = f64, Output = wc::WoodiePivotsOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = self.inner.update(input.price);
        self.last = out;
        self.last.as_ref().map(|last| last.pp)
    }
    fn fields(&self) -> Vec<(&'static str, f64)> {
        self.last
            .as_ref()
            .map(|last| {
                vec![
                    ("pp", last.pp),
                    ("r1", last.r1),
                    ("r2", last.r2),
                    ("s1", last.s1),
                    ("s2", last.s2),
                ]
            })
            .unwrap_or_default()
    }
    fn warmup(&self) -> usize {
        self.inner.warmup_period()
    }
}

impl<I> TickIndicator for CandleInFields<I, wc::WoodiePivotsOutput>
where
    I: Indicator<Input = Candle, Output = wc::WoodiePivotsOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = input.candle.and_then(|c| self.inner.update(c));
        self.last = out;
        self.last.as_ref().map(|last| last.pp)
    }
    fn fields(&self) -> Vec<(&'static str, f64)> {
        self.last
            .as_ref()
            .map(|last| {
                vec![
                    ("pp", last.pp),
                    ("r1", last.r1),
                    ("r2", last.r2),
                    ("s1", last.s1),
                    ("s2", last.s2),
                ]
            })
            .unwrap_or_default()
    }
    fn warmup(&self) -> usize {
        self.inner.warmup_period()
    }
}

impl<I> TickIndicator for ScalarPriceFields<I, wc::ZeroLagMacdOutput>
where
    I: Indicator<Input = f64, Output = wc::ZeroLagMacdOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = self.inner.update(input.price);
        self.last = out;
        self.last.as_ref().map(|last| last.macd)
    }
    fn fields(&self) -> Vec<(&'static str, f64)> {
        self.last
            .as_ref()
            .map(|last| {
                vec![
                    ("macd", last.macd),
                    ("signal", last.signal),
                    ("histogram", last.histogram),
                ]
            })
            .unwrap_or_default()
    }
    fn warmup(&self) -> usize {
        self.inner.warmup_period()
    }
}

impl<I> TickIndicator for CandleInFields<I, wc::ZeroLagMacdOutput>
where
    I: Indicator<Input = Candle, Output = wc::ZeroLagMacdOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = input.candle.and_then(|c| self.inner.update(c));
        self.last = out;
        self.last.as_ref().map(|last| last.macd)
    }
    fn fields(&self) -> Vec<(&'static str, f64)> {
        self.last
            .as_ref()
            .map(|last| {
                vec![
                    ("macd", last.macd),
                    ("signal", last.signal),
                    ("histogram", last.histogram),
                ]
            })
            .unwrap_or_default()
    }
    fn warmup(&self) -> usize {
        self.inner.warmup_period()
    }
}

impl<I> TickIndicator for ScalarPriceFields<I, wc::ZigZagOutput>
where
    I: Indicator<Input = f64, Output = wc::ZigZagOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = self.inner.update(input.price);
        self.last = out;
        self.last.as_ref().map(|last| last.swing)
    }
    fn fields(&self) -> Vec<(&'static str, f64)> {
        self.last
            .as_ref()
            .map(|last| vec![("swing", last.swing), ("direction", last.direction)])
            .unwrap_or_default()
    }
    fn warmup(&self) -> usize {
        self.inner.warmup_period()
    }
}

impl<I> TickIndicator for CandleInFields<I, wc::ZigZagOutput>
where
    I: Indicator<Input = Candle, Output = wc::ZigZagOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = input.candle.and_then(|c| self.inner.update(c));
        self.last = out;
        self.last.as_ref().map(|last| last.swing)
    }
    fn fields(&self) -> Vec<(&'static str, f64)> {
        self.last
            .as_ref()
            .map(|last| vec![("swing", last.swing), ("direction", last.direction)])
            .unwrap_or_default()
    }
    fn warmup(&self) -> usize {
        self.inner.warmup_period()
    }
}

/// Read a positional parameter as a count.
fn usize_param(params: &[f64], idx: usize, kind: &str) -> Result<usize> {
    let value = params
        .get(idx)
        .copied()
        .ok_or_else(|| Error::Config(format!("{kind}: missing parameter {idx}")))?;
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

/// Every registered indicator name, sorted.
pub const KINDS: [&str; 423] = [
    "AbandonedBaby",
    "Abcd",
    "AccelerationBands",
    "AcceleratorOscillator",
    "AdOscillator",
    "AdaptiveCci",
    "AdaptiveCycle",
    "AdaptiveLaguerreFilter",
    "AdaptiveRsi",
    "Adl",
    "AdvanceBlock",
    "Adx",
    "Adxr",
    "Alligator",
    "Alma",
    "AnchoredRsi",
    "AnchoredVwap",
    "AndrewsPitchfork",
    "Apo",
    "Aroon",
    "AroonOscillator",
    "Atr",
    "AtrBands",
    "AtrRatchet",
    "AtrTrailingStop",
    "AutoFib",
    "Autocorrelation",
    "AutocorrelationPeriodogram",
    "AverageDailyRange",
    "AverageDrawdown",
    "AvgPrice",
    "AwesomeOscillator",
    "AwesomeOscillatorHistogram",
    "BalanceOfPower",
    "BandpassFilter",
    "Bat",
    "BeltHold",
    "BetterVolume",
    "BipowerVariation",
    "BodySizePct",
    "Bollinger",
    "BollingerBands",
    "BollingerBandwidth",
    "BomarBands",
    "Breakaway",
    "BurkeRatio",
    "Butterfly",
    "CalmarRatio",
    "Camarilla",
    "CandleVolume",
    "Cci",
    "CenterOfGravity",
    "CentralPivotRange",
    "Cfo",
    "ChaikinMoneyFlow",
    "ChaikinOscillator",
    "ChaikinVolatility",
    "ChandeKrollStop",
    "ChandelierExit",
    "ChoppinessIndex",
    "ClassicPivots",
    "CloseVsOpen",
    "ClosingMarubozu",
    "Cmo",
    "CoefficientOfVariation",
    "CommonSenseRatio",
    "CompositeProfile",
    "ConcealingBabySwallow",
    "ConditionalValueAtRisk",
    "ConnorsRsi",
    "Coppock",
    "CorrelationTrendIndicator",
    "Counterattack",
    "Crab",
    "CupAndHandle",
    "CyberneticCycle",
    "Cypher",
    "Decycler",
    "DecyclerOscillator",
    "Dema",
    "DemandIndex",
    "DemarkPivots",
    "DerivativeOscillator",
    "DetrendedStdDev",
    "DisparityIndex",
    "Doji",
    "DojiStar",
    "Donchian",
    "DonchianStop",
    "DoubleBollinger",
    "DoubleTopBottom",
    "DownsideGapThreeMethods",
    "Dpo",
    "DragonflyDoji",
    "DumplingTop",
    "Dx",
    "DynamicMomentumIndex",
    "EaseOfMovement",
    "EhlersStochastic",
    "Ehma",
    "ElderImpulse",
    "ElderRay",
    "ElderSafeZone",
    "Ema",
    "EmpiricalModeDecomposition",
    "Engulfing",
    "Equivolume",
    "EvenBetterSinewave",
    "EveningDojiStar",
    "Evwma",
    "EwmaVolatility",
    "Expectancy",
    "FallingThreeMethods",
    "Fama",
    "FibArcs",
    "FibChannel",
    "FibConfluence",
    "FibExtension",
    "FibFan",
    "FibProjection",
    "FibRetracement",
    "FibTimeZones",
    "FibonacciPivots",
    "FisherRsi",
    "FisherTransform",
    "FlagPennant",
    "ForceIndex",
    "FractalChaosBands",
    "Frama",
    "FryPanBottom",
    "GainLossRatio",
    "GainToPainRatio",
    "GapSideBySideWhite",
    "Garch11",
    "GarmanKlassVolatility",
    "Gartley",
    "GatorOscillator",
    "GeneralizedDema",
    "GeometricMa",
    "GoldenPocket",
    "GravestoneDoji",
    "Hammer",
    "HangingMan",
    "Harami",
    "HaramiCross",
    "HeadAndShoulders",
    "HeikinAshi",
    "HeikinAshiOscillator",
    "HiLoActivator",
    "HighLowRange",
    "HighLowVolumeNodes",
    "HighWave",
    "HighpassFilter",
    "Hikkake",
    "HikkakeModified",
    "HilbertDominantCycle",
    "HistoricalVolatility",
    "Hma",
    "HoltWinters",
    "HomingPigeon",
    "HtDcPhase",
    "HtPhasor",
    "HtTrendMode",
    "HurstChannel",
    "HurstExponent",
    "IdenticalThreeCrows",
    "InNeck",
    "Inertia",
    "InitialBalance",
    "InstantaneousTrendline",
    "IntradayIntensity",
    "IntradayMomentumIndex",
    "InverseFisherTransform",
    "InvertedHammer",
    "JarqueBera",
    "Jma",
    "JumpIndicator",
    "KRatio",
    "Kama",
    "KaseDevStop",
    "KasePermissionStochastic",
    "KellyCriterion",
    "Keltner",
    "Kicking",
    "KickingByLength",
    "Kst",
    "Kurtosis",
    "Kvo",
    "LadderBottom",
    "LaguerreRsi",
    "LinRegAngle",
    "LinRegChannel",
    "LinRegIntercept",
    "LinRegSlope",
    "LinearRegression",
    "LogReturn",
    "LongLeggedDoji",
    "LongLine",
    "M2Measure",
    "MaEnvelope",
    "Macd",
    "MacdFix",
    "MacdHistogram",
    "MacdIndicator",
    "Mama",
    "MarketFacilitationIndex",
    "MartinRatio",
    "Marubozu",
    "MassIndex",
    "MatHold",
    "MatchingLow",
    "MaxDrawdown",
    "McGinleyDynamic",
    "MedianAbsoluteDeviation",
    "MedianChannel",
    "MedianMa",
    "MedianPrice",
    "Mfi",
    "MidPoint",
    "MidPrice",
    "MinusDi",
    "MinusDm",
    "ModifiedMaStop",
    "Mom",
    "MorningDojiStar",
    "MorningEveningStar",
    "MurreyMathLines",
    "NakedPoc",
    "Natr",
    "NewPriceLines",
    "Nrtr",
    "Nvi",
    "Obv",
    "OmegaRatio",
    "OnNeck",
    "OpeningMarubozu",
    "OpeningRange",
    "OvernightGap",
    "OvernightIntradayReturn",
    "PainIndex",
    "ParkinsonVolatility",
    "PercentB",
    "PercentageTrailingStop",
    "Pgo",
    "PiercingDarkCloud",
    "PivotReversal",
    "PlusDi",
    "PlusDm",
    "Pmo",
    "PolarizedFractalEfficiency",
    "Ppo",
    "PpoHistogram",
    "ProfileShape",
    "ProfitFactor",
    "ProjectionBands",
    "ProjectionOscillator",
    "Psar",
    "Pvi",
    "Qqe",
    "Qstick",
    "QuartileBands",
    "RSquared",
    "RealizedVolatility",
    "RecoveryFactor",
    "RectangleRange",
    "Reflex",
    "RegimeLabel",
    "RenkoTrailingStop",
    "RickshawMan",
    "RisingThreeMethods",
    "Rmi",
    "Roc",
    "Rocp",
    "Rocr",
    "Rocr100",
    "RogersSatchellVolatility",
    "RollingIqr",
    "RollingMinMaxScaler",
    "RollingPercentileRank",
    "RollingQuantile",
    "RollingVwap",
    "RoofingFilter",
    "Rsi",
    "Rsx",
    "Rvi",
    "RviVolatility",
    "Rwi",
    "SampleEntropy",
    "SarExt",
    "SeasonalZScore",
    "SeparatingLines",
    "SessionHighLow",
    "SessionRange",
    "SessionVwap",
    "ShannonEntropy",
    "Shark",
    "SharpeRatio",
    "ShootingStar",
    "ShortLine",
    "SineWave",
    "SineWeightedMa",
    "SinglePrints",
    "Skewness",
    "Sma",
    "Smi",
    "Smma",
    "SmoothedHeikinAshi",
    "SortinoRatio",
    "SpinningTop",
    "StalledPattern",
    "StandardError",
    "StandardErrorBands",
    "StarcBands",
    "Stc",
    "StdDev",
    "StepTrailingStop",
    "SterlingRatio",
    "StickSandwich",
    "StochRsi",
    "Stochastic",
    "StochasticCci",
    "SuperSmoother",
    "SuperTrend",
    "T3",
    "TailRatio",
    "Takuri",
    "TasukiGap",
    "TdCamouflage",
    "TdClop",
    "TdClopwin",
    "TdCombo",
    "TdCountdown",
    "TdDWave",
    "TdDeMarker",
    "TdDifferential",
    "TdLines",
    "TdMovingAverage",
    "TdOpen",
    "TdPressure",
    "TdPropulsion",
    "TdRangeProjection",
    "TdRei",
    "TdRiskLevel",
    "TdSequential",
    "TdSetup",
    "TdTrap",
    "Tema",
    "ThreeDrives",
    "ThreeInside",
    "ThreeLineBreak",
    "ThreeLineStrike",
    "ThreeOutside",
    "ThreeSoldiersOrCrows",
    "ThreeStarsInSouth",
    "Thrusting",
    "Tii",
    "TimeBasedStop",
    "TowerTopBottom",
    "TpoProfile",
    "TradeVolumeIndex",
    "TrendLabel",
    "TrendStrengthIndex",
    "Trendflex",
    "Triangle",
    "Trima",
    "TripleTopBottom",
    "Tristar",
    "Trix",
    "TrueRange",
    "Tsf",
    "TsfOscillator",
    "Tsi",
    "Tsv",
    "TtmSqueeze",
    "TtmTrend",
    "TurnOfMonth",
    "Tweezer",
    "TwiggsMoneyFlow",
    "TwoCrows",
    "TypicalPrice",
    "UlcerIndex",
    "UltimateOscillator",
    "UniqueThreeRiver",
    "UniversalOscillator",
    "UpsideGapThreeMethods",
    "UpsideGapTwoCrows",
    "UpsidePotentialRatio",
    "ValueArea",
    "ValueAtRisk",
    "Variance",
    "VerticalHorizontalFilter",
    "Vidya",
    "VolatilityCone",
    "VolatilityOfVolatility",
    "VolatilityRatio",
    "VoltyStop",
    "VolumeOscillator",
    "VolumePriceTrend",
    "VolumeProfile",
    "VolumeRsi",
    "VolumeWeightedMacd",
    "VolumeWeightedSr",
    "Vortex",
    "Vwap",
    "VwapStdDevBands",
    "Vwma",
    "Vzo",
    "Wad",
    "WavePm",
    "WaveTrend",
    "Wedge",
    "WeightedClose",
    "WickRatio",
    "WilliamsR",
    "WinRate",
    "Wma",
    "WoodiePivots",
    "YangZhangVolatility",
    "YoyoExit",
    "ZScore",
    "ZeroLagMacd",
    "ZigZag",
    "Zlema",
];

/// Default constructor parameters, taken from the wickra golden manifest — the
/// same values the library pins its own reference outputs with. Used by the
/// build-all test so every registered indicator is constructed the way wickra
/// constructs it, rather than with a guessed parameter count.
pub const DEFAULTS: [(&str, &[f64]); 421] = [
    ("AbandonedBaby", &[]),
    ("Abcd", &[]),
    ("AccelerationBands", &[14.0, 2.0]),
    ("AcceleratorOscillator", &[3.0, 7.0, 14.0]),
    ("AdOscillator", &[]),
    ("AdaptiveCci", &[14.0]),
    ("AdaptiveCycle", &[]),
    ("AdaptiveLaguerreFilter", &[20.0]),
    ("AdaptiveRsi", &[14.0]),
    ("Adl", &[]),
    ("AdvanceBlock", &[]),
    ("Adx", &[14.0]),
    ("Adxr", &[14.0]),
    ("Alligator", &[3.0, 7.0, 14.0]),
    ("Alma", &[9.0, 0.85, 6.0]),
    ("AnchoredRsi", &[]),
    ("AnchoredVwap", &[]),
    ("AndrewsPitchfork", &[14.0]),
    ("Apo", &[3.0, 7.0]),
    ("Aroon", &[14.0]),
    ("AroonOscillator", &[14.0]),
    ("Atr", &[14.0]),
    ("AtrBands", &[14.0, 2.0]),
    ("AtrRatchet", &[14.0, 2.0, 0.5]),
    ("AtrTrailingStop", &[14.0, 2.0]),
    ("AutoFib", &[]),
    ("Autocorrelation", &[10.0, 1.0]),
    ("AutocorrelationPeriodogram", &[10.0, 48.0]),
    ("AverageDailyRange", &[14.0, 0.0]),
    ("AverageDrawdown", &[14.0]),
    ("AvgPrice", &[]),
    ("AwesomeOscillator", &[3.0, 7.0]),
    ("AwesomeOscillatorHistogram", &[3.0, 7.0, 14.0]),
    ("BalanceOfPower", &[]),
    ("BandpassFilter", &[20.0, 0.3]),
    ("Bat", &[]),
    ("BeltHold", &[]),
    ("BetterVolume", &[14.0]),
    ("BipowerVariation", &[14.0]),
    ("BodySizePct", &[]),
    ("BollingerBands", &[20.0, 2.0]),
    ("BollingerBandwidth", &[14.0, 2.0]),
    ("BomarBands", &[4.0, 0.85]),
    ("Breakaway", &[]),
    ("BurkeRatio", &[14.0]),
    ("Butterfly", &[]),
    ("CalmarRatio", &[14.0]),
    ("Camarilla", &[]),
    ("CandleVolume", &[14.0]),
    ("Cci", &[14.0]),
    ("CenterOfGravity", &[14.0]),
    ("CentralPivotRange", &[]),
    ("Cfo", &[14.0]),
    ("ChaikinMoneyFlow", &[20.0]),
    ("ChaikinOscillator", &[3.0, 7.0]),
    ("ChaikinVolatility", &[3.0, 7.0]),
    ("ChandeKrollStop", &[3.0, 2.0, 7.0]),
    ("ChandelierExit", &[14.0, 2.0]),
    ("ChoppinessIndex", &[14.0]),
    ("ClassicPivots", &[]),
    ("CloseVsOpen", &[]),
    ("ClosingMarubozu", &[]),
    ("Cmo", &[14.0]),
    ("CoefficientOfVariation", &[14.0]),
    ("CommonSenseRatio", &[14.0]),
    ("CompositeProfile", &[20.0, 24.0, 0.7]),
    ("ConcealingBabySwallow", &[]),
    ("ConditionalValueAtRisk", &[20.0, 0.95]),
    ("ConnorsRsi", &[3.0, 7.0, 14.0]),
    ("Coppock", &[3.0, 7.0, 14.0]),
    ("CorrelationTrendIndicator", &[14.0]),
    ("Counterattack", &[]),
    ("Crab", &[]),
    ("CupAndHandle", &[]),
    ("CyberneticCycle", &[14.0]),
    ("Cypher", &[]),
    ("Decycler", &[14.0]),
    ("DecyclerOscillator", &[3.0, 7.0]),
    ("Dema", &[14.0]),
    ("DemandIndex", &[14.0]),
    ("DemarkPivots", &[]),
    ("DerivativeOscillator", &[3.0, 7.0, 14.0, 28.0]),
    ("DetrendedStdDev", &[14.0]),
    ("DisparityIndex", &[14.0]),
    ("Doji", &[]),
    ("DojiStar", &[]),
    ("Donchian", &[14.0]),
    ("DonchianStop", &[14.0]),
    ("DoubleBollinger", &[20.0, 1.0, 2.0]),
    ("DoubleTopBottom", &[]),
    ("DownsideGapThreeMethods", &[]),
    ("Dpo", &[14.0]),
    ("DragonflyDoji", &[]),
    ("DumplingTop", &[14.0]),
    ("Dx", &[14.0]),
    ("DynamicMomentumIndex", &[14.0]),
    ("EaseOfMovement", &[14.0]),
    ("EhlersStochastic", &[14.0]),
    ("Ehma", &[14.0]),
    ("ElderImpulse", &[3.0, 7.0, 14.0, 28.0]),
    ("ElderRay", &[14.0]),
    ("ElderSafeZone", &[10.0, 2.0]),
    ("Ema", &[14.0]),
    ("EmpiricalModeDecomposition", &[20.0, 0.1]),
    ("Engulfing", &[]),
    ("Equivolume", &[14.0]),
    ("EvenBetterSinewave", &[40.0, 10.0]),
    ("EveningDojiStar", &[]),
    ("Evwma", &[14.0]),
    ("EwmaVolatility", &[0.94]),
    ("Expectancy", &[14.0]),
    ("FallingThreeMethods", &[]),
    ("Fama", &[0.5, 0.05]),
    ("FibArcs", &[]),
    ("FibChannel", &[]),
    ("FibConfluence", &[]),
    ("FibExtension", &[]),
    ("FibFan", &[]),
    ("FibProjection", &[]),
    ("FibRetracement", &[]),
    ("FibTimeZones", &[]),
    ("FibonacciPivots", &[]),
    ("FisherRsi", &[14.0]),
    ("FisherTransform", &[14.0]),
    ("FlagPennant", &[]),
    ("ForceIndex", &[14.0]),
    ("FractalChaosBands", &[14.0]),
    ("Frama", &[14.0]),
    ("FryPanBottom", &[14.0]),
    ("GainLossRatio", &[14.0]),
    ("GainToPainRatio", &[14.0]),
    ("GapSideBySideWhite", &[]),
    ("Garch11", &[2e-06, 0.1, 0.88]),
    ("GarmanKlassVolatility", &[20.0, 252.0]),
    ("Gartley", &[]),
    ("GatorOscillator", &[3.0, 7.0, 14.0]),
    ("GeneralizedDema", &[5.0, 0.7]),
    ("GeometricMa", &[14.0]),
    ("GoldenPocket", &[]),
    ("GravestoneDoji", &[]),
    ("Hammer", &[]),
    ("HangingMan", &[]),
    ("Harami", &[]),
    ("HaramiCross", &[]),
    ("HeadAndShoulders", &[]),
    ("HeikinAshi", &[]),
    ("HeikinAshiOscillator", &[14.0]),
    ("HiLoActivator", &[14.0]),
    ("HighLowRange", &[]),
    ("HighLowVolumeNodes", &[3.0, 7.0]),
    ("HighWave", &[]),
    ("HighpassFilter", &[14.0]),
    ("Hikkake", &[]),
    ("HikkakeModified", &[]),
    ("HilbertDominantCycle", &[]),
    ("HistoricalVolatility", &[3.0, 7.0]),
    ("Hma", &[14.0]),
    ("HoltWinters", &[0.5, 0.1]),
    ("HomingPigeon", &[]),
    ("HtDcPhase", &[]),
    ("HtPhasor", &[]),
    ("HtTrendMode", &[]),
    ("HurstChannel", &[14.0, 2.0]),
    ("HurstExponent", &[100.0, 4.0]),
    ("IdenticalThreeCrows", &[]),
    ("InNeck", &[]),
    ("Inertia", &[3.0, 7.0]),
    ("InitialBalance", &[14.0]),
    ("InstantaneousTrendline", &[14.0]),
    ("IntradayIntensity", &[]),
    ("IntradayMomentumIndex", &[14.0]),
    ("InverseFisherTransform", &[2.0]),
    ("InvertedHammer", &[]),
    ("JarqueBera", &[14.0]),
    ("Jma", &[7.0, 0.0, 2.0]),
    ("JumpIndicator", &[14.0, 2.0]),
    ("KRatio", &[14.0]),
    ("Kama", &[3.0, 7.0, 14.0]),
    ("KaseDevStop", &[14.0, 2.0]),
    ("KasePermissionStochastic", &[3.0, 7.0]),
    ("KellyCriterion", &[14.0]),
    ("Keltner", &[3.0, 7.0, 2.0]),
    ("Kicking", &[]),
    ("KickingByLength", &[]),
    ("Kst", &[3.0, 7.0, 14.0, 28.0, 35.0, 42.0, 56.0, 63.0, 70.0]),
    ("Kurtosis", &[14.0]),
    ("Kvo", &[3.0, 7.0]),
    ("LadderBottom", &[]),
    ("LaguerreRsi", &[0.5]),
    ("LinRegAngle", &[14.0]),
    ("LinRegChannel", &[14.0, 2.0]),
    ("LinRegIntercept", &[14.0]),
    ("LinRegSlope", &[14.0]),
    ("LinearRegression", &[14.0]),
    ("LogReturn", &[14.0]),
    ("LongLeggedDoji", &[]),
    ("LongLine", &[]),
    ("M2Measure", &[14.0, 2.0, 0.5]),
    ("MaEnvelope", &[14.0, 2.0]),
    ("MacdFix", &[9.0]),
    ("MacdHistogram", &[3.0, 7.0, 14.0]),
    ("MacdIndicator", &[12.0, 26.0, 9.0]),
    ("Mama", &[0.5, 0.05]),
    ("MarketFacilitationIndex", &[]),
    ("MartinRatio", &[14.0]),
    ("Marubozu", &[]),
    ("MassIndex", &[3.0, 7.0]),
    ("MatHold", &[]),
    ("MatchingLow", &[]),
    ("MaxDrawdown", &[14.0]),
    ("McGinleyDynamic", &[14.0]),
    ("MedianAbsoluteDeviation", &[14.0]),
    ("MedianChannel", &[14.0, 2.0]),
    ("MedianMa", &[14.0]),
    ("MedianPrice", &[]),
    ("Mfi", &[14.0]),
    ("MidPoint", &[14.0]),
    ("MidPrice", &[14.0]),
    ("MinusDi", &[14.0]),
    ("MinusDm", &[14.0]),
    ("ModifiedMaStop", &[14.0]),
    ("Mom", &[14.0]),
    ("MorningDojiStar", &[]),
    ("MorningEveningStar", &[]),
    ("MurreyMathLines", &[14.0]),
    ("NakedPoc", &[3.0, 7.0]),
    ("Natr", &[14.0]),
    ("NewPriceLines", &[14.0]),
    ("Nrtr", &[2.0]),
    ("Nvi", &[]),
    ("Obv", &[]),
    ("OmegaRatio", &[14.0, 2.0]),
    ("OnNeck", &[]),
    ("OpeningMarubozu", &[]),
    ("OpeningRange", &[14.0]),
    ("OvernightGap", &[0.0]),
    ("OvernightIntradayReturn", &[14.0]),
    ("PainIndex", &[14.0]),
    ("ParkinsonVolatility", &[20.0, 252.0]),
    ("PercentB", &[14.0, 2.0]),
    ("PercentageTrailingStop", &[2.0]),
    ("Pgo", &[14.0]),
    ("PiercingDarkCloud", &[]),
    ("PivotReversal", &[3.0, 7.0]),
    ("PlusDi", &[14.0]),
    ("PlusDm", &[14.0]),
    ("Pmo", &[3.0, 7.0]),
    ("PolarizedFractalEfficiency", &[10.0, 5.0]),
    ("Ppo", &[3.0, 7.0]),
    ("PpoHistogram", &[3.0, 7.0, 14.0]),
    ("ProfileShape", &[3.0, 7.0]),
    ("ProfitFactor", &[14.0]),
    ("ProjectionBands", &[14.0]),
    ("ProjectionOscillator", &[14.0]),
    ("Psar", &[0.02, 0.02, 0.2]),
    ("Pvi", &[]),
    ("Qqe", &[3.0, 7.0, 2.0]),
    ("Qstick", &[14.0]),
    ("QuartileBands", &[14.0]),
    ("RSquared", &[14.0]),
    ("RealizedVolatility", &[14.0]),
    ("RecoveryFactor", &[]),
    ("RectangleRange", &[]),
    ("Reflex", &[14.0]),
    ("RegimeLabel", &[3.0, 7.0]),
    ("RenkoTrailingStop", &[2.0]),
    ("RickshawMan", &[]),
    ("RisingThreeMethods", &[]),
    ("Rmi", &[3.0, 7.0]),
    ("Roc", &[14.0]),
    ("Rocp", &[14.0]),
    ("Rocr", &[14.0]),
    ("Rocr100", &[14.0]),
    ("RogersSatchellVolatility", &[20.0, 252.0]),
    ("RollingIqr", &[14.0]),
    ("RollingMinMaxScaler", &[14.0]),
    ("RollingPercentileRank", &[14.0]),
    ("RollingQuantile", &[20.0, 0.5]),
    ("RollingVwap", &[14.0]),
    ("RoofingFilter", &[3.0, 7.0]),
    ("Rsi", &[14.0]),
    ("Rsx", &[14.0]),
    ("Rvi", &[14.0]),
    ("RviVolatility", &[14.0]),
    ("Rwi", &[14.0]),
    ("SampleEntropy", &[20.0, 2.0, 0.2]),
    ("SarExt", &[2.0, 0.5, 0.5, 0.5, 0.5, 0.5, 0.5, 0.5]),
    ("SeasonalZScore", &[14.0]),
    ("SeparatingLines", &[]),
    ("SessionHighLow", &[14.0]),
    ("SessionRange", &[14.0]),
    ("SessionVwap", &[14.0]),
    ("ShannonEntropy", &[3.0, 7.0]),
    ("Shark", &[]),
    ("SharpeRatio", &[14.0, 2.0]),
    ("ShootingStar", &[]),
    ("ShortLine", &[]),
    ("SineWave", &[]),
    ("SineWeightedMa", &[14.0]),
    ("SinglePrints", &[3.0, 7.0]),
    ("Skewness", &[14.0]),
    ("Sma", &[14.0]),
    ("Smi", &[3.0, 7.0, 14.0]),
    ("Smma", &[14.0]),
    ("SmoothedHeikinAshi", &[14.0]),
    ("SortinoRatio", &[14.0, 2.0]),
    ("SpinningTop", &[]),
    ("StalledPattern", &[]),
    ("StandardError", &[14.0]),
    ("StandardErrorBands", &[14.0, 2.0]),
    ("StarcBands", &[3.0, 7.0, 2.0]),
    ("Stc", &[10.0, 23.0, 10.0, 0.5]),
    ("StdDev", &[14.0]),
    ("StepTrailingStop", &[2.0]),
    ("SterlingRatio", &[14.0]),
    ("StickSandwich", &[]),
    ("StochRsi", &[3.0, 7.0]),
    ("Stochastic", &[3.0, 7.0]),
    ("StochasticCci", &[14.0]),
    ("SuperSmoother", &[14.0]),
    ("SuperTrend", &[14.0, 2.0]),
    ("T3", &[5.0, 0.7]),
    ("TailRatio", &[14.0]),
    ("Takuri", &[]),
    ("TasukiGap", &[]),
    ("TdCamouflage", &[]),
    ("TdClop", &[]),
    ("TdClopwin", &[]),
    ("TdCombo", &[3.0, 7.0, 14.0, 28.0]),
    ("TdCountdown", &[3.0, 7.0, 14.0, 28.0]),
    ("TdDWave", &[2.0]),
    ("TdDeMarker", &[14.0]),
    ("TdDifferential", &[]),
    ("TdLines", &[3.0, 7.0]),
    ("TdMovingAverage", &[3.0, 7.0]),
    ("TdOpen", &[]),
    ("TdPressure", &[14.0]),
    ("TdPropulsion", &[]),
    ("TdRangeProjection", &[]),
    ("TdRei", &[14.0]),
    ("TdRiskLevel", &[3.0, 7.0]),
    ("TdSequential", &[3.0, 7.0, 14.0, 28.0]),
    ("TdSetup", &[3.0, 7.0]),
    ("TdTrap", &[]),
    ("Tema", &[14.0]),
    ("ThreeDrives", &[]),
    ("ThreeInside", &[]),
    ("ThreeLineBreak", &[14.0]),
    ("ThreeLineStrike", &[]),
    ("ThreeOutside", &[]),
    ("ThreeSoldiersOrCrows", &[]),
    ("ThreeStarsInSouth", &[]),
    ("Thrusting", &[]),
    ("Tii", &[3.0, 7.0]),
    ("TimeBasedStop", &[14.0]),
    ("TowerTopBottom", &[]),
    ("TpoProfile", &[30.0, 50.0]),
    ("TradeVolumeIndex", &[2.0]),
    ("TrendLabel", &[14.0]),
    ("TrendStrengthIndex", &[14.0]),
    ("Trendflex", &[14.0]),
    ("Triangle", &[]),
    ("Trima", &[14.0]),
    ("TripleTopBottom", &[]),
    ("Tristar", &[]),
    ("Trix", &[14.0]),
    ("TrueRange", &[]),
    ("Tsf", &[14.0]),
    ("TsfOscillator", &[14.0]),
    ("Tsi", &[3.0, 7.0]),
    ("Tsv", &[14.0]),
    ("TtmSqueeze", &[14.0, 2.0, 0.5]),
    ("TtmTrend", &[14.0]),
    ("TurnOfMonth", &[3.0, 3.0, 0.0]),
    ("Tweezer", &[]),
    ("TwiggsMoneyFlow", &[14.0]),
    ("TwoCrows", &[]),
    ("TypicalPrice", &[]),
    ("UlcerIndex", &[14.0]),
    ("UltimateOscillator", &[3.0, 7.0, 14.0]),
    ("UniqueThreeRiver", &[]),
    ("UniversalOscillator", &[14.0]),
    ("UpsideGapThreeMethods", &[]),
    ("UpsideGapTwoCrows", &[]),
    ("UpsidePotentialRatio", &[14.0, 2.0]),
    ("ValueArea", &[20.0, 50.0, 0.7]),
    ("ValueAtRisk", &[20.0, 0.95]),
    ("Variance", &[14.0]),
    ("VerticalHorizontalFilter", &[14.0]),
    ("Vidya", &[3.0, 7.0]),
    ("VolatilityCone", &[3.0, 7.0]),
    ("VolatilityOfVolatility", &[3.0, 7.0]),
    ("VolatilityRatio", &[14.0]),
    ("VoltyStop", &[14.0, 2.0]),
    ("VolumeOscillator", &[3.0, 7.0]),
    ("VolumePriceTrend", &[]),
    ("VolumeProfile", &[20.0, 50.0]),
    ("VolumeRsi", &[14.0]),
    ("VolumeWeightedMacd", &[3.0, 7.0, 14.0]),
    ("VolumeWeightedSr", &[14.0]),
    ("Vortex", &[14.0]),
    ("Vwap", &[]),
    ("VwapStdDevBands", &[2.0]),
    ("Vwma", &[14.0]),
    ("Vzo", &[14.0]),
    ("Wad", &[]),
    ("WavePm", &[3.0, 7.0]),
    ("WaveTrend", &[3.0, 7.0, 14.0]),
    ("Wedge", &[]),
    ("WeightedClose", &[]),
    ("WickRatio", &[]),
    ("WilliamsR", &[14.0]),
    ("WinRate", &[14.0]),
    ("Wma", &[14.0]),
    ("WoodiePivots", &[]),
    ("YangZhangVolatility", &[20.0, 252.0]),
    ("YoyoExit", &[14.0, 2.0]),
    ("ZScore", &[14.0]),
    ("ZeroLagMacd", &[3.0, 7.0, 14.0]),
    ("ZigZag", &[0.02]),
    ("Zlema", &[14.0]),
];

/// Construct an indicator by name with positional parameters.
///
/// # Errors
///
/// Returns [`Error::Config`] if the name is unknown, a parameter is missing or
/// out of range, or wickra-core rejects the parameters.
pub fn build(kind: &str, params: &[f64]) -> Result<Box<dyn TickIndicator>> {
    match kind {
        "AbandonedBaby" => Ok(Box::new(CandleIn(wc::AbandonedBaby::new()))),
        "Abcd" => Ok(Box::new(CandleIn(wc::Abcd::new()))),
        "AccelerationBands" => Ok(Box::new(CandleInFields {
            inner: map_new(
                kind,
                wc::AccelerationBands::new(
                    usize_param(params, 0, kind)?,
                    float_param(params, 1, kind)?,
                ),
            )?,
            last: None,
        })),
        "AcceleratorOscillator" => Ok(Box::new(CandleIn(map_new(
            kind,
            wc::AcceleratorOscillator::new(
                usize_param(params, 0, kind)?,
                usize_param(params, 1, kind)?,
                usize_param(params, 2, kind)?,
            ),
        )?))),
        "AdOscillator" => Ok(Box::new(CandleIn(wc::AdOscillator::new()))),
        "AdaptiveCci" => Ok(Box::new(CandleIn(map_new(
            kind,
            wc::AdaptiveCci::new(usize_param(params, 0, kind)?),
        )?))),
        "AdaptiveCycle" => Ok(Box::new(ScalarPrice(wc::AdaptiveCycle::new()))),
        "AdaptiveLaguerreFilter" => Ok(Box::new(ScalarPrice(map_new(
            kind,
            wc::AdaptiveLaguerreFilter::new(usize_param(params, 0, kind)?),
        )?))),
        "AdaptiveRsi" => Ok(Box::new(ScalarPrice(map_new(
            kind,
            wc::AdaptiveRsi::new(usize_param(params, 0, kind)?),
        )?))),
        "Adl" => Ok(Box::new(CandleIn(wc::Adl::new()))),
        "AdvanceBlock" => Ok(Box::new(CandleIn(wc::AdvanceBlock::new()))),
        "Adx" => Ok(Box::new(CandleInFields {
            inner: map_new(kind, wc::Adx::new(usize_param(params, 0, kind)?))?,
            last: None,
        })),
        "Adxr" => Ok(Box::new(CandleIn(map_new(
            kind,
            wc::Adxr::new(usize_param(params, 0, kind)?),
        )?))),
        "Alligator" => Ok(Box::new(CandleInFields {
            inner: map_new(
                kind,
                wc::Alligator::new(
                    usize_param(params, 0, kind)?,
                    usize_param(params, 1, kind)?,
                    usize_param(params, 2, kind)?,
                ),
            )?,
            last: None,
        })),
        "Alma" => Ok(Box::new(ScalarPrice(map_new(
            kind,
            wc::Alma::new(
                usize_param(params, 0, kind)?,
                float_param(params, 1, kind)?,
                float_param(params, 2, kind)?,
            ),
        )?))),
        "AnchoredRsi" => Ok(Box::new(ScalarPrice(wc::AnchoredRsi::new()))),
        "AnchoredVwap" => Ok(Box::new(CandleIn(wc::AnchoredVwap::new()))),
        "AndrewsPitchfork" => Ok(Box::new(CandleInFields {
            inner: map_new(
                kind,
                wc::AndrewsPitchfork::new(usize_param(params, 0, kind)?),
            )?,
            last: None,
        })),
        "Apo" => Ok(Box::new(ScalarPrice(map_new(
            kind,
            wc::Apo::new(usize_param(params, 0, kind)?, usize_param(params, 1, kind)?),
        )?))),
        "Aroon" => Ok(Box::new(CandleInFields {
            inner: map_new(kind, wc::Aroon::new(usize_param(params, 0, kind)?))?,
            last: None,
        })),
        "AroonOscillator" => Ok(Box::new(CandleIn(map_new(
            kind,
            wc::AroonOscillator::new(usize_param(params, 0, kind)?),
        )?))),
        "Atr" => Ok(Box::new(CandleIn(map_new(
            kind,
            wc::Atr::new(usize_param(params, 0, kind)?),
        )?))),
        "AtrBands" => Ok(Box::new(CandleInFields {
            inner: map_new(
                kind,
                wc::AtrBands::new(usize_param(params, 0, kind)?, float_param(params, 1, kind)?),
            )?,
            last: None,
        })),
        "AtrRatchet" => Ok(Box::new(CandleInFields {
            inner: map_new(
                kind,
                wc::AtrRatchet::new(
                    usize_param(params, 0, kind)?,
                    float_param(params, 1, kind)?,
                    float_param(params, 2, kind)?,
                ),
            )?,
            last: None,
        })),
        "AtrTrailingStop" => Ok(Box::new(CandleIn(map_new(
            kind,
            wc::AtrTrailingStop::new(usize_param(params, 0, kind)?, float_param(params, 1, kind)?),
        )?))),
        "AutoFib" => Ok(Box::new(CandleInFields {
            inner: wc::AutoFib::new(),
            last: None,
        })),
        "Autocorrelation" => Ok(Box::new(ScalarPrice(map_new(
            kind,
            wc::Autocorrelation::new(usize_param(params, 0, kind)?, usize_param(params, 1, kind)?),
        )?))),
        "AutocorrelationPeriodogram" => Ok(Box::new(ScalarPrice(map_new(
            kind,
            wc::AutocorrelationPeriodogram::new(
                usize_param(params, 0, kind)?,
                usize_param(params, 1, kind)?,
            ),
        )?))),
        "AverageDailyRange" => Ok(Box::new(CandleIn(map_new(
            kind,
            wc::AverageDailyRange::new(usize_param(params, 0, kind)?, i32_param(params, 1, kind)?),
        )?))),
        "AverageDrawdown" => Ok(Box::new(ScalarPrice(map_new(
            kind,
            wc::AverageDrawdown::new(usize_param(params, 0, kind)?),
        )?))),
        "AvgPrice" => Ok(Box::new(CandleIn(wc::AvgPrice::new()))),
        "AwesomeOscillator" => Ok(Box::new(CandleIn(map_new(
            kind,
            wc::AwesomeOscillator::new(
                usize_param(params, 0, kind)?,
                usize_param(params, 1, kind)?,
            ),
        )?))),
        "AwesomeOscillatorHistogram" => Ok(Box::new(CandleIn(map_new(
            kind,
            wc::AwesomeOscillatorHistogram::new(
                usize_param(params, 0, kind)?,
                usize_param(params, 1, kind)?,
                usize_param(params, 2, kind)?,
            ),
        )?))),
        "BalanceOfPower" => Ok(Box::new(CandleIn(wc::BalanceOfPower::new()))),
        "BandpassFilter" => Ok(Box::new(ScalarPrice(map_new(
            kind,
            wc::BandpassFilter::new(usize_param(params, 0, kind)?, float_param(params, 1, kind)?),
        )?))),
        "Bat" => Ok(Box::new(CandleIn(wc::Bat::new()))),
        "BeltHold" => Ok(Box::new(CandleIn(wc::BeltHold::new()))),
        "BetterVolume" => Ok(Box::new(CandleIn(map_new(
            kind,
            wc::BetterVolume::new(usize_param(params, 0, kind)?),
        )?))),
        "BipowerVariation" => Ok(Box::new(ScalarPrice(map_new(
            kind,
            wc::BipowerVariation::new(usize_param(params, 0, kind)?),
        )?))),
        "BodySizePct" => Ok(Box::new(CandleIn(wc::BodySizePct::new()))),
        "BollingerBands" => Ok(Box::new(ScalarPriceFields {
            inner: map_new(
                kind,
                wc::BollingerBands::new(
                    usize_param(params, 0, kind)?,
                    float_param(params, 1, kind)?,
                ),
            )?,
            last: None,
        })),
        "BollingerBandwidth" => Ok(Box::new(ScalarPrice(map_new(
            kind,
            wc::BollingerBandwidth::new(
                usize_param(params, 0, kind)?,
                float_param(params, 1, kind)?,
            ),
        )?))),
        "BomarBands" => Ok(Box::new(ScalarPriceFields {
            inner: map_new(
                kind,
                wc::BomarBands::new(usize_param(params, 0, kind)?, float_param(params, 1, kind)?),
            )?,
            last: None,
        })),
        "Breakaway" => Ok(Box::new(CandleIn(wc::Breakaway::new()))),
        "BurkeRatio" => Ok(Box::new(ScalarPrice(map_new(
            kind,
            wc::BurkeRatio::new(usize_param(params, 0, kind)?),
        )?))),
        "Butterfly" => Ok(Box::new(CandleIn(wc::Butterfly::new()))),
        "CalmarRatio" => Ok(Box::new(ScalarPrice(map_new(
            kind,
            wc::CalmarRatio::new(usize_param(params, 0, kind)?),
        )?))),
        "Camarilla" => Ok(Box::new(CandleInFields {
            inner: wc::Camarilla::new(),
            last: None,
        })),
        "CandleVolume" => Ok(Box::new(CandleInFields {
            inner: map_new(kind, wc::CandleVolume::new(usize_param(params, 0, kind)?))?,
            last: None,
        })),
        "Cci" => Ok(Box::new(CandleIn(map_new(
            kind,
            wc::Cci::new(usize_param(params, 0, kind)?),
        )?))),
        "CenterOfGravity" => Ok(Box::new(ScalarPrice(map_new(
            kind,
            wc::CenterOfGravity::new(usize_param(params, 0, kind)?),
        )?))),
        "CentralPivotRange" => Ok(Box::new(CandleInFields {
            inner: wc::CentralPivotRange::new(),
            last: None,
        })),
        "Cfo" => Ok(Box::new(ScalarPrice(map_new(
            kind,
            wc::Cfo::new(usize_param(params, 0, kind)?),
        )?))),
        "ChaikinMoneyFlow" => Ok(Box::new(CandleIn(map_new(
            kind,
            wc::ChaikinMoneyFlow::new(usize_param(params, 0, kind)?),
        )?))),
        "ChaikinOscillator" => Ok(Box::new(CandleIn(map_new(
            kind,
            wc::ChaikinOscillator::new(
                usize_param(params, 0, kind)?,
                usize_param(params, 1, kind)?,
            ),
        )?))),
        "ChaikinVolatility" => Ok(Box::new(CandleIn(map_new(
            kind,
            wc::ChaikinVolatility::new(
                usize_param(params, 0, kind)?,
                usize_param(params, 1, kind)?,
            ),
        )?))),
        "ChandeKrollStop" => Ok(Box::new(CandleInFields {
            inner: map_new(
                kind,
                wc::ChandeKrollStop::new(
                    usize_param(params, 0, kind)?,
                    float_param(params, 1, kind)?,
                    usize_param(params, 2, kind)?,
                ),
            )?,
            last: None,
        })),
        "ChandelierExit" => Ok(Box::new(CandleInFields {
            inner: map_new(
                kind,
                wc::ChandelierExit::new(
                    usize_param(params, 0, kind)?,
                    float_param(params, 1, kind)?,
                ),
            )?,
            last: None,
        })),
        "ChoppinessIndex" => Ok(Box::new(CandleIn(map_new(
            kind,
            wc::ChoppinessIndex::new(usize_param(params, 0, kind)?),
        )?))),
        "ClassicPivots" => Ok(Box::new(CandleInFields {
            inner: wc::ClassicPivots::new(),
            last: None,
        })),
        "CloseVsOpen" => Ok(Box::new(CandleIn(wc::CloseVsOpen::new()))),
        "ClosingMarubozu" => Ok(Box::new(CandleIn(wc::ClosingMarubozu::new()))),
        "Cmo" => Ok(Box::new(ScalarPrice(map_new(
            kind,
            wc::Cmo::new(usize_param(params, 0, kind)?),
        )?))),
        "CoefficientOfVariation" => Ok(Box::new(ScalarPrice(map_new(
            kind,
            wc::CoefficientOfVariation::new(usize_param(params, 0, kind)?),
        )?))),
        "CommonSenseRatio" => Ok(Box::new(ScalarPrice(map_new(
            kind,
            wc::CommonSenseRatio::new(usize_param(params, 0, kind)?),
        )?))),
        "CompositeProfile" => Ok(Box::new(CandleInFields {
            inner: map_new(
                kind,
                wc::CompositeProfile::new(
                    usize_param(params, 0, kind)?,
                    usize_param(params, 1, kind)?,
                    float_param(params, 2, kind)?,
                ),
            )?,
            last: None,
        })),
        "ConcealingBabySwallow" => Ok(Box::new(CandleIn(wc::ConcealingBabySwallow::new()))),
        "ConditionalValueAtRisk" => Ok(Box::new(ScalarPrice(map_new(
            kind,
            wc::ConditionalValueAtRisk::new(
                usize_param(params, 0, kind)?,
                float_param(params, 1, kind)?,
            ),
        )?))),
        "ConnorsRsi" => Ok(Box::new(ScalarPrice(map_new(
            kind,
            wc::ConnorsRsi::new(
                usize_param(params, 0, kind)?,
                usize_param(params, 1, kind)?,
                usize_param(params, 2, kind)?,
            ),
        )?))),
        "Coppock" => Ok(Box::new(ScalarPrice(map_new(
            kind,
            wc::Coppock::new(
                usize_param(params, 0, kind)?,
                usize_param(params, 1, kind)?,
                usize_param(params, 2, kind)?,
            ),
        )?))),
        "CorrelationTrendIndicator" => Ok(Box::new(ScalarPrice(map_new(
            kind,
            wc::CorrelationTrendIndicator::new(usize_param(params, 0, kind)?),
        )?))),
        "Counterattack" => Ok(Box::new(CandleIn(wc::Counterattack::new()))),
        "Crab" => Ok(Box::new(CandleIn(wc::Crab::new()))),
        "CupAndHandle" => Ok(Box::new(CandleIn(wc::CupAndHandle::new()))),
        "CyberneticCycle" => Ok(Box::new(ScalarPrice(map_new(
            kind,
            wc::CyberneticCycle::new(usize_param(params, 0, kind)?),
        )?))),
        "Cypher" => Ok(Box::new(CandleIn(wc::Cypher::new()))),
        "Decycler" => Ok(Box::new(ScalarPrice(map_new(
            kind,
            wc::Decycler::new(usize_param(params, 0, kind)?),
        )?))),
        "DecyclerOscillator" => Ok(Box::new(ScalarPrice(map_new(
            kind,
            wc::DecyclerOscillator::new(
                usize_param(params, 0, kind)?,
                usize_param(params, 1, kind)?,
            ),
        )?))),
        "Dema" => Ok(Box::new(ScalarPrice(map_new(
            kind,
            wc::Dema::new(usize_param(params, 0, kind)?),
        )?))),
        "DemandIndex" => Ok(Box::new(CandleIn(map_new(
            kind,
            wc::DemandIndex::new(usize_param(params, 0, kind)?),
        )?))),
        "DemarkPivots" => Ok(Box::new(CandleInFields {
            inner: wc::DemarkPivots::new(),
            last: None,
        })),
        "DerivativeOscillator" => Ok(Box::new(ScalarPrice(map_new(
            kind,
            wc::DerivativeOscillator::new(
                usize_param(params, 0, kind)?,
                usize_param(params, 1, kind)?,
                usize_param(params, 2, kind)?,
                usize_param(params, 3, kind)?,
            ),
        )?))),
        "DetrendedStdDev" => Ok(Box::new(ScalarPrice(map_new(
            kind,
            wc::DetrendedStdDev::new(usize_param(params, 0, kind)?),
        )?))),
        "DisparityIndex" => Ok(Box::new(ScalarPrice(map_new(
            kind,
            wc::DisparityIndex::new(usize_param(params, 0, kind)?),
        )?))),
        "Doji" => Ok(Box::new(CandleIn(wc::Doji::new()))),
        "DojiStar" => Ok(Box::new(CandleIn(wc::DojiStar::new()))),
        "Donchian" => Ok(Box::new(CandleInFields {
            inner: map_new(kind, wc::Donchian::new(usize_param(params, 0, kind)?))?,
            last: None,
        })),
        "DonchianStop" => Ok(Box::new(CandleInFields {
            inner: map_new(kind, wc::DonchianStop::new(usize_param(params, 0, kind)?))?,
            last: None,
        })),
        "DoubleBollinger" => Ok(Box::new(ScalarPriceFields {
            inner: map_new(
                kind,
                wc::DoubleBollinger::new(
                    usize_param(params, 0, kind)?,
                    float_param(params, 1, kind)?,
                    float_param(params, 2, kind)?,
                ),
            )?,
            last: None,
        })),
        "DoubleTopBottom" => Ok(Box::new(CandleIn(wc::DoubleTopBottom::new()))),
        "DownsideGapThreeMethods" => Ok(Box::new(CandleIn(wc::DownsideGapThreeMethods::new()))),
        "Dpo" => Ok(Box::new(ScalarPrice(map_new(
            kind,
            wc::Dpo::new(usize_param(params, 0, kind)?),
        )?))),
        "DragonflyDoji" => Ok(Box::new(CandleIn(wc::DragonflyDoji::new()))),
        "DumplingTop" => Ok(Box::new(CandleIn(map_new(
            kind,
            wc::DumplingTop::new(usize_param(params, 0, kind)?),
        )?))),
        "Dx" => Ok(Box::new(CandleIn(map_new(
            kind,
            wc::Dx::new(usize_param(params, 0, kind)?),
        )?))),
        "DynamicMomentumIndex" => Ok(Box::new(ScalarPrice(map_new(
            kind,
            wc::DynamicMomentumIndex::new(usize_param(params, 0, kind)?),
        )?))),
        "EaseOfMovement" => Ok(Box::new(CandleIn(map_new(
            kind,
            wc::EaseOfMovement::new(usize_param(params, 0, kind)?),
        )?))),
        "EhlersStochastic" => Ok(Box::new(ScalarPrice(map_new(
            kind,
            wc::EhlersStochastic::new(usize_param(params, 0, kind)?),
        )?))),
        "Ehma" => Ok(Box::new(ScalarPrice(map_new(
            kind,
            wc::Ehma::new(usize_param(params, 0, kind)?),
        )?))),
        "ElderImpulse" => Ok(Box::new(ScalarPrice(map_new(
            kind,
            wc::ElderImpulse::new(
                usize_param(params, 0, kind)?,
                usize_param(params, 1, kind)?,
                usize_param(params, 2, kind)?,
                usize_param(params, 3, kind)?,
            ),
        )?))),
        "ElderRay" => Ok(Box::new(CandleInFields {
            inner: map_new(kind, wc::ElderRay::new(usize_param(params, 0, kind)?))?,
            last: None,
        })),
        "ElderSafeZone" => Ok(Box::new(CandleInFields {
            inner: map_new(
                kind,
                wc::ElderSafeZone::new(
                    usize_param(params, 0, kind)?,
                    float_param(params, 1, kind)?,
                ),
            )?,
            last: None,
        })),
        "Ema" => Ok(Box::new(ScalarPrice(map_new(
            kind,
            wc::Ema::new(usize_param(params, 0, kind)?),
        )?))),
        "EmpiricalModeDecomposition" => Ok(Box::new(ScalarPrice(map_new(
            kind,
            wc::EmpiricalModeDecomposition::new(
                usize_param(params, 0, kind)?,
                float_param(params, 1, kind)?,
            ),
        )?))),
        "Engulfing" => Ok(Box::new(CandleIn(wc::Engulfing::new()))),
        "Equivolume" => Ok(Box::new(CandleInFields {
            inner: map_new(kind, wc::Equivolume::new(usize_param(params, 0, kind)?))?,
            last: None,
        })),
        "EvenBetterSinewave" => Ok(Box::new(ScalarPrice(map_new(
            kind,
            wc::EvenBetterSinewave::new(
                usize_param(params, 0, kind)?,
                usize_param(params, 1, kind)?,
            ),
        )?))),
        "EveningDojiStar" => Ok(Box::new(CandleIn(wc::EveningDojiStar::new()))),
        "Evwma" => Ok(Box::new(CandleIn(map_new(
            kind,
            wc::Evwma::new(usize_param(params, 0, kind)?),
        )?))),
        "EwmaVolatility" => Ok(Box::new(ScalarPrice(map_new(
            kind,
            wc::EwmaVolatility::new(float_param(params, 0, kind)?),
        )?))),
        "Expectancy" => Ok(Box::new(ScalarPrice(map_new(
            kind,
            wc::Expectancy::new(usize_param(params, 0, kind)?),
        )?))),
        "FallingThreeMethods" => Ok(Box::new(CandleIn(wc::FallingThreeMethods::new()))),
        "Fama" => Ok(Box::new(ScalarPrice(map_new(
            kind,
            wc::Fama::new(float_param(params, 0, kind)?, float_param(params, 1, kind)?),
        )?))),
        "FibArcs" => Ok(Box::new(CandleInFields {
            inner: wc::FibArcs::new(),
            last: None,
        })),
        "FibChannel" => Ok(Box::new(CandleInFields {
            inner: wc::FibChannel::new(),
            last: None,
        })),
        "FibConfluence" => Ok(Box::new(CandleInFields {
            inner: wc::FibConfluence::new(),
            last: None,
        })),
        "FibExtension" => Ok(Box::new(CandleInFields {
            inner: wc::FibExtension::new(),
            last: None,
        })),
        "FibFan" => Ok(Box::new(CandleInFields {
            inner: wc::FibFan::new(),
            last: None,
        })),
        "FibProjection" => Ok(Box::new(CandleInFields {
            inner: wc::FibProjection::new(),
            last: None,
        })),
        "FibRetracement" => Ok(Box::new(CandleInFields {
            inner: wc::FibRetracement::new(),
            last: None,
        })),
        "FibTimeZones" => Ok(Box::new(CandleInFields {
            inner: wc::FibTimeZones::new(),
            last: None,
        })),
        "FibonacciPivots" => Ok(Box::new(CandleInFields {
            inner: wc::FibonacciPivots::new(),
            last: None,
        })),
        "FisherRsi" => Ok(Box::new(ScalarPrice(map_new(
            kind,
            wc::FisherRsi::new(usize_param(params, 0, kind)?),
        )?))),
        "FisherTransform" => Ok(Box::new(ScalarPrice(map_new(
            kind,
            wc::FisherTransform::new(usize_param(params, 0, kind)?),
        )?))),
        "FlagPennant" => Ok(Box::new(CandleIn(wc::FlagPennant::new()))),
        "ForceIndex" => Ok(Box::new(CandleIn(map_new(
            kind,
            wc::ForceIndex::new(usize_param(params, 0, kind)?),
        )?))),
        "FractalChaosBands" => Ok(Box::new(CandleInFields {
            inner: map_new(
                kind,
                wc::FractalChaosBands::new(usize_param(params, 0, kind)?),
            )?,
            last: None,
        })),
        "Frama" => Ok(Box::new(ScalarPrice(map_new(
            kind,
            wc::Frama::new(usize_param(params, 0, kind)?),
        )?))),
        "FryPanBottom" => Ok(Box::new(CandleIn(map_new(
            kind,
            wc::FryPanBottom::new(usize_param(params, 0, kind)?),
        )?))),
        "GainLossRatio" => Ok(Box::new(ScalarPrice(map_new(
            kind,
            wc::GainLossRatio::new(usize_param(params, 0, kind)?),
        )?))),
        "GainToPainRatio" => Ok(Box::new(ScalarPrice(map_new(
            kind,
            wc::GainToPainRatio::new(usize_param(params, 0, kind)?),
        )?))),
        "GapSideBySideWhite" => Ok(Box::new(CandleIn(wc::GapSideBySideWhite::new()))),
        "Garch11" => Ok(Box::new(ScalarPrice(map_new(
            kind,
            wc::Garch11::new(
                float_param(params, 0, kind)?,
                float_param(params, 1, kind)?,
                float_param(params, 2, kind)?,
            ),
        )?))),
        "GarmanKlassVolatility" => Ok(Box::new(CandleIn(map_new(
            kind,
            wc::GarmanKlassVolatility::new(
                usize_param(params, 0, kind)?,
                usize_param(params, 1, kind)?,
            ),
        )?))),
        "Gartley" => Ok(Box::new(CandleIn(wc::Gartley::new()))),
        "GatorOscillator" => Ok(Box::new(CandleInFields {
            inner: map_new(
                kind,
                wc::GatorOscillator::new(
                    usize_param(params, 0, kind)?,
                    usize_param(params, 1, kind)?,
                    usize_param(params, 2, kind)?,
                ),
            )?,
            last: None,
        })),
        "GeneralizedDema" => Ok(Box::new(ScalarPrice(map_new(
            kind,
            wc::GeneralizedDema::new(usize_param(params, 0, kind)?, float_param(params, 1, kind)?),
        )?))),
        "GeometricMa" => Ok(Box::new(ScalarPrice(map_new(
            kind,
            wc::GeometricMa::new(usize_param(params, 0, kind)?),
        )?))),
        "GoldenPocket" => Ok(Box::new(CandleInFields {
            inner: wc::GoldenPocket::new(),
            last: None,
        })),
        "GravestoneDoji" => Ok(Box::new(CandleIn(wc::GravestoneDoji::new()))),
        "Hammer" => Ok(Box::new(CandleIn(wc::Hammer::new()))),
        "HangingMan" => Ok(Box::new(CandleIn(wc::HangingMan::new()))),
        "Harami" => Ok(Box::new(CandleIn(wc::Harami::new()))),
        "HaramiCross" => Ok(Box::new(CandleIn(wc::HaramiCross::new()))),
        "HeadAndShoulders" => Ok(Box::new(CandleIn(wc::HeadAndShoulders::new()))),
        "HeikinAshi" => Ok(Box::new(CandleInFields {
            inner: wc::HeikinAshi::new(),
            last: None,
        })),
        "HeikinAshiOscillator" => Ok(Box::new(CandleIn(map_new(
            kind,
            wc::HeikinAshiOscillator::new(usize_param(params, 0, kind)?),
        )?))),
        "HiLoActivator" => Ok(Box::new(CandleIn(map_new(
            kind,
            wc::HiLoActivator::new(usize_param(params, 0, kind)?),
        )?))),
        "HighLowRange" => Ok(Box::new(CandleIn(wc::HighLowRange::new()))),
        "HighLowVolumeNodes" => Ok(Box::new(CandleInFields {
            inner: map_new(
                kind,
                wc::HighLowVolumeNodes::new(
                    usize_param(params, 0, kind)?,
                    usize_param(params, 1, kind)?,
                ),
            )?,
            last: None,
        })),
        "HighWave" => Ok(Box::new(CandleIn(wc::HighWave::new()))),
        "HighpassFilter" => Ok(Box::new(ScalarPrice(map_new(
            kind,
            wc::HighpassFilter::new(usize_param(params, 0, kind)?),
        )?))),
        "Hikkake" => Ok(Box::new(CandleIn(wc::Hikkake::new()))),
        "HikkakeModified" => Ok(Box::new(CandleIn(wc::HikkakeModified::new()))),
        "HilbertDominantCycle" => Ok(Box::new(ScalarPrice(wc::HilbertDominantCycle::new()))),
        "HistoricalVolatility" => Ok(Box::new(ScalarPrice(map_new(
            kind,
            wc::HistoricalVolatility::new(
                usize_param(params, 0, kind)?,
                usize_param(params, 1, kind)?,
            ),
        )?))),
        "Hma" => Ok(Box::new(ScalarPrice(map_new(
            kind,
            wc::Hma::new(usize_param(params, 0, kind)?),
        )?))),
        "HoltWinters" => Ok(Box::new(ScalarPrice(map_new(
            kind,
            wc::HoltWinters::new(float_param(params, 0, kind)?, float_param(params, 1, kind)?),
        )?))),
        "HomingPigeon" => Ok(Box::new(CandleIn(wc::HomingPigeon::new()))),
        "HtDcPhase" => Ok(Box::new(ScalarPrice(wc::HtDcPhase::new()))),
        "HtPhasor" => Ok(Box::new(ScalarPriceFields {
            inner: wc::HtPhasor::new(),
            last: None,
        })),
        "HtTrendMode" => Ok(Box::new(ScalarPrice(wc::HtTrendMode::new()))),
        "HurstChannel" => Ok(Box::new(CandleInFields {
            inner: map_new(
                kind,
                wc::HurstChannel::new(usize_param(params, 0, kind)?, float_param(params, 1, kind)?),
            )?,
            last: None,
        })),
        "HurstExponent" => Ok(Box::new(ScalarPrice(map_new(
            kind,
            wc::HurstExponent::new(usize_param(params, 0, kind)?, usize_param(params, 1, kind)?),
        )?))),
        "IdenticalThreeCrows" => Ok(Box::new(CandleIn(wc::IdenticalThreeCrows::new()))),
        "InNeck" => Ok(Box::new(CandleIn(wc::InNeck::new()))),
        "Inertia" => Ok(Box::new(CandleIn(map_new(
            kind,
            wc::Inertia::new(usize_param(params, 0, kind)?, usize_param(params, 1, kind)?),
        )?))),
        "InitialBalance" => Ok(Box::new(CandleInFields {
            inner: map_new(kind, wc::InitialBalance::new(usize_param(params, 0, kind)?))?,
            last: None,
        })),
        "InstantaneousTrendline" => Ok(Box::new(ScalarPrice(map_new(
            kind,
            wc::InstantaneousTrendline::new(usize_param(params, 0, kind)?),
        )?))),
        "IntradayIntensity" => Ok(Box::new(CandleIn(wc::IntradayIntensity::new()))),
        "IntradayMomentumIndex" => Ok(Box::new(CandleIn(map_new(
            kind,
            wc::IntradayMomentumIndex::new(usize_param(params, 0, kind)?),
        )?))),
        "InverseFisherTransform" => Ok(Box::new(ScalarPrice(map_new(
            kind,
            wc::InverseFisherTransform::new(float_param(params, 0, kind)?),
        )?))),
        "InvertedHammer" => Ok(Box::new(CandleIn(wc::InvertedHammer::new()))),
        "JarqueBera" => Ok(Box::new(ScalarPrice(map_new(
            kind,
            wc::JarqueBera::new(usize_param(params, 0, kind)?),
        )?))),
        "Jma" => Ok(Box::new(ScalarPrice(map_new(
            kind,
            wc::Jma::new(
                usize_param(params, 0, kind)?,
                float_param(params, 1, kind)?,
                u32_param(params, 2, kind)?,
            ),
        )?))),
        "JumpIndicator" => Ok(Box::new(ScalarPrice(map_new(
            kind,
            wc::JumpIndicator::new(usize_param(params, 0, kind)?, float_param(params, 1, kind)?),
        )?))),
        "KRatio" => Ok(Box::new(ScalarPrice(map_new(
            kind,
            wc::KRatio::new(usize_param(params, 0, kind)?),
        )?))),
        "Kama" => Ok(Box::new(ScalarPrice(map_new(
            kind,
            wc::Kama::new(
                usize_param(params, 0, kind)?,
                usize_param(params, 1, kind)?,
                usize_param(params, 2, kind)?,
            ),
        )?))),
        "KaseDevStop" => Ok(Box::new(CandleInFields {
            inner: map_new(
                kind,
                wc::KaseDevStop::new(usize_param(params, 0, kind)?, float_param(params, 1, kind)?),
            )?,
            last: None,
        })),
        "KasePermissionStochastic" => Ok(Box::new(CandleInFields {
            inner: map_new(
                kind,
                wc::KasePermissionStochastic::new(
                    usize_param(params, 0, kind)?,
                    usize_param(params, 1, kind)?,
                ),
            )?,
            last: None,
        })),
        "KellyCriterion" => Ok(Box::new(ScalarPrice(map_new(
            kind,
            wc::KellyCriterion::new(usize_param(params, 0, kind)?),
        )?))),
        "Keltner" => Ok(Box::new(CandleInFields {
            inner: map_new(
                kind,
                wc::Keltner::new(
                    usize_param(params, 0, kind)?,
                    usize_param(params, 1, kind)?,
                    float_param(params, 2, kind)?,
                ),
            )?,
            last: None,
        })),
        "Kicking" => Ok(Box::new(CandleIn(wc::Kicking::new()))),
        "KickingByLength" => Ok(Box::new(CandleIn(wc::KickingByLength::new()))),
        "Kst" => Ok(Box::new(ScalarPriceFields {
            inner: map_new(
                kind,
                wc::Kst::new(
                    usize_param(params, 0, kind)?,
                    usize_param(params, 1, kind)?,
                    usize_param(params, 2, kind)?,
                    usize_param(params, 3, kind)?,
                    usize_param(params, 4, kind)?,
                    usize_param(params, 5, kind)?,
                    usize_param(params, 6, kind)?,
                    usize_param(params, 7, kind)?,
                    usize_param(params, 8, kind)?,
                ),
            )?,
            last: None,
        })),
        "Kurtosis" => Ok(Box::new(ScalarPrice(map_new(
            kind,
            wc::Kurtosis::new(usize_param(params, 0, kind)?),
        )?))),
        "Kvo" => Ok(Box::new(CandleIn(map_new(
            kind,
            wc::Kvo::new(usize_param(params, 0, kind)?, usize_param(params, 1, kind)?),
        )?))),
        "LadderBottom" => Ok(Box::new(CandleIn(wc::LadderBottom::new()))),
        "LaguerreRsi" => Ok(Box::new(ScalarPrice(map_new(
            kind,
            wc::LaguerreRsi::new(float_param(params, 0, kind)?),
        )?))),
        "LinRegAngle" => Ok(Box::new(ScalarPrice(map_new(
            kind,
            wc::LinRegAngle::new(usize_param(params, 0, kind)?),
        )?))),
        "LinRegChannel" => Ok(Box::new(ScalarPriceFields {
            inner: map_new(
                kind,
                wc::LinRegChannel::new(
                    usize_param(params, 0, kind)?,
                    float_param(params, 1, kind)?,
                ),
            )?,
            last: None,
        })),
        "LinRegIntercept" => Ok(Box::new(ScalarPrice(map_new(
            kind,
            wc::LinRegIntercept::new(usize_param(params, 0, kind)?),
        )?))),
        "LinRegSlope" => Ok(Box::new(ScalarPrice(map_new(
            kind,
            wc::LinRegSlope::new(usize_param(params, 0, kind)?),
        )?))),
        "LinearRegression" => Ok(Box::new(ScalarPrice(map_new(
            kind,
            wc::LinearRegression::new(usize_param(params, 0, kind)?),
        )?))),
        "LogReturn" => Ok(Box::new(ScalarPrice(map_new(
            kind,
            wc::LogReturn::new(usize_param(params, 0, kind)?),
        )?))),
        "LongLeggedDoji" => Ok(Box::new(CandleIn(wc::LongLeggedDoji::new()))),
        "LongLine" => Ok(Box::new(CandleIn(wc::LongLine::new()))),
        "M2Measure" => Ok(Box::new(ScalarPrice(map_new(
            kind,
            wc::M2Measure::new(
                usize_param(params, 0, kind)?,
                float_param(params, 1, kind)?,
                float_param(params, 2, kind)?,
            ),
        )?))),
        "MaEnvelope" => Ok(Box::new(ScalarPriceFields {
            inner: map_new(
                kind,
                wc::MaEnvelope::new(usize_param(params, 0, kind)?, float_param(params, 1, kind)?),
            )?,
            last: None,
        })),
        "MacdFix" => Ok(Box::new(ScalarPriceFields {
            inner: map_new(kind, wc::MacdFix::new(usize_param(params, 0, kind)?))?,
            last: None,
        })),
        "MacdHistogram" => Ok(Box::new(ScalarPrice(map_new(
            kind,
            wc::MacdHistogram::new(
                usize_param(params, 0, kind)?,
                usize_param(params, 1, kind)?,
                usize_param(params, 2, kind)?,
            ),
        )?))),
        "MacdIndicator" => Ok(Box::new(ScalarPriceFields {
            inner: map_new(
                kind,
                wc::MacdIndicator::new(
                    usize_param(params, 0, kind)?,
                    usize_param(params, 1, kind)?,
                    usize_param(params, 2, kind)?,
                ),
            )?,
            last: None,
        })),
        "Mama" => Ok(Box::new(ScalarPriceFields {
            inner: map_new(
                kind,
                wc::Mama::new(float_param(params, 0, kind)?, float_param(params, 1, kind)?),
            )?,
            last: None,
        })),
        "MarketFacilitationIndex" => Ok(Box::new(CandleIn(wc::MarketFacilitationIndex::new()))),
        "MartinRatio" => Ok(Box::new(ScalarPrice(map_new(
            kind,
            wc::MartinRatio::new(usize_param(params, 0, kind)?),
        )?))),
        "Marubozu" => Ok(Box::new(CandleIn(wc::Marubozu::new()))),
        "MassIndex" => Ok(Box::new(CandleIn(map_new(
            kind,
            wc::MassIndex::new(usize_param(params, 0, kind)?, usize_param(params, 1, kind)?),
        )?))),
        "MatHold" => Ok(Box::new(CandleIn(wc::MatHold::new()))),
        "MatchingLow" => Ok(Box::new(CandleIn(wc::MatchingLow::new()))),
        "MaxDrawdown" => Ok(Box::new(ScalarPrice(map_new(
            kind,
            wc::MaxDrawdown::new(usize_param(params, 0, kind)?),
        )?))),
        "McGinleyDynamic" => Ok(Box::new(ScalarPrice(map_new(
            kind,
            wc::McGinleyDynamic::new(usize_param(params, 0, kind)?),
        )?))),
        "MedianAbsoluteDeviation" => Ok(Box::new(ScalarPrice(map_new(
            kind,
            wc::MedianAbsoluteDeviation::new(usize_param(params, 0, kind)?),
        )?))),
        "MedianChannel" => Ok(Box::new(ScalarPriceFields {
            inner: map_new(
                kind,
                wc::MedianChannel::new(
                    usize_param(params, 0, kind)?,
                    float_param(params, 1, kind)?,
                ),
            )?,
            last: None,
        })),
        "MedianMa" => Ok(Box::new(ScalarPrice(map_new(
            kind,
            wc::MedianMa::new(usize_param(params, 0, kind)?),
        )?))),
        "MedianPrice" => Ok(Box::new(CandleIn(wc::MedianPrice::new()))),
        "Mfi" => Ok(Box::new(CandleIn(map_new(
            kind,
            wc::Mfi::new(usize_param(params, 0, kind)?),
        )?))),
        "MidPoint" => Ok(Box::new(ScalarPrice(map_new(
            kind,
            wc::MidPoint::new(usize_param(params, 0, kind)?),
        )?))),
        "MidPrice" => Ok(Box::new(CandleIn(map_new(
            kind,
            wc::MidPrice::new(usize_param(params, 0, kind)?),
        )?))),
        "MinusDi" => Ok(Box::new(CandleIn(map_new(
            kind,
            wc::MinusDi::new(usize_param(params, 0, kind)?),
        )?))),
        "MinusDm" => Ok(Box::new(CandleIn(map_new(
            kind,
            wc::MinusDm::new(usize_param(params, 0, kind)?),
        )?))),
        "ModifiedMaStop" => Ok(Box::new(CandleInFields {
            inner: map_new(kind, wc::ModifiedMaStop::new(usize_param(params, 0, kind)?))?,
            last: None,
        })),
        "Mom" => Ok(Box::new(ScalarPrice(map_new(
            kind,
            wc::Mom::new(usize_param(params, 0, kind)?),
        )?))),
        "MorningDojiStar" => Ok(Box::new(CandleIn(wc::MorningDojiStar::new()))),
        "MorningEveningStar" => Ok(Box::new(CandleIn(wc::MorningEveningStar::new()))),
        "MurreyMathLines" => Ok(Box::new(CandleInFields {
            inner: map_new(
                kind,
                wc::MurreyMathLines::new(usize_param(params, 0, kind)?),
            )?,
            last: None,
        })),
        "NakedPoc" => Ok(Box::new(CandleIn(map_new(
            kind,
            wc::NakedPoc::new(usize_param(params, 0, kind)?, usize_param(params, 1, kind)?),
        )?))),
        "Natr" => Ok(Box::new(CandleIn(map_new(
            kind,
            wc::Natr::new(usize_param(params, 0, kind)?),
        )?))),
        "NewPriceLines" => Ok(Box::new(CandleIn(map_new(
            kind,
            wc::NewPriceLines::new(usize_param(params, 0, kind)?),
        )?))),
        "Nrtr" => Ok(Box::new(CandleInFields {
            inner: map_new(kind, wc::Nrtr::new(float_param(params, 0, kind)?))?,
            last: None,
        })),
        "Nvi" => Ok(Box::new(CandleIn(wc::Nvi::new()))),
        "Obv" => Ok(Box::new(CandleIn(wc::Obv::new()))),
        "OmegaRatio" => Ok(Box::new(ScalarPrice(map_new(
            kind,
            wc::OmegaRatio::new(usize_param(params, 0, kind)?, float_param(params, 1, kind)?),
        )?))),
        "OnNeck" => Ok(Box::new(CandleIn(wc::OnNeck::new()))),
        "OpeningMarubozu" => Ok(Box::new(CandleIn(wc::OpeningMarubozu::new()))),
        "OpeningRange" => Ok(Box::new(CandleInFields {
            inner: map_new(kind, wc::OpeningRange::new(usize_param(params, 0, kind)?))?,
            last: None,
        })),
        "OvernightGap" => Ok(Box::new(CandleIn(wc::OvernightGap::new(i32_param(
            params, 0, kind,
        )?)))),
        "OvernightIntradayReturn" => Ok(Box::new(CandleInFields {
            inner: wc::OvernightIntradayReturn::new(i32_param(params, 0, kind)?),
            last: None,
        })),
        "PainIndex" => Ok(Box::new(ScalarPrice(map_new(
            kind,
            wc::PainIndex::new(usize_param(params, 0, kind)?),
        )?))),
        "ParkinsonVolatility" => Ok(Box::new(CandleIn(map_new(
            kind,
            wc::ParkinsonVolatility::new(
                usize_param(params, 0, kind)?,
                usize_param(params, 1, kind)?,
            ),
        )?))),
        "PercentB" => Ok(Box::new(ScalarPrice(map_new(
            kind,
            wc::PercentB::new(usize_param(params, 0, kind)?, float_param(params, 1, kind)?),
        )?))),
        "PercentageTrailingStop" => Ok(Box::new(ScalarPrice(map_new(
            kind,
            wc::PercentageTrailingStop::new(float_param(params, 0, kind)?),
        )?))),
        "Pgo" => Ok(Box::new(CandleIn(map_new(
            kind,
            wc::Pgo::new(usize_param(params, 0, kind)?),
        )?))),
        "PiercingDarkCloud" => Ok(Box::new(CandleIn(wc::PiercingDarkCloud::new()))),
        "PivotReversal" => Ok(Box::new(CandleIn(map_new(
            kind,
            wc::PivotReversal::new(usize_param(params, 0, kind)?, usize_param(params, 1, kind)?),
        )?))),
        "PlusDi" => Ok(Box::new(CandleIn(map_new(
            kind,
            wc::PlusDi::new(usize_param(params, 0, kind)?),
        )?))),
        "PlusDm" => Ok(Box::new(CandleIn(map_new(
            kind,
            wc::PlusDm::new(usize_param(params, 0, kind)?),
        )?))),
        "Pmo" => Ok(Box::new(ScalarPrice(map_new(
            kind,
            wc::Pmo::new(usize_param(params, 0, kind)?, usize_param(params, 1, kind)?),
        )?))),
        "PolarizedFractalEfficiency" => Ok(Box::new(ScalarPrice(map_new(
            kind,
            wc::PolarizedFractalEfficiency::new(
                usize_param(params, 0, kind)?,
                usize_param(params, 1, kind)?,
            ),
        )?))),
        "Ppo" => Ok(Box::new(ScalarPrice(map_new(
            kind,
            wc::Ppo::new(usize_param(params, 0, kind)?, usize_param(params, 1, kind)?),
        )?))),
        "PpoHistogram" => Ok(Box::new(ScalarPrice(map_new(
            kind,
            wc::PpoHistogram::new(
                usize_param(params, 0, kind)?,
                usize_param(params, 1, kind)?,
                usize_param(params, 2, kind)?,
            ),
        )?))),
        "ProfileShape" => Ok(Box::new(CandleIn(map_new(
            kind,
            wc::ProfileShape::new(usize_param(params, 0, kind)?, usize_param(params, 1, kind)?),
        )?))),
        "ProfitFactor" => Ok(Box::new(ScalarPrice(map_new(
            kind,
            wc::ProfitFactor::new(usize_param(params, 0, kind)?),
        )?))),
        "ProjectionBands" => Ok(Box::new(CandleInFields {
            inner: map_new(
                kind,
                wc::ProjectionBands::new(usize_param(params, 0, kind)?),
            )?,
            last: None,
        })),
        "ProjectionOscillator" => Ok(Box::new(CandleIn(map_new(
            kind,
            wc::ProjectionOscillator::new(usize_param(params, 0, kind)?),
        )?))),
        "Psar" => Ok(Box::new(CandleIn(map_new(
            kind,
            wc::Psar::new(
                float_param(params, 0, kind)?,
                float_param(params, 1, kind)?,
                float_param(params, 2, kind)?,
            ),
        )?))),
        "Pvi" => Ok(Box::new(CandleIn(wc::Pvi::new()))),
        "Qqe" => Ok(Box::new(ScalarPriceFields {
            inner: map_new(
                kind,
                wc::Qqe::new(
                    usize_param(params, 0, kind)?,
                    usize_param(params, 1, kind)?,
                    float_param(params, 2, kind)?,
                ),
            )?,
            last: None,
        })),
        "Qstick" => Ok(Box::new(CandleIn(map_new(
            kind,
            wc::Qstick::new(usize_param(params, 0, kind)?),
        )?))),
        "QuartileBands" => Ok(Box::new(ScalarPriceFields {
            inner: map_new(kind, wc::QuartileBands::new(usize_param(params, 0, kind)?))?,
            last: None,
        })),
        "RSquared" => Ok(Box::new(ScalarPrice(map_new(
            kind,
            wc::RSquared::new(usize_param(params, 0, kind)?),
        )?))),
        "RealizedVolatility" => Ok(Box::new(ScalarPrice(map_new(
            kind,
            wc::RealizedVolatility::new(usize_param(params, 0, kind)?),
        )?))),
        "RecoveryFactor" => Ok(Box::new(ScalarPrice(wc::RecoveryFactor::new()))),
        "RectangleRange" => Ok(Box::new(CandleIn(wc::RectangleRange::new()))),
        "Reflex" => Ok(Box::new(ScalarPrice(map_new(
            kind,
            wc::Reflex::new(usize_param(params, 0, kind)?),
        )?))),
        "RegimeLabel" => Ok(Box::new(ScalarPrice(map_new(
            kind,
            wc::RegimeLabel::new(usize_param(params, 0, kind)?, usize_param(params, 1, kind)?),
        )?))),
        "RenkoTrailingStop" => Ok(Box::new(ScalarPrice(map_new(
            kind,
            wc::RenkoTrailingStop::new(float_param(params, 0, kind)?),
        )?))),
        "RickshawMan" => Ok(Box::new(CandleIn(wc::RickshawMan::new()))),
        "RisingThreeMethods" => Ok(Box::new(CandleIn(wc::RisingThreeMethods::new()))),
        "Rmi" => Ok(Box::new(ScalarPrice(map_new(
            kind,
            wc::Rmi::new(usize_param(params, 0, kind)?, usize_param(params, 1, kind)?),
        )?))),
        "Roc" => Ok(Box::new(ScalarPrice(map_new(
            kind,
            wc::Roc::new(usize_param(params, 0, kind)?),
        )?))),
        "Rocp" => Ok(Box::new(ScalarPrice(map_new(
            kind,
            wc::Rocp::new(usize_param(params, 0, kind)?),
        )?))),
        "Rocr" => Ok(Box::new(ScalarPrice(map_new(
            kind,
            wc::Rocr::new(usize_param(params, 0, kind)?),
        )?))),
        "Rocr100" => Ok(Box::new(ScalarPrice(map_new(
            kind,
            wc::Rocr100::new(usize_param(params, 0, kind)?),
        )?))),
        "RogersSatchellVolatility" => Ok(Box::new(CandleIn(map_new(
            kind,
            wc::RogersSatchellVolatility::new(
                usize_param(params, 0, kind)?,
                usize_param(params, 1, kind)?,
            ),
        )?))),
        "RollingIqr" => Ok(Box::new(ScalarPrice(map_new(
            kind,
            wc::RollingIqr::new(usize_param(params, 0, kind)?),
        )?))),
        "RollingMinMaxScaler" => Ok(Box::new(ScalarPrice(map_new(
            kind,
            wc::RollingMinMaxScaler::new(usize_param(params, 0, kind)?),
        )?))),
        "RollingPercentileRank" => Ok(Box::new(ScalarPrice(map_new(
            kind,
            wc::RollingPercentileRank::new(usize_param(params, 0, kind)?),
        )?))),
        "RollingQuantile" => Ok(Box::new(ScalarPrice(map_new(
            kind,
            wc::RollingQuantile::new(usize_param(params, 0, kind)?, float_param(params, 1, kind)?),
        )?))),
        "RollingVwap" => Ok(Box::new(CandleIn(map_new(
            kind,
            wc::RollingVwap::new(usize_param(params, 0, kind)?),
        )?))),
        "RoofingFilter" => Ok(Box::new(ScalarPrice(map_new(
            kind,
            wc::RoofingFilter::new(usize_param(params, 0, kind)?, usize_param(params, 1, kind)?),
        )?))),
        "Rsi" => Ok(Box::new(ScalarPrice(map_new(
            kind,
            wc::Rsi::new(usize_param(params, 0, kind)?),
        )?))),
        "Rsx" => Ok(Box::new(ScalarPrice(map_new(
            kind,
            wc::Rsx::new(usize_param(params, 0, kind)?),
        )?))),
        "Rvi" => Ok(Box::new(CandleIn(map_new(
            kind,
            wc::Rvi::new(usize_param(params, 0, kind)?),
        )?))),
        "RviVolatility" => Ok(Box::new(ScalarPrice(map_new(
            kind,
            wc::RviVolatility::new(usize_param(params, 0, kind)?),
        )?))),
        "Rwi" => Ok(Box::new(CandleInFields {
            inner: map_new(kind, wc::Rwi::new(usize_param(params, 0, kind)?))?,
            last: None,
        })),
        "SampleEntropy" => Ok(Box::new(ScalarPrice(map_new(
            kind,
            wc::SampleEntropy::new(
                usize_param(params, 0, kind)?,
                usize_param(params, 1, kind)?,
                float_param(params, 2, kind)?,
            ),
        )?))),
        "SarExt" => Ok(Box::new(CandleIn(map_new(
            kind,
            wc::SarExt::new(
                float_param(params, 0, kind)?,
                float_param(params, 1, kind)?,
                float_param(params, 2, kind)?,
                float_param(params, 3, kind)?,
                float_param(params, 4, kind)?,
                float_param(params, 5, kind)?,
                float_param(params, 6, kind)?,
                float_param(params, 7, kind)?,
            ),
        )?))),
        "SeasonalZScore" => Ok(Box::new(CandleIn(wc::SeasonalZScore::new(i32_param(
            params, 0, kind,
        )?)))),
        "SeparatingLines" => Ok(Box::new(CandleIn(wc::SeparatingLines::new()))),
        "SessionHighLow" => Ok(Box::new(CandleInFields {
            inner: wc::SessionHighLow::new(i32_param(params, 0, kind)?),
            last: None,
        })),
        "SessionRange" => Ok(Box::new(CandleInFields {
            inner: wc::SessionRange::new(i32_param(params, 0, kind)?),
            last: None,
        })),
        "SessionVwap" => Ok(Box::new(CandleIn(wc::SessionVwap::new(i32_param(
            params, 0, kind,
        )?)))),
        "ShannonEntropy" => Ok(Box::new(ScalarPrice(map_new(
            kind,
            wc::ShannonEntropy::new(usize_param(params, 0, kind)?, usize_param(params, 1, kind)?),
        )?))),
        "Shark" => Ok(Box::new(CandleIn(wc::Shark::new()))),
        "SharpeRatio" => Ok(Box::new(ScalarPrice(map_new(
            kind,
            wc::SharpeRatio::new(usize_param(params, 0, kind)?, float_param(params, 1, kind)?),
        )?))),
        "ShootingStar" => Ok(Box::new(CandleIn(wc::ShootingStar::new()))),
        "ShortLine" => Ok(Box::new(CandleIn(wc::ShortLine::new()))),
        "SineWave" => Ok(Box::new(ScalarPrice(wc::SineWave::new()))),
        "SineWeightedMa" => Ok(Box::new(ScalarPrice(map_new(
            kind,
            wc::SineWeightedMa::new(usize_param(params, 0, kind)?),
        )?))),
        "SinglePrints" => Ok(Box::new(CandleIn(map_new(
            kind,
            wc::SinglePrints::new(usize_param(params, 0, kind)?, usize_param(params, 1, kind)?),
        )?))),
        "Skewness" => Ok(Box::new(ScalarPrice(map_new(
            kind,
            wc::Skewness::new(usize_param(params, 0, kind)?),
        )?))),
        "Sma" => Ok(Box::new(ScalarPrice(map_new(
            kind,
            wc::Sma::new(usize_param(params, 0, kind)?),
        )?))),
        "Smi" => Ok(Box::new(CandleIn(map_new(
            kind,
            wc::Smi::new(
                usize_param(params, 0, kind)?,
                usize_param(params, 1, kind)?,
                usize_param(params, 2, kind)?,
            ),
        )?))),
        "Smma" => Ok(Box::new(ScalarPrice(map_new(
            kind,
            wc::Smma::new(usize_param(params, 0, kind)?),
        )?))),
        "SmoothedHeikinAshi" => Ok(Box::new(CandleInFields {
            inner: map_new(
                kind,
                wc::SmoothedHeikinAshi::new(usize_param(params, 0, kind)?),
            )?,
            last: None,
        })),
        "SortinoRatio" => Ok(Box::new(ScalarPrice(map_new(
            kind,
            wc::SortinoRatio::new(usize_param(params, 0, kind)?, float_param(params, 1, kind)?),
        )?))),
        "SpinningTop" => Ok(Box::new(CandleIn(wc::SpinningTop::new()))),
        "StalledPattern" => Ok(Box::new(CandleIn(wc::StalledPattern::new()))),
        "StandardError" => Ok(Box::new(ScalarPrice(map_new(
            kind,
            wc::StandardError::new(usize_param(params, 0, kind)?),
        )?))),
        "StandardErrorBands" => Ok(Box::new(ScalarPriceFields {
            inner: map_new(
                kind,
                wc::StandardErrorBands::new(
                    usize_param(params, 0, kind)?,
                    float_param(params, 1, kind)?,
                ),
            )?,
            last: None,
        })),
        "StarcBands" => Ok(Box::new(CandleInFields {
            inner: map_new(
                kind,
                wc::StarcBands::new(
                    usize_param(params, 0, kind)?,
                    usize_param(params, 1, kind)?,
                    float_param(params, 2, kind)?,
                ),
            )?,
            last: None,
        })),
        "Stc" => Ok(Box::new(ScalarPrice(map_new(
            kind,
            wc::Stc::new(
                usize_param(params, 0, kind)?,
                usize_param(params, 1, kind)?,
                usize_param(params, 2, kind)?,
                float_param(params, 3, kind)?,
            ),
        )?))),
        "StdDev" => Ok(Box::new(ScalarPrice(map_new(
            kind,
            wc::StdDev::new(usize_param(params, 0, kind)?),
        )?))),
        "StepTrailingStop" => Ok(Box::new(ScalarPrice(map_new(
            kind,
            wc::StepTrailingStop::new(float_param(params, 0, kind)?),
        )?))),
        "SterlingRatio" => Ok(Box::new(ScalarPrice(map_new(
            kind,
            wc::SterlingRatio::new(usize_param(params, 0, kind)?),
        )?))),
        "StickSandwich" => Ok(Box::new(CandleIn(wc::StickSandwich::new()))),
        "StochRsi" => Ok(Box::new(ScalarPrice(map_new(
            kind,
            wc::StochRsi::new(usize_param(params, 0, kind)?, usize_param(params, 1, kind)?),
        )?))),
        "Stochastic" => Ok(Box::new(CandleInFields {
            inner: map_new(
                kind,
                wc::Stochastic::new(usize_param(params, 0, kind)?, usize_param(params, 1, kind)?),
            )?,
            last: None,
        })),
        "StochasticCci" => Ok(Box::new(CandleIn(map_new(
            kind,
            wc::StochasticCci::new(usize_param(params, 0, kind)?),
        )?))),
        "SuperSmoother" => Ok(Box::new(ScalarPrice(map_new(
            kind,
            wc::SuperSmoother::new(usize_param(params, 0, kind)?),
        )?))),
        "SuperTrend" => Ok(Box::new(CandleInFields {
            inner: map_new(
                kind,
                wc::SuperTrend::new(usize_param(params, 0, kind)?, float_param(params, 1, kind)?),
            )?,
            last: None,
        })),
        "T3" => Ok(Box::new(ScalarPrice(map_new(
            kind,
            wc::T3::new(usize_param(params, 0, kind)?, float_param(params, 1, kind)?),
        )?))),
        "TailRatio" => Ok(Box::new(ScalarPrice(map_new(
            kind,
            wc::TailRatio::new(usize_param(params, 0, kind)?),
        )?))),
        "Takuri" => Ok(Box::new(CandleIn(wc::Takuri::new()))),
        "TasukiGap" => Ok(Box::new(CandleIn(wc::TasukiGap::new()))),
        "TdCamouflage" => Ok(Box::new(CandleIn(wc::TdCamouflage::new()))),
        "TdClop" => Ok(Box::new(CandleIn(wc::TdClop::new()))),
        "TdClopwin" => Ok(Box::new(CandleIn(wc::TdClopwin::new()))),
        "TdCombo" => Ok(Box::new(CandleIn(map_new(
            kind,
            wc::TdCombo::new(
                usize_param(params, 0, kind)?,
                usize_param(params, 1, kind)?,
                usize_param(params, 2, kind)?,
                usize_param(params, 3, kind)?,
            ),
        )?))),
        "TdCountdown" => Ok(Box::new(CandleIn(map_new(
            kind,
            wc::TdCountdown::new(
                usize_param(params, 0, kind)?,
                usize_param(params, 1, kind)?,
                usize_param(params, 2, kind)?,
                usize_param(params, 3, kind)?,
            ),
        )?))),
        "TdDWave" => Ok(Box::new(CandleIn(map_new(
            kind,
            wc::TdDWave::new(usize_param(params, 0, kind)?),
        )?))),
        "TdDeMarker" => Ok(Box::new(CandleIn(map_new(
            kind,
            wc::TdDeMarker::new(usize_param(params, 0, kind)?),
        )?))),
        "TdDifferential" => Ok(Box::new(CandleIn(wc::TdDifferential::new()))),
        "TdLines" => Ok(Box::new(CandleInFields {
            inner: map_new(
                kind,
                wc::TdLines::new(usize_param(params, 0, kind)?, usize_param(params, 1, kind)?),
            )?,
            last: None,
        })),
        "TdMovingAverage" => Ok(Box::new(CandleInFields {
            inner: map_new(
                kind,
                wc::TdMovingAverage::new(
                    usize_param(params, 0, kind)?,
                    usize_param(params, 1, kind)?,
                ),
            )?,
            last: None,
        })),
        "TdOpen" => Ok(Box::new(CandleIn(wc::TdOpen::new()))),
        "TdPressure" => Ok(Box::new(CandleIn(map_new(
            kind,
            wc::TdPressure::new(usize_param(params, 0, kind)?),
        )?))),
        "TdPropulsion" => Ok(Box::new(CandleIn(wc::TdPropulsion::new()))),
        "TdRangeProjection" => Ok(Box::new(CandleInFields {
            inner: wc::TdRangeProjection::new(),
            last: None,
        })),
        "TdRei" => Ok(Box::new(CandleIn(map_new(
            kind,
            wc::TdRei::new(usize_param(params, 0, kind)?),
        )?))),
        "TdRiskLevel" => Ok(Box::new(CandleInFields {
            inner: map_new(
                kind,
                wc::TdRiskLevel::new(usize_param(params, 0, kind)?, usize_param(params, 1, kind)?),
            )?,
            last: None,
        })),
        "TdSequential" => Ok(Box::new(CandleInFields {
            inner: map_new(
                kind,
                wc::TdSequential::new(
                    usize_param(params, 0, kind)?,
                    usize_param(params, 1, kind)?,
                    usize_param(params, 2, kind)?,
                    usize_param(params, 3, kind)?,
                ),
            )?,
            last: None,
        })),
        "TdSetup" => Ok(Box::new(CandleIn(map_new(
            kind,
            wc::TdSetup::new(usize_param(params, 0, kind)?, usize_param(params, 1, kind)?),
        )?))),
        "TdTrap" => Ok(Box::new(CandleIn(wc::TdTrap::new()))),
        "Tema" => Ok(Box::new(ScalarPrice(map_new(
            kind,
            wc::Tema::new(usize_param(params, 0, kind)?),
        )?))),
        "ThreeDrives" => Ok(Box::new(CandleIn(wc::ThreeDrives::new()))),
        "ThreeInside" => Ok(Box::new(CandleIn(wc::ThreeInside::new()))),
        "ThreeLineBreak" => Ok(Box::new(CandleIn(map_new(
            kind,
            wc::ThreeLineBreak::new(usize_param(params, 0, kind)?),
        )?))),
        "ThreeLineStrike" => Ok(Box::new(CandleIn(wc::ThreeLineStrike::new()))),
        "ThreeOutside" => Ok(Box::new(CandleIn(wc::ThreeOutside::new()))),
        "ThreeSoldiersOrCrows" => Ok(Box::new(CandleIn(wc::ThreeSoldiersOrCrows::new()))),
        "ThreeStarsInSouth" => Ok(Box::new(CandleIn(wc::ThreeStarsInSouth::new()))),
        "Thrusting" => Ok(Box::new(CandleIn(wc::Thrusting::new()))),
        "Tii" => Ok(Box::new(ScalarPrice(map_new(
            kind,
            wc::Tii::new(usize_param(params, 0, kind)?, usize_param(params, 1, kind)?),
        )?))),
        "TimeBasedStop" => Ok(Box::new(CandleIn(map_new(
            kind,
            wc::TimeBasedStop::new(usize_param(params, 0, kind)?),
        )?))),
        "TowerTopBottom" => Ok(Box::new(CandleIn(wc::TowerTopBottom::new()))),
        "TpoProfile" => Ok(Box::new(CandleInFields {
            inner: map_new(
                kind,
                wc::TpoProfile::new(usize_param(params, 0, kind)?, usize_param(params, 1, kind)?),
            )?,
            last: None,
        })),
        "TradeVolumeIndex" => Ok(Box::new(CandleIn(map_new(
            kind,
            wc::TradeVolumeIndex::new(float_param(params, 0, kind)?),
        )?))),
        "TrendLabel" => Ok(Box::new(ScalarPrice(map_new(
            kind,
            wc::TrendLabel::new(usize_param(params, 0, kind)?),
        )?))),
        "TrendStrengthIndex" => Ok(Box::new(ScalarPrice(map_new(
            kind,
            wc::TrendStrengthIndex::new(usize_param(params, 0, kind)?),
        )?))),
        "Trendflex" => Ok(Box::new(ScalarPrice(map_new(
            kind,
            wc::Trendflex::new(usize_param(params, 0, kind)?),
        )?))),
        "Triangle" => Ok(Box::new(CandleIn(wc::Triangle::new()))),
        "Trima" => Ok(Box::new(ScalarPrice(map_new(
            kind,
            wc::Trima::new(usize_param(params, 0, kind)?),
        )?))),
        "TripleTopBottom" => Ok(Box::new(CandleIn(wc::TripleTopBottom::new()))),
        "Tristar" => Ok(Box::new(CandleIn(wc::Tristar::new()))),
        "Trix" => Ok(Box::new(ScalarPrice(map_new(
            kind,
            wc::Trix::new(usize_param(params, 0, kind)?),
        )?))),
        "TrueRange" => Ok(Box::new(CandleIn(wc::TrueRange::new()))),
        "Tsf" => Ok(Box::new(ScalarPrice(map_new(
            kind,
            wc::Tsf::new(usize_param(params, 0, kind)?),
        )?))),
        "TsfOscillator" => Ok(Box::new(ScalarPrice(map_new(
            kind,
            wc::TsfOscillator::new(usize_param(params, 0, kind)?),
        )?))),
        "Tsi" => Ok(Box::new(ScalarPrice(map_new(
            kind,
            wc::Tsi::new(usize_param(params, 0, kind)?, usize_param(params, 1, kind)?),
        )?))),
        "Tsv" => Ok(Box::new(CandleIn(map_new(
            kind,
            wc::Tsv::new(usize_param(params, 0, kind)?),
        )?))),
        "TtmSqueeze" => Ok(Box::new(CandleInFields {
            inner: map_new(
                kind,
                wc::TtmSqueeze::new(
                    usize_param(params, 0, kind)?,
                    float_param(params, 1, kind)?,
                    float_param(params, 2, kind)?,
                ),
            )?,
            last: None,
        })),
        "TtmTrend" => Ok(Box::new(CandleIn(map_new(
            kind,
            wc::TtmTrend::new(usize_param(params, 0, kind)?),
        )?))),
        "TurnOfMonth" => Ok(Box::new(CandleIn(map_new(
            kind,
            wc::TurnOfMonth::new(
                u32_param(params, 0, kind)?,
                u32_param(params, 1, kind)?,
                i32_param(params, 2, kind)?,
            ),
        )?))),
        "Tweezer" => Ok(Box::new(CandleIn(wc::Tweezer::new()))),
        "TwiggsMoneyFlow" => Ok(Box::new(CandleIn(map_new(
            kind,
            wc::TwiggsMoneyFlow::new(usize_param(params, 0, kind)?),
        )?))),
        "TwoCrows" => Ok(Box::new(CandleIn(wc::TwoCrows::new()))),
        "TypicalPrice" => Ok(Box::new(CandleIn(wc::TypicalPrice::new()))),
        "UlcerIndex" => Ok(Box::new(ScalarPrice(map_new(
            kind,
            wc::UlcerIndex::new(usize_param(params, 0, kind)?),
        )?))),
        "UltimateOscillator" => Ok(Box::new(CandleIn(map_new(
            kind,
            wc::UltimateOscillator::new(
                usize_param(params, 0, kind)?,
                usize_param(params, 1, kind)?,
                usize_param(params, 2, kind)?,
            ),
        )?))),
        "UniqueThreeRiver" => Ok(Box::new(CandleIn(wc::UniqueThreeRiver::new()))),
        "UniversalOscillator" => Ok(Box::new(ScalarPrice(map_new(
            kind,
            wc::UniversalOscillator::new(usize_param(params, 0, kind)?),
        )?))),
        "UpsideGapThreeMethods" => Ok(Box::new(CandleIn(wc::UpsideGapThreeMethods::new()))),
        "UpsideGapTwoCrows" => Ok(Box::new(CandleIn(wc::UpsideGapTwoCrows::new()))),
        "UpsidePotentialRatio" => Ok(Box::new(ScalarPrice(map_new(
            kind,
            wc::UpsidePotentialRatio::new(
                usize_param(params, 0, kind)?,
                float_param(params, 1, kind)?,
            ),
        )?))),
        "ValueArea" => Ok(Box::new(CandleInFields {
            inner: map_new(
                kind,
                wc::ValueArea::new(
                    usize_param(params, 0, kind)?,
                    usize_param(params, 1, kind)?,
                    float_param(params, 2, kind)?,
                ),
            )?,
            last: None,
        })),
        "ValueAtRisk" => Ok(Box::new(ScalarPrice(map_new(
            kind,
            wc::ValueAtRisk::new(usize_param(params, 0, kind)?, float_param(params, 1, kind)?),
        )?))),
        "Variance" => Ok(Box::new(ScalarPrice(map_new(
            kind,
            wc::Variance::new(usize_param(params, 0, kind)?),
        )?))),
        "VerticalHorizontalFilter" => Ok(Box::new(ScalarPrice(map_new(
            kind,
            wc::VerticalHorizontalFilter::new(usize_param(params, 0, kind)?),
        )?))),
        "Vidya" => Ok(Box::new(ScalarPrice(map_new(
            kind,
            wc::Vidya::new(usize_param(params, 0, kind)?, usize_param(params, 1, kind)?),
        )?))),
        "VolatilityCone" => Ok(Box::new(CandleInFields {
            inner: map_new(
                kind,
                wc::VolatilityCone::new(
                    usize_param(params, 0, kind)?,
                    usize_param(params, 1, kind)?,
                ),
            )?,
            last: None,
        })),
        "VolatilityOfVolatility" => Ok(Box::new(ScalarPrice(map_new(
            kind,
            wc::VolatilityOfVolatility::new(
                usize_param(params, 0, kind)?,
                usize_param(params, 1, kind)?,
            ),
        )?))),
        "VolatilityRatio" => Ok(Box::new(CandleIn(map_new(
            kind,
            wc::VolatilityRatio::new(usize_param(params, 0, kind)?),
        )?))),
        "VoltyStop" => Ok(Box::new(CandleIn(map_new(
            kind,
            wc::VoltyStop::new(usize_param(params, 0, kind)?, float_param(params, 1, kind)?),
        )?))),
        "VolumeOscillator" => Ok(Box::new(CandleIn(map_new(
            kind,
            wc::VolumeOscillator::new(usize_param(params, 0, kind)?, usize_param(params, 1, kind)?),
        )?))),
        "VolumePriceTrend" => Ok(Box::new(CandleIn(wc::VolumePriceTrend::new()))),
        "VolumeProfile" => Ok(Box::new(CandleInFields {
            inner: map_new(
                kind,
                wc::VolumeProfile::new(
                    usize_param(params, 0, kind)?,
                    usize_param(params, 1, kind)?,
                ),
            )?,
            last: None,
        })),
        "VolumeRsi" => Ok(Box::new(CandleIn(map_new(
            kind,
            wc::VolumeRsi::new(usize_param(params, 0, kind)?),
        )?))),
        "VolumeWeightedMacd" => Ok(Box::new(CandleInFields {
            inner: map_new(
                kind,
                wc::VolumeWeightedMacd::new(
                    usize_param(params, 0, kind)?,
                    usize_param(params, 1, kind)?,
                    usize_param(params, 2, kind)?,
                ),
            )?,
            last: None,
        })),
        "VolumeWeightedSr" => Ok(Box::new(CandleInFields {
            inner: map_new(
                kind,
                wc::VolumeWeightedSr::new(usize_param(params, 0, kind)?),
            )?,
            last: None,
        })),
        "Vortex" => Ok(Box::new(CandleInFields {
            inner: map_new(kind, wc::Vortex::new(usize_param(params, 0, kind)?))?,
            last: None,
        })),
        "Vwap" => Ok(Box::new(CandleIn(wc::Vwap::new()))),
        "VwapStdDevBands" => Ok(Box::new(CandleInFields {
            inner: map_new(
                kind,
                wc::VwapStdDevBands::new(float_param(params, 0, kind)?),
            )?,
            last: None,
        })),
        "Vwma" => Ok(Box::new(CandleIn(map_new(
            kind,
            wc::Vwma::new(usize_param(params, 0, kind)?),
        )?))),
        "Vzo" => Ok(Box::new(CandleIn(map_new(
            kind,
            wc::Vzo::new(usize_param(params, 0, kind)?),
        )?))),
        "Wad" => Ok(Box::new(CandleIn(wc::Wad::new()))),
        "WavePm" => Ok(Box::new(ScalarPrice(map_new(
            kind,
            wc::WavePm::new(usize_param(params, 0, kind)?, usize_param(params, 1, kind)?),
        )?))),
        "WaveTrend" => Ok(Box::new(CandleInFields {
            inner: map_new(
                kind,
                wc::WaveTrend::new(
                    usize_param(params, 0, kind)?,
                    usize_param(params, 1, kind)?,
                    usize_param(params, 2, kind)?,
                ),
            )?,
            last: None,
        })),
        "Wedge" => Ok(Box::new(CandleIn(wc::Wedge::new()))),
        "WeightedClose" => Ok(Box::new(CandleIn(wc::WeightedClose::new()))),
        "WickRatio" => Ok(Box::new(CandleIn(wc::WickRatio::new()))),
        "WilliamsR" => Ok(Box::new(CandleIn(map_new(
            kind,
            wc::WilliamsR::new(usize_param(params, 0, kind)?),
        )?))),
        "WinRate" => Ok(Box::new(ScalarPrice(map_new(
            kind,
            wc::WinRate::new(usize_param(params, 0, kind)?),
        )?))),
        "Wma" => Ok(Box::new(ScalarPrice(map_new(
            kind,
            wc::Wma::new(usize_param(params, 0, kind)?),
        )?))),
        "WoodiePivots" => Ok(Box::new(CandleInFields {
            inner: wc::WoodiePivots::new(),
            last: None,
        })),
        "YangZhangVolatility" => Ok(Box::new(CandleIn(map_new(
            kind,
            wc::YangZhangVolatility::new(
                usize_param(params, 0, kind)?,
                usize_param(params, 1, kind)?,
            ),
        )?))),
        "YoyoExit" => Ok(Box::new(CandleIn(map_new(
            kind,
            wc::YoyoExit::new(usize_param(params, 0, kind)?, float_param(params, 1, kind)?),
        )?))),
        "ZScore" => Ok(Box::new(ScalarPrice(map_new(
            kind,
            wc::ZScore::new(usize_param(params, 0, kind)?),
        )?))),
        "ZeroLagMacd" => Ok(Box::new(ScalarPriceFields {
            inner: map_new(
                kind,
                wc::ZeroLagMacd::new(
                    usize_param(params, 0, kind)?,
                    usize_param(params, 1, kind)?,
                    usize_param(params, 2, kind)?,
                ),
            )?,
            last: None,
        })),
        "ZigZag" => Ok(Box::new(CandleInFields {
            inner: map_new(kind, wc::ZigZag::new(float_param(params, 0, kind)?))?,
            last: None,
        })),
        "Zlema" => Ok(Box::new(ScalarPrice(map_new(
            kind,
            wc::Zlema::new(usize_param(params, 0, kind)?),
        )?))),
        "Bollinger" => build("BollingerBands", params),
        "Macd" => build("MacdIndicator", params),
        _ => Err(Error::Config(format!("unknown indicator: {kind}"))),
    }
}
