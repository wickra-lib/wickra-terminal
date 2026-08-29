//! Indicator registry: constructs `wickra-core` indicators by name and wraps
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

/// Wraps a price (`Input = f64`) single-output indicator.
struct ScalarPrice<I> {
    inner: I,
}

impl<I> TickIndicator for ScalarPrice<I>
where
    I: Indicator<Input = f64, Output = f64> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        self.inner
            .update(input.price)
            .filter(|value| value.is_finite())
    }
    fn fields(&self) -> Vec<(&'static str, f64)> {
        Vec::new()
    }
    fn warmup(&self) -> usize {
        self.inner.warmup_period()
    }
}

/// Wraps a bar (`Input = Candle`) single-output indicator. Ticks that did
/// not close a bar yield `None` without advancing it.
struct CandleIn<I> {
    inner: I,
}

impl<I> TickIndicator for CandleIn<I>
where
    I: Indicator<Input = Candle, Output = f64> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        input
            .candle
            .and_then(|c| self.inner.update(c))
            .filter(|value| value.is_finite())
    }
    fn fields(&self) -> Vec<(&'static str, f64)> {
        Vec::new()
    }
    fn warmup(&self) -> usize {
        self.inner.warmup_period()
    }
}

/// Wraps a tape (`Input = Trade`) single-output indicator, fed the print
/// with its size and aggressor side rather than the price alone.
struct TradeIn<I> {
    inner: I,
}

impl<I> TickIndicator for TradeIn<I>
where
    I: Indicator<Input = wc::Trade, Output = f64> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        input
            .trade
            .and_then(|t| self.inner.update(t))
            .filter(|value| value.is_finite())
    }
    fn fields(&self) -> Vec<(&'static str, f64)> {
        Vec::new()
    }
    fn warmup(&self) -> usize {
        self.inner.warmup_period()
    }
}

/// Wraps a book (`Input = OrderBook`) single-output indicator. Ticks whose
/// book is one-sided yield `None` without advancing it.
struct BookIn<I> {
    inner: I,
}

impl<I> TickIndicator for BookIn<I>
where
    I: Indicator<Input = wc::OrderBook, Output = f64> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        input
            .book
            .clone()
            .and_then(|b| self.inner.update(b))
            .filter(|value| value.is_finite())
    }
    fn fields(&self) -> Vec<(&'static str, f64)> {
        Vec::new()
    }
    fn warmup(&self) -> usize {
        self.inner.warmup_period()
    }
    fn wants_book(&self) -> bool {
        true
    }
}

/// Wraps a pairwise (`Input = (f64, f64)`) single-output indicator: this
/// market's price against a reference market's. Ticks on which the reference
/// has not printed yet yield `None` without advancing it.
struct PairIn<I> {
    inner: I,
    reference: String,
}

impl<I> TickIndicator for PairIn<I>
where
    I: Indicator<Input = (f64, f64), Output = f64> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        input
            .reference(&self.reference)
            .and_then(|other| self.inner.update((input.price, other)))
            .filter(|value| value.is_finite())
    }
    fn fields(&self) -> Vec<(&'static str, f64)> {
        Vec::new()
    }
    fn warmup(&self) -> usize {
        self.inner.warmup_period()
    }
    fn wants_reference(&self) -> bool {
        true
    }
}

/// Wraps a breadth (`Input = CrossSection`) single-output indicator: the
/// whole tracked universe on one tick, not one market. Ticks before any
/// market has closed a bar yield `None` without advancing it.
struct CrossIn<I> {
    inner: I,
}

impl<I> TickIndicator for CrossIn<I>
where
    I: Indicator<Input = wc::CrossSection, Output = f64> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        input
            .cross_section
            .clone()
            .and_then(|universe| self.inner.update(universe))
            .filter(|value| value.is_finite())
    }
    fn fields(&self) -> Vec<(&'static str, f64)> {
        Vec::new()
    }
    fn warmup(&self) -> usize {
        self.inner.warmup_period()
    }
}

/// Wraps a derivatives (`Input = DerivativesTick`) single-output indicator:
/// funding, open interest, positioning and the mark/index/futures prices of
/// one perpetual market. Ticks before the host has fed those prices yield
/// `None` without advancing it.
struct DerivIn<I> {
    inner: I,
}

impl<I> TickIndicator for DerivIn<I>
where
    I: Indicator<Input = wc::DerivativesTick, Output = f64> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        input
            .derivatives
            .and_then(|derivatives| self.inner.update(derivatives))
            .filter(|value| value.is_finite())
    }
    fn fields(&self) -> Vec<(&'static str, f64)> {
        Vec::new()
    }
    fn warmup(&self) -> usize {
        self.inner.warmup_period()
    }
}

/// Wraps a microstructure (`Input = TradeQuote`) single-output indicator: one
/// print paired with the mid that was standing when it arrived. Ticks with a
/// one-sided book yield `None` without advancing it -- there is no mid to
/// measure the print against.
struct QuoteIn<I> {
    inner: I,
}

impl<I> TickIndicator for QuoteIn<I>
where
    I: Indicator<Input = wc::TradeQuote, Output = f64> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        input
            .trade_quote
            .and_then(|quote| self.inner.update(quote))
            .filter(|value| value.is_finite())
    }
    fn fields(&self) -> Vec<(&'static str, f64)> {
        Vec::new()
    }
    fn warmup(&self) -> usize {
        self.inner.warmup_period()
    }
}

/// Wraps an indicator whose `Input = f64` is a per-period RETURN rather
/// than a price, feeding it the close-to-close return of each closed bar.
/// The first bar establishes the close to difference against and yields
/// `None`; a previous close that is not a normal number is not divided by.
struct ReturnsIn<I> {
    inner: I,
    previous_close: Option<f64>,
}

impl<I> TickIndicator for ReturnsIn<I>
where
    I: Indicator<Input = f64, Output = f64> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        input
            .candle
            .and_then(|candle| {
                let close = candle.close;
                self.previous_close
                    .replace(close)
                    .filter(|previous| previous.is_normal())
                    .and_then(|previous| self.inner.update(close / previous - 1.0))
            })
            .filter(|value| value.is_finite())
    }
    fn fields(&self) -> Vec<(&'static str, f64)> {
        Vec::new()
    }
    fn warmup(&self) -> usize {
        self.inner.warmup_period() + 1
    }
}

/// Wraps a price (`Input = f64`) single-output indicator.
///
/// This one carries an indicator whose output is a whole number -- a count of
/// bars, not a price -- converted to the `f64` the boundary speaks.
struct ScalarPriceInt<I> {
    inner: I,
}

impl<I, O> TickIndicator for ScalarPriceInt<I>
where
    I: Indicator<Input = f64, Output = O> + Send,
    O: Into<f64> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        self.inner.update(input.price).map(Into::into)
    }
    fn fields(&self) -> Vec<(&'static str, f64)> {
        Vec::new()
    }
    fn warmup(&self) -> usize {
        self.inner.warmup_period()
    }
}

/// Wraps a price indicator whose output is a struct of `f64` fields. The
/// primary value is the first field; every field is reachable by name.
struct ScalarPriceFields<I, O> {
    inner: I,
    last: Option<O>,
}

/// Wraps a bar indicator whose output is a struct of `f64` fields.
struct CandleInFields<I, O> {
    inner: I,
    last: Option<O>,
}

/// Wraps a pairwise indicator whose output is a struct of `f64` fields.
struct PairInFields<I, O> {
    inner: I,
    last: Option<O>,
    reference: String,
}

/// Wraps a derivatives indicator whose output is a struct of fields. The
/// primary value is the first field; every field is reachable by name.
struct DerivInFields<I, O> {
    inner: I,
    last: Option<O>,
}

impl<I> TickIndicator for PairInFields<I, wc::CointegrationOutput>
where
    I: Indicator<Input = (f64, f64), Output = wc::CointegrationOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = input
            .reference(&self.reference)
            .and_then(|other| self.inner.update((input.price, other)))
            .filter(|last| {
                last.hedge_ratio.is_finite() && last.spread.is_finite() && last.adf_stat.is_finite()
            });
        self.last = out;
        self.last.as_ref().map(|last| last.hedge_ratio)
    }
    fn fields(&self) -> Vec<(&'static str, f64)> {
        self.last
            .as_ref()
            .map(|last| {
                vec![
                    ("hedge_ratio", last.hedge_ratio),
                    ("spread", last.spread),
                    ("adf_stat", last.adf_stat),
                ]
            })
            .unwrap_or_default()
    }
    fn warmup(&self) -> usize {
        self.inner.warmup_period()
    }
    fn wants_reference(&self) -> bool {
        true
    }
}

impl<I> TickIndicator for PairInFields<I, wc::KalmanHedgeRatioOutput>
where
    I: Indicator<Input = (f64, f64), Output = wc::KalmanHedgeRatioOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = input
            .reference(&self.reference)
            .and_then(|other| self.inner.update((input.price, other)))
            .filter(|last| {
                last.hedge_ratio.is_finite()
                    && last.intercept.is_finite()
                    && last.spread.is_finite()
            });
        self.last = out;
        self.last.as_ref().map(|last| last.hedge_ratio)
    }
    fn fields(&self) -> Vec<(&'static str, f64)> {
        self.last
            .as_ref()
            .map(|last| {
                vec![
                    ("hedge_ratio", last.hedge_ratio),
                    ("intercept", last.intercept),
                    ("spread", last.spread),
                ]
            })
            .unwrap_or_default()
    }
    fn warmup(&self) -> usize {
        self.inner.warmup_period()
    }
    fn wants_reference(&self) -> bool {
        true
    }
}

impl<I> TickIndicator for PairInFields<I, wc::LeadLagCrossCorrelationOutput>
where
    I: Indicator<Input = (f64, f64), Output = wc::LeadLagCrossCorrelationOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = input
            .reference(&self.reference)
            .and_then(|other| self.inner.update((input.price, other)))
            .filter(|last| last.correlation.is_finite());
        self.last = out;
        self.last.as_ref().map(|last| last.lag as f64)
    }
    fn fields(&self) -> Vec<(&'static str, f64)> {
        self.last
            .as_ref()
            .map(|last| vec![("lag", last.lag as f64), ("correlation", last.correlation)])
            .unwrap_or_default()
    }
    fn warmup(&self) -> usize {
        self.inner.warmup_period()
    }
    fn wants_reference(&self) -> bool {
        true
    }
}

impl<I> TickIndicator for PairInFields<I, wc::RelativeStrengthOutput>
where
    I: Indicator<Input = (f64, f64), Output = wc::RelativeStrengthOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = input
            .reference(&self.reference)
            .and_then(|other| self.inner.update((input.price, other)))
            .filter(|last| {
                last.ratio.is_finite() && last.ratio_ma.is_finite() && last.ratio_rsi.is_finite()
            });
        self.last = out;
        self.last.as_ref().map(|last| last.ratio)
    }
    fn fields(&self) -> Vec<(&'static str, f64)> {
        self.last
            .as_ref()
            .map(|last| {
                vec![
                    ("ratio", last.ratio),
                    ("ratio_ma", last.ratio_ma),
                    ("ratio_rsi", last.ratio_rsi),
                ]
            })
            .unwrap_or_default()
    }
    fn warmup(&self) -> usize {
        self.inner.warmup_period()
    }
    fn wants_reference(&self) -> bool {
        true
    }
}

impl<I> TickIndicator for PairInFields<I, wc::SpreadBollingerBandsOutput>
where
    I: Indicator<Input = (f64, f64), Output = wc::SpreadBollingerBandsOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = input
            .reference(&self.reference)
            .and_then(|other| self.inner.update((input.price, other)))
            .filter(|last| {
                last.middle.is_finite()
                    && last.upper.is_finite()
                    && last.lower.is_finite()
                    && last.percent_b.is_finite()
            });
        self.last = out;
        self.last.as_ref().map(|last| last.middle)
    }
    fn fields(&self) -> Vec<(&'static str, f64)> {
        self.last
            .as_ref()
            .map(|last| {
                vec![
                    ("middle", last.middle),
                    ("upper", last.upper),
                    ("lower", last.lower),
                    ("percent_b", last.percent_b),
                ]
            })
            .unwrap_or_default()
    }
    fn warmup(&self) -> usize {
        self.inner.warmup_period()
    }
    fn wants_reference(&self) -> bool {
        true
    }
}

impl<I> TickIndicator for CandleInFields<I, wc::AccelerationBandsOutput>
where
    I: Indicator<Input = Candle, Output = wc::AccelerationBandsOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = input
            .candle
            .and_then(|c| self.inner.update(c))
            .filter(|last| {
                last.upper.is_finite() && last.middle.is_finite() && last.lower.is_finite()
            });
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

impl<I> TickIndicator for CandleInFields<I, wc::AdxOutput>
where
    I: Indicator<Input = Candle, Output = wc::AdxOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = input
            .candle
            .and_then(|c| self.inner.update(c))
            .filter(|last| {
                last.plus_di.is_finite() && last.minus_di.is_finite() && last.adx.is_finite()
            });
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

impl<I> TickIndicator for CandleInFields<I, wc::AlligatorOutput>
where
    I: Indicator<Input = Candle, Output = wc::AlligatorOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = input
            .candle
            .and_then(|c| self.inner.update(c))
            .filter(|last| last.jaw.is_finite() && last.teeth.is_finite() && last.lips.is_finite());
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

impl<I> TickIndicator for CandleInFields<I, wc::AndrewsPitchforkOutput>
where
    I: Indicator<Input = Candle, Output = wc::AndrewsPitchforkOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = input
            .candle
            .and_then(|c| self.inner.update(c))
            .filter(|last| {
                last.median.is_finite() && last.upper.is_finite() && last.lower.is_finite()
            });
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

impl<I> TickIndicator for CandleInFields<I, wc::AroonOutput>
where
    I: Indicator<Input = Candle, Output = wc::AroonOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = input
            .candle
            .and_then(|c| self.inner.update(c))
            .filter(|last| last.up.is_finite() && last.down.is_finite());
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

impl<I> TickIndicator for CandleInFields<I, wc::AtrBandsOutput>
where
    I: Indicator<Input = Candle, Output = wc::AtrBandsOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = input
            .candle
            .and_then(|c| self.inner.update(c))
            .filter(|last| {
                last.upper.is_finite() && last.middle.is_finite() && last.lower.is_finite()
            });
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

impl<I> TickIndicator for CandleInFields<I, wc::AtrRatchetOutput>
where
    I: Indicator<Input = Candle, Output = wc::AtrRatchetOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = input
            .candle
            .and_then(|c| self.inner.update(c))
            .filter(|last| last.value.is_finite() && last.direction.is_finite());
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

impl<I> TickIndicator for CandleInFields<I, wc::AutoFibOutput>
where
    I: Indicator<Input = Candle, Output = wc::AutoFibOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = input
            .candle
            .and_then(|c| self.inner.update(c))
            .filter(|last| {
                last.level_0.is_finite()
                    && last.level_236.is_finite()
                    && last.level_382.is_finite()
                    && last.level_500.is_finite()
                    && last.level_618.is_finite()
                    && last.level_786.is_finite()
                    && last.level_1000.is_finite()
            });
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

impl<I> TickIndicator for CandleInFields<I, wc::CamarillaPivotsOutput>
where
    I: Indicator<Input = Candle, Output = wc::CamarillaPivotsOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = input
            .candle
            .and_then(|c| self.inner.update(c))
            .filter(|last| {
                last.pp.is_finite()
                    && last.r1.is_finite()
                    && last.r2.is_finite()
                    && last.r3.is_finite()
                    && last.r4.is_finite()
                    && last.s1.is_finite()
                    && last.s2.is_finite()
                    && last.s3.is_finite()
                    && last.s4.is_finite()
            });
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

impl<I> TickIndicator for CandleInFields<I, wc::CandleVolumeOutput>
where
    I: Indicator<Input = Candle, Output = wc::CandleVolumeOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = input
            .candle
            .and_then(|c| self.inner.update(c))
            .filter(|last| last.body.is_finite() && last.width.is_finite());
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

impl<I> TickIndicator for CandleInFields<I, wc::CentralPivotRangeOutput>
where
    I: Indicator<Input = Candle, Output = wc::CentralPivotRangeOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = input
            .candle
            .and_then(|c| self.inner.update(c))
            .filter(|last| last.pivot.is_finite() && last.tc.is_finite() && last.bc.is_finite());
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

impl<I> TickIndicator for CandleInFields<I, wc::ChandeKrollStopOutput>
where
    I: Indicator<Input = Candle, Output = wc::ChandeKrollStopOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = input
            .candle
            .and_then(|c| self.inner.update(c))
            .filter(|last| last.stop_long.is_finite() && last.stop_short.is_finite());
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

impl<I> TickIndicator for CandleInFields<I, wc::ChandelierExitOutput>
where
    I: Indicator<Input = Candle, Output = wc::ChandelierExitOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = input
            .candle
            .and_then(|c| self.inner.update(c))
            .filter(|last| last.long_stop.is_finite() && last.short_stop.is_finite());
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

impl<I> TickIndicator for CandleInFields<I, wc::ClassicPivotsOutput>
where
    I: Indicator<Input = Candle, Output = wc::ClassicPivotsOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = input
            .candle
            .and_then(|c| self.inner.update(c))
            .filter(|last| {
                last.pp.is_finite()
                    && last.r1.is_finite()
                    && last.r2.is_finite()
                    && last.r3.is_finite()
                    && last.s1.is_finite()
                    && last.s2.is_finite()
                    && last.s3.is_finite()
            });
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

impl<I> TickIndicator for CandleInFields<I, wc::CompositeProfileOutput>
where
    I: Indicator<Input = Candle, Output = wc::CompositeProfileOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = input
            .candle
            .and_then(|c| self.inner.update(c))
            .filter(|last| last.poc.is_finite() && last.vah.is_finite() && last.val.is_finite());
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

impl<I> TickIndicator for CandleInFields<I, wc::DemarkPivotsOutput>
where
    I: Indicator<Input = Candle, Output = wc::DemarkPivotsOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = input
            .candle
            .and_then(|c| self.inner.update(c))
            .filter(|last| last.pp.is_finite() && last.r1.is_finite() && last.s1.is_finite());
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

impl<I> TickIndicator for CandleInFields<I, wc::DonchianOutput>
where
    I: Indicator<Input = Candle, Output = wc::DonchianOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = input
            .candle
            .and_then(|c| self.inner.update(c))
            .filter(|last| {
                last.upper.is_finite() && last.middle.is_finite() && last.lower.is_finite()
            });
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

impl<I> TickIndicator for CandleInFields<I, wc::DonchianStopOutput>
where
    I: Indicator<Input = Candle, Output = wc::DonchianStopOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = input
            .candle
            .and_then(|c| self.inner.update(c))
            .filter(|last| last.stop_long.is_finite() && last.stop_short.is_finite());
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

impl<I> TickIndicator for CandleInFields<I, wc::ElderRayOutput>
where
    I: Indicator<Input = Candle, Output = wc::ElderRayOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = input
            .candle
            .and_then(|c| self.inner.update(c))
            .filter(|last| last.bull_power.is_finite() && last.bear_power.is_finite());
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

impl<I> TickIndicator for CandleInFields<I, wc::ElderSafeZoneOutput>
where
    I: Indicator<Input = Candle, Output = wc::ElderSafeZoneOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = input
            .candle
            .and_then(|c| self.inner.update(c))
            .filter(|last| last.value.is_finite() && last.direction.is_finite());
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

impl<I> TickIndicator for CandleInFields<I, wc::EquivolumeOutput>
where
    I: Indicator<Input = Candle, Output = wc::EquivolumeOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = input
            .candle
            .and_then(|c| self.inner.update(c))
            .filter(|last| last.height.is_finite() && last.width.is_finite());
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

impl<I> TickIndicator for CandleInFields<I, wc::FibArcsOutput>
where
    I: Indicator<Input = Candle, Output = wc::FibArcsOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = input
            .candle
            .and_then(|c| self.inner.update(c))
            .filter(|last| {
                last.arc_382.is_finite() && last.arc_500.is_finite() && last.arc_618.is_finite()
            });
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

impl<I> TickIndicator for CandleInFields<I, wc::FibChannelOutput>
where
    I: Indicator<Input = Candle, Output = wc::FibChannelOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = input
            .candle
            .and_then(|c| self.inner.update(c))
            .filter(|last| {
                last.base.is_finite()
                    && last.level_618.is_finite()
                    && last.level_1000.is_finite()
                    && last.level_1618.is_finite()
            });
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

impl<I> TickIndicator for CandleInFields<I, wc::FibConfluenceOutput>
where
    I: Indicator<Input = Candle, Output = wc::FibConfluenceOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = input
            .candle
            .and_then(|c| self.inner.update(c))
            .filter(|last| last.price.is_finite() && last.strength.is_finite());
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

impl<I> TickIndicator for CandleInFields<I, wc::FibExtensionOutput>
where
    I: Indicator<Input = Candle, Output = wc::FibExtensionOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = input
            .candle
            .and_then(|c| self.inner.update(c))
            .filter(|last| {
                last.level_1272.is_finite()
                    && last.level_1414.is_finite()
                    && last.level_1618.is_finite()
                    && last.level_2000.is_finite()
                    && last.level_2618.is_finite()
            });
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

impl<I> TickIndicator for CandleInFields<I, wc::FibFanOutput>
where
    I: Indicator<Input = Candle, Output = wc::FibFanOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = input
            .candle
            .and_then(|c| self.inner.update(c))
            .filter(|last| {
                last.fan_382.is_finite() && last.fan_500.is_finite() && last.fan_618.is_finite()
            });
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

impl<I> TickIndicator for CandleInFields<I, wc::FibProjectionOutput>
where
    I: Indicator<Input = Candle, Output = wc::FibProjectionOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = input
            .candle
            .and_then(|c| self.inner.update(c))
            .filter(|last| {
                last.level_618.is_finite()
                    && last.level_1000.is_finite()
                    && last.level_1618.is_finite()
                    && last.level_2618.is_finite()
            });
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

impl<I> TickIndicator for CandleInFields<I, wc::FibRetracementOutput>
where
    I: Indicator<Input = Candle, Output = wc::FibRetracementOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = input
            .candle
            .and_then(|c| self.inner.update(c))
            .filter(|last| {
                last.level_0.is_finite()
                    && last.level_236.is_finite()
                    && last.level_382.is_finite()
                    && last.level_500.is_finite()
                    && last.level_618.is_finite()
                    && last.level_786.is_finite()
                    && last.level_1000.is_finite()
            });
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

impl<I> TickIndicator for CandleInFields<I, wc::FibTimeZonesOutput>
where
    I: Indicator<Input = Candle, Output = wc::FibTimeZonesOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = input
            .candle
            .and_then(|c| self.inner.update(c))
            .filter(|last| last.on_zone.is_finite() && last.bars_to_next.is_finite());
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

impl<I> TickIndicator for CandleInFields<I, wc::FibonacciPivotsOutput>
where
    I: Indicator<Input = Candle, Output = wc::FibonacciPivotsOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = input
            .candle
            .and_then(|c| self.inner.update(c))
            .filter(|last| {
                last.pp.is_finite()
                    && last.r1.is_finite()
                    && last.r2.is_finite()
                    && last.r3.is_finite()
                    && last.s1.is_finite()
                    && last.s2.is_finite()
                    && last.s3.is_finite()
            });
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

impl<I> TickIndicator for CandleInFields<I, wc::FractalChaosBandsOutput>
where
    I: Indicator<Input = Candle, Output = wc::FractalChaosBandsOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = input
            .candle
            .and_then(|c| self.inner.update(c))
            .filter(|last| last.upper.is_finite() && last.lower.is_finite());
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
        let out = input
            .candle
            .and_then(|c| self.inner.update(c))
            .filter(|last| last.upper.is_finite() && last.lower.is_finite());
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

impl<I> TickIndicator for CandleInFields<I, wc::GoldenPocketOutput>
where
    I: Indicator<Input = Candle, Output = wc::GoldenPocketOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = input
            .candle
            .and_then(|c| self.inner.update(c))
            .filter(|last| last.low.is_finite() && last.mid.is_finite() && last.high.is_finite());
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

impl<I> TickIndicator for CandleInFields<I, wc::HeikinAshiOutput>
where
    I: Indicator<Input = Candle, Output = wc::HeikinAshiOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = input
            .candle
            .and_then(|c| self.inner.update(c))
            .filter(|last| {
                last.open.is_finite()
                    && last.high.is_finite()
                    && last.low.is_finite()
                    && last.close.is_finite()
            });
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

impl<I> TickIndicator for CandleInFields<I, wc::HighLowVolumeNodesOutput>
where
    I: Indicator<Input = Candle, Output = wc::HighLowVolumeNodesOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = input
            .candle
            .and_then(|c| self.inner.update(c))
            .filter(|last| last.hvn.is_finite() && last.lvn.is_finite());
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

impl<I> TickIndicator for CandleInFields<I, wc::HurstChannelOutput>
where
    I: Indicator<Input = Candle, Output = wc::HurstChannelOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = input
            .candle
            .and_then(|c| self.inner.update(c))
            .filter(|last| {
                last.upper.is_finite() && last.middle.is_finite() && last.lower.is_finite()
            });
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

impl<I> TickIndicator for CandleInFields<I, wc::IchimokuOutput>
where
    I: Indicator<Input = Candle, Output = wc::IchimokuOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = input
            .candle
            .and_then(|c| self.inner.update(c))
            .filter(|last| {
                last.tenkan.is_none_or(f64::is_finite)
                    && last.kijun.is_none_or(f64::is_finite)
                    && last.senkou_a.is_none_or(f64::is_finite)
                    && last.senkou_b.is_none_or(f64::is_finite)
                    && last.chikou.is_none_or(f64::is_finite)
            });
        self.last = out;
        self.last.as_ref().and_then(|last| last.tenkan)
    }
    fn fields(&self) -> Vec<(&'static str, f64)> {
        let Some(last) = self.last.as_ref() else {
            return Vec::new();
        };
        let mut out = Vec::new();
        if let Some(value) = last.tenkan {
            out.push(("tenkan", value));
        }
        if let Some(value) = last.kijun {
            out.push(("kijun", value));
        }
        if let Some(value) = last.senkou_a {
            out.push(("senkou_a", value));
        }
        if let Some(value) = last.senkou_b {
            out.push(("senkou_b", value));
        }
        if let Some(value) = last.chikou {
            out.push(("chikou", value));
        }
        out
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
        let out = input
            .candle
            .and_then(|c| self.inner.update(c))
            .filter(|last| last.high.is_finite() && last.low.is_finite());
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

impl<I> TickIndicator for CandleInFields<I, wc::KaseDevStopOutput>
where
    I: Indicator<Input = Candle, Output = wc::KaseDevStopOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = input
            .candle
            .and_then(|c| self.inner.update(c))
            .filter(|last| last.value.is_finite() && last.direction.is_finite());
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

impl<I> TickIndicator for CandleInFields<I, wc::KasePermissionStochasticOutput>
where
    I: Indicator<Input = Candle, Output = wc::KasePermissionStochasticOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = input
            .candle
            .and_then(|c| self.inner.update(c))
            .filter(|last| last.fast.is_finite() && last.slow.is_finite());
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

impl<I> TickIndicator for CandleInFields<I, wc::KeltnerOutput>
where
    I: Indicator<Input = Candle, Output = wc::KeltnerOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = input
            .candle
            .and_then(|c| self.inner.update(c))
            .filter(|last| {
                last.upper.is_finite() && last.middle.is_finite() && last.lower.is_finite()
            });
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

impl<I> TickIndicator for CandleInFields<I, wc::ModifiedMaStopOutput>
where
    I: Indicator<Input = Candle, Output = wc::ModifiedMaStopOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = input
            .candle
            .and_then(|c| self.inner.update(c))
            .filter(|last| last.value.is_finite() && last.direction.is_finite());
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

impl<I> TickIndicator for CandleInFields<I, wc::MurreyMathLinesOutput>
where
    I: Indicator<Input = Candle, Output = wc::MurreyMathLinesOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = input
            .candle
            .and_then(|c| self.inner.update(c))
            .filter(|last| {
                last.mm8_8.is_finite()
                    && last.mm7_8.is_finite()
                    && last.mm6_8.is_finite()
                    && last.mm5_8.is_finite()
                    && last.mm4_8.is_finite()
                    && last.mm3_8.is_finite()
                    && last.mm2_8.is_finite()
                    && last.mm1_8.is_finite()
                    && last.mm0_8.is_finite()
            });
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

impl<I> TickIndicator for CandleInFields<I, wc::NrtrOutput>
where
    I: Indicator<Input = Candle, Output = wc::NrtrOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = input
            .candle
            .and_then(|c| self.inner.update(c))
            .filter(|last| last.value.is_finite() && last.direction.is_finite());
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

impl<I> TickIndicator for CandleInFields<I, wc::OpeningRangeOutput>
where
    I: Indicator<Input = Candle, Output = wc::OpeningRangeOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = input
            .candle
            .and_then(|c| self.inner.update(c))
            .filter(|last| {
                last.high.is_finite() && last.low.is_finite() && last.breakout_distance.is_finite()
            });
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

impl<I> TickIndicator for CandleInFields<I, wc::OvernightIntradayReturnOutput>
where
    I: Indicator<Input = Candle, Output = wc::OvernightIntradayReturnOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = input
            .candle
            .and_then(|c| self.inner.update(c))
            .filter(|last| last.overnight.is_finite() && last.intraday.is_finite());
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

impl<I> TickIndicator for CandleInFields<I, wc::ProjectionBandsOutput>
where
    I: Indicator<Input = Candle, Output = wc::ProjectionBandsOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = input
            .candle
            .and_then(|c| self.inner.update(c))
            .filter(|last| {
                last.upper.is_finite() && last.middle.is_finite() && last.lower.is_finite()
            });
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

impl<I> TickIndicator for CandleInFields<I, wc::RwiOutput>
where
    I: Indicator<Input = Candle, Output = wc::RwiOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = input
            .candle
            .and_then(|c| self.inner.update(c))
            .filter(|last| last.high.is_finite() && last.low.is_finite());
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
        let out = input
            .candle
            .and_then(|c| self.inner.update(c))
            .filter(|last| last.high.is_finite() && last.low.is_finite());
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

impl<I> TickIndicator for CandleInFields<I, wc::SessionRangeOutput>
where
    I: Indicator<Input = Candle, Output = wc::SessionRangeOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = input
            .candle
            .and_then(|c| self.inner.update(c))
            .filter(|last| last.asia.is_finite() && last.eu.is_finite() && last.us.is_finite());
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

impl<I> TickIndicator for CandleInFields<I, wc::SmoothedHeikinAshiOutput>
where
    I: Indicator<Input = Candle, Output = wc::SmoothedHeikinAshiOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = input
            .candle
            .and_then(|c| self.inner.update(c))
            .filter(|last| {
                last.open.is_finite()
                    && last.high.is_finite()
                    && last.low.is_finite()
                    && last.close.is_finite()
            });
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

impl<I> TickIndicator for CandleInFields<I, wc::StarcBandsOutput>
where
    I: Indicator<Input = Candle, Output = wc::StarcBandsOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = input
            .candle
            .and_then(|c| self.inner.update(c))
            .filter(|last| {
                last.upper.is_finite() && last.middle.is_finite() && last.lower.is_finite()
            });
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

impl<I> TickIndicator for CandleInFields<I, wc::StochasticOutput>
where
    I: Indicator<Input = Candle, Output = wc::StochasticOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = input
            .candle
            .and_then(|c| self.inner.update(c))
            .filter(|last| last.k.is_finite() && last.d.is_finite());
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

impl<I> TickIndicator for CandleInFields<I, wc::SuperTrendOutput>
where
    I: Indicator<Input = Candle, Output = wc::SuperTrendOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = input
            .candle
            .and_then(|c| self.inner.update(c))
            .filter(|last| last.value.is_finite() && last.direction.is_finite());
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

impl<I> TickIndicator for CandleInFields<I, wc::TdLinesOutput>
where
    I: Indicator<Input = Candle, Output = wc::TdLinesOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = input
            .candle
            .and_then(|c| self.inner.update(c))
            .filter(|last| last.resistance.is_finite() && last.support.is_finite());
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

impl<I> TickIndicator for CandleInFields<I, wc::TdMovingAverageOutput>
where
    I: Indicator<Input = Candle, Output = wc::TdMovingAverageOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = input
            .candle
            .and_then(|c| self.inner.update(c))
            .filter(|last| last.st1.is_finite() && last.st2.is_finite());
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

impl<I> TickIndicator for CandleInFields<I, wc::TdRangeProjectionOutput>
where
    I: Indicator<Input = Candle, Output = wc::TdRangeProjectionOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = input
            .candle
            .and_then(|c| self.inner.update(c))
            .filter(|last| last.high.is_finite() && last.low.is_finite());
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

impl<I> TickIndicator for CandleInFields<I, wc::TdRiskLevelOutput>
where
    I: Indicator<Input = Candle, Output = wc::TdRiskLevelOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = input
            .candle
            .and_then(|c| self.inner.update(c))
            .filter(|last| last.buy_risk.is_finite() && last.sell_risk.is_finite());
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

impl<I> TickIndicator for CandleInFields<I, wc::TdSequentialOutput>
where
    I: Indicator<Input = Candle, Output = wc::TdSequentialOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = input
            .candle
            .and_then(|c| self.inner.update(c))
            .filter(|last| {
                last.setup.is_finite() && last.countdown.is_finite() && last.direction.is_finite()
            });
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

impl<I> TickIndicator for CandleInFields<I, wc::TtmSqueezeOutput>
where
    I: Indicator<Input = Candle, Output = wc::TtmSqueezeOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = input
            .candle
            .and_then(|c| self.inner.update(c))
            .filter(|last| last.squeeze.is_finite() && last.momentum.is_finite());
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

impl<I> TickIndicator for CandleInFields<I, wc::ValueAreaOutput>
where
    I: Indicator<Input = Candle, Output = wc::ValueAreaOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = input
            .candle
            .and_then(|c| self.inner.update(c))
            .filter(|last| last.poc.is_finite() && last.vah.is_finite() && last.val.is_finite());
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

impl<I> TickIndicator for CandleInFields<I, wc::VolatilityConeOutput>
where
    I: Indicator<Input = Candle, Output = wc::VolatilityConeOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = input
            .candle
            .and_then(|c| self.inner.update(c))
            .filter(|last| {
                last.current.is_finite()
                    && last.min.is_finite()
                    && last.median.is_finite()
                    && last.max.is_finite()
                    && last.percentile.is_finite()
            });
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

impl<I> TickIndicator for CandleInFields<I, wc::VolumeWeightedMacdOutput>
where
    I: Indicator<Input = Candle, Output = wc::VolumeWeightedMacdOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = input
            .candle
            .and_then(|c| self.inner.update(c))
            .filter(|last| {
                last.macd.is_finite() && last.signal.is_finite() && last.histogram.is_finite()
            });
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

impl<I> TickIndicator for CandleInFields<I, wc::VolumeWeightedSrOutput>
where
    I: Indicator<Input = Candle, Output = wc::VolumeWeightedSrOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = input
            .candle
            .and_then(|c| self.inner.update(c))
            .filter(|last| last.support.is_finite() && last.resistance.is_finite());
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

impl<I> TickIndicator for CandleInFields<I, wc::VortexOutput>
where
    I: Indicator<Input = Candle, Output = wc::VortexOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = input
            .candle
            .and_then(|c| self.inner.update(c))
            .filter(|last| last.plus.is_finite() && last.minus.is_finite());
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

impl<I> TickIndicator for CandleInFields<I, wc::VwapStdDevBandsOutput>
where
    I: Indicator<Input = Candle, Output = wc::VwapStdDevBandsOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = input
            .candle
            .and_then(|c| self.inner.update(c))
            .filter(|last| {
                last.upper.is_finite()
                    && last.middle.is_finite()
                    && last.lower.is_finite()
                    && last.stddev.is_finite()
            });
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

impl<I> TickIndicator for CandleInFields<I, wc::WaveTrendOutput>
where
    I: Indicator<Input = Candle, Output = wc::WaveTrendOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = input
            .candle
            .and_then(|c| self.inner.update(c))
            .filter(|last| last.wt1.is_finite() && last.wt2.is_finite());
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

impl<I> TickIndicator for CandleInFields<I, wc::WilliamsFractalsOutput>
where
    I: Indicator<Input = Candle, Output = wc::WilliamsFractalsOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = input
            .candle
            .and_then(|c| self.inner.update(c))
            .filter(|last| {
                last.up.is_none_or(f64::is_finite) && last.down.is_none_or(f64::is_finite)
            });
        self.last = out;
        self.last.as_ref().and_then(|last| last.up)
    }
    fn fields(&self) -> Vec<(&'static str, f64)> {
        let Some(last) = self.last.as_ref() else {
            return Vec::new();
        };
        let mut out = Vec::new();
        if let Some(value) = last.up {
            out.push(("up", value));
        }
        if let Some(value) = last.down {
            out.push(("down", value));
        }
        out
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
        let out = input
            .candle
            .and_then(|c| self.inner.update(c))
            .filter(|last| {
                last.pp.is_finite()
                    && last.r1.is_finite()
                    && last.r2.is_finite()
                    && last.s1.is_finite()
                    && last.s2.is_finite()
            });
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

impl<I> TickIndicator for CandleInFields<I, wc::ZigZagOutput>
where
    I: Indicator<Input = Candle, Output = wc::ZigZagOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = input
            .candle
            .and_then(|c| self.inner.update(c))
            .filter(|last| last.swing.is_finite() && last.direction.is_finite());
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

impl<I> TickIndicator for DerivInFields<I, wc::LiquidationFeaturesOutput>
where
    I: Indicator<Input = wc::DerivativesTick, Output = wc::LiquidationFeaturesOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = input
            .derivatives
            .and_then(|derivatives| self.inner.update(derivatives))
            .filter(|last| {
                last.long.is_finite()
                    && last.short.is_finite()
                    && last.net.is_finite()
                    && last.total.is_finite()
                    && last.imbalance.is_finite()
            });
        self.last = out;
        self.last.as_ref().map(|last| last.long)
    }
    fn fields(&self) -> Vec<(&'static str, f64)> {
        self.last
            .as_ref()
            .map(|last| {
                vec![
                    ("long", last.long),
                    ("short", last.short),
                    ("net", last.net),
                    ("total", last.total),
                    ("imbalance", last.imbalance),
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
        let out = self.inner.update(input.price).filter(|last| {
            last.upper.is_finite()
                && last.middle.is_finite()
                && last.lower.is_finite()
                && last.stddev.is_finite()
        });
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
        let out = self.inner.update(input.price).filter(|last| {
            last.upper.is_finite() && last.middle.is_finite() && last.lower.is_finite()
        });
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

impl<I> TickIndicator for ScalarPriceFields<I, wc::DoubleBollingerOutput>
where
    I: Indicator<Input = f64, Output = wc::DoubleBollingerOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = self.inner.update(input.price).filter(|last| {
            last.upper_outer.is_finite()
                && last.upper_inner.is_finite()
                && last.middle.is_finite()
                && last.lower_inner.is_finite()
                && last.lower_outer.is_finite()
        });
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

impl<I> TickIndicator for ScalarPriceFields<I, wc::HtPhasorOutput>
where
    I: Indicator<Input = f64, Output = wc::HtPhasorOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = self
            .inner
            .update(input.price)
            .filter(|last| last.inphase.is_finite() && last.quadrature.is_finite());
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

impl<I> TickIndicator for ScalarPriceFields<I, wc::KstOutput>
where
    I: Indicator<Input = f64, Output = wc::KstOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = self
            .inner
            .update(input.price)
            .filter(|last| last.kst.is_finite() && last.signal.is_finite());
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
        let out = self.inner.update(input.price).filter(|last| {
            last.upper.is_finite() && last.middle.is_finite() && last.lower.is_finite()
        });
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
        let out = self.inner.update(input.price).filter(|last| {
            last.upper.is_finite() && last.middle.is_finite() && last.lower.is_finite()
        });
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
        let out = self.inner.update(input.price).filter(|last| {
            last.macd.is_finite() && last.signal.is_finite() && last.histogram.is_finite()
        });
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
        let out = self
            .inner
            .update(input.price)
            .filter(|last| last.mama.is_finite() && last.fama.is_finite());
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
        let out = self.inner.update(input.price).filter(|last| {
            last.upper.is_finite() && last.middle.is_finite() && last.lower.is_finite()
        });
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
        let out = self
            .inner
            .update(input.price)
            .filter(|last| last.rsi_ma.is_finite() && last.trailing_line.is_finite());
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
        let out = self.inner.update(input.price).filter(|last| {
            last.upper.is_finite() && last.middle.is_finite() && last.lower.is_finite()
        });
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

impl<I> TickIndicator for ScalarPriceFields<I, wc::StandardErrorBandsOutput>
where
    I: Indicator<Input = f64, Output = wc::StandardErrorBandsOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = self.inner.update(input.price).filter(|last| {
            last.upper.is_finite() && last.middle.is_finite() && last.lower.is_finite()
        });
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

impl<I> TickIndicator for ScalarPriceFields<I, wc::ZeroLagMacdOutput>
where
    I: Indicator<Input = f64, Output = wc::ZeroLagMacdOutput> + Send,
{
    fn update(&mut self, input: &TickInput) -> Option<f64> {
        let out = self.inner.update(input.price).filter(|last| {
            last.macd.is_finite() && last.signal.is_finite() && last.histogram.is_finite()
        });
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
pub const KINDS: [&str; 499] = [
    "AbandonedBaby",
    "Abcd",
    "AbsoluteBreadthIndex",
    "AccelerationBands",
    "AcceleratorOscillator",
    "AdOscillator",
    "AdVolumeLine",
    "AdaptiveCci",
    "AdaptiveCycle",
    "AdaptiveLaguerreFilter",
    "AdaptiveRsi",
    "Adl",
    "AdvanceBlock",
    "AdvanceDecline",
    "AdvanceDeclineRatio",
    "Adx",
    "Adxr",
    "Alligator",
    "Alma",
    "Alpha",
    "AmihudIlliquidity",
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
    "Beta",
    "BetaNeutralSpread",
    "BetterVolume",
    "BipowerVariation",
    "BodySizePct",
    "Bollinger",
    "BollingerBands",
    "BollingerBandwidth",
    "BomarBands",
    "BreadthThrust",
    "Breakaway",
    "BullishPercentIndex",
    "BurkeRatio",
    "Butterfly",
    "CalendarSpread",
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
    "Cointegration",
    "CommonSenseRatio",
    "CompositeProfile",
    "ConcealingBabySwallow",
    "ConditionalValueAtRisk",
    "ConnorsRsi",
    "Coppock",
    "CorrelationTrendIndicator",
    "Counterattack",
    "Crab",
    "CumulativeVolumeDelta",
    "CumulativeVolumeIndex",
    "CupAndHandle",
    "CyberneticCycle",
    "Cypher",
    "Decycler",
    "DecyclerOscillator",
    "Dema",
    "DemandIndex",
    "DemarkPivots",
    "DepthSlope",
    "DerivativeOscillator",
    "DetrendedStdDev",
    "DisparityIndex",
    "DistanceSsd",
    "Doji",
    "DojiStar",
    "Donchian",
    "DonchianStop",
    "DoubleBollinger",
    "DoubleTopBottom",
    "DownsideGapThreeMethods",
    "Dpo",
    "DragonflyDoji",
    "DrawdownDuration",
    "DumplingTop",
    "Dx",
    "DynamicMomentumIndex",
    "EaseOfMovement",
    "EffectiveSpread",
    "EhlersStochastic",
    "Ehma",
    "ElderImpulse",
    "ElderRay",
    "ElderSafeZone",
    "Ema",
    "EmpiricalModeDecomposition",
    "Engulfing",
    "Equivolume",
    "EstimatedLeverageRatio",
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
    "FundingBasis",
    "FundingImpliedApr",
    "FundingRate",
    "FundingRateMean",
    "FundingRateZScore",
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
    "GrangerCausality",
    "GravestoneDoji",
    "Hammer",
    "HangingMan",
    "Harami",
    "HaramiCross",
    "HasbrouckInformationShare",
    "HeadAndShoulders",
    "HeikinAshi",
    "HeikinAshiOscillator",
    "HiLoActivator",
    "HighLowIndex",
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
    "Ichimoku",
    "IdenticalThreeCrows",
    "InNeck",
    "Inertia",
    "InformationRatio",
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
    "KalmanHedgeRatio",
    "Kama",
    "KaseDevStop",
    "KasePermissionStochastic",
    "KellyCriterion",
    "Keltner",
    "KendallTau",
    "Kicking",
    "KickingByLength",
    "Kst",
    "Kurtosis",
    "Kvo",
    "KylesLambda",
    "LadderBottom",
    "LaguerreRsi",
    "LeadLagCrossCorrelation",
    "LinRegAngle",
    "LinRegChannel",
    "LinRegIntercept",
    "LinRegSlope",
    "LinearRegression",
    "LiquidationFeatures",
    "LogReturn",
    "LongLeggedDoji",
    "LongLine",
    "LongShortRatio",
    "M2Measure",
    "MaEnvelope",
    "Macd",
    "MacdExt",
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
    "McClellanOscillator",
    "McClellanSummationIndex",
    "McGinleyDynamic",
    "MedianAbsoluteDeviation",
    "MedianChannel",
    "MedianMa",
    "MedianPrice",
    "Mfi",
    "Microprice",
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
    "NewHighsNewLows",
    "NewPriceLines",
    "Nrtr",
    "Nvi",
    "OIPriceDivergence",
    "OIWeighted",
    "Obv",
    "OiToVolumeRatio",
    "OmegaRatio",
    "OnNeck",
    "OpenInterestDelta",
    "OpenInterestMomentum",
    "OpeningMarubozu",
    "OpeningRange",
    "OrderBookImbalanceFull",
    "OrderBookImbalanceTop1",
    "OrderBookImbalanceTopN",
    "OrderFlowImbalance",
    "OuHalfLife",
    "OvernightGap",
    "OvernightIntradayReturn",
    "PainIndex",
    "PairSpreadZScore",
    "PairwiseBeta",
    "ParkinsonVolatility",
    "PearsonCorrelation",
    "PercentAboveMa",
    "PercentB",
    "PercentageTrailingStop",
    "PerpetualPremiumIndex",
    "Pgo",
    "PiercingDarkCloud",
    "Pin",
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
    "QuotedSpread",
    "RSquared",
    "RealizedSpread",
    "RealizedVolatility",
    "RecoveryFactor",
    "RectangleRange",
    "Reflex",
    "RegimeLabel",
    "RelativeStrengthAB",
    "RenkoTrailingStop",
    "RickshawMan",
    "RisingThreeMethods",
    "Rmi",
    "Roc",
    "Rocp",
    "Rocr",
    "Rocr100",
    "RogersSatchellVolatility",
    "RollMeasure",
    "RollingCorrelation",
    "RollingCovariance",
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
    "SignedVolume",
    "SineWave",
    "SineWeightedMa",
    "SinglePrints",
    "Skewness",
    "Sma",
    "Smi",
    "Smma",
    "SmoothedHeikinAshi",
    "SortinoRatio",
    "SpearmanCorrelation",
    "SpinningTop",
    "SpreadAr1Coefficient",
    "SpreadBollingerBands",
    "SpreadHurst",
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
    "TakerBuySellRatio",
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
    "TermStructureBasis",
    "ThreeDrives",
    "ThreeInside",
    "ThreeLineBreak",
    "ThreeLineStrike",
    "ThreeOutside",
    "ThreeSoldiersOrCrows",
    "ThreeStarsInSouth",
    "Thrusting",
    "TickIndex",
    "Tii",
    "TimeBasedStop",
    "TowerTopBottom",
    "TradeImbalance",
    "TradeSignAutocorrelation",
    "TradeVolumeIndex",
    "TrendLabel",
    "TrendStrengthIndex",
    "Trendflex",
    "TreynorRatio",
    "Triangle",
    "Trima",
    "Trin",
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
    "UpDownVolumeRatio",
    "UpsideGapThreeMethods",
    "UpsideGapTwoCrows",
    "UpsidePotentialRatio",
    "ValueArea",
    "ValueAtRisk",
    "Variance",
    "VarianceRatio",
    "VerticalHorizontalFilter",
    "Vidya",
    "VolatilityCone",
    "VolatilityOfVolatility",
    "VolatilityRatio",
    "VoltyStop",
    "VolumeOscillator",
    "VolumePriceTrend",
    "VolumeRsi",
    "VolumeWeightedMacd",
    "VolumeWeightedSr",
    "Vortex",
    "Vpin",
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
    "WilliamsFractals",
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
pub const DEFAULTS: [(&str, &[f64]); 497] = [
    ("AbandonedBaby", &[]),
    ("Abcd", &[]),
    ("AbsoluteBreadthIndex", &[]),
    ("AccelerationBands", &[14.0, 2.0]),
    ("AcceleratorOscillator", &[3.0, 7.0, 14.0]),
    ("AdOscillator", &[]),
    ("AdVolumeLine", &[]),
    ("AdaptiveCci", &[14.0]),
    ("AdaptiveCycle", &[]),
    ("AdaptiveLaguerreFilter", &[20.0]),
    ("AdaptiveRsi", &[14.0]),
    ("Adl", &[]),
    ("AdvanceBlock", &[]),
    ("AdvanceDecline", &[]),
    ("AdvanceDeclineRatio", &[]),
    ("Adx", &[14.0]),
    ("Adxr", &[14.0]),
    ("Alligator", &[3.0, 7.0, 14.0]),
    ("Alma", &[9.0, 0.85, 6.0]),
    ("Alpha", &[14.0, 2.0]),
    ("AmihudIlliquidity", &[20.0]),
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
    ("Beta", &[14.0]),
    ("BetaNeutralSpread", &[14.0]),
    ("BetterVolume", &[14.0]),
    ("BipowerVariation", &[14.0]),
    ("BodySizePct", &[]),
    ("BollingerBands", &[20.0, 2.0]),
    ("BollingerBandwidth", &[14.0, 2.0]),
    ("BomarBands", &[4.0, 0.85]),
    ("BreadthThrust", &[10.0]),
    ("Breakaway", &[]),
    ("BullishPercentIndex", &[]),
    ("BurkeRatio", &[14.0]),
    ("Butterfly", &[]),
    ("CalendarSpread", &[]),
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
    ("Cointegration", &[40.0, 1.0]),
    ("CommonSenseRatio", &[14.0]),
    ("CompositeProfile", &[20.0, 24.0, 0.7]),
    ("ConcealingBabySwallow", &[]),
    ("ConditionalValueAtRisk", &[20.0, 0.95]),
    ("ConnorsRsi", &[3.0, 7.0, 14.0]),
    ("Coppock", &[3.0, 7.0, 14.0]),
    ("CorrelationTrendIndicator", &[14.0]),
    ("Counterattack", &[]),
    ("Crab", &[]),
    ("CumulativeVolumeDelta", &[]),
    ("CumulativeVolumeIndex", &[]),
    ("CupAndHandle", &[]),
    ("CyberneticCycle", &[14.0]),
    ("Cypher", &[]),
    ("Decycler", &[14.0]),
    ("DecyclerOscillator", &[3.0, 7.0]),
    ("Dema", &[14.0]),
    ("DemandIndex", &[14.0]),
    ("DemarkPivots", &[]),
    ("DepthSlope", &[]),
    ("DerivativeOscillator", &[3.0, 7.0, 14.0, 28.0]),
    ("DetrendedStdDev", &[14.0]),
    ("DisparityIndex", &[14.0]),
    ("DistanceSsd", &[14.0]),
    ("Doji", &[]),
    ("DojiStar", &[]),
    ("Donchian", &[14.0]),
    ("DonchianStop", &[14.0]),
    ("DoubleBollinger", &[20.0, 1.0, 2.0]),
    ("DoubleTopBottom", &[]),
    ("DownsideGapThreeMethods", &[]),
    ("Dpo", &[14.0]),
    ("DragonflyDoji", &[]),
    ("DrawdownDuration", &[]),
    ("DumplingTop", &[14.0]),
    ("Dx", &[14.0]),
    ("DynamicMomentumIndex", &[14.0]),
    ("EaseOfMovement", &[14.0]),
    ("EffectiveSpread", &[]),
    ("EhlersStochastic", &[14.0]),
    ("Ehma", &[14.0]),
    ("ElderImpulse", &[3.0, 7.0, 14.0, 28.0]),
    ("ElderRay", &[14.0]),
    ("ElderSafeZone", &[10.0, 2.0]),
    ("Ema", &[14.0]),
    ("EmpiricalModeDecomposition", &[20.0, 0.1]),
    ("Engulfing", &[]),
    ("Equivolume", &[14.0]),
    ("EstimatedLeverageRatio", &[]),
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
    ("FundingBasis", &[]),
    ("FundingImpliedApr", &[1095.0]),
    ("FundingRate", &[]),
    ("FundingRateMean", &[20.0]),
    ("FundingRateZScore", &[20.0]),
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
    ("GrangerCausality", &[60.0, 1.0]),
    ("GravestoneDoji", &[]),
    ("Hammer", &[]),
    ("HangingMan", &[]),
    ("Harami", &[]),
    ("HaramiCross", &[]),
    ("HasbrouckInformationShare", &[14.0]),
    ("HeadAndShoulders", &[]),
    ("HeikinAshi", &[]),
    ("HeikinAshiOscillator", &[14.0]),
    ("HiLoActivator", &[14.0]),
    ("HighLowIndex", &[10.0]),
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
    ("Ichimoku", &[9.0, 26.0, 52.0, 26.0]),
    ("IdenticalThreeCrows", &[]),
    ("InNeck", &[]),
    ("Inertia", &[3.0, 7.0]),
    ("InformationRatio", &[14.0]),
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
    ("KalmanHedgeRatio", &[0.01, 0.001]),
    ("Kama", &[3.0, 7.0, 14.0]),
    ("KaseDevStop", &[14.0, 2.0]),
    ("KasePermissionStochastic", &[3.0, 7.0]),
    ("KellyCriterion", &[14.0]),
    ("Keltner", &[3.0, 7.0, 2.0]),
    ("KendallTau", &[14.0]),
    ("Kicking", &[]),
    ("KickingByLength", &[]),
    ("Kst", &[3.0, 7.0, 14.0, 28.0, 35.0, 42.0, 56.0, 63.0, 70.0]),
    ("Kurtosis", &[14.0]),
    ("Kvo", &[3.0, 7.0]),
    ("KylesLambda", &[20.0]),
    ("LadderBottom", &[]),
    ("LaguerreRsi", &[0.5]),
    ("LeadLagCrossCorrelation", &[20.0, 10.0]),
    ("LinRegAngle", &[14.0]),
    ("LinRegChannel", &[14.0, 2.0]),
    ("LinRegIntercept", &[14.0]),
    ("LinRegSlope", &[14.0]),
    ("LinearRegression", &[14.0]),
    ("LiquidationFeatures", &[]),
    ("LogReturn", &[14.0]),
    ("LongLeggedDoji", &[]),
    ("LongLine", &[]),
    ("LongShortRatio", &[]),
    ("M2Measure", &[14.0, 2.0, 0.5]),
    ("MaEnvelope", &[14.0, 2.0]),
    ("MacdExt", &[12.0, 0.0, 26.0, 0.0, 9.0, 0.0]),
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
    ("McClellanOscillator", &[]),
    ("McClellanSummationIndex", &[]),
    ("McGinleyDynamic", &[14.0]),
    ("MedianAbsoluteDeviation", &[14.0]),
    ("MedianChannel", &[14.0, 2.0]),
    ("MedianMa", &[14.0]),
    ("MedianPrice", &[]),
    ("Mfi", &[14.0]),
    ("Microprice", &[]),
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
    ("NewHighsNewLows", &[]),
    ("NewPriceLines", &[14.0]),
    ("Nrtr", &[2.0]),
    ("Nvi", &[]),
    ("OIPriceDivergence", &[20.0]),
    ("OIWeighted", &[]),
    ("Obv", &[]),
    ("OiToVolumeRatio", &[]),
    ("OmegaRatio", &[14.0, 2.0]),
    ("OnNeck", &[]),
    ("OpenInterestDelta", &[]),
    ("OpenInterestMomentum", &[10.0]),
    ("OpeningMarubozu", &[]),
    ("OpeningRange", &[14.0]),
    ("OrderBookImbalanceFull", &[]),
    ("OrderBookImbalanceTop1", &[]),
    ("OrderBookImbalanceTopN", &[5.0]),
    ("OrderFlowImbalance", &[20.0]),
    ("OuHalfLife", &[14.0]),
    ("OvernightGap", &[0.0]),
    ("OvernightIntradayReturn", &[14.0]),
    ("PainIndex", &[14.0]),
    ("PairSpreadZScore", &[20.0, 20.0]),
    ("PairwiseBeta", &[14.0]),
    ("ParkinsonVolatility", &[20.0, 252.0]),
    ("PearsonCorrelation", &[14.0]),
    ("PercentAboveMa", &[]),
    ("PercentB", &[14.0, 2.0]),
    ("PercentageTrailingStop", &[2.0]),
    ("PerpetualPremiumIndex", &[]),
    ("Pgo", &[14.0]),
    ("PiercingDarkCloud", &[]),
    ("Pin", &[20.0]),
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
    ("QuotedSpread", &[]),
    ("RSquared", &[14.0]),
    ("RealizedSpread", &[20.0]),
    ("RealizedVolatility", &[14.0]),
    ("RecoveryFactor", &[]),
    ("RectangleRange", &[]),
    ("Reflex", &[14.0]),
    ("RegimeLabel", &[3.0, 7.0]),
    ("RelativeStrengthAB", &[14.0, 14.0]),
    ("RenkoTrailingStop", &[2.0]),
    ("RickshawMan", &[]),
    ("RisingThreeMethods", &[]),
    ("Rmi", &[3.0, 7.0]),
    ("Roc", &[14.0]),
    ("Rocp", &[14.0]),
    ("Rocr", &[14.0]),
    ("Rocr100", &[14.0]),
    ("RogersSatchellVolatility", &[20.0, 252.0]),
    ("RollMeasure", &[20.0]),
    ("RollingCorrelation", &[14.0]),
    ("RollingCovariance", &[14.0]),
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
    ("SignedVolume", &[]),
    ("SineWave", &[]),
    ("SineWeightedMa", &[14.0]),
    ("SinglePrints", &[3.0, 7.0]),
    ("Skewness", &[14.0]),
    ("Sma", &[14.0]),
    ("Smi", &[3.0, 7.0, 14.0]),
    ("Smma", &[14.0]),
    ("SmoothedHeikinAshi", &[14.0]),
    ("SortinoRatio", &[14.0, 2.0]),
    ("SpearmanCorrelation", &[14.0]),
    ("SpinningTop", &[]),
    ("SpreadAr1Coefficient", &[14.0]),
    ("SpreadBollingerBands", &[14.0, 2.0]),
    ("SpreadHurst", &[14.0]),
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
    ("TakerBuySellRatio", &[]),
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
    ("TermStructureBasis", &[]),
    ("ThreeDrives", &[]),
    ("ThreeInside", &[]),
    ("ThreeLineBreak", &[14.0]),
    ("ThreeLineStrike", &[]),
    ("ThreeOutside", &[]),
    ("ThreeSoldiersOrCrows", &[]),
    ("ThreeStarsInSouth", &[]),
    ("Thrusting", &[]),
    ("TickIndex", &[]),
    ("Tii", &[3.0, 7.0]),
    ("TimeBasedStop", &[14.0]),
    ("TowerTopBottom", &[]),
    ("TradeImbalance", &[20.0]),
    ("TradeSignAutocorrelation", &[20.0]),
    ("TradeVolumeIndex", &[2.0]),
    ("TrendLabel", &[14.0]),
    ("TrendStrengthIndex", &[14.0]),
    ("Trendflex", &[14.0]),
    ("TreynorRatio", &[14.0, 2.0]),
    ("Triangle", &[]),
    ("Trima", &[14.0]),
    ("Trin", &[]),
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
    ("UpDownVolumeRatio", &[]),
    ("UpsideGapThreeMethods", &[]),
    ("UpsideGapTwoCrows", &[]),
    ("UpsidePotentialRatio", &[14.0, 2.0]),
    ("ValueArea", &[20.0, 50.0, 0.7]),
    ("ValueAtRisk", &[20.0, 0.95]),
    ("Variance", &[14.0]),
    ("VarianceRatio", &[60.0, 2.0]),
    ("VerticalHorizontalFilter", &[14.0]),
    ("Vidya", &[3.0, 7.0]),
    ("VolatilityCone", &[3.0, 7.0]),
    ("VolatilityOfVolatility", &[3.0, 7.0]),
    ("VolatilityRatio", &[14.0]),
    ("VoltyStop", &[14.0, 2.0]),
    ("VolumeOscillator", &[3.0, 7.0]),
    ("VolumePriceTrend", &[]),
    ("VolumeRsi", &[14.0]),
    ("VolumeWeightedMacd", &[3.0, 7.0, 14.0]),
    ("VolumeWeightedSr", &[14.0]),
    ("Vortex", &[14.0]),
    ("Vpin", &[5000.0, 10.0]),
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
    ("WilliamsFractals", &[]),
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

/// The friendly aliases, each paired with the canonical kind it builds.
///
/// Emitted rather than kept only in the generator, because a consumer needs
/// them: `Catalogue::current` used to walk `DEFAULTS`, which holds canonical
/// names only, so both aliases were constructible and invisible to
/// `ListIndicators` -- the discovery surface every binding reads.
pub const ALIASES: [(&str, &str); 2] = [("Bollinger", "BollingerBands"), ("Macd", "MacdIndicator")];

/// Every registered indicator that reads a second market, sorted.
///
/// These are the kinds [`build`] refuses: they need a reference symbol, which is
/// a property of the spec rather than of the parameters. Exposed so a caller can
/// tell a user which indicators need one before they try.
pub const PAIRWISE: [&str; 24] = [
    "Alpha",
    "Beta",
    "BetaNeutralSpread",
    "Cointegration",
    "DistanceSsd",
    "GrangerCausality",
    "HasbrouckInformationShare",
    "InformationRatio",
    "KalmanHedgeRatio",
    "KendallTau",
    "LeadLagCrossCorrelation",
    "OuHalfLife",
    "PairSpreadZScore",
    "PairwiseBeta",
    "PearsonCorrelation",
    "RelativeStrengthAB",
    "RollingCorrelation",
    "RollingCovariance",
    "SpearmanCorrelation",
    "SpreadAr1Coefficient",
    "SpreadBollingerBands",
    "SpreadHurst",
    "TreynorRatio",
    "VarianceRatio",
];

/// Every registered indicator that reads the whole tracked universe, sorted.
pub const CROSS_SECTION: [&str; 15] = [
    "AbsoluteBreadthIndex",
    "AdVolumeLine",
    "AdvanceDecline",
    "AdvanceDeclineRatio",
    "BreadthThrust",
    "BullishPercentIndex",
    "CumulativeVolumeIndex",
    "HighLowIndex",
    "McClellanOscillator",
    "McClellanSummationIndex",
    "NewHighsNewLows",
    "PercentAboveMa",
    "TickIndex",
    "Trin",
    "UpDownVolumeRatio",
];

/// Whether `kind` reads the universe rather than one market.
///
/// Asked by the state before it borrows a market, because assembling the
/// universe walks every market and so cannot happen while one is borrowed.
/// Unlike a pairwise reference -- which is a field on the spec, and readable
/// without the registry -- nothing in a spec says a kind reads breadth. The
/// registry is the only thing that knows, so it says so.
#[must_use]
pub fn is_cross_section(kind: &str) -> bool {
    CROSS_SECTION.binary_search(&kind).is_ok()
}

/// The reference symbol a pairwise indicator was configured with.
fn pair_reference<'a>(kind: &str, reference: Option<&'a str>) -> Result<&'a str> {
    reference.ok_or_else(|| {
        Error::Config(format!(
            "{kind} compares two markets, so it needs a reference symbol"
        ))
    })
}

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
pub fn build(kind: &str, params: &[f64]) -> Result<Box<dyn TickIndicator>> {
    build_inner(kind, params, None)
}

/// Construct an indicator that compares this market against `reference`.
///
/// # Errors
///
/// As [`build`]. A kind that is not pairwise ignores the reference rather than
/// failing, so a caller may pass one uniformly.
pub fn build_paired(kind: &str, params: &[f64], reference: &str) -> Result<Box<dyn TickIndicator>> {
    build_inner(kind, params, Some(reference))
}

fn build_inner(
    kind: &str,
    params: &[f64],
    reference: Option<&str>,
) -> Result<Box<dyn TickIndicator>> {
    match kind {
        "AbandonedBaby" => Ok(Box::new(CandleIn {
            inner: wc::AbandonedBaby::new(),
        })),
        "Abcd" => Ok(Box::new(CandleIn {
            inner: wc::Abcd::new(),
        })),
        "AbsoluteBreadthIndex" => Ok(Box::new(CrossIn {
            inner: wc::AbsoluteBreadthIndex::new(),
        })),
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
        "AcceleratorOscillator" => Ok(Box::new(CandleIn {
            inner: map_new(
                kind,
                wc::AcceleratorOscillator::new(
                    usize_param(params, 0, kind)?,
                    usize_param(params, 1, kind)?,
                    usize_param(params, 2, kind)?,
                ),
            )?,
        })),
        "AdOscillator" => Ok(Box::new(CandleIn {
            inner: wc::AdOscillator::new(),
        })),
        "AdVolumeLine" => Ok(Box::new(CrossIn {
            inner: wc::AdVolumeLine::new(),
        })),
        "AdaptiveCci" => Ok(Box::new(CandleIn {
            inner: map_new(kind, wc::AdaptiveCci::new(usize_param(params, 0, kind)?))?,
        })),
        "AdaptiveCycle" => Ok(Box::new(ScalarPrice {
            inner: wc::AdaptiveCycle::new(),
        })),
        "AdaptiveLaguerreFilter" => Ok(Box::new(ScalarPrice {
            inner: map_new(
                kind,
                wc::AdaptiveLaguerreFilter::new(usize_param(params, 0, kind)?),
            )?,
        })),
        "AdaptiveRsi" => Ok(Box::new(ScalarPrice {
            inner: map_new(kind, wc::AdaptiveRsi::new(usize_param(params, 0, kind)?))?,
        })),
        "Adl" => Ok(Box::new(CandleIn {
            inner: wc::Adl::new(),
        })),
        "AdvanceBlock" => Ok(Box::new(CandleIn {
            inner: wc::AdvanceBlock::new(),
        })),
        "AdvanceDecline" => Ok(Box::new(CrossIn {
            inner: wc::AdvanceDecline::new(),
        })),
        "AdvanceDeclineRatio" => Ok(Box::new(CrossIn {
            inner: wc::AdvanceDeclineRatio::new(),
        })),
        "Adx" => Ok(Box::new(CandleInFields {
            inner: map_new(kind, wc::Adx::new(usize_param(params, 0, kind)?))?,
            last: None,
        })),
        "Adxr" => Ok(Box::new(CandleIn {
            inner: map_new(kind, wc::Adxr::new(usize_param(params, 0, kind)?))?,
        })),
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
        "Alma" => Ok(Box::new(ScalarPrice {
            inner: map_new(
                kind,
                wc::Alma::new(
                    usize_param(params, 0, kind)?,
                    float_param(params, 1, kind)?,
                    float_param(params, 2, kind)?,
                ),
            )?,
        })),
        "Alpha" => Ok(Box::new(PairIn {
            inner: map_new(
                kind,
                wc::Alpha::new(usize_param(params, 0, kind)?, float_param(params, 1, kind)?),
            )?,
            reference: pair_reference(kind, reference)?.to_string(),
        })),
        "AmihudIlliquidity" => Ok(Box::new(TradeIn {
            inner: map_new(
                kind,
                wc::AmihudIlliquidity::new(usize_param(params, 0, kind)?),
            )?,
        })),
        "AnchoredRsi" => Ok(Box::new(ScalarPrice {
            inner: wc::AnchoredRsi::new(),
        })),
        "AnchoredVwap" => Ok(Box::new(CandleIn {
            inner: wc::AnchoredVwap::new(),
        })),
        "AndrewsPitchfork" => Ok(Box::new(CandleInFields {
            inner: map_new(
                kind,
                wc::AndrewsPitchfork::new(usize_param(params, 0, kind)?),
            )?,
            last: None,
        })),
        "Apo" => Ok(Box::new(ScalarPrice {
            inner: map_new(
                kind,
                wc::Apo::new(usize_param(params, 0, kind)?, usize_param(params, 1, kind)?),
            )?,
        })),
        "Aroon" => Ok(Box::new(CandleInFields {
            inner: map_new(kind, wc::Aroon::new(usize_param(params, 0, kind)?))?,
            last: None,
        })),
        "AroonOscillator" => Ok(Box::new(CandleIn {
            inner: map_new(
                kind,
                wc::AroonOscillator::new(usize_param(params, 0, kind)?),
            )?,
        })),
        "Atr" => Ok(Box::new(CandleIn {
            inner: map_new(kind, wc::Atr::new(usize_param(params, 0, kind)?))?,
        })),
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
        "AtrTrailingStop" => Ok(Box::new(CandleIn {
            inner: map_new(
                kind,
                wc::AtrTrailingStop::new(
                    usize_param(params, 0, kind)?,
                    float_param(params, 1, kind)?,
                ),
            )?,
        })),
        "AutoFib" => Ok(Box::new(CandleInFields {
            inner: wc::AutoFib::new(),
            last: None,
        })),
        "Autocorrelation" => Ok(Box::new(ScalarPrice {
            inner: map_new(
                kind,
                wc::Autocorrelation::new(
                    usize_param(params, 0, kind)?,
                    usize_param(params, 1, kind)?,
                ),
            )?,
        })),
        "AutocorrelationPeriodogram" => Ok(Box::new(ScalarPrice {
            inner: map_new(
                kind,
                wc::AutocorrelationPeriodogram::new(
                    usize_param(params, 0, kind)?,
                    usize_param(params, 1, kind)?,
                ),
            )?,
        })),
        "AverageDailyRange" => Ok(Box::new(CandleIn {
            inner: map_new(
                kind,
                wc::AverageDailyRange::new(
                    usize_param(params, 0, kind)?,
                    i32_param(params, 1, kind)?,
                ),
            )?,
        })),
        "AverageDrawdown" => Ok(Box::new(ScalarPrice {
            inner: map_new(
                kind,
                wc::AverageDrawdown::new(usize_param(params, 0, kind)?),
            )?,
        })),
        "AvgPrice" => Ok(Box::new(CandleIn {
            inner: wc::AvgPrice::new(),
        })),
        "AwesomeOscillator" => Ok(Box::new(CandleIn {
            inner: map_new(
                kind,
                wc::AwesomeOscillator::new(
                    usize_param(params, 0, kind)?,
                    usize_param(params, 1, kind)?,
                ),
            )?,
        })),
        "AwesomeOscillatorHistogram" => Ok(Box::new(CandleIn {
            inner: map_new(
                kind,
                wc::AwesomeOscillatorHistogram::new(
                    usize_param(params, 0, kind)?,
                    usize_param(params, 1, kind)?,
                    usize_param(params, 2, kind)?,
                ),
            )?,
        })),
        "BalanceOfPower" => Ok(Box::new(CandleIn {
            inner: wc::BalanceOfPower::new(),
        })),
        "BandpassFilter" => Ok(Box::new(ScalarPrice {
            inner: map_new(
                kind,
                wc::BandpassFilter::new(
                    usize_param(params, 0, kind)?,
                    float_param(params, 1, kind)?,
                ),
            )?,
        })),
        "Bat" => Ok(Box::new(CandleIn {
            inner: wc::Bat::new(),
        })),
        "BeltHold" => Ok(Box::new(CandleIn {
            inner: wc::BeltHold::new(),
        })),
        "Beta" => Ok(Box::new(PairIn {
            inner: map_new(kind, wc::Beta::new(usize_param(params, 0, kind)?))?,
            reference: pair_reference(kind, reference)?.to_string(),
        })),
        "BetaNeutralSpread" => Ok(Box::new(PairIn {
            inner: map_new(
                kind,
                wc::BetaNeutralSpread::new(usize_param(params, 0, kind)?),
            )?,
            reference: pair_reference(kind, reference)?.to_string(),
        })),
        "BetterVolume" => Ok(Box::new(CandleIn {
            inner: map_new(kind, wc::BetterVolume::new(usize_param(params, 0, kind)?))?,
        })),
        "BipowerVariation" => Ok(Box::new(ScalarPrice {
            inner: map_new(
                kind,
                wc::BipowerVariation::new(usize_param(params, 0, kind)?),
            )?,
        })),
        "BodySizePct" => Ok(Box::new(CandleIn {
            inner: wc::BodySizePct::new(),
        })),
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
        "BollingerBandwidth" => Ok(Box::new(ScalarPrice {
            inner: map_new(
                kind,
                wc::BollingerBandwidth::new(
                    usize_param(params, 0, kind)?,
                    float_param(params, 1, kind)?,
                ),
            )?,
        })),
        "BomarBands" => Ok(Box::new(ScalarPriceFields {
            inner: map_new(
                kind,
                wc::BomarBands::new(usize_param(params, 0, kind)?, float_param(params, 1, kind)?),
            )?,
            last: None,
        })),
        "BreadthThrust" => Ok(Box::new(CrossIn {
            inner: map_new(kind, wc::BreadthThrust::new(usize_param(params, 0, kind)?))?,
        })),
        "Breakaway" => Ok(Box::new(CandleIn {
            inner: wc::Breakaway::new(),
        })),
        "BullishPercentIndex" => Ok(Box::new(CrossIn {
            inner: wc::BullishPercentIndex::new(),
        })),
        "BurkeRatio" => Ok(Box::new(ScalarPrice {
            inner: map_new(kind, wc::BurkeRatio::new(usize_param(params, 0, kind)?))?,
        })),
        "Butterfly" => Ok(Box::new(CandleIn {
            inner: wc::Butterfly::new(),
        })),
        "CalendarSpread" => Ok(Box::new(DerivIn {
            inner: wc::CalendarSpread::new(),
        })),
        "CalmarRatio" => Ok(Box::new(ScalarPrice {
            inner: map_new(kind, wc::CalmarRatio::new(usize_param(params, 0, kind)?))?,
        })),
        "Camarilla" => Ok(Box::new(CandleInFields {
            inner: wc::Camarilla::new(),
            last: None,
        })),
        "CandleVolume" => Ok(Box::new(CandleInFields {
            inner: map_new(kind, wc::CandleVolume::new(usize_param(params, 0, kind)?))?,
            last: None,
        })),
        "Cci" => Ok(Box::new(CandleIn {
            inner: map_new(kind, wc::Cci::new(usize_param(params, 0, kind)?))?,
        })),
        "CenterOfGravity" => Ok(Box::new(ScalarPrice {
            inner: map_new(
                kind,
                wc::CenterOfGravity::new(usize_param(params, 0, kind)?),
            )?,
        })),
        "CentralPivotRange" => Ok(Box::new(CandleInFields {
            inner: wc::CentralPivotRange::new(),
            last: None,
        })),
        "Cfo" => Ok(Box::new(ScalarPrice {
            inner: map_new(kind, wc::Cfo::new(usize_param(params, 0, kind)?))?,
        })),
        "ChaikinMoneyFlow" => Ok(Box::new(CandleIn {
            inner: map_new(
                kind,
                wc::ChaikinMoneyFlow::new(usize_param(params, 0, kind)?),
            )?,
        })),
        "ChaikinOscillator" => Ok(Box::new(CandleIn {
            inner: map_new(
                kind,
                wc::ChaikinOscillator::new(
                    usize_param(params, 0, kind)?,
                    usize_param(params, 1, kind)?,
                ),
            )?,
        })),
        "ChaikinVolatility" => Ok(Box::new(CandleIn {
            inner: map_new(
                kind,
                wc::ChaikinVolatility::new(
                    usize_param(params, 0, kind)?,
                    usize_param(params, 1, kind)?,
                ),
            )?,
        })),
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
        "ChoppinessIndex" => Ok(Box::new(CandleIn {
            inner: map_new(
                kind,
                wc::ChoppinessIndex::new(usize_param(params, 0, kind)?),
            )?,
        })),
        "ClassicPivots" => Ok(Box::new(CandleInFields {
            inner: wc::ClassicPivots::new(),
            last: None,
        })),
        "CloseVsOpen" => Ok(Box::new(CandleIn {
            inner: wc::CloseVsOpen::new(),
        })),
        "ClosingMarubozu" => Ok(Box::new(CandleIn {
            inner: wc::ClosingMarubozu::new(),
        })),
        "Cmo" => Ok(Box::new(ScalarPrice {
            inner: map_new(kind, wc::Cmo::new(usize_param(params, 0, kind)?))?,
        })),
        "CoefficientOfVariation" => Ok(Box::new(ScalarPrice {
            inner: map_new(
                kind,
                wc::CoefficientOfVariation::new(usize_param(params, 0, kind)?),
            )?,
        })),
        "Cointegration" => Ok(Box::new(PairInFields {
            inner: map_new(
                kind,
                wc::Cointegration::new(
                    usize_param(params, 0, kind)?,
                    usize_param(params, 1, kind)?,
                ),
            )?,
            last: None,
            reference: pair_reference(kind, reference)?.to_string(),
        })),
        "CommonSenseRatio" => Ok(Box::new(ScalarPrice {
            inner: map_new(
                kind,
                wc::CommonSenseRatio::new(usize_param(params, 0, kind)?),
            )?,
        })),
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
        "ConcealingBabySwallow" => Ok(Box::new(CandleIn {
            inner: wc::ConcealingBabySwallow::new(),
        })),
        "ConditionalValueAtRisk" => Ok(Box::new(ScalarPrice {
            inner: map_new(
                kind,
                wc::ConditionalValueAtRisk::new(
                    usize_param(params, 0, kind)?,
                    float_param(params, 1, kind)?,
                ),
            )?,
        })),
        "ConnorsRsi" => Ok(Box::new(ScalarPrice {
            inner: map_new(
                kind,
                wc::ConnorsRsi::new(
                    usize_param(params, 0, kind)?,
                    usize_param(params, 1, kind)?,
                    usize_param(params, 2, kind)?,
                ),
            )?,
        })),
        "Coppock" => Ok(Box::new(ScalarPrice {
            inner: map_new(
                kind,
                wc::Coppock::new(
                    usize_param(params, 0, kind)?,
                    usize_param(params, 1, kind)?,
                    usize_param(params, 2, kind)?,
                ),
            )?,
        })),
        "CorrelationTrendIndicator" => Ok(Box::new(ScalarPrice {
            inner: map_new(
                kind,
                wc::CorrelationTrendIndicator::new(usize_param(params, 0, kind)?),
            )?,
        })),
        "Counterattack" => Ok(Box::new(CandleIn {
            inner: wc::Counterattack::new(),
        })),
        "Crab" => Ok(Box::new(CandleIn {
            inner: wc::Crab::new(),
        })),
        "CumulativeVolumeDelta" => Ok(Box::new(TradeIn {
            inner: wc::CumulativeVolumeDelta::new(),
        })),
        "CumulativeVolumeIndex" => Ok(Box::new(CrossIn {
            inner: wc::CumulativeVolumeIndex::new(),
        })),
        "CupAndHandle" => Ok(Box::new(CandleIn {
            inner: wc::CupAndHandle::new(),
        })),
        "CyberneticCycle" => Ok(Box::new(ScalarPrice {
            inner: map_new(
                kind,
                wc::CyberneticCycle::new(usize_param(params, 0, kind)?),
            )?,
        })),
        "Cypher" => Ok(Box::new(CandleIn {
            inner: wc::Cypher::new(),
        })),
        "Decycler" => Ok(Box::new(ScalarPrice {
            inner: map_new(kind, wc::Decycler::new(usize_param(params, 0, kind)?))?,
        })),
        "DecyclerOscillator" => Ok(Box::new(ScalarPrice {
            inner: map_new(
                kind,
                wc::DecyclerOscillator::new(
                    usize_param(params, 0, kind)?,
                    usize_param(params, 1, kind)?,
                ),
            )?,
        })),
        "Dema" => Ok(Box::new(ScalarPrice {
            inner: map_new(kind, wc::Dema::new(usize_param(params, 0, kind)?))?,
        })),
        "DemandIndex" => Ok(Box::new(CandleIn {
            inner: map_new(kind, wc::DemandIndex::new(usize_param(params, 0, kind)?))?,
        })),
        "DemarkPivots" => Ok(Box::new(CandleInFields {
            inner: wc::DemarkPivots::new(),
            last: None,
        })),
        "DepthSlope" => Ok(Box::new(BookIn {
            inner: wc::DepthSlope::new(),
        })),
        "DerivativeOscillator" => Ok(Box::new(ScalarPrice {
            inner: map_new(
                kind,
                wc::DerivativeOscillator::new(
                    usize_param(params, 0, kind)?,
                    usize_param(params, 1, kind)?,
                    usize_param(params, 2, kind)?,
                    usize_param(params, 3, kind)?,
                ),
            )?,
        })),
        "DetrendedStdDev" => Ok(Box::new(ScalarPrice {
            inner: map_new(
                kind,
                wc::DetrendedStdDev::new(usize_param(params, 0, kind)?),
            )?,
        })),
        "DisparityIndex" => Ok(Box::new(ScalarPrice {
            inner: map_new(kind, wc::DisparityIndex::new(usize_param(params, 0, kind)?))?,
        })),
        "DistanceSsd" => Ok(Box::new(PairIn {
            inner: map_new(kind, wc::DistanceSsd::new(usize_param(params, 0, kind)?))?,
            reference: pair_reference(kind, reference)?.to_string(),
        })),
        "Doji" => Ok(Box::new(CandleIn {
            inner: wc::Doji::new(),
        })),
        "DojiStar" => Ok(Box::new(CandleIn {
            inner: wc::DojiStar::new(),
        })),
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
        "DoubleTopBottom" => Ok(Box::new(CandleIn {
            inner: wc::DoubleTopBottom::new(),
        })),
        "DownsideGapThreeMethods" => Ok(Box::new(CandleIn {
            inner: wc::DownsideGapThreeMethods::new(),
        })),
        "Dpo" => Ok(Box::new(ScalarPrice {
            inner: map_new(kind, wc::Dpo::new(usize_param(params, 0, kind)?))?,
        })),
        "DragonflyDoji" => Ok(Box::new(CandleIn {
            inner: wc::DragonflyDoji::new(),
        })),
        "DrawdownDuration" => Ok(Box::new(ScalarPriceInt {
            inner: wc::DrawdownDuration::new(),
        })),
        "DumplingTop" => Ok(Box::new(CandleIn {
            inner: map_new(kind, wc::DumplingTop::new(usize_param(params, 0, kind)?))?,
        })),
        "Dx" => Ok(Box::new(CandleIn {
            inner: map_new(kind, wc::Dx::new(usize_param(params, 0, kind)?))?,
        })),
        "DynamicMomentumIndex" => Ok(Box::new(ScalarPrice {
            inner: map_new(
                kind,
                wc::DynamicMomentumIndex::new(usize_param(params, 0, kind)?),
            )?,
        })),
        "EaseOfMovement" => Ok(Box::new(CandleIn {
            inner: map_new(kind, wc::EaseOfMovement::new(usize_param(params, 0, kind)?))?,
        })),
        "EffectiveSpread" => Ok(Box::new(QuoteIn {
            inner: wc::EffectiveSpread::new(),
        })),
        "EhlersStochastic" => Ok(Box::new(ScalarPrice {
            inner: map_new(
                kind,
                wc::EhlersStochastic::new(usize_param(params, 0, kind)?),
            )?,
        })),
        "Ehma" => Ok(Box::new(ScalarPrice {
            inner: map_new(kind, wc::Ehma::new(usize_param(params, 0, kind)?))?,
        })),
        "ElderImpulse" => Ok(Box::new(ScalarPrice {
            inner: map_new(
                kind,
                wc::ElderImpulse::new(
                    usize_param(params, 0, kind)?,
                    usize_param(params, 1, kind)?,
                    usize_param(params, 2, kind)?,
                    usize_param(params, 3, kind)?,
                ),
            )?,
        })),
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
        "Ema" => Ok(Box::new(ScalarPrice {
            inner: map_new(kind, wc::Ema::new(usize_param(params, 0, kind)?))?,
        })),
        "EmpiricalModeDecomposition" => Ok(Box::new(ScalarPrice {
            inner: map_new(
                kind,
                wc::EmpiricalModeDecomposition::new(
                    usize_param(params, 0, kind)?,
                    float_param(params, 1, kind)?,
                ),
            )?,
        })),
        "Engulfing" => Ok(Box::new(CandleIn {
            inner: wc::Engulfing::new(),
        })),
        "Equivolume" => Ok(Box::new(CandleInFields {
            inner: map_new(kind, wc::Equivolume::new(usize_param(params, 0, kind)?))?,
            last: None,
        })),
        "EstimatedLeverageRatio" => Ok(Box::new(DerivIn {
            inner: wc::EstimatedLeverageRatio::new(),
        })),
        "EvenBetterSinewave" => Ok(Box::new(ScalarPrice {
            inner: map_new(
                kind,
                wc::EvenBetterSinewave::new(
                    usize_param(params, 0, kind)?,
                    usize_param(params, 1, kind)?,
                ),
            )?,
        })),
        "EveningDojiStar" => Ok(Box::new(CandleIn {
            inner: wc::EveningDojiStar::new(),
        })),
        "Evwma" => Ok(Box::new(CandleIn {
            inner: map_new(kind, wc::Evwma::new(usize_param(params, 0, kind)?))?,
        })),
        "EwmaVolatility" => Ok(Box::new(ScalarPrice {
            inner: map_new(kind, wc::EwmaVolatility::new(float_param(params, 0, kind)?))?,
        })),
        "Expectancy" => Ok(Box::new(ScalarPrice {
            inner: map_new(kind, wc::Expectancy::new(usize_param(params, 0, kind)?))?,
        })),
        "FallingThreeMethods" => Ok(Box::new(CandleIn {
            inner: wc::FallingThreeMethods::new(),
        })),
        "Fama" => Ok(Box::new(ScalarPrice {
            inner: map_new(
                kind,
                wc::Fama::new(float_param(params, 0, kind)?, float_param(params, 1, kind)?),
            )?,
        })),
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
        "FisherRsi" => Ok(Box::new(ScalarPrice {
            inner: map_new(kind, wc::FisherRsi::new(usize_param(params, 0, kind)?))?,
        })),
        "FisherTransform" => Ok(Box::new(ScalarPrice {
            inner: map_new(
                kind,
                wc::FisherTransform::new(usize_param(params, 0, kind)?),
            )?,
        })),
        "FlagPennant" => Ok(Box::new(CandleIn {
            inner: wc::FlagPennant::new(),
        })),
        "ForceIndex" => Ok(Box::new(CandleIn {
            inner: map_new(kind, wc::ForceIndex::new(usize_param(params, 0, kind)?))?,
        })),
        "FractalChaosBands" => Ok(Box::new(CandleInFields {
            inner: map_new(
                kind,
                wc::FractalChaosBands::new(usize_param(params, 0, kind)?),
            )?,
            last: None,
        })),
        "Frama" => Ok(Box::new(ScalarPrice {
            inner: map_new(kind, wc::Frama::new(usize_param(params, 0, kind)?))?,
        })),
        "FryPanBottom" => Ok(Box::new(CandleIn {
            inner: map_new(kind, wc::FryPanBottom::new(usize_param(params, 0, kind)?))?,
        })),
        "FundingBasis" => Ok(Box::new(DerivIn {
            inner: wc::FundingBasis::new(),
        })),
        "FundingImpliedApr" => Ok(Box::new(DerivIn {
            inner: map_new(
                kind,
                wc::FundingImpliedApr::new(float_param(params, 0, kind)?),
            )?,
        })),
        "FundingRate" => Ok(Box::new(DerivIn {
            inner: wc::FundingRate::new(),
        })),
        "FundingRateMean" => Ok(Box::new(DerivIn {
            inner: map_new(
                kind,
                wc::FundingRateMean::new(usize_param(params, 0, kind)?),
            )?,
        })),
        "FundingRateZScore" => Ok(Box::new(DerivIn {
            inner: map_new(
                kind,
                wc::FundingRateZScore::new(usize_param(params, 0, kind)?),
            )?,
        })),
        "GainLossRatio" => Ok(Box::new(ReturnsIn {
            inner: map_new(kind, wc::GainLossRatio::new(usize_param(params, 0, kind)?))?,
            previous_close: None,
        })),
        "GainToPainRatio" => Ok(Box::new(ScalarPrice {
            inner: map_new(
                kind,
                wc::GainToPainRatio::new(usize_param(params, 0, kind)?),
            )?,
        })),
        "GapSideBySideWhite" => Ok(Box::new(CandleIn {
            inner: wc::GapSideBySideWhite::new(),
        })),
        "Garch11" => Ok(Box::new(ScalarPrice {
            inner: map_new(
                kind,
                wc::Garch11::new(
                    float_param(params, 0, kind)?,
                    float_param(params, 1, kind)?,
                    float_param(params, 2, kind)?,
                ),
            )?,
        })),
        "GarmanKlassVolatility" => Ok(Box::new(CandleIn {
            inner: map_new(
                kind,
                wc::GarmanKlassVolatility::new(
                    usize_param(params, 0, kind)?,
                    usize_param(params, 1, kind)?,
                ),
            )?,
        })),
        "Gartley" => Ok(Box::new(CandleIn {
            inner: wc::Gartley::new(),
        })),
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
        "GeneralizedDema" => Ok(Box::new(ScalarPrice {
            inner: map_new(
                kind,
                wc::GeneralizedDema::new(
                    usize_param(params, 0, kind)?,
                    float_param(params, 1, kind)?,
                ),
            )?,
        })),
        "GeometricMa" => Ok(Box::new(ScalarPrice {
            inner: map_new(kind, wc::GeometricMa::new(usize_param(params, 0, kind)?))?,
        })),
        "GoldenPocket" => Ok(Box::new(CandleInFields {
            inner: wc::GoldenPocket::new(),
            last: None,
        })),
        "GrangerCausality" => Ok(Box::new(PairIn {
            inner: map_new(
                kind,
                wc::GrangerCausality::new(
                    usize_param(params, 0, kind)?,
                    usize_param(params, 1, kind)?,
                ),
            )?,
            reference: pair_reference(kind, reference)?.to_string(),
        })),
        "GravestoneDoji" => Ok(Box::new(CandleIn {
            inner: wc::GravestoneDoji::new(),
        })),
        "Hammer" => Ok(Box::new(CandleIn {
            inner: wc::Hammer::new(),
        })),
        "HangingMan" => Ok(Box::new(CandleIn {
            inner: wc::HangingMan::new(),
        })),
        "Harami" => Ok(Box::new(CandleIn {
            inner: wc::Harami::new(),
        })),
        "HaramiCross" => Ok(Box::new(CandleIn {
            inner: wc::HaramiCross::new(),
        })),
        "HasbrouckInformationShare" => Ok(Box::new(PairIn {
            inner: map_new(
                kind,
                wc::HasbrouckInformationShare::new(usize_param(params, 0, kind)?),
            )?,
            reference: pair_reference(kind, reference)?.to_string(),
        })),
        "HeadAndShoulders" => Ok(Box::new(CandleIn {
            inner: wc::HeadAndShoulders::new(),
        })),
        "HeikinAshi" => Ok(Box::new(CandleInFields {
            inner: wc::HeikinAshi::new(),
            last: None,
        })),
        "HeikinAshiOscillator" => Ok(Box::new(CandleIn {
            inner: map_new(
                kind,
                wc::HeikinAshiOscillator::new(usize_param(params, 0, kind)?),
            )?,
        })),
        "HiLoActivator" => Ok(Box::new(CandleIn {
            inner: map_new(kind, wc::HiLoActivator::new(usize_param(params, 0, kind)?))?,
        })),
        "HighLowIndex" => Ok(Box::new(CrossIn {
            inner: map_new(kind, wc::HighLowIndex::new(usize_param(params, 0, kind)?))?,
        })),
        "HighLowRange" => Ok(Box::new(CandleIn {
            inner: wc::HighLowRange::new(),
        })),
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
        "HighWave" => Ok(Box::new(CandleIn {
            inner: wc::HighWave::new(),
        })),
        "HighpassFilter" => Ok(Box::new(ScalarPrice {
            inner: map_new(kind, wc::HighpassFilter::new(usize_param(params, 0, kind)?))?,
        })),
        "Hikkake" => Ok(Box::new(CandleIn {
            inner: wc::Hikkake::new(),
        })),
        "HikkakeModified" => Ok(Box::new(CandleIn {
            inner: wc::HikkakeModified::new(),
        })),
        "HilbertDominantCycle" => Ok(Box::new(ScalarPrice {
            inner: wc::HilbertDominantCycle::new(),
        })),
        "HistoricalVolatility" => Ok(Box::new(ScalarPrice {
            inner: map_new(
                kind,
                wc::HistoricalVolatility::new(
                    usize_param(params, 0, kind)?,
                    usize_param(params, 1, kind)?,
                ),
            )?,
        })),
        "Hma" => Ok(Box::new(ScalarPrice {
            inner: map_new(kind, wc::Hma::new(usize_param(params, 0, kind)?))?,
        })),
        "HoltWinters" => Ok(Box::new(ScalarPrice {
            inner: map_new(
                kind,
                wc::HoltWinters::new(float_param(params, 0, kind)?, float_param(params, 1, kind)?),
            )?,
        })),
        "HomingPigeon" => Ok(Box::new(CandleIn {
            inner: wc::HomingPigeon::new(),
        })),
        "HtDcPhase" => Ok(Box::new(ScalarPrice {
            inner: wc::HtDcPhase::new(),
        })),
        "HtPhasor" => Ok(Box::new(ScalarPriceFields {
            inner: wc::HtPhasor::new(),
            last: None,
        })),
        "HtTrendMode" => Ok(Box::new(ScalarPrice {
            inner: wc::HtTrendMode::new(),
        })),
        "HurstChannel" => Ok(Box::new(CandleInFields {
            inner: map_new(
                kind,
                wc::HurstChannel::new(usize_param(params, 0, kind)?, float_param(params, 1, kind)?),
            )?,
            last: None,
        })),
        "HurstExponent" => Ok(Box::new(ScalarPrice {
            inner: map_new(
                kind,
                wc::HurstExponent::new(
                    usize_param(params, 0, kind)?,
                    usize_param(params, 1, kind)?,
                ),
            )?,
        })),
        "Ichimoku" => Ok(Box::new(CandleInFields {
            inner: map_new(
                kind,
                wc::Ichimoku::new(
                    usize_param(params, 0, kind)?,
                    usize_param(params, 1, kind)?,
                    usize_param(params, 2, kind)?,
                    usize_param(params, 3, kind)?,
                ),
            )?,
            last: None,
        })),
        "IdenticalThreeCrows" => Ok(Box::new(CandleIn {
            inner: wc::IdenticalThreeCrows::new(),
        })),
        "InNeck" => Ok(Box::new(CandleIn {
            inner: wc::InNeck::new(),
        })),
        "Inertia" => Ok(Box::new(CandleIn {
            inner: map_new(
                kind,
                wc::Inertia::new(usize_param(params, 0, kind)?, usize_param(params, 1, kind)?),
            )?,
        })),
        "InformationRatio" => Ok(Box::new(PairIn {
            inner: map_new(
                kind,
                wc::InformationRatio::new(usize_param(params, 0, kind)?),
            )?,
            reference: pair_reference(kind, reference)?.to_string(),
        })),
        "InitialBalance" => Ok(Box::new(CandleInFields {
            inner: map_new(kind, wc::InitialBalance::new(usize_param(params, 0, kind)?))?,
            last: None,
        })),
        "InstantaneousTrendline" => Ok(Box::new(ScalarPrice {
            inner: map_new(
                kind,
                wc::InstantaneousTrendline::new(usize_param(params, 0, kind)?),
            )?,
        })),
        "IntradayIntensity" => Ok(Box::new(CandleIn {
            inner: wc::IntradayIntensity::new(),
        })),
        "IntradayMomentumIndex" => Ok(Box::new(CandleIn {
            inner: map_new(
                kind,
                wc::IntradayMomentumIndex::new(usize_param(params, 0, kind)?),
            )?,
        })),
        "InverseFisherTransform" => Ok(Box::new(ScalarPrice {
            inner: map_new(
                kind,
                wc::InverseFisherTransform::new(float_param(params, 0, kind)?),
            )?,
        })),
        "InvertedHammer" => Ok(Box::new(CandleIn {
            inner: wc::InvertedHammer::new(),
        })),
        "JarqueBera" => Ok(Box::new(ScalarPrice {
            inner: map_new(kind, wc::JarqueBera::new(usize_param(params, 0, kind)?))?,
        })),
        "Jma" => Ok(Box::new(ScalarPrice {
            inner: map_new(
                kind,
                wc::Jma::new(
                    usize_param(params, 0, kind)?,
                    float_param(params, 1, kind)?,
                    u32_param(params, 2, kind)?,
                ),
            )?,
        })),
        "JumpIndicator" => Ok(Box::new(ScalarPrice {
            inner: map_new(
                kind,
                wc::JumpIndicator::new(
                    usize_param(params, 0, kind)?,
                    float_param(params, 1, kind)?,
                ),
            )?,
        })),
        "KRatio" => Ok(Box::new(ScalarPrice {
            inner: map_new(kind, wc::KRatio::new(usize_param(params, 0, kind)?))?,
        })),
        "KalmanHedgeRatio" => Ok(Box::new(PairInFields {
            inner: map_new(
                kind,
                wc::KalmanHedgeRatio::new(
                    float_param(params, 0, kind)?,
                    float_param(params, 1, kind)?,
                ),
            )?,
            last: None,
            reference: pair_reference(kind, reference)?.to_string(),
        })),
        "Kama" => Ok(Box::new(ScalarPrice {
            inner: map_new(
                kind,
                wc::Kama::new(
                    usize_param(params, 0, kind)?,
                    usize_param(params, 1, kind)?,
                    usize_param(params, 2, kind)?,
                ),
            )?,
        })),
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
        "KellyCriterion" => Ok(Box::new(ScalarPrice {
            inner: map_new(kind, wc::KellyCriterion::new(usize_param(params, 0, kind)?))?,
        })),
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
        "KendallTau" => Ok(Box::new(PairIn {
            inner: map_new(kind, wc::KendallTau::new(usize_param(params, 0, kind)?))?,
            reference: pair_reference(kind, reference)?.to_string(),
        })),
        "Kicking" => Ok(Box::new(CandleIn {
            inner: wc::Kicking::new(),
        })),
        "KickingByLength" => Ok(Box::new(CandleIn {
            inner: wc::KickingByLength::new(),
        })),
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
        "Kurtosis" => Ok(Box::new(ScalarPrice {
            inner: map_new(kind, wc::Kurtosis::new(usize_param(params, 0, kind)?))?,
        })),
        "Kvo" => Ok(Box::new(CandleIn {
            inner: map_new(
                kind,
                wc::Kvo::new(usize_param(params, 0, kind)?, usize_param(params, 1, kind)?),
            )?,
        })),
        "KylesLambda" => Ok(Box::new(QuoteIn {
            inner: map_new(kind, wc::KylesLambda::new(usize_param(params, 0, kind)?))?,
        })),
        "LadderBottom" => Ok(Box::new(CandleIn {
            inner: wc::LadderBottom::new(),
        })),
        "LaguerreRsi" => Ok(Box::new(ScalarPrice {
            inner: map_new(kind, wc::LaguerreRsi::new(float_param(params, 0, kind)?))?,
        })),
        "LeadLagCrossCorrelation" => Ok(Box::new(PairInFields {
            inner: map_new(
                kind,
                wc::LeadLagCrossCorrelation::new(
                    usize_param(params, 0, kind)?,
                    usize_param(params, 1, kind)?,
                ),
            )?,
            last: None,
            reference: pair_reference(kind, reference)?.to_string(),
        })),
        "LinRegAngle" => Ok(Box::new(ScalarPrice {
            inner: map_new(kind, wc::LinRegAngle::new(usize_param(params, 0, kind)?))?,
        })),
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
        "LinRegIntercept" => Ok(Box::new(ScalarPrice {
            inner: map_new(
                kind,
                wc::LinRegIntercept::new(usize_param(params, 0, kind)?),
            )?,
        })),
        "LinRegSlope" => Ok(Box::new(ScalarPrice {
            inner: map_new(kind, wc::LinRegSlope::new(usize_param(params, 0, kind)?))?,
        })),
        "LinearRegression" => Ok(Box::new(ScalarPrice {
            inner: map_new(
                kind,
                wc::LinearRegression::new(usize_param(params, 0, kind)?),
            )?,
        })),
        "LiquidationFeatures" => Ok(Box::new(DerivInFields {
            inner: wc::LiquidationFeatures::new(),
            last: None,
        })),
        "LogReturn" => Ok(Box::new(ScalarPrice {
            inner: map_new(kind, wc::LogReturn::new(usize_param(params, 0, kind)?))?,
        })),
        "LongLeggedDoji" => Ok(Box::new(CandleIn {
            inner: wc::LongLeggedDoji::new(),
        })),
        "LongLine" => Ok(Box::new(CandleIn {
            inner: wc::LongLine::new(),
        })),
        "LongShortRatio" => Ok(Box::new(DerivIn {
            inner: wc::LongShortRatio::new(),
        })),
        "M2Measure" => Ok(Box::new(ScalarPrice {
            inner: map_new(
                kind,
                wc::M2Measure::new(
                    usize_param(params, 0, kind)?,
                    float_param(params, 1, kind)?,
                    float_param(params, 2, kind)?,
                ),
            )?,
        })),
        "MaEnvelope" => Ok(Box::new(ScalarPriceFields {
            inner: map_new(
                kind,
                wc::MaEnvelope::new(usize_param(params, 0, kind)?, float_param(params, 1, kind)?),
            )?,
            last: None,
        })),
        "MacdExt" => Ok(Box::new(ScalarPriceFields {
            inner: map_new(
                kind,
                wc::MacdExt::new(
                    usize_param(params, 0, kind)?,
                    map_new(kind, wc::MaType::from_code(u32_param(params, 1, kind)?))?,
                    usize_param(params, 2, kind)?,
                    map_new(kind, wc::MaType::from_code(u32_param(params, 3, kind)?))?,
                    usize_param(params, 4, kind)?,
                    map_new(kind, wc::MaType::from_code(u32_param(params, 5, kind)?))?,
                ),
            )?,
            last: None,
        })),
        "MacdFix" => Ok(Box::new(ScalarPriceFields {
            inner: map_new(kind, wc::MacdFix::new(usize_param(params, 0, kind)?))?,
            last: None,
        })),
        "MacdHistogram" => Ok(Box::new(ScalarPrice {
            inner: map_new(
                kind,
                wc::MacdHistogram::new(
                    usize_param(params, 0, kind)?,
                    usize_param(params, 1, kind)?,
                    usize_param(params, 2, kind)?,
                ),
            )?,
        })),
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
        "MarketFacilitationIndex" => Ok(Box::new(CandleIn {
            inner: wc::MarketFacilitationIndex::new(),
        })),
        "MartinRatio" => Ok(Box::new(ScalarPrice {
            inner: map_new(kind, wc::MartinRatio::new(usize_param(params, 0, kind)?))?,
        })),
        "Marubozu" => Ok(Box::new(CandleIn {
            inner: wc::Marubozu::new(),
        })),
        "MassIndex" => Ok(Box::new(CandleIn {
            inner: map_new(
                kind,
                wc::MassIndex::new(usize_param(params, 0, kind)?, usize_param(params, 1, kind)?),
            )?,
        })),
        "MatHold" => Ok(Box::new(CandleIn {
            inner: wc::MatHold::new(),
        })),
        "MatchingLow" => Ok(Box::new(CandleIn {
            inner: wc::MatchingLow::new(),
        })),
        "MaxDrawdown" => Ok(Box::new(ScalarPrice {
            inner: map_new(kind, wc::MaxDrawdown::new(usize_param(params, 0, kind)?))?,
        })),
        "McClellanOscillator" => Ok(Box::new(CrossIn {
            inner: wc::McClellanOscillator::new(),
        })),
        "McClellanSummationIndex" => Ok(Box::new(CrossIn {
            inner: wc::McClellanSummationIndex::new(),
        })),
        "McGinleyDynamic" => Ok(Box::new(ScalarPrice {
            inner: map_new(
                kind,
                wc::McGinleyDynamic::new(usize_param(params, 0, kind)?),
            )?,
        })),
        "MedianAbsoluteDeviation" => Ok(Box::new(ScalarPrice {
            inner: map_new(
                kind,
                wc::MedianAbsoluteDeviation::new(usize_param(params, 0, kind)?),
            )?,
        })),
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
        "MedianMa" => Ok(Box::new(ScalarPrice {
            inner: map_new(kind, wc::MedianMa::new(usize_param(params, 0, kind)?))?,
        })),
        "MedianPrice" => Ok(Box::new(CandleIn {
            inner: wc::MedianPrice::new(),
        })),
        "Mfi" => Ok(Box::new(CandleIn {
            inner: map_new(kind, wc::Mfi::new(usize_param(params, 0, kind)?))?,
        })),
        "Microprice" => Ok(Box::new(BookIn {
            inner: wc::Microprice::new(),
        })),
        "MidPoint" => Ok(Box::new(ScalarPrice {
            inner: map_new(kind, wc::MidPoint::new(usize_param(params, 0, kind)?))?,
        })),
        "MidPrice" => Ok(Box::new(CandleIn {
            inner: map_new(kind, wc::MidPrice::new(usize_param(params, 0, kind)?))?,
        })),
        "MinusDi" => Ok(Box::new(CandleIn {
            inner: map_new(kind, wc::MinusDi::new(usize_param(params, 0, kind)?))?,
        })),
        "MinusDm" => Ok(Box::new(CandleIn {
            inner: map_new(kind, wc::MinusDm::new(usize_param(params, 0, kind)?))?,
        })),
        "ModifiedMaStop" => Ok(Box::new(CandleInFields {
            inner: map_new(kind, wc::ModifiedMaStop::new(usize_param(params, 0, kind)?))?,
            last: None,
        })),
        "Mom" => Ok(Box::new(ScalarPrice {
            inner: map_new(kind, wc::Mom::new(usize_param(params, 0, kind)?))?,
        })),
        "MorningDojiStar" => Ok(Box::new(CandleIn {
            inner: wc::MorningDojiStar::new(),
        })),
        "MorningEveningStar" => Ok(Box::new(CandleIn {
            inner: wc::MorningEveningStar::new(),
        })),
        "MurreyMathLines" => Ok(Box::new(CandleInFields {
            inner: map_new(
                kind,
                wc::MurreyMathLines::new(usize_param(params, 0, kind)?),
            )?,
            last: None,
        })),
        "NakedPoc" => Ok(Box::new(CandleIn {
            inner: map_new(
                kind,
                wc::NakedPoc::new(usize_param(params, 0, kind)?, usize_param(params, 1, kind)?),
            )?,
        })),
        "Natr" => Ok(Box::new(CandleIn {
            inner: map_new(kind, wc::Natr::new(usize_param(params, 0, kind)?))?,
        })),
        "NewHighsNewLows" => Ok(Box::new(CrossIn {
            inner: wc::NewHighsNewLows::new(),
        })),
        "NewPriceLines" => Ok(Box::new(CandleIn {
            inner: map_new(kind, wc::NewPriceLines::new(usize_param(params, 0, kind)?))?,
        })),
        "Nrtr" => Ok(Box::new(CandleInFields {
            inner: map_new(kind, wc::Nrtr::new(float_param(params, 0, kind)?))?,
            last: None,
        })),
        "Nvi" => Ok(Box::new(CandleIn {
            inner: wc::Nvi::new(),
        })),
        "OIPriceDivergence" => Ok(Box::new(DerivIn {
            inner: map_new(
                kind,
                wc::OIPriceDivergence::new(usize_param(params, 0, kind)?),
            )?,
        })),
        "OIWeighted" => Ok(Box::new(DerivIn {
            inner: wc::OIWeighted::new(),
        })),
        "Obv" => Ok(Box::new(CandleIn {
            inner: wc::Obv::new(),
        })),
        "OiToVolumeRatio" => Ok(Box::new(DerivIn {
            inner: wc::OiToVolumeRatio::new(),
        })),
        "OmegaRatio" => Ok(Box::new(ReturnsIn {
            inner: map_new(
                kind,
                wc::OmegaRatio::new(usize_param(params, 0, kind)?, float_param(params, 1, kind)?),
            )?,
            previous_close: None,
        })),
        "OnNeck" => Ok(Box::new(CandleIn {
            inner: wc::OnNeck::new(),
        })),
        "OpenInterestDelta" => Ok(Box::new(DerivIn {
            inner: wc::OpenInterestDelta::new(),
        })),
        "OpenInterestMomentum" => Ok(Box::new(DerivIn {
            inner: map_new(
                kind,
                wc::OpenInterestMomentum::new(usize_param(params, 0, kind)?),
            )?,
        })),
        "OpeningMarubozu" => Ok(Box::new(CandleIn {
            inner: wc::OpeningMarubozu::new(),
        })),
        "OpeningRange" => Ok(Box::new(CandleInFields {
            inner: map_new(kind, wc::OpeningRange::new(usize_param(params, 0, kind)?))?,
            last: None,
        })),
        "OrderBookImbalanceFull" => Ok(Box::new(BookIn {
            inner: wc::OrderBookImbalanceFull::new(),
        })),
        "OrderBookImbalanceTop1" => Ok(Box::new(BookIn {
            inner: wc::OrderBookImbalanceTop1::new(),
        })),
        "OrderBookImbalanceTopN" => Ok(Box::new(BookIn {
            inner: map_new(
                kind,
                wc::OrderBookImbalanceTopN::new(usize_param(params, 0, kind)?),
            )?,
        })),
        "OrderFlowImbalance" => Ok(Box::new(BookIn {
            inner: map_new(
                kind,
                wc::OrderFlowImbalance::new(usize_param(params, 0, kind)?),
            )?,
        })),
        "OuHalfLife" => Ok(Box::new(PairIn {
            inner: map_new(kind, wc::OuHalfLife::new(usize_param(params, 0, kind)?))?,
            reference: pair_reference(kind, reference)?.to_string(),
        })),
        "OvernightGap" => Ok(Box::new(CandleIn {
            inner: wc::OvernightGap::new(i32_param(params, 0, kind)?),
        })),
        "OvernightIntradayReturn" => Ok(Box::new(CandleInFields {
            inner: wc::OvernightIntradayReturn::new(i32_param(params, 0, kind)?),
            last: None,
        })),
        "PainIndex" => Ok(Box::new(ScalarPrice {
            inner: map_new(kind, wc::PainIndex::new(usize_param(params, 0, kind)?))?,
        })),
        "PairSpreadZScore" => Ok(Box::new(PairIn {
            inner: map_new(
                kind,
                wc::PairSpreadZScore::new(
                    usize_param(params, 0, kind)?,
                    usize_param(params, 1, kind)?,
                ),
            )?,
            reference: pair_reference(kind, reference)?.to_string(),
        })),
        "PairwiseBeta" => Ok(Box::new(PairIn {
            inner: map_new(kind, wc::PairwiseBeta::new(usize_param(params, 0, kind)?))?,
            reference: pair_reference(kind, reference)?.to_string(),
        })),
        "ParkinsonVolatility" => Ok(Box::new(CandleIn {
            inner: map_new(
                kind,
                wc::ParkinsonVolatility::new(
                    usize_param(params, 0, kind)?,
                    usize_param(params, 1, kind)?,
                ),
            )?,
        })),
        "PearsonCorrelation" => Ok(Box::new(PairIn {
            inner: map_new(
                kind,
                wc::PearsonCorrelation::new(usize_param(params, 0, kind)?),
            )?,
            reference: pair_reference(kind, reference)?.to_string(),
        })),
        "PercentAboveMa" => Ok(Box::new(CrossIn {
            inner: wc::PercentAboveMa::new(),
        })),
        "PercentB" => Ok(Box::new(ScalarPrice {
            inner: map_new(
                kind,
                wc::PercentB::new(usize_param(params, 0, kind)?, float_param(params, 1, kind)?),
            )?,
        })),
        "PercentageTrailingStop" => Ok(Box::new(ScalarPrice {
            inner: map_new(
                kind,
                wc::PercentageTrailingStop::new(float_param(params, 0, kind)?),
            )?,
        })),
        "PerpetualPremiumIndex" => Ok(Box::new(DerivIn {
            inner: wc::PerpetualPremiumIndex::new(),
        })),
        "Pgo" => Ok(Box::new(CandleIn {
            inner: map_new(kind, wc::Pgo::new(usize_param(params, 0, kind)?))?,
        })),
        "PiercingDarkCloud" => Ok(Box::new(CandleIn {
            inner: wc::PiercingDarkCloud::new(),
        })),
        "Pin" => Ok(Box::new(TradeIn {
            inner: map_new(kind, wc::Pin::new(usize_param(params, 0, kind)?))?,
        })),
        "PivotReversal" => Ok(Box::new(CandleIn {
            inner: map_new(
                kind,
                wc::PivotReversal::new(
                    usize_param(params, 0, kind)?,
                    usize_param(params, 1, kind)?,
                ),
            )?,
        })),
        "PlusDi" => Ok(Box::new(CandleIn {
            inner: map_new(kind, wc::PlusDi::new(usize_param(params, 0, kind)?))?,
        })),
        "PlusDm" => Ok(Box::new(CandleIn {
            inner: map_new(kind, wc::PlusDm::new(usize_param(params, 0, kind)?))?,
        })),
        "Pmo" => Ok(Box::new(ScalarPrice {
            inner: map_new(
                kind,
                wc::Pmo::new(usize_param(params, 0, kind)?, usize_param(params, 1, kind)?),
            )?,
        })),
        "PolarizedFractalEfficiency" => Ok(Box::new(ScalarPrice {
            inner: map_new(
                kind,
                wc::PolarizedFractalEfficiency::new(
                    usize_param(params, 0, kind)?,
                    usize_param(params, 1, kind)?,
                ),
            )?,
        })),
        "Ppo" => Ok(Box::new(ScalarPrice {
            inner: map_new(
                kind,
                wc::Ppo::new(usize_param(params, 0, kind)?, usize_param(params, 1, kind)?),
            )?,
        })),
        "PpoHistogram" => Ok(Box::new(ScalarPrice {
            inner: map_new(
                kind,
                wc::PpoHistogram::new(
                    usize_param(params, 0, kind)?,
                    usize_param(params, 1, kind)?,
                    usize_param(params, 2, kind)?,
                ),
            )?,
        })),
        "ProfileShape" => Ok(Box::new(CandleIn {
            inner: map_new(
                kind,
                wc::ProfileShape::new(usize_param(params, 0, kind)?, usize_param(params, 1, kind)?),
            )?,
        })),
        "ProfitFactor" => Ok(Box::new(ReturnsIn {
            inner: map_new(kind, wc::ProfitFactor::new(usize_param(params, 0, kind)?))?,
            previous_close: None,
        })),
        "ProjectionBands" => Ok(Box::new(CandleInFields {
            inner: map_new(
                kind,
                wc::ProjectionBands::new(usize_param(params, 0, kind)?),
            )?,
            last: None,
        })),
        "ProjectionOscillator" => Ok(Box::new(CandleIn {
            inner: map_new(
                kind,
                wc::ProjectionOscillator::new(usize_param(params, 0, kind)?),
            )?,
        })),
        "Psar" => Ok(Box::new(CandleIn {
            inner: map_new(
                kind,
                wc::Psar::new(
                    float_param(params, 0, kind)?,
                    float_param(params, 1, kind)?,
                    float_param(params, 2, kind)?,
                ),
            )?,
        })),
        "Pvi" => Ok(Box::new(CandleIn {
            inner: wc::Pvi::new(),
        })),
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
        "Qstick" => Ok(Box::new(CandleIn {
            inner: map_new(kind, wc::Qstick::new(usize_param(params, 0, kind)?))?,
        })),
        "QuartileBands" => Ok(Box::new(ScalarPriceFields {
            inner: map_new(kind, wc::QuartileBands::new(usize_param(params, 0, kind)?))?,
            last: None,
        })),
        "QuotedSpread" => Ok(Box::new(BookIn {
            inner: wc::QuotedSpread::new(),
        })),
        "RSquared" => Ok(Box::new(ScalarPrice {
            inner: map_new(kind, wc::RSquared::new(usize_param(params, 0, kind)?))?,
        })),
        "RealizedSpread" => Ok(Box::new(QuoteIn {
            inner: map_new(kind, wc::RealizedSpread::new(usize_param(params, 0, kind)?))?,
        })),
        "RealizedVolatility" => Ok(Box::new(ScalarPrice {
            inner: map_new(
                kind,
                wc::RealizedVolatility::new(usize_param(params, 0, kind)?),
            )?,
        })),
        "RecoveryFactor" => Ok(Box::new(ScalarPrice {
            inner: wc::RecoveryFactor::new(),
        })),
        "RectangleRange" => Ok(Box::new(CandleIn {
            inner: wc::RectangleRange::new(),
        })),
        "Reflex" => Ok(Box::new(ScalarPrice {
            inner: map_new(kind, wc::Reflex::new(usize_param(params, 0, kind)?))?,
        })),
        "RegimeLabel" => Ok(Box::new(ScalarPrice {
            inner: map_new(
                kind,
                wc::RegimeLabel::new(usize_param(params, 0, kind)?, usize_param(params, 1, kind)?),
            )?,
        })),
        "RelativeStrengthAB" => Ok(Box::new(PairInFields {
            inner: map_new(
                kind,
                wc::RelativeStrengthAB::new(
                    usize_param(params, 0, kind)?,
                    usize_param(params, 1, kind)?,
                ),
            )?,
            last: None,
            reference: pair_reference(kind, reference)?.to_string(),
        })),
        "RenkoTrailingStop" => Ok(Box::new(ScalarPrice {
            inner: map_new(
                kind,
                wc::RenkoTrailingStop::new(float_param(params, 0, kind)?),
            )?,
        })),
        "RickshawMan" => Ok(Box::new(CandleIn {
            inner: wc::RickshawMan::new(),
        })),
        "RisingThreeMethods" => Ok(Box::new(CandleIn {
            inner: wc::RisingThreeMethods::new(),
        })),
        "Rmi" => Ok(Box::new(ScalarPrice {
            inner: map_new(
                kind,
                wc::Rmi::new(usize_param(params, 0, kind)?, usize_param(params, 1, kind)?),
            )?,
        })),
        "Roc" => Ok(Box::new(ScalarPrice {
            inner: map_new(kind, wc::Roc::new(usize_param(params, 0, kind)?))?,
        })),
        "Rocp" => Ok(Box::new(ScalarPrice {
            inner: map_new(kind, wc::Rocp::new(usize_param(params, 0, kind)?))?,
        })),
        "Rocr" => Ok(Box::new(ScalarPrice {
            inner: map_new(kind, wc::Rocr::new(usize_param(params, 0, kind)?))?,
        })),
        "Rocr100" => Ok(Box::new(ScalarPrice {
            inner: map_new(kind, wc::Rocr100::new(usize_param(params, 0, kind)?))?,
        })),
        "RogersSatchellVolatility" => Ok(Box::new(CandleIn {
            inner: map_new(
                kind,
                wc::RogersSatchellVolatility::new(
                    usize_param(params, 0, kind)?,
                    usize_param(params, 1, kind)?,
                ),
            )?,
        })),
        "RollMeasure" => Ok(Box::new(TradeIn {
            inner: map_new(kind, wc::RollMeasure::new(usize_param(params, 0, kind)?))?,
        })),
        "RollingCorrelation" => Ok(Box::new(PairIn {
            inner: map_new(
                kind,
                wc::RollingCorrelation::new(usize_param(params, 0, kind)?),
            )?,
            reference: pair_reference(kind, reference)?.to_string(),
        })),
        "RollingCovariance" => Ok(Box::new(PairIn {
            inner: map_new(
                kind,
                wc::RollingCovariance::new(usize_param(params, 0, kind)?),
            )?,
            reference: pair_reference(kind, reference)?.to_string(),
        })),
        "RollingIqr" => Ok(Box::new(ScalarPrice {
            inner: map_new(kind, wc::RollingIqr::new(usize_param(params, 0, kind)?))?,
        })),
        "RollingMinMaxScaler" => Ok(Box::new(ScalarPrice {
            inner: map_new(
                kind,
                wc::RollingMinMaxScaler::new(usize_param(params, 0, kind)?),
            )?,
        })),
        "RollingPercentileRank" => Ok(Box::new(ScalarPrice {
            inner: map_new(
                kind,
                wc::RollingPercentileRank::new(usize_param(params, 0, kind)?),
            )?,
        })),
        "RollingQuantile" => Ok(Box::new(ScalarPrice {
            inner: map_new(
                kind,
                wc::RollingQuantile::new(
                    usize_param(params, 0, kind)?,
                    float_param(params, 1, kind)?,
                ),
            )?,
        })),
        "RollingVwap" => Ok(Box::new(CandleIn {
            inner: map_new(kind, wc::RollingVwap::new(usize_param(params, 0, kind)?))?,
        })),
        "RoofingFilter" => Ok(Box::new(ScalarPrice {
            inner: map_new(
                kind,
                wc::RoofingFilter::new(
                    usize_param(params, 0, kind)?,
                    usize_param(params, 1, kind)?,
                ),
            )?,
        })),
        "Rsi" => Ok(Box::new(ScalarPrice {
            inner: map_new(kind, wc::Rsi::new(usize_param(params, 0, kind)?))?,
        })),
        "Rsx" => Ok(Box::new(ScalarPrice {
            inner: map_new(kind, wc::Rsx::new(usize_param(params, 0, kind)?))?,
        })),
        "Rvi" => Ok(Box::new(CandleIn {
            inner: map_new(kind, wc::Rvi::new(usize_param(params, 0, kind)?))?,
        })),
        "RviVolatility" => Ok(Box::new(ScalarPrice {
            inner: map_new(kind, wc::RviVolatility::new(usize_param(params, 0, kind)?))?,
        })),
        "Rwi" => Ok(Box::new(CandleInFields {
            inner: map_new(kind, wc::Rwi::new(usize_param(params, 0, kind)?))?,
            last: None,
        })),
        "SampleEntropy" => Ok(Box::new(ScalarPrice {
            inner: map_new(
                kind,
                wc::SampleEntropy::new(
                    usize_param(params, 0, kind)?,
                    usize_param(params, 1, kind)?,
                    float_param(params, 2, kind)?,
                ),
            )?,
        })),
        "SarExt" => Ok(Box::new(CandleIn {
            inner: map_new(
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
            )?,
        })),
        "SeasonalZScore" => Ok(Box::new(CandleIn {
            inner: wc::SeasonalZScore::new(i32_param(params, 0, kind)?),
        })),
        "SeparatingLines" => Ok(Box::new(CandleIn {
            inner: wc::SeparatingLines::new(),
        })),
        "SessionHighLow" => Ok(Box::new(CandleInFields {
            inner: wc::SessionHighLow::new(i32_param(params, 0, kind)?),
            last: None,
        })),
        "SessionRange" => Ok(Box::new(CandleInFields {
            inner: wc::SessionRange::new(i32_param(params, 0, kind)?),
            last: None,
        })),
        "SessionVwap" => Ok(Box::new(CandleIn {
            inner: wc::SessionVwap::new(i32_param(params, 0, kind)?),
        })),
        "ShannonEntropy" => Ok(Box::new(ScalarPrice {
            inner: map_new(
                kind,
                wc::ShannonEntropy::new(
                    usize_param(params, 0, kind)?,
                    usize_param(params, 1, kind)?,
                ),
            )?,
        })),
        "Shark" => Ok(Box::new(CandleIn {
            inner: wc::Shark::new(),
        })),
        "SharpeRatio" => Ok(Box::new(ScalarPrice {
            inner: map_new(
                kind,
                wc::SharpeRatio::new(usize_param(params, 0, kind)?, float_param(params, 1, kind)?),
            )?,
        })),
        "ShootingStar" => Ok(Box::new(CandleIn {
            inner: wc::ShootingStar::new(),
        })),
        "ShortLine" => Ok(Box::new(CandleIn {
            inner: wc::ShortLine::new(),
        })),
        "SignedVolume" => Ok(Box::new(TradeIn {
            inner: wc::SignedVolume::new(),
        })),
        "SineWave" => Ok(Box::new(ScalarPrice {
            inner: wc::SineWave::new(),
        })),
        "SineWeightedMa" => Ok(Box::new(ScalarPrice {
            inner: map_new(kind, wc::SineWeightedMa::new(usize_param(params, 0, kind)?))?,
        })),
        "SinglePrints" => Ok(Box::new(CandleIn {
            inner: map_new(
                kind,
                wc::SinglePrints::new(usize_param(params, 0, kind)?, usize_param(params, 1, kind)?),
            )?,
        })),
        "Skewness" => Ok(Box::new(ScalarPrice {
            inner: map_new(kind, wc::Skewness::new(usize_param(params, 0, kind)?))?,
        })),
        "Sma" => Ok(Box::new(ScalarPrice {
            inner: map_new(kind, wc::Sma::new(usize_param(params, 0, kind)?))?,
        })),
        "Smi" => Ok(Box::new(CandleIn {
            inner: map_new(
                kind,
                wc::Smi::new(
                    usize_param(params, 0, kind)?,
                    usize_param(params, 1, kind)?,
                    usize_param(params, 2, kind)?,
                ),
            )?,
        })),
        "Smma" => Ok(Box::new(ScalarPrice {
            inner: map_new(kind, wc::Smma::new(usize_param(params, 0, kind)?))?,
        })),
        "SmoothedHeikinAshi" => Ok(Box::new(CandleInFields {
            inner: map_new(
                kind,
                wc::SmoothedHeikinAshi::new(usize_param(params, 0, kind)?),
            )?,
            last: None,
        })),
        "SortinoRatio" => Ok(Box::new(ScalarPrice {
            inner: map_new(
                kind,
                wc::SortinoRatio::new(usize_param(params, 0, kind)?, float_param(params, 1, kind)?),
            )?,
        })),
        "SpearmanCorrelation" => Ok(Box::new(PairIn {
            inner: map_new(
                kind,
                wc::SpearmanCorrelation::new(usize_param(params, 0, kind)?),
            )?,
            reference: pair_reference(kind, reference)?.to_string(),
        })),
        "SpinningTop" => Ok(Box::new(CandleIn {
            inner: wc::SpinningTop::new(),
        })),
        "SpreadAr1Coefficient" => Ok(Box::new(PairIn {
            inner: map_new(
                kind,
                wc::SpreadAr1Coefficient::new(usize_param(params, 0, kind)?),
            )?,
            reference: pair_reference(kind, reference)?.to_string(),
        })),
        "SpreadBollingerBands" => Ok(Box::new(PairInFields {
            inner: map_new(
                kind,
                wc::SpreadBollingerBands::new(
                    usize_param(params, 0, kind)?,
                    float_param(params, 1, kind)?,
                ),
            )?,
            last: None,
            reference: pair_reference(kind, reference)?.to_string(),
        })),
        "SpreadHurst" => Ok(Box::new(PairIn {
            inner: map_new(kind, wc::SpreadHurst::new(usize_param(params, 0, kind)?))?,
            reference: pair_reference(kind, reference)?.to_string(),
        })),
        "StalledPattern" => Ok(Box::new(CandleIn {
            inner: wc::StalledPattern::new(),
        })),
        "StandardError" => Ok(Box::new(ScalarPrice {
            inner: map_new(kind, wc::StandardError::new(usize_param(params, 0, kind)?))?,
        })),
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
        "Stc" => Ok(Box::new(ScalarPrice {
            inner: map_new(
                kind,
                wc::Stc::new(
                    usize_param(params, 0, kind)?,
                    usize_param(params, 1, kind)?,
                    usize_param(params, 2, kind)?,
                    float_param(params, 3, kind)?,
                ),
            )?,
        })),
        "StdDev" => Ok(Box::new(ScalarPrice {
            inner: map_new(kind, wc::StdDev::new(usize_param(params, 0, kind)?))?,
        })),
        "StepTrailingStop" => Ok(Box::new(ScalarPrice {
            inner: map_new(
                kind,
                wc::StepTrailingStop::new(float_param(params, 0, kind)?),
            )?,
        })),
        "SterlingRatio" => Ok(Box::new(ScalarPrice {
            inner: map_new(kind, wc::SterlingRatio::new(usize_param(params, 0, kind)?))?,
        })),
        "StickSandwich" => Ok(Box::new(CandleIn {
            inner: wc::StickSandwich::new(),
        })),
        "StochRsi" => Ok(Box::new(ScalarPrice {
            inner: map_new(
                kind,
                wc::StochRsi::new(usize_param(params, 0, kind)?, usize_param(params, 1, kind)?),
            )?,
        })),
        "Stochastic" => Ok(Box::new(CandleInFields {
            inner: map_new(
                kind,
                wc::Stochastic::new(usize_param(params, 0, kind)?, usize_param(params, 1, kind)?),
            )?,
            last: None,
        })),
        "StochasticCci" => Ok(Box::new(CandleIn {
            inner: map_new(kind, wc::StochasticCci::new(usize_param(params, 0, kind)?))?,
        })),
        "SuperSmoother" => Ok(Box::new(ScalarPrice {
            inner: map_new(kind, wc::SuperSmoother::new(usize_param(params, 0, kind)?))?,
        })),
        "SuperTrend" => Ok(Box::new(CandleInFields {
            inner: map_new(
                kind,
                wc::SuperTrend::new(usize_param(params, 0, kind)?, float_param(params, 1, kind)?),
            )?,
            last: None,
        })),
        "T3" => Ok(Box::new(ScalarPrice {
            inner: map_new(
                kind,
                wc::T3::new(usize_param(params, 0, kind)?, float_param(params, 1, kind)?),
            )?,
        })),
        "TailRatio" => Ok(Box::new(ScalarPrice {
            inner: map_new(kind, wc::TailRatio::new(usize_param(params, 0, kind)?))?,
        })),
        "TakerBuySellRatio" => Ok(Box::new(DerivIn {
            inner: wc::TakerBuySellRatio::new(),
        })),
        "Takuri" => Ok(Box::new(CandleIn {
            inner: wc::Takuri::new(),
        })),
        "TasukiGap" => Ok(Box::new(CandleIn {
            inner: wc::TasukiGap::new(),
        })),
        "TdCamouflage" => Ok(Box::new(CandleIn {
            inner: wc::TdCamouflage::new(),
        })),
        "TdClop" => Ok(Box::new(CandleIn {
            inner: wc::TdClop::new(),
        })),
        "TdClopwin" => Ok(Box::new(CandleIn {
            inner: wc::TdClopwin::new(),
        })),
        "TdCombo" => Ok(Box::new(CandleIn {
            inner: map_new(
                kind,
                wc::TdCombo::new(
                    usize_param(params, 0, kind)?,
                    usize_param(params, 1, kind)?,
                    usize_param(params, 2, kind)?,
                    usize_param(params, 3, kind)?,
                ),
            )?,
        })),
        "TdCountdown" => Ok(Box::new(CandleIn {
            inner: map_new(
                kind,
                wc::TdCountdown::new(
                    usize_param(params, 0, kind)?,
                    usize_param(params, 1, kind)?,
                    usize_param(params, 2, kind)?,
                    usize_param(params, 3, kind)?,
                ),
            )?,
        })),
        "TdDWave" => Ok(Box::new(CandleIn {
            inner: map_new(kind, wc::TdDWave::new(usize_param(params, 0, kind)?))?,
        })),
        "TdDeMarker" => Ok(Box::new(CandleIn {
            inner: map_new(kind, wc::TdDeMarker::new(usize_param(params, 0, kind)?))?,
        })),
        "TdDifferential" => Ok(Box::new(CandleIn {
            inner: wc::TdDifferential::new(),
        })),
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
        "TdOpen" => Ok(Box::new(CandleIn {
            inner: wc::TdOpen::new(),
        })),
        "TdPressure" => Ok(Box::new(CandleIn {
            inner: map_new(kind, wc::TdPressure::new(usize_param(params, 0, kind)?))?,
        })),
        "TdPropulsion" => Ok(Box::new(CandleIn {
            inner: wc::TdPropulsion::new(),
        })),
        "TdRangeProjection" => Ok(Box::new(CandleInFields {
            inner: wc::TdRangeProjection::new(),
            last: None,
        })),
        "TdRei" => Ok(Box::new(CandleIn {
            inner: map_new(kind, wc::TdRei::new(usize_param(params, 0, kind)?))?,
        })),
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
        "TdSetup" => Ok(Box::new(CandleIn {
            inner: map_new(
                kind,
                wc::TdSetup::new(usize_param(params, 0, kind)?, usize_param(params, 1, kind)?),
            )?,
        })),
        "TdTrap" => Ok(Box::new(CandleIn {
            inner: wc::TdTrap::new(),
        })),
        "Tema" => Ok(Box::new(ScalarPrice {
            inner: map_new(kind, wc::Tema::new(usize_param(params, 0, kind)?))?,
        })),
        "TermStructureBasis" => Ok(Box::new(DerivIn {
            inner: wc::TermStructureBasis::new(),
        })),
        "ThreeDrives" => Ok(Box::new(CandleIn {
            inner: wc::ThreeDrives::new(),
        })),
        "ThreeInside" => Ok(Box::new(CandleIn {
            inner: wc::ThreeInside::new(),
        })),
        "ThreeLineBreak" => Ok(Box::new(CandleIn {
            inner: map_new(kind, wc::ThreeLineBreak::new(usize_param(params, 0, kind)?))?,
        })),
        "ThreeLineStrike" => Ok(Box::new(CandleIn {
            inner: wc::ThreeLineStrike::new(),
        })),
        "ThreeOutside" => Ok(Box::new(CandleIn {
            inner: wc::ThreeOutside::new(),
        })),
        "ThreeSoldiersOrCrows" => Ok(Box::new(CandleIn {
            inner: wc::ThreeSoldiersOrCrows::new(),
        })),
        "ThreeStarsInSouth" => Ok(Box::new(CandleIn {
            inner: wc::ThreeStarsInSouth::new(),
        })),
        "Thrusting" => Ok(Box::new(CandleIn {
            inner: wc::Thrusting::new(),
        })),
        "TickIndex" => Ok(Box::new(CrossIn {
            inner: wc::TickIndex::new(),
        })),
        "Tii" => Ok(Box::new(ScalarPrice {
            inner: map_new(
                kind,
                wc::Tii::new(usize_param(params, 0, kind)?, usize_param(params, 1, kind)?),
            )?,
        })),
        "TimeBasedStop" => Ok(Box::new(CandleIn {
            inner: map_new(kind, wc::TimeBasedStop::new(usize_param(params, 0, kind)?))?,
        })),
        "TowerTopBottom" => Ok(Box::new(CandleIn {
            inner: wc::TowerTopBottom::new(),
        })),
        "TradeImbalance" => Ok(Box::new(TradeIn {
            inner: map_new(kind, wc::TradeImbalance::new(usize_param(params, 0, kind)?))?,
        })),
        "TradeSignAutocorrelation" => Ok(Box::new(TradeIn {
            inner: map_new(
                kind,
                wc::TradeSignAutocorrelation::new(usize_param(params, 0, kind)?),
            )?,
        })),
        "TradeVolumeIndex" => Ok(Box::new(CandleIn {
            inner: map_new(
                kind,
                wc::TradeVolumeIndex::new(float_param(params, 0, kind)?),
            )?,
        })),
        "TrendLabel" => Ok(Box::new(ScalarPrice {
            inner: map_new(kind, wc::TrendLabel::new(usize_param(params, 0, kind)?))?,
        })),
        "TrendStrengthIndex" => Ok(Box::new(ScalarPrice {
            inner: map_new(
                kind,
                wc::TrendStrengthIndex::new(usize_param(params, 0, kind)?),
            )?,
        })),
        "Trendflex" => Ok(Box::new(ScalarPrice {
            inner: map_new(kind, wc::Trendflex::new(usize_param(params, 0, kind)?))?,
        })),
        "TreynorRatio" => Ok(Box::new(PairIn {
            inner: map_new(
                kind,
                wc::TreynorRatio::new(usize_param(params, 0, kind)?, float_param(params, 1, kind)?),
            )?,
            reference: pair_reference(kind, reference)?.to_string(),
        })),
        "Triangle" => Ok(Box::new(CandleIn {
            inner: wc::Triangle::new(),
        })),
        "Trima" => Ok(Box::new(ScalarPrice {
            inner: map_new(kind, wc::Trima::new(usize_param(params, 0, kind)?))?,
        })),
        "Trin" => Ok(Box::new(CrossIn {
            inner: wc::Trin::new(),
        })),
        "TripleTopBottom" => Ok(Box::new(CandleIn {
            inner: wc::TripleTopBottom::new(),
        })),
        "Tristar" => Ok(Box::new(CandleIn {
            inner: wc::Tristar::new(),
        })),
        "Trix" => Ok(Box::new(ScalarPrice {
            inner: map_new(kind, wc::Trix::new(usize_param(params, 0, kind)?))?,
        })),
        "TrueRange" => Ok(Box::new(CandleIn {
            inner: wc::TrueRange::new(),
        })),
        "Tsf" => Ok(Box::new(ScalarPrice {
            inner: map_new(kind, wc::Tsf::new(usize_param(params, 0, kind)?))?,
        })),
        "TsfOscillator" => Ok(Box::new(ScalarPrice {
            inner: map_new(kind, wc::TsfOscillator::new(usize_param(params, 0, kind)?))?,
        })),
        "Tsi" => Ok(Box::new(ScalarPrice {
            inner: map_new(
                kind,
                wc::Tsi::new(usize_param(params, 0, kind)?, usize_param(params, 1, kind)?),
            )?,
        })),
        "Tsv" => Ok(Box::new(CandleIn {
            inner: map_new(kind, wc::Tsv::new(usize_param(params, 0, kind)?))?,
        })),
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
        "TtmTrend" => Ok(Box::new(CandleIn {
            inner: map_new(kind, wc::TtmTrend::new(usize_param(params, 0, kind)?))?,
        })),
        "TurnOfMonth" => Ok(Box::new(CandleIn {
            inner: map_new(
                kind,
                wc::TurnOfMonth::new(
                    u32_param(params, 0, kind)?,
                    u32_param(params, 1, kind)?,
                    i32_param(params, 2, kind)?,
                ),
            )?,
        })),
        "Tweezer" => Ok(Box::new(CandleIn {
            inner: wc::Tweezer::new(),
        })),
        "TwiggsMoneyFlow" => Ok(Box::new(CandleIn {
            inner: map_new(
                kind,
                wc::TwiggsMoneyFlow::new(usize_param(params, 0, kind)?),
            )?,
        })),
        "TwoCrows" => Ok(Box::new(CandleIn {
            inner: wc::TwoCrows::new(),
        })),
        "TypicalPrice" => Ok(Box::new(CandleIn {
            inner: wc::TypicalPrice::new(),
        })),
        "UlcerIndex" => Ok(Box::new(ScalarPrice {
            inner: map_new(kind, wc::UlcerIndex::new(usize_param(params, 0, kind)?))?,
        })),
        "UltimateOscillator" => Ok(Box::new(CandleIn {
            inner: map_new(
                kind,
                wc::UltimateOscillator::new(
                    usize_param(params, 0, kind)?,
                    usize_param(params, 1, kind)?,
                    usize_param(params, 2, kind)?,
                ),
            )?,
        })),
        "UniqueThreeRiver" => Ok(Box::new(CandleIn {
            inner: wc::UniqueThreeRiver::new(),
        })),
        "UniversalOscillator" => Ok(Box::new(ScalarPrice {
            inner: map_new(
                kind,
                wc::UniversalOscillator::new(usize_param(params, 0, kind)?),
            )?,
        })),
        "UpDownVolumeRatio" => Ok(Box::new(CrossIn {
            inner: wc::UpDownVolumeRatio::new(),
        })),
        "UpsideGapThreeMethods" => Ok(Box::new(CandleIn {
            inner: wc::UpsideGapThreeMethods::new(),
        })),
        "UpsideGapTwoCrows" => Ok(Box::new(CandleIn {
            inner: wc::UpsideGapTwoCrows::new(),
        })),
        "UpsidePotentialRatio" => Ok(Box::new(ScalarPrice {
            inner: map_new(
                kind,
                wc::UpsidePotentialRatio::new(
                    usize_param(params, 0, kind)?,
                    float_param(params, 1, kind)?,
                ),
            )?,
        })),
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
        "ValueAtRisk" => Ok(Box::new(ScalarPrice {
            inner: map_new(
                kind,
                wc::ValueAtRisk::new(usize_param(params, 0, kind)?, float_param(params, 1, kind)?),
            )?,
        })),
        "Variance" => Ok(Box::new(ScalarPrice {
            inner: map_new(kind, wc::Variance::new(usize_param(params, 0, kind)?))?,
        })),
        "VarianceRatio" => Ok(Box::new(PairIn {
            inner: map_new(
                kind,
                wc::VarianceRatio::new(
                    usize_param(params, 0, kind)?,
                    usize_param(params, 1, kind)?,
                ),
            )?,
            reference: pair_reference(kind, reference)?.to_string(),
        })),
        "VerticalHorizontalFilter" => Ok(Box::new(ScalarPrice {
            inner: map_new(
                kind,
                wc::VerticalHorizontalFilter::new(usize_param(params, 0, kind)?),
            )?,
        })),
        "Vidya" => Ok(Box::new(ScalarPrice {
            inner: map_new(
                kind,
                wc::Vidya::new(usize_param(params, 0, kind)?, usize_param(params, 1, kind)?),
            )?,
        })),
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
        "VolatilityOfVolatility" => Ok(Box::new(ScalarPrice {
            inner: map_new(
                kind,
                wc::VolatilityOfVolatility::new(
                    usize_param(params, 0, kind)?,
                    usize_param(params, 1, kind)?,
                ),
            )?,
        })),
        "VolatilityRatio" => Ok(Box::new(CandleIn {
            inner: map_new(
                kind,
                wc::VolatilityRatio::new(usize_param(params, 0, kind)?),
            )?,
        })),
        "VoltyStop" => Ok(Box::new(CandleIn {
            inner: map_new(
                kind,
                wc::VoltyStop::new(usize_param(params, 0, kind)?, float_param(params, 1, kind)?),
            )?,
        })),
        "VolumeOscillator" => Ok(Box::new(CandleIn {
            inner: map_new(
                kind,
                wc::VolumeOscillator::new(
                    usize_param(params, 0, kind)?,
                    usize_param(params, 1, kind)?,
                ),
            )?,
        })),
        "VolumePriceTrend" => Ok(Box::new(CandleIn {
            inner: wc::VolumePriceTrend::new(),
        })),
        "VolumeRsi" => Ok(Box::new(CandleIn {
            inner: map_new(kind, wc::VolumeRsi::new(usize_param(params, 0, kind)?))?,
        })),
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
        "Vpin" => Ok(Box::new(TradeIn {
            inner: map_new(
                kind,
                wc::Vpin::new(float_param(params, 0, kind)?, usize_param(params, 1, kind)?),
            )?,
        })),
        "Vwap" => Ok(Box::new(CandleIn {
            inner: wc::Vwap::new(),
        })),
        "VwapStdDevBands" => Ok(Box::new(CandleInFields {
            inner: map_new(
                kind,
                wc::VwapStdDevBands::new(float_param(params, 0, kind)?),
            )?,
            last: None,
        })),
        "Vwma" => Ok(Box::new(CandleIn {
            inner: map_new(kind, wc::Vwma::new(usize_param(params, 0, kind)?))?,
        })),
        "Vzo" => Ok(Box::new(CandleIn {
            inner: map_new(kind, wc::Vzo::new(usize_param(params, 0, kind)?))?,
        })),
        "Wad" => Ok(Box::new(CandleIn {
            inner: wc::Wad::new(),
        })),
        "WavePm" => Ok(Box::new(ScalarPrice {
            inner: map_new(
                kind,
                wc::WavePm::new(usize_param(params, 0, kind)?, usize_param(params, 1, kind)?),
            )?,
        })),
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
        "Wedge" => Ok(Box::new(CandleIn {
            inner: wc::Wedge::new(),
        })),
        "WeightedClose" => Ok(Box::new(CandleIn {
            inner: wc::WeightedClose::new(),
        })),
        "WickRatio" => Ok(Box::new(CandleIn {
            inner: wc::WickRatio::new(),
        })),
        "WilliamsFractals" => Ok(Box::new(CandleInFields {
            inner: wc::WilliamsFractals::new(),
            last: None,
        })),
        "WilliamsR" => Ok(Box::new(CandleIn {
            inner: map_new(kind, wc::WilliamsR::new(usize_param(params, 0, kind)?))?,
        })),
        "WinRate" => Ok(Box::new(ScalarPrice {
            inner: map_new(kind, wc::WinRate::new(usize_param(params, 0, kind)?))?,
        })),
        "Wma" => Ok(Box::new(ScalarPrice {
            inner: map_new(kind, wc::Wma::new(usize_param(params, 0, kind)?))?,
        })),
        "WoodiePivots" => Ok(Box::new(CandleInFields {
            inner: wc::WoodiePivots::new(),
            last: None,
        })),
        "YangZhangVolatility" => Ok(Box::new(CandleIn {
            inner: map_new(
                kind,
                wc::YangZhangVolatility::new(
                    usize_param(params, 0, kind)?,
                    usize_param(params, 1, kind)?,
                ),
            )?,
        })),
        "YoyoExit" => Ok(Box::new(CandleIn {
            inner: map_new(
                kind,
                wc::YoyoExit::new(usize_param(params, 0, kind)?, float_param(params, 1, kind)?),
            )?,
        })),
        "ZScore" => Ok(Box::new(ScalarPrice {
            inner: map_new(kind, wc::ZScore::new(usize_param(params, 0, kind)?))?,
        })),
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
        "Zlema" => Ok(Box::new(ScalarPrice {
            inner: map_new(kind, wc::Zlema::new(usize_param(params, 0, kind)?))?,
        })),
        "Bollinger" => build_inner("BollingerBands", params, reference),
        "Macd" => build_inner("MacdIndicator", params, reference),
        _ => Err(Error::Config(format!("unknown indicator: {kind}"))),
    }
}
