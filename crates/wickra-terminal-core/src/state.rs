//! The terminal's application state — folded from events in O(1) per event.
//!
//! [`AppState`] holds one [`SymbolState`] per `(SourceId, Symbol)` so it is
//! multi-symbol by construction, and [`AppState::fold`] applies a single event
//! incrementally: an order-book diff mutates the local book, a print pushes into
//! a bounded tape ring and the footprint, indicator state advances by one input.
//! Nothing is ever recomputed over history — that is the whole moat, and the
//! golden corpus pins the folded state byte-for-byte.

use std::collections::{BTreeMap, HashMap, VecDeque};

use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use wickra_core::{self as wc, Indicator};
use wickra_exchange_core::{BookDelta, BookLevel, Event, OrderBookSnapshot, OrderSide, TradePrint};

use crate::candle::{CandleBuilder, Timeframe};
use crate::config::IndicatorSpec;
use crate::error::{Error, Result};
use crate::registry::{self, TickIndicator, TickInput};
use crate::source::{DataSource, SourceId, Symbol};

/// The reference moving average the `% Above Moving Average` breadth reading is
/// taken against.
///
/// Fifty bars, because that is the conventional reference for that indicator and
/// the flag it consumes is a boolean the caller supplies rather than a parameter
/// the indicator carries — something has to choose, and choosing silently would
/// be worse than choosing here in the open. A configurable period is a
/// reasonable follow-up; it is not needed for the reading to be correct against
/// the convention.
const BREADTH_MA_PERIOD: usize = 50;

/// One derivatives update from the host, in the terminal's wire shape.
///
/// Every field is optional because the channels arrive on their own cadences: a
/// venue publishes funding eight-hourly, open interest by the minute and
/// mark/index continuously. A host sends whichever it just received and the
/// terminal folds it into what it already holds.
///
/// Defined here rather than reusing `wickra_exchange_core::DerivativesFeed`,
/// which carries exchange `Decimal`s and no serde derives — this is the command
/// schema, and the command boundary is JSON.
#[derive(Debug, Clone, Copy, Default, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct DerivativesUpdate {
    /// Funding rate for the interval, as a fraction (0.0001 is one basis point).
    pub funding_rate: Option<f64>,
    /// Mark price, the venue's fair value for the perpetual.
    pub mark_price: Option<f64>,
    /// Index price, the underlying spot reference.
    pub index_price: Option<f64>,
    /// Futures price, for the dated contract the basis is measured against.
    pub futures_price: Option<f64>,
    /// Open interest, in contracts.
    pub open_interest: Option<f64>,
    /// Aggregate long positioning.
    pub long_size: Option<f64>,
    /// Aggregate short positioning.
    pub short_size: Option<f64>,
    /// Long notional forcibly liquidated since the last update.
    pub long_liquidation: Option<f64>,
    /// Short notional forcibly liquidated since the last update.
    pub short_liquidation: Option<f64>,
    /// Venue timestamp for this update.
    pub timestamp: i64,
}

/// The derivatives microstructure of one market, folded from host updates.
///
/// Held per symbol and folded rather than replaced, because the channels are
/// independent: a funding print carries no open interest, and an open-interest
/// print carries no mark price. Replacing would blank every field the update did
/// not mention.
///
/// Kept here rather than delegating to `wickra_exchange_core`'s
/// `DerivativesTickBuilder`, and for one concrete reason: that builder passes
/// **zero** for taker buy and sell volume, with a comment saying they stay zero
/// "until a trade-derived source sets it". This terminal is a trade-derived
/// source — it holds the tape, with an aggressor side on every print — so going
/// through the builder would make `TakerBuySellRatio` read a constant 0/0 under
/// a name that promises a ratio. Folding the channels here lets the taker
/// volumes come from where they actually are.
#[derive(Debug, Default)]
struct DerivativesState {
    funding_rate: f64,
    mark_price: f64,
    index_price: f64,
    futures_price: f64,
    open_interest: f64,
    long_size: f64,
    short_size: f64,
    /// Accumulated from this market's own prints, by aggressor side.
    taker_buy_volume: f64,
    taker_sell_volume: f64,
    /// Accumulated since the last update, the way a liquidation flow is read.
    long_liquidation: f64,
    short_liquidation: f64,
    timestamp: i64,
    /// False until mark, index and futures prices have all been set.
    ///
    /// `DerivativesTick::new` rejects a non-positive price, so a tick cannot be
    /// built before they arrive. Tracked rather than inferred from the values
    /// being non-zero, so a venue legitimately publishing a price this terminal
    /// then loses is not mistaken for one that never sent one.
    priced: bool,
}

impl DerivativesState {
    /// Fold one host update. Absent fields leave what is already held.
    fn apply(&mut self, update: &DerivativesUpdate) {
        let set = |target: &mut f64, value: Option<f64>| {
            if let Some(value) = value {
                if value.is_finite() {
                    *target = value;
                }
            }
        };
        set(&mut self.funding_rate, update.funding_rate);
        set(&mut self.mark_price, update.mark_price);
        set(&mut self.index_price, update.index_price);
        set(&mut self.futures_price, update.futures_price);
        set(&mut self.open_interest, update.open_interest);
        set(&mut self.long_size, update.long_size);
        set(&mut self.short_size, update.short_size);

        // Liquidations accumulate rather than replace: they are a flow over the
        // interval, and two prints inside one interval are two liquidations.
        for (target, value) in [
            (&mut self.long_liquidation, update.long_liquidation),
            (&mut self.short_liquidation, update.short_liquidation),
        ] {
            if let Some(value) = value {
                if value.is_finite() && value >= 0.0 {
                    *target += value;
                }
            }
        }

        if update.timestamp != 0 {
            self.timestamp = update.timestamp;
        }
        self.priced = self.mark_price > 0.0 && self.index_price > 0.0 && self.futures_price > 0.0;
    }

    /// Accumulate one print into the taker flow, by aggressor side.
    fn add_trade(&mut self, quantity: f64, aggressor: OrderSide) {
        if !quantity.is_finite() || quantity < 0.0 {
            return;
        }
        match aggressor {
            OrderSide::Buy => self.taker_buy_volume += quantity,
            OrderSide::Sell => self.taker_sell_volume += quantity,
        }
    }

    /// This market's derivatives tick, or `None` before its prices have arrived.
    fn tick(&self) -> Option<wc::DerivativesTick> {
        if !self.priced {
            return None;
        }
        wc::DerivativesTick::new(
            self.funding_rate,
            self.mark_price,
            self.index_price,
            self.futures_price,
            self.open_interest,
            self.long_size,
            self.short_size,
            self.taker_buy_volume,
            self.taker_sell_volume,
            self.long_liquidation,
            self.short_liquidation,
            self.timestamp,
        )
        .ok()
    }
}

/// What one market contributes to a market-wide cross-section.
///
/// The breadth family does not read a price; it reads a *universe* — how many
/// symbols advanced, how many printed a new high, what share trade above their
/// moving average. So the terminal keeps, per symbol, the handful of per-bar
/// facts a member is assembled from, and folds them once per closed bar rather
/// than recomputing them across the universe on every tick.
///
/// Everything here is per **bar**, not per tick. A breadth reading compares
/// closes, and a tick-to-tick change would make `AdvanceDecline` count the same
/// symbol as advancing and declining several times within one bar.
#[derive(Debug)]
#[allow(
    clippy::struct_excessive_bools,
    reason = "three per-bar breadth signals plus a readiness flag, each independent; \
              `wc::Member` carries the same set for the same reason"
)]
struct BreadthState {
    /// The close of the previous bar, which `change` is measured against.
    previous_close: Option<f64>,
    /// Close-to-close change on the last closed bar.
    change: f64,
    /// Volume of the last closed bar.
    volume: f64,
    /// The session extremes, and whether the last bar set one.
    high: f64,
    low: f64,
    new_high: bool,
    new_low: bool,
    /// The reference moving average, and whether the last close sat above it.
    average: wc::Sma,
    above_ma: bool,
    /// False until a bar has closed. A symbol that has not produced one is left
    /// out of the universe rather than entered as an unchanged member, which
    /// would drag every ratio toward the middle.
    ready: bool,
}

impl BreadthState {
    fn new() -> Self {
        Self {
            previous_close: None,
            change: 0.0,
            volume: 0.0,
            high: f64::NEG_INFINITY,
            low: f64::INFINITY,
            new_high: false,
            new_low: false,
            average: wc::Sma::new(BREADTH_MA_PERIOD)
                .expect("BREADTH_MA_PERIOD is a non-zero constant"),
            above_ma: false,
            ready: false,
        }
    }

    /// Fold one closed bar.
    fn update(&mut self, candle: &wc::Candle) {
        let close = candle.close;
        if !close.is_finite() {
            return;
        }
        self.change = self.previous_close.map_or(0.0, |previous| close - previous);
        self.previous_close = Some(close);
        self.volume = if candle.volume.is_finite() && candle.volume >= 0.0 {
            candle.volume
        } else {
            0.0
        };

        // A new extreme is set by the bar's own high and low, not by its close:
        // a symbol that printed above its previous high and closed back inside
        // it did make a new high, and that is what the breadth family counts.
        self.new_high = candle.high > self.high;
        self.new_low = candle.low < self.low;
        self.high = self.high.max(candle.high);
        self.low = self.low.min(candle.low);

        self.above_ma = self
            .average
            .update(close)
            .is_some_and(|average| close > average);
        self.ready = true;
    }

    /// This market as a cross-section member, or `None` before its first bar.
    fn member(&self) -> Option<wc::Member> {
        if !self.ready {
            return None;
        }
        Some(wc::Member::with_signals(
            self.change,
            self.volume,
            self.new_high,
            self.new_low,
            self.above_ma,
            // Left false, and the one indicator that reads it is not registered
            // because of that. A point-and-figure buy signal needs P&F column
            // state per symbol, which this terminal does not keep; reporting
            // `false` for every member would make BullishPercentIndex read a
            // constant zero under a name that promises a breadth reading.
            false,
        ))
    }
}

/// A locally maintained L2 order book: price → resting quantity per side.
#[derive(Debug, Default, Clone)]
pub struct BookState {
    bids: BTreeMap<Decimal, Decimal>,
    asks: BTreeMap<Decimal, Decimal>,
}

impl BookState {
    /// Replace the book with a full snapshot.
    pub fn apply_snapshot(&mut self, snap: &OrderBookSnapshot) {
        self.bids.clear();
        self.asks.clear();
        for level in &snap.bids {
            self.bids.insert(level.price, level.quantity);
        }
        for level in &snap.asks {
            self.asks.insert(level.price, level.quantity);
        }
    }

    /// Apply an incremental diff: a zero quantity removes the level.
    pub fn apply_delta(&mut self, delta: &BookDelta) {
        apply_levels(&mut self.bids, &delta.bids);
        apply_levels(&mut self.asks, &delta.asks);
    }

    /// The best (highest) bid, or `None` if the bid side is empty.
    #[must_use]
    pub fn best_bid(&self) -> Option<(Decimal, Decimal)> {
        self.bids.iter().next_back().map(|(p, q)| (*p, *q))
    }

    /// The best (lowest) ask, or `None` if the ask side is empty.
    #[must_use]
    pub fn best_ask(&self) -> Option<(Decimal, Decimal)> {
        self.asks.iter().next().map(|(p, q)| (*p, *q))
    }

    /// The book as wickra-core's [`wc::OrderBook`], for the indicators that read
    /// it, or `None` if this book is not one the core will accept.
    ///
    /// `None` rather than an error because a book that is momentarily one-sided
    /// or crossed is an ordinary thing to see on a live feed between a snapshot
    /// and the diffs that follow it. The indicators that read the book simply do
    /// not advance on such a tick, which is the same thing they do while warming
    /// up; raising here would turn a normal feed hiccup into a dead terminal.
    #[must_use]
    pub fn to_core(&self) -> Option<wc::OrderBook> {
        // Bids reversed: the core wants each side best-first, and a BTreeMap
        // iterates ascending, which is best-first for asks but not for bids.
        let bids = levels(self.bids.iter().rev())?;
        let asks = levels(self.asks.iter())?;
        wc::OrderBook::new(bids, asks).ok()
    }

    /// The bid/ask spread, or `None` if either side is empty.
    #[must_use]
    pub fn spread(&self) -> Option<Decimal> {
        match (self.best_bid(), self.best_ask()) {
            (Some((bid, _)), Some((ask, _))) => Some(ask - bid),
            _ => None,
        }
    }

    /// The mid price, or `None` if either side is empty.
    ///
    /// What the microstructure family measures a print against: the effective
    /// spread is how far a trade printed from the mid that was standing when
    /// it arrived, so this has to be read BEFORE the print is folded. A trade
    /// does not move the book, so in the fold that ordering is free.
    #[must_use]
    pub fn mid(&self) -> Option<Decimal> {
        match (self.best_bid(), self.best_ask()) {
            (Some((bid, _)), Some((ask, _))) => Some((bid + ask) / Decimal::TWO),
            _ => None,
        }
    }

    /// The top `n` bid levels, best (highest) first.
    #[must_use]
    pub fn top_bids(&self, n: usize) -> Vec<(Decimal, Decimal)> {
        self.bids
            .iter()
            .rev()
            .take(n)
            .map(|(p, q)| (*p, *q))
            .collect()
    }

    /// The top `n` ask levels, best (lowest) first.
    #[must_use]
    pub fn top_asks(&self, n: usize) -> Vec<(Decimal, Decimal)> {
        self.asks.iter().take(n).map(|(p, q)| (*p, *q)).collect()
    }
}

/// Insert/remove changed levels into one side of a book.
/// Build the indicator a spec asks for, pairing it when it names a reference.
///
/// A pairwise kind with no reference is rejected by the registry rather than
/// here, so the error names the indicator and says what it wants.
fn build_spec(spec: &IndicatorSpec) -> Result<Box<dyn TickIndicator>> {
    match &spec.reference {
        Some(reference) => registry::build_paired(&spec.kind, &spec.params, reference),
        None => registry::build(&spec.kind, &spec.params),
    }
}

/// Convert a print into the core's [`wc::Trade`], for the tape indicator family.
///
/// `None` if the price or quantity does not survive the move from `Decimal` to
/// `f64`, or if the core rejects them — a zero or negative print price, which
/// the core refuses because the measures built on it are ratios.
fn core_trade(print: &TradePrint) -> Option<wc::Trade> {
    let side = match print.aggressor {
        OrderSide::Buy => wc::Side::Buy,
        OrderSide::Sell => wc::Side::Sell,
    };
    wc::Trade::new(
        print.price.to_f64()?,
        print.quantity.to_f64()?,
        side,
        print.timestamp,
    )
    .ok()
}

/// Convert one side of the book into the core's levels, in the order given.
///
/// `None` if any price or size does not survive the move from `Decimal` to
/// `f64`, or if the core rejects a level. Dropping the whole side rather than
/// the offending level keeps the book self-consistent: a book missing a level
/// in the middle would still satisfy the core's ordering checks while quietly
/// misstating the depth every book indicator reads.
fn levels<'a>(side: impl Iterator<Item = (&'a Decimal, &'a Decimal)>) -> Option<Vec<wc::Level>> {
    side.map(|(price, size)| wc::Level::new(price.to_f64()?, size.to_f64()?).ok())
        .collect()
}

fn apply_levels(side: &mut BTreeMap<Decimal, Decimal>, changes: &[BookLevel]) {
    for level in changes {
        if level.quantity.is_zero() {
            side.remove(&level.price);
        } else {
            side.insert(level.price, level.quantity);
        }
    }
}

/// A bounded ring of the most recent trade prints (newest at the back).
#[derive(Debug, Clone)]
pub struct TapeRing {
    prints: VecDeque<TradePrint>,
    cap: usize,
}

impl TapeRing {
    /// A ring holding at most `cap` prints.
    #[must_use]
    pub fn new(cap: usize) -> Self {
        Self {
            prints: VecDeque::with_capacity(cap),
            cap,
        }
    }

    /// Push a print, evicting the oldest once the cap is exceeded. O(1).
    pub fn push(&mut self, print: TradePrint) {
        if self.prints.len() == self.cap {
            self.prints.pop_front();
        }
        self.prints.push_back(print);
    }

    /// The most recent `n` prints, newest first.
    #[must_use]
    pub fn recent(&self, n: usize) -> Vec<TradePrint> {
        self.prints.iter().rev().take(n).cloned().collect()
    }

    /// The number of buffered prints.
    #[must_use]
    pub fn len(&self) -> usize {
        self.prints.len()
    }

    /// Whether the ring holds no prints.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.prints.is_empty()
    }
}

impl Default for TapeRing {
    fn default() -> Self {
        Self::new(256)
    }
}

/// The most price levels a [`Footprint`] keeps.
///
/// The footprint was the one collection in the fold that grew without limit: an
/// entry per distinct traded price, never evicted, while the tape (256), the
/// price history (512) and each indicator series (120) are all bounded. A
/// synthetic 200k-print walk left 2,926 levels still climbing, and a BTC/USDT
/// feed quoting to the cent gives hundreds of thousands.
///
/// A thousand levels is far more profile than the twelve a panel renders, and at
/// a one-cent tick it spans a ten-dollar band around where the market has been
/// trading -- wide enough that the profile is a profile rather than a keyhole,
/// and small enough that a session cannot grow into it.
const MAX_FOOTPRINT_LEVELS: usize = 1024;

/// Volume traded at each price, split by aggressor side (a footprint / volume
/// profile).
///
/// Bounded to [`MAX_FOOTPRINT_LEVELS`], evicting whichever end is furthest from
/// the price being traded, so the profile follows the market rather than
/// accumulating every price a session ever touched.
#[derive(Debug, Default, Clone)]
pub struct Footprint {
    levels: BTreeMap<Decimal, (Decimal, Decimal)>,
}

impl Footprint {
    /// Add a print's quantity to the (buy, sell) volume at its price. Saturating:
    /// an accumulated volume that would overflow `Decimal` (only reachable with
    /// adversarial fuzz input) keeps the previous total instead of panicking.
    pub fn add(&mut self, print: &TradePrint) {
        let entry = self.levels.entry(print.price).or_default();
        let side = match print.aggressor {
            OrderSide::Buy => &mut entry.0,
            OrderSide::Sell => &mut entry.1,
        };
        *side = side.checked_add(print.quantity).unwrap_or(*side);

        // One insert per print, so at most one eviction -- but a `while` keeps
        // this correct rather than merely sufficient if the cap ever changes.
        // The furthest level is at one end or the other, the map being ordered,
        // so this costs a lookup rather than a scan.
        while self.levels.len() > MAX_FOOTPRINT_LEVELS {
            let lowest = self.levels.keys().next().copied().unwrap_or(print.price);
            let highest = self
                .levels
                .keys()
                .next_back()
                .copied()
                .unwrap_or(print.price);
            let furthest = if print.price.saturating_sub(lowest).abs()
                >= highest.saturating_sub(print.price).abs()
            {
                lowest
            } else {
                highest
            };
            self.levels.remove(&furthest);
        }
    }

    /// The (buy, sell) volume at `price`, if any has traded there.
    #[must_use]
    pub fn at(&self, price: Decimal) -> Option<(Decimal, Decimal)> {
        self.levels.get(&price).copied()
    }

    /// The `depth` levels nearest `anchor`, highest price first, as
    /// `(price, buy, sell)`.
    ///
    /// Anchored rather than absolute. This used to return the `n` HIGHEST prices
    /// ever traded, which made the panel stop tracking the market: on a synthetic
    /// walk of 200k prints the last trade was 495.19 while the panel showed
    /// 513.03 down to 512.81, each holding one or two units -- prices the market
    /// had left long before and would never come back to. A ladder around the
    /// last trade is what a footprint panel is for.
    ///
    /// Walks outward from `anchor` through the ordered map, so it costs the
    /// levels it returns rather than the levels it holds.
    #[must_use]
    pub fn around(&self, anchor: Decimal, depth: usize) -> Vec<(Decimal, Decimal, Decimal)> {
        let mut below = self.levels.range(..anchor).rev().peekable();
        let mut above = self.levels.range(anchor..).peekable();
        let mut picked = Vec::with_capacity(depth.min(self.levels.len()));

        while picked.len() < depth {
            match (below.peek().copied(), above.peek().copied()) {
                (None, None) => break,
                (Some((&price, &(buy, sell))), None) => {
                    picked.push((price, buy, sell));
                    let _ = below.next();
                }
                (None, Some((&price, &(buy, sell)))) => {
                    picked.push((price, buy, sell));
                    let _ = above.next();
                }
                (Some((&low, &(low_buy, low_sell))), Some((&high, &(high_buy, high_sell)))) => {
                    if high.saturating_sub(anchor) <= anchor.saturating_sub(low) {
                        picked.push((high, high_buy, high_sell));
                        let _ = above.next();
                    } else {
                        picked.push((low, low_buy, low_sell));
                        let _ = below.next();
                    }
                }
            }
        }

        // Highest price first, which is how a ladder reads.
        picked.sort_by_key(|level| std::cmp::Reverse(level.0));
        picked
    }

    /// The number of price levels with recorded volume.
    #[must_use]
    pub fn len(&self) -> usize {
        self.levels.len()
    }

    /// Whether no volume has been recorded yet.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.levels.is_empty()
    }
}

/// One named indicator plus its latest output.
struct IndicatorEntry {
    label: String,
    indicator: Box<dyn TickIndicator>,
    last: Option<f64>,
    fields: Vec<(&'static str, f64)>,
    /// A bounded recent series, for renderers that draw the indicator as a line
    /// rather than a number.
    series: VecDeque<f64>,
}

/// How many recent points each indicator keeps for renderers that draw it as a
/// line. Matches the chart panel's series length: a longer one would be trimmed
/// on the way out, a shorter one would leave the overlay short of the price line.
const INDICATOR_SERIES: usize = 120;

/// One indicator's latest reading: its label, its primary value and, for a
/// multi-output indicator, its named fields.
#[derive(Debug, Clone, PartialEq)]
pub struct IndicatorReading {
    /// The display label, derived from the spec.
    pub label: String,
    /// The primary value, or `None` while warming up.
    pub value: Option<f64>,
    /// Named outputs in declaration order; empty for a single-output indicator.
    pub fields: Vec<(&'static str, f64)>,
    /// A bounded recent series, oldest first, ending at the current tick.
    pub series: Vec<f64>,
}

impl std::fmt::Debug for IndicatorEntry {
    /// The indicator itself is a trait object with no `Debug` bound, so the
    /// label stands in for it -- it is what identifies the row everywhere else
    /// -- and the series is reported by length rather than dumped.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("IndicatorEntry")
            .field("label", &self.label)
            .field("last", &self.last)
            .field("fields", &self.fields)
            .field("series", &self.series.len())
            .finish_non_exhaustive()
    }
}

/// The set of indicators tracked for a symbol.
#[derive(Debug)]
pub struct IndicatorSet {
    entries: Vec<IndicatorEntry>,
}

impl IndicatorSet {
    /// Build the set a config asks for.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Config`] naming the indicator if a spec's kind is not in
    /// the registry or its parameters are rejected.
    pub fn from_specs(specs: &[IndicatorSpec]) -> Result<Self> {
        let entries = specs
            .iter()
            .map(|spec| {
                Ok(IndicatorEntry {
                    label: spec.label(),
                    indicator: build_spec(spec)?,
                    last: None,
                    fields: Vec::new(),
                    series: VecDeque::with_capacity(INDICATOR_SERIES),
                })
            })
            .collect::<Result<Vec<_>>>()?;
        Ok(Self { entries })
    }

    /// Add one indicator, which starts cold and warms up from the next tick.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Config`] naming the spec if the registry rejects it.
    pub fn push(&mut self, spec: &IndicatorSpec) -> Result<()> {
        self.entries.push(IndicatorEntry {
            label: spec.label(),
            indicator: build_spec(spec)?,
            last: None,
            fields: Vec::new(),
            series: VecDeque::with_capacity(INDICATOR_SERIES),
        });
        Ok(())
    }

    /// Whether any indicator in this set reads the order book.
    ///
    /// Scanned rather than cached: the set holds a handful of indicators and
    /// this is asked once per tick, so the scan costs less than the bookkeeping
    /// a cached flag would need on every add and remove — and it cannot go
    /// stale, which a cached flag can.
    #[must_use]
    pub fn wants_book(&self) -> bool {
        self.entries.iter().any(|e| e.indicator.wants_book())
    }

    /// Whether any indicator in this set reads another market's price.
    ///
    /// Scanned rather than cached, for the same reason as
    /// [`IndicatorSet::wants_book`].
    #[must_use]
    pub fn wants_references(&self) -> bool {
        self.entries.iter().any(|e| e.indicator.wants_reference())
    }

    /// Drop the indicator with this label. Returns whether one was removed.
    pub fn remove(&mut self, label: &str) -> bool {
        let before = self.entries.len();
        self.entries.retain(|entry| entry.label != label);
        self.entries.len() != before
    }

    /// Feed one tick into every indicator.
    ///
    /// Each indicator that has produced a value records one point per tick, not
    /// one per update. A bar indicator only advances when a bar closes, so
    /// recording only its updates would give it a series several times shorter
    /// than the price series and an overlay drawn from it would sit at the wrong
    /// place on the x-axis. Carrying the last value forward makes it a step line
    /// over the same ticks, which is what it actually means.
    ///
    /// Indicators warm up at different lengths, so the series are not all the
    /// same length. They all end at the current tick, so a renderer aligns them
    /// to the right of the price series.
    pub fn update(&mut self, input: &TickInput) {
        for entry in &mut self.entries {
            if let Some(value) = entry.indicator.update(input) {
                entry.last = Some(value);
                entry.fields = entry.indicator.fields();
            }
            if let Some(value) = entry.last {
                if entry.series.len() == INDICATOR_SERIES {
                    entry.series.pop_front();
                }
                entry.series.push_back(value);
            }
        }
    }

    /// The latest `(label, value)` of each indicator (`value` is `None` while
    /// still warming up).
    #[must_use]
    pub fn values(&self) -> Vec<(String, Option<f64>)> {
        self.entries
            .iter()
            .map(|entry| (entry.label.clone(), entry.last))
            .collect()
    }

    /// The latest reading of every indicator, in one pass.
    ///
    /// One call rather than a `values` and a `fields` the caller has to zip: two
    /// parallel lists is one refactor away from a chart showing one indicator's
    /// value under another's name.
    #[must_use]
    pub fn snapshot(&self) -> Vec<IndicatorReading> {
        self.entries
            .iter()
            .map(|entry| IndicatorReading {
                label: entry.label.clone(),
                value: entry.last,
                fields: entry.fields.clone(),
                series: entry.series.iter().copied().collect(),
            })
            .collect()
    }
}

impl Default for IndicatorSet {
    /// The default overlay, which the registry is guaranteed to accept.
    fn default() -> Self {
        Self::from_specs(&crate::config::default_indicators())
            .expect("the default indicator overlay must be constructible")
    }
}

/// All state for a single market on a single source.
#[derive(Debug)]
pub struct SymbolState {
    /// The local L2 order book.
    pub book: BookState,
    /// The recent trade tape.
    pub tape: TapeRing,
    /// The per-price volume footprint.
    pub footprint: Footprint,
    /// The chart indicator set.
    pub indicators: IndicatorSet,
    /// The last traded price seen.
    pub last: Decimal,
    /// A bounded recent price history for the chart series.
    pub history: VecDeque<Decimal>,
    /// Aggregates the trade stream into the bars the candle indicators read.
    pub candles: CandleBuilder,
    /// What this market contributes to a market-wide cross-section.
    breadth: BreadthState,
    /// The derivatives microstructure of this market, folded from host updates.
    derivatives: DerivativesState,
}

impl SymbolState {
    /// Fresh state for one market, with the indicator set and bar size the
    /// config asks for.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Config`] if an indicator spec is not constructible.
    pub fn new(specs: &[IndicatorSpec], timeframe: Timeframe) -> Result<Self> {
        Ok(Self {
            book: BookState::default(),
            tape: TapeRing::default(),
            footprint: Footprint::default(),
            indicators: IndicatorSet::from_specs(specs)?,
            last: Decimal::ZERO,
            history: VecDeque::with_capacity(512),
            candles: CandleBuilder::new(timeframe),
            breadth: BreadthState::new(),
            derivatives: DerivativesState::default(),
        })
    }
}

impl SymbolState {
    /// Fold one derivatives update into this market's microstructure.
    pub(crate) fn apply_derivatives(&mut self, update: &DerivativesUpdate) {
        self.derivatives.apply(update);
    }
}

impl Default for SymbolState {
    fn default() -> Self {
        Self {
            book: BookState::default(),
            tape: TapeRing::default(),
            footprint: Footprint::default(),
            indicators: IndicatorSet::default(),
            last: Decimal::ZERO,
            history: VecDeque::with_capacity(512),
            candles: CandleBuilder::new(Timeframe::default()),
            breadth: BreadthState::new(),
            derivatives: DerivativesState::default(),
        }
    }
}

impl SymbolState {
    /// A bounded recent price series (oldest first) for the chart.
    #[must_use]
    pub fn series(&self, n: usize) -> Vec<f64> {
        let skip = self.history.len().saturating_sub(n);
        self.history
            .iter()
            .skip(skip)
            .map(|d| d.to_f64().unwrap_or(0.0))
            .collect()
    }
}

/// A `(source, symbol)` key.
pub type Key = (SourceId, Symbol);

/// The whole terminal application state.
#[derive(Default)]
pub struct AppState {
    /// The open feed sources.
    pub sources: Vec<Box<dyn DataSource>>,
    /// Per-market state, keyed by `(source, symbol)`.
    pub symbols: HashMap<Key, SymbolState>,
    /// The focused market, if any is subscribed.
    pub focus: Option<Key>,
    /// The tracked markets, in display order.
    pub watchlist: Vec<Key>,
    /// The indicator specs every market is tracked with. Validated once, when
    /// the terminal is built or a spec is added, so building a market's set can
    /// no longer fail.
    pub indicators: Vec<IndicatorSpec>,
    /// The bar size the candle-input indicators are fed at.
    pub timeframe: Timeframe,
}

impl std::fmt::Debug for AppState {
    /// `sources` is a vector of trait objects, so it is reported by count; the
    /// rest of the state is what a reader is actually after.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AppState")
            .field("sources", &self.sources.len())
            .field("symbols", &self.symbols)
            .field("focus", &self.focus)
            .field("watchlist", &self.watchlist)
            .field("indicators", &self.indicators)
            .field("timeframe", &self.timeframe)
            .finish()
    }
}

impl AppState {
    /// Fold one event for `(src, sym)` into state, in O(1) per event (bounded by
    /// the event's own size, never by history).
    ///
    /// Every collection it writes into is bounded, including the footprint, which
    /// used to be the exception.
    pub fn fold(&mut self, src: SourceId, sym: &Symbol, event: &Event) {
        self.fold_scoped(src, sym, event, None);
    }

    /// `fold`, with the reference scope a seek's re-fold needs. See
    /// [`AppState::reference_prices`].
    pub(crate) fn fold_scoped(
        &mut self,
        src: SourceId,
        sym: &Symbol,
        event: &Event,
        reference_scope: Option<SourceId>,
    ) {
        // Collected before the market's own state is borrowed mutably, which is
        // also the only order that gives the right answer: the reference markets
        // are read as they stand now, while this market's last is still the
        // previous print until this event is folded.
        //
        // Asked of the specs rather than of an indicator set because the sets
        // all come from these specs, and this one is reachable without holding a
        // borrow of any market.
        let references = if self.indicators.iter().any(|s| s.reference.is_some()) {
            self.reference_prices(reference_scope)
        } else {
            BTreeMap::new()
        };

        // The universe, gathered the same way and for the same reason. The
        // breadth family reads every market at once, so this cannot be asked
        // of one market's indicator set while that market is borrowed.
        //
        // Asked of the kind rather than of a config field, because unlike a
        // pairwise reference there is nothing in a spec that says "this one
        // reads the universe" -- the registry knows, and says so.
        let cross_section = if self
            .indicators
            .iter()
            .any(|s| registry::is_cross_section(&s.kind))
        {
            match event {
                Event::Trade(print) => self.cross_section(print.timestamp),
                // Only a print advances a bar, and only a closed bar changes a
                // member, so no other event can produce a reading.
                _ => None,
            }
        } else {
            None
        };
        // `expect` rather than a fallback: every spec in `self.indicators` was
        // accepted by the registry when it was set, so construction here cannot
        // fail. A silent `unwrap_or_default` would hide a market quietly losing
        // its indicators.
        // A print claiming a negative size is malformed, and folding any part of it
        // invents data: the footprint would subtract it from the volume already
        // traded at that price, and the bar builder carried it into a candle
        // `Candle::new` would reject -- which every volume-reading bar indicator
        // then read. Nothing validates a `TradePrint.quantity`; it is a `Decimal`
        // off the wire. Checked before the entry below, so a market is not brought
        // into existence by an event that is then discarded.
        if matches!(event, Event::Trade(print) if print.quantity < Decimal::ZERO) {
            return;
        }
        let state = self.symbols.entry((src, sym.clone())).or_insert_with(|| {
            SymbolState::new(&self.indicators, self.timeframe)
                .expect("indicator specs are validated before they reach the state")
        });
        match event {
            Event::Trade(print) => {
                let price = print.price.to_f64().unwrap_or(0.0);
                state.last = print.price;
                state.tape.push(print.clone());
                state.footprint.add(print);
                // A trade both advances the price indicators and may close a bar
                // for the candle ones; the builder decides which.
                let closed = state.candles.update(
                    price,
                    print.quantity.to_f64().unwrap_or(0.0),
                    print.timestamp,
                );
                // The book is converted only when something reads it: the
                // default indicator set is all price and bar indicators, and a
                // deep book would otherwise be walked on every print for
                // nothing.
                let book = if state.indicators.wants_book() {
                    state.book.to_core()
                } else {
                    None
                };
                // A closed bar is what a breadth reading compares: fed per
                // tick, the same symbol would count as advancing and
                // declining several times inside one bar.
                if let Some(bar) = closed.as_ref() {
                    state.breadth.update(bar);
                }
                // The taker flow is this terminal's own: the aggressor side
                // is on every print, and the derivatives feeds do not carry
                // it -- wickra-exchange's builder passes zero for it and says
                // so, which is why the fold above is here rather than there.
                state
                    .derivatives
                    .add_trade(print.quantity.to_f64().unwrap_or(0.0), print.aggressor);
                let mut tick = TickInput::price(price);
                tick.candle = closed;
                tick.cross_section = cross_section;
                tick.derivatives = state.derivatives.tick();
                tick.trade = core_trade(print);
                // The mid as it stands NOW, which is the mid this print
                // arrived against: a trade does not move the book, so it is
                // still the one the last depth update left. That ordering is
                // the whole measurement -- an effective spread taken against
                // a mid the trade itself moved would be measuring nothing.
                tick.trade_quote = tick.trade.and_then(|trade| {
                    state
                        .book
                        .mid()
                        .and_then(|mid| mid.to_f64())
                        .and_then(|mid| wc::TradeQuote::new(trade, mid).ok())
                });
                tick.book = book;
                tick.references = references;
                state.indicators.update(&tick);
                if state.history.len() == 512 {
                    state.history.pop_front();
                }
                state.history.push_back(print.price);
            }
            Event::Ticker(ticker) => state.last = ticker.last,
            Event::BookSnapshot(snap) => state.book.apply_snapshot(snap),
            Event::BookDelta(delta) => state.book.apply_delta(delta),
            // Account and lifecycle events do not affect per-symbol market state.
            Event::OrderUpdate(_)
            | Event::BalanceUpdate(_)
            | Event::Subscribed { .. }
            | Event::Disconnected
            | Event::Reconnected => {}
        }
    }

    /// The last price of every tracked market, keyed by symbol.
    ///
    /// A market that has not printed yet is left out rather than entered at
    /// zero, so a pairwise indicator waits for a real price instead of being
    /// warmed up on a placeholder.
    ///
    /// Keyed by symbol alone, not by `(source, symbol)`: a reference is written
    /// as `ETH/USDT` in a config, and the same market on two feeds is the same
    /// price. When both carry it, the later one in iteration order wins, which
    /// is arbitrary but not wrong -- they are quotes for the same thing.
    /// `scope` restricts which sources may supply a reference. `None` is the live
    /// path and reads every market; `Some(id)` is a seek's re-fold and reads only
    /// that source's, because those are the markets the re-fold has reset and is
    /// replaying in order.
    ///
    /// Without the scope a re-fold paired every historical tick with the
    /// reference market's *present* price, since a market on another source is
    /// neither reset nor replayed. That made `Seek` non-deterministic for the
    /// whole pairwise family -- a correlation of 0.88 became 0.0 after seeking to
    /// the position it was already at -- while the entire justification for
    /// re-folding rather than snapshotting is that it rebuilds identical state.
    ///
    /// A reference outside the scope is absent rather than stale, so the
    /// indicator simply does not advance, which is what it already does before
    /// its reference has printed.
    #[must_use]
    fn reference_prices(&self, scope: Option<SourceId>) -> BTreeMap<String, f64> {
        self.symbols
            .iter()
            .filter(|((src, _), _)| scope.is_none_or(|id| *src == id))
            .filter_map(|((_, symbol), state)| {
                state.last.to_f64().map(|price| (symbol.to_string(), price))
            })
            .filter(|(_, price)| *price > 0.0)
            .collect()
    }

    /// Every tracked market as one cross-section, for the breadth family.
    ///
    /// The universe is every market this terminal holds state for, across
    /// every source: breadth is a property of what is being watched, and a
    /// terminal watching two feeds is watching one market list.
    ///
    /// A market that has not closed a bar yet is left out rather than entered
    /// as an unchanged member. Entering it would count it as neither advancing
    /// nor declining and drag every ratio toward the middle while the terminal
    /// warms up -- the same call `reference_prices` makes for a market that has
    /// not printed.
    ///
    /// `None` when nothing is ready, because `CrossSection::new` rejects an
    /// empty universe, and an empty one is not a reading of anything.
    ///
    /// The timestamp is the one on the event being folded, not a counter of
    /// this method's own: the breadth indicators order their history by it,
    /// and the feed already carries the real answer.
    #[must_use]
    fn cross_section(&self, timestamp: i64) -> Option<wc::CrossSection> {
        let members: Vec<wc::Member> = self
            .symbols
            .values()
            .filter_map(|state| state.breadth.member())
            .collect();
        if members.is_empty() {
            return None;
        }
        wc::CrossSection::new(members, timestamp).ok()
    }

    /// Fresh state for a market, carrying the indicator set and bar size this
    /// terminal was configured with.
    ///
    /// `expect` rather than a fallback, for the same reason `fold` does: every
    /// spec in `self.indicators` was accepted by the registry before it got
    /// there, so this cannot fail, and a silent default would open a market with
    /// the wrong indicators rather than none at all.
    #[must_use]
    pub fn fresh_market(&self) -> SymbolState {
        SymbolState::new(&self.indicators, self.timeframe)
            .expect("indicator specs are validated before they reach the state")
    }

    /// Track one more indicator on every market, now and for markets opened later.
    ///
    /// It starts cold: a market that has been running keeps its history, but the
    /// new indicator warms up from the next tick, because the inputs it missed
    /// are gone. Re-adding a label that is already tracked is rejected rather
    /// than silently duplicating a row in the chart panel.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Config`] if the registry rejects the spec, or if this
    /// label is already tracked.
    pub fn add_indicator(&mut self, spec: &IndicatorSpec) -> Result<()> {
        let label = spec.label();
        if self.indicators.iter().any(|s| s.label() == label) {
            return Err(Error::Config(format!("indicator already tracked: {label}")));
        }
        // Build once before mutating anything, so a rejected spec cannot leave
        // some markets updated and others not.
        build_spec(spec)?;
        for state in self.symbols.values_mut() {
            state.indicators.push(spec)?;
        }
        self.indicators.push(spec.clone());
        Ok(())
    }

    /// Change the bar size every market is aggregated at.
    ///
    /// Restarts the bar-derived state: each market's candle builder starts a new
    /// bar, and the indicator set is rebuilt. Rebuilding the whole set rather
    /// than only the bar indicators is deliberate — an indicator's history is a
    /// sequence of readings at one bar size, and continuing it across a change
    /// would blend two, which is not a smaller bar or a larger one but a
    /// meaningless mixture. Nothing in the registry marks which indicators read
    /// bars, so the choice is between rebuilding all of them and knowingly
    /// corrupting some.
    ///
    /// Price history, the tape, the book and the footprint are untouched: none
    /// of them is derived from bars.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Config`] if an indicator spec is not constructible,
    /// which cannot happen for specs already accepted.
    pub fn set_timeframe(&mut self, timeframe: Timeframe) -> Result<()> {
        self.timeframe = timeframe;
        for state in self.symbols.values_mut() {
            state.candles = CandleBuilder::new(timeframe);
            state.indicators = IndicatorSet::from_specs(&self.indicators)?;
        }
        Ok(())
    }

    /// Stop tracking the indicator with this label. Returns whether one matched.
    pub fn remove_indicator(&mut self, label: &str) -> bool {
        let known = self.indicators.iter().any(|s| s.label() == label);
        if !known {
            return false;
        }
        self.indicators.retain(|s| s.label() != label);
        for state in self.symbols.values_mut() {
            state.indicators.remove(label);
        }
        true
    }

    /// Poll every source and fold what they yield. Returns the number of events
    /// folded this pump.
    pub fn pump(&mut self) -> usize {
        let mut batch: Vec<(SourceId, Symbol, Event)> = Vec::new();
        for source in &mut self.sources {
            let id = source.id();
            for (sym, ev) in source.poll() {
                batch.push((id, sym, ev));
            }
        }
        let folded = batch.len();
        for (id, sym, ev) in batch {
            self.fold(id, &sym, &ev);
        }
        folded
    }

    /// Get the state for a key, if present.
    #[must_use]
    pub fn get(&self, key: &Key) -> Option<&SymbolState> {
        self.symbols.get(key)
    }

    /// Find a source by id.
    pub fn source_mut(&mut self, id: SourceId) -> Option<&mut Box<dyn DataSource>> {
        self.sources.iter_mut().find(|s| s.id() == id)
    }

    /// Drop a source and every market it owned, repairing focus/watchlist.
    pub fn remove_source(&mut self, id: SourceId) {
        self.sources.retain(|s| s.id() != id);
        self.symbols.retain(|(src, _), _| *src != id);
        self.watchlist.retain(|(src, _)| *src != id);
        if matches!(&self.focus, Some((src, _)) if *src == id) {
            self.focus = self.watchlist.first().cloned();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;
    use wickra_exchange_core::Symbol;

    fn trade(sym: &Symbol, price: Decimal, side: OrderSide) -> Event {
        Event::Trade(TradePrint {
            symbol: sym.clone(),
            price,
            quantity: dec!(2),
            aggressor: side,
            timestamp: 0,
        })
    }

    #[test]
    fn fold_trade_updates_last_tape_footprint_and_history() {
        let sym = Symbol::new("BTC", "USDT");
        let mut state = AppState::default();
        state.fold(0, &sym, &trade(&sym, dec!(100), OrderSide::Buy));
        state.fold(0, &sym, &trade(&sym, dec!(101), OrderSide::Sell));
        let st = state.get(&(0, sym.clone())).unwrap();
        assert_eq!(st.last, dec!(101));
        assert_eq!(st.tape.len(), 2);
        assert_eq!(st.footprint.at(dec!(100)), Some((dec!(2), dec!(0))));
        assert_eq!(st.footprint.at(dec!(101)), Some((dec!(0), dec!(2))));
        assert_eq!(st.series(10), vec![100.0, 101.0]);
    }

    fn print_at(sym: &Symbol, price: Decimal) -> Event {
        Event::Trade(TradePrint {
            symbol: sym.clone(),
            price,
            quantity: Decimal::ONE,
            aggressor: OrderSide::Buy,
            timestamp: 0,
        })
    }

    #[test]
    fn the_footprint_does_not_grow_with_the_session() {
        // It was the one collection in the fold that did: an entry per distinct
        // traded price, never evicted, while the tape, the price history and every
        // indicator series are bounded. A 200k-print walk left 2,926 levels still
        // climbing.
        let sym = Symbol::new("BTC", "USDT");
        let mut state = AppState::default();
        for tick in 0..5_000 {
            let price = Decimal::from(100_000 + tick) / dec!(100);
            state.fold(0, &sym, &print_at(&sym, price));
        }
        let st = state.get(&(0, sym)).unwrap();
        assert!(
            st.footprint.len() <= MAX_FOOTPRINT_LEVELS,
            "the footprint holds {} levels",
            st.footprint.len()
        );
    }

    #[test]
    fn the_footprint_keeps_the_levels_nearest_the_market() {
        // Eviction has to drop the far end, not the newest: a profile that kept
        // the prices the market has left behind is the drift this fixes, one step
        // removed.
        let sym = Symbol::new("BTC", "USDT");
        let mut state = AppState::default();
        for tick in 0..5_000 {
            let price = Decimal::from(100_000 + tick) / dec!(100);
            state.fold(0, &sym, &print_at(&sym, price));
        }
        let st = state.get(&(0, sym)).unwrap();
        let last = st.last;
        assert_eq!(last, dec!(1049.99));
        assert!(
            st.footprint.at(last).is_some(),
            "the level just traded was evicted"
        );
        let ladder = st.footprint.around(last, 3);
        assert_eq!(ladder.len(), 3);
        for (price, _, _) in &ladder {
            let gap = (*price - last).abs();
            assert!(gap < dec!(1), "{price} is {gap} away from a last of {last}");
        }
    }

    #[test]
    fn around_returns_a_ladder_centred_on_the_anchor() {
        let sym = Symbol::new("BTC", "USDT");
        let mut state = AppState::default();
        for cents in 0..21 {
            let price = Decimal::from(10_000 + cents) / dec!(100);
            state.fold(0, &sym, &print_at(&sym, price));
        }
        let st = state.get(&(0, sym)).unwrap();
        let ladder = st.footprint.around(dec!(100.10), 5);
        let prices: Vec<Decimal> = ladder.iter().map(|(price, _, _)| *price).collect();
        // Highest first, and the five nearest 100.10 rather than the five highest.
        assert_eq!(
            prices,
            vec![
                dec!(100.12),
                dec!(100.11),
                dec!(100.10),
                dec!(100.09),
                dec!(100.08)
            ]
        );
    }

    #[test]
    fn a_negative_size_print_is_not_folded_at_all() {
        // The footprint accumulates with `checked_add`, so a negative quantity
        // subtracted from the volume already traded at that price: two prints of
        // 2 and -2 left a level reading zero, as if nothing had traded there.
        let sym = Symbol::new("BTC", "USDT");
        let mut state = AppState::default();
        state.fold(0, &sym, &trade(&sym, dec!(100), OrderSide::Buy));
        let mut bad = trade(&sym, dec!(100), OrderSide::Buy);
        if let Event::Trade(print) = &mut bad {
            print.quantity = dec!(-2);
        }
        state.fold(0, &sym, &bad);
        let st = state.get(&(0, sym.clone())).unwrap();
        assert_eq!(st.footprint.at(dec!(100)), Some((dec!(2), dec!(0))));
        assert_eq!(st.tape.len(), 1, "a malformed print reached the tape");
    }

    #[test]
    fn a_rejected_print_does_not_bring_a_market_into_existence() {
        let sym = Symbol::new("BTC", "USDT");
        let mut state = AppState::default();
        let mut bad = trade(&sym, dec!(100), OrderSide::Buy);
        if let Event::Trade(print) = &mut bad {
            print.quantity = dec!(-1);
        }
        state.fold(0, &sym, &bad);
        assert!(
            state.get(&(0, sym)).is_none(),
            "an empty market was created"
        );
    }

    #[test]
    fn book_snapshot_then_delta_apply() {
        let sym = Symbol::new("BTC", "USDT");
        let mut book = BookState::default();
        book.apply_snapshot(&OrderBookSnapshot {
            symbol: sym.clone(),
            last_update_id: 1,
            bids: vec![
                BookLevel::new(dec!(100), dec!(1)),
                BookLevel::new(dec!(99), dec!(2)),
            ],
            asks: vec![BookLevel::new(dec!(101), dec!(1))],
        });
        assert_eq!(book.best_bid(), Some((dec!(100), dec!(1))));
        assert_eq!(book.best_ask(), Some((dec!(101), dec!(1))));
        assert_eq!(book.spread(), Some(dec!(1)));
        // A delta removes the top bid and adds a new ask level.
        book.apply_delta(&BookDelta {
            symbol: sym,
            first_update_id: 2,
            final_update_id: 2,
            bids: vec![BookLevel::new(dec!(100), dec!(0))],
            asks: vec![BookLevel::new(dec!(102), dec!(3))],
        });
        assert_eq!(book.best_bid(), Some((dec!(99), dec!(2))));
        assert_eq!(
            book.top_asks(2),
            vec![(dec!(101), dec!(1)), (dec!(102), dec!(3))]
        );
    }

    #[test]
    fn tape_ring_respects_cap() {
        let sym = Symbol::new("BTC", "USDT");
        let mut ring = TapeRing::new(3);
        for i in 0..5 {
            ring.push(TradePrint {
                symbol: sym.clone(),
                price: Decimal::from(i),
                quantity: dec!(1),
                aggressor: OrderSide::Buy,
                timestamp: i,
            });
        }
        assert_eq!(ring.len(), 3);
        // Newest first: 4, 3, 2.
        let recent = ring.recent(3);
        assert_eq!(recent[0].price, dec!(4));
        assert_eq!(recent[2].price, dec!(2));
    }

    #[test]
    fn footprint_add_saturates_on_overflow() {
        let sym = Symbol::new("BTC", "USDT");
        let mut footprint = Footprint::default();
        let huge = |quantity: Decimal| TradePrint {
            symbol: sym.clone(),
            price: dec!(100),
            quantity,
            aggressor: OrderSide::Buy,
            timestamp: 0,
        };
        footprint.add(&huge(Decimal::MAX));
        // A second near-max add would overflow Decimal; it saturates instead.
        footprint.add(&huge(Decimal::MAX));
        assert_eq!(footprint.at(dec!(100)), Some((Decimal::MAX, Decimal::ZERO)));
    }

    #[test]
    fn indicator_set_warms_up_then_reports() {
        let price = TickInput::price;
        let mut set = IndicatorSet::default();
        for _ in 0..19 {
            set.update(&price(100.0));
        }
        // Sma(20) is still warming up after 19 inputs.
        assert_eq!(set.values()[0].1, None);
        set.update(&price(100.0));
        assert_eq!(set.values()[0].1, Some(100.0));
    }

    #[test]
    fn an_indicator_series_records_one_point_per_tick_after_warmup() {
        let price = TickInput::price;
        let mut set = IndicatorSet::from_specs(&[IndicatorSpec::new("Sma", vec![3.0])]).unwrap();
        for step in 0..10 {
            set.update(&price(100.0 + f64::from(step)));
        }
        // Sma(3) is silent for the first two ticks, so eight of the ten recorded.
        assert_eq!(set.snapshot()[0].series.len(), 8);
    }

    #[test]
    fn a_bar_indicator_series_carries_its_value_forward_between_bars() {
        // Atr only advances when a bar closes. Recording only its updates would
        // give it a series several times shorter than the price series, and an
        // overlay drawn from it would sit at the wrong place on the x-axis.
        let mut set = IndicatorSet::from_specs(&[IndicatorSpec::new("Atr", vec![2.0])]).unwrap();
        let mut builder = CandleBuilder::new(Timeframe::parse("1s").unwrap());
        let mut ticks = 0;
        for step in 0..40_i64 {
            // Four trades per bar, so only one tick in four closes one.
            let price = 100.0 + (step % 4) as f64;
            let closed = builder.update(price, 1.0, step * 250);
            let mut tick = TickInput::price(price);
            tick.candle = closed;
            set.update(&tick);
            ticks += 1;
        }
        let series = &set.snapshot()[0].series;
        assert!(!series.is_empty(), "Atr recorded nothing over ten bars");
        assert!(
            series.len() > ticks / 4,
            "series of {} is barely longer than the {} bars, so it is not carrying forward",
            series.len(),
            ticks / 4
        );
    }

    #[test]
    fn an_indicator_series_is_bounded() {
        let price = TickInput::price;
        let mut set = IndicatorSet::from_specs(&[IndicatorSpec::new("Sma", vec![2.0])]).unwrap();
        for step in 0..500 {
            set.update(&price(100.0 + f64::from(step)));
        }
        assert_eq!(set.snapshot()[0].series.len(), INDICATOR_SERIES);
    }

    #[test]
    fn a_warming_up_indicator_has_no_series_yet() {
        let price = TickInput::price;
        let mut set = IndicatorSet::from_specs(&[IndicatorSpec::new("Sma", vec![50.0])]).unwrap();
        for _ in 0..10 {
            set.update(&price(100.0));
        }
        assert!(set.snapshot()[0].series.is_empty());
    }

    #[test]
    fn indicator_labels_come_from_the_spec() {
        let set = IndicatorSet::default();
        let labels: Vec<String> = set.values().into_iter().map(|(l, _)| l).collect();
        assert_eq!(labels, vec!["Sma(20)".to_string(), "Ema(50)".to_string()]);
    }

    #[test]
    fn an_unknown_indicator_spec_is_rejected_by_name() {
        let err = IndicatorSet::from_specs(&[IndicatorSpec::new("NotReal", vec![])])
            .expect_err("an unknown indicator must be rejected")
            .to_string();
        assert!(
            err.contains("NotReal"),
            "error does not name the spec: {err}"
        );
    }

    #[test]
    fn a_configured_indicator_set_replaces_the_default() {
        let set = IndicatorSet::from_specs(&[IndicatorSpec::new("Rsi", vec![14.0])]).unwrap();
        let labels: Vec<String> = set.values().into_iter().map(|(l, _)| l).collect();
        assert_eq!(labels, vec!["Rsi(14)".to_string()]);
    }

    #[test]
    fn a_candle_indicator_advances_only_when_a_bar_closes() {
        // Atr reads bars. Feeding one-second trades under a one-minute bar size
        // must leave it silent until the minute rolls over.
        let sym = Symbol::new("BTC", "USDT");
        let mut state = AppState {
            indicators: vec![IndicatorSpec::new("Atr", vec![2.0])],
            timeframe: Timeframe::parse("1m").unwrap(),
            ..AppState::default()
        };
        for step in 0..30_i64 {
            let Event::Trade(mut print) = trade(&sym, dec!(100), OrderSide::Buy) else {
                panic!("the trade helper must produce a trade event");
            };
            print.timestamp = step * 1_000;
            state.fold(0, &sym, &Event::Trade(print));
        }
        let market = state.get(&(0, sym.clone())).unwrap();
        assert_eq!(
            market.indicators.values()[0].1,
            None,
            "Atr reported before a bar closed"
        );
        assert!(
            market.candles.partial().is_some(),
            "the builder should hold a bar in progress"
        );
    }

    #[test]
    fn account_and_lifecycle_events_do_not_change_market_state() {
        let sym = Symbol::new("BTC", "USDT");
        let mut state = AppState::default();
        state.fold(0, &sym, &trade(&sym, dec!(100), OrderSide::Buy));
        let before = state.get(&(0, sym.clone())).unwrap().last;
        state.fold(0, &sym, &Event::Disconnected);
        state.fold(0, &sym, &Event::BalanceUpdate(vec![]));
        let after = state.get(&(0, sym.clone())).unwrap().last;
        assert_eq!(before, after);
    }

    /// Two markets, several bars each, folded through the real path.
    ///
    /// Everything the breadth family reads is assembled by `AppState` rather
    /// than handed in, so this drives the wiring the registry tests cannot:
    /// the per-symbol fold on each closed bar, the universe gathered across
    /// markets before one is borrowed, and the reading that comes back out.
    fn breadth_terminal(kind: &str) -> (AppState, Symbol, Symbol) {
        let state = AppState {
            indicators: vec![IndicatorSpec {
                kind: kind.to_string(),
                params: Vec::new(),
                reference: None,
            }],
            ..Default::default()
        };
        (
            state,
            Symbol::new("BTC", "USDT"),
            Symbol::new("ETH", "USDT"),
        )
    }

    /// A print that lands in the bar starting at `bar`, at `price`.
    fn print_in_bar(sym: &Symbol, price: Decimal, bar: i64) -> Event {
        Event::Trade(TradePrint {
            symbol: sym.clone(),
            price,
            quantity: dec!(3),
            aggressor: OrderSide::Buy,
            timestamp: bar * 60_000 + 1,
        })
    }

    #[test]
    fn a_breadth_indicator_reads_the_whole_universe() {
        let (mut state, btc, eth) = breadth_terminal("AdvanceDecline");
        // Two markets moving in opposite directions, so an advance/decline
        // reading has both sides to count. Enough bars to clear the warmup.
        for bar in 0..40 {
            let up = Decimal::from(100 + bar);
            let down = Decimal::from(100 - bar);
            state.fold(0, &btc, &print_in_bar(&btc, up, bar));
            state.fold(0, &eth, &print_in_bar(&eth, down, bar));
        }
        let reading = state
            .get(&(0, btc.clone()))
            .expect("BTC is tracked")
            .indicators
            .values()
            .first()
            .and_then(|(_, reading)| *reading);
        assert!(
            reading.is_some(),
            "AdvanceDecline produced no reading after 40 bars across two markets"
        );
    }

    #[test]
    fn the_universe_is_absent_until_a_bar_closes() {
        let (mut state, btc, _eth) = breadth_terminal("AdvanceDecline");
        // One print does not close a bar, so no market is ready and the
        // universe cannot be assembled. `CrossSection::new` rejects an empty
        // one, which is why this has to be None rather than an empty reading.
        state.fold(0, &btc, &print_in_bar(&btc, dec!(100), 0));
        assert!(state.cross_section(0).is_none());
    }

    #[test]
    fn the_universe_is_not_assembled_when_nothing_reads_it() {
        // The default overlay has no breadth indicator, so the gather is
        // skipped entirely -- the same economy `references` makes. Asserted
        // through the state rather than by timing: a market that never folded a
        // bar into its breadth state stays unready.
        let mut state = AppState::default();
        let btc = Symbol::new("BTC", "USDT");
        for bar in 0..5 {
            state.fold(0, &btc, &print_in_bar(&btc, Decimal::from(100 + bar), bar));
        }
        assert!(
            !state
                .indicators
                .iter()
                .any(|s| registry::is_cross_section(&s.kind)),
            "the default overlay should carry no breadth indicator"
        );
    }

    #[test]
    fn a_market_that_has_not_closed_a_bar_is_left_out_of_the_universe() {
        let (mut state, btc, eth) = breadth_terminal("AdvanceDecline");
        // BTC closes bars; ETH prints once and never closes one. The universe
        // must hold one member, not two -- entering ETH as unchanged would
        // count it as neither advancing nor declining and pull every ratio
        // toward the middle.
        for bar in 0..4 {
            state.fold(0, &btc, &print_in_bar(&btc, Decimal::from(100 + bar), bar));
        }
        state.fold(0, &eth, &print_in_bar(&eth, dec!(50), 0));
        let universe = state.cross_section(1).expect("BTC has closed bars");
        assert_eq!(universe.members.len(), 1);
    }

    #[test]
    fn breadth_is_folded_per_bar_not_per_tick() {
        let (mut state, btc, _eth) = breadth_terminal("AdvanceDecline");
        // Four prints inside one bar, then one that closes it. A per-tick fold
        // would move `change` four times and count the market as advancing
        // repeatedly within a single bar.
        for step in 0..4 {
            state.fold(0, &btc, &print_in_bar(&btc, Decimal::from(100 + step), 0));
        }
        assert!(
            state.cross_section(0).is_none(),
            "no bar has closed yet, so there is nothing to read"
        );
        state.fold(0, &btc, &print_in_bar(&btc, dec!(110), 1));
        assert!(
            state.cross_section(1).is_some(),
            "the first bar closed, so the market is now a member"
        );
    }

    /// The taker flow is the terminal's own contribution to a derivatives tick.
    ///
    /// wickra-exchange's `DerivativesTickBuilder` passes zero for both taker
    /// volumes and says so in a comment: they stay zero "until a trade-derived
    /// source sets it". This terminal is one -- the aggressor side is on every
    /// print -- which is the entire reason the channels are folded here rather
    /// than there. Without this, `TakerBuySellRatio` reads a constant.
    #[test]
    fn the_taker_flow_comes_from_the_tape_not_the_derivatives_feed() {
        let mut derivatives = DerivativesState::default();
        derivatives.apply(&DerivativesUpdate {
            mark_price: Some(20_000.0),
            index_price: Some(20_000.0),
            futures_price: Some(20_050.0),
            timestamp: 1,
            ..DerivativesUpdate::default()
        });
        for _ in 0..3 {
            derivatives.add_trade(2.0, OrderSide::Buy);
        }
        derivatives.add_trade(2.0, OrderSide::Sell);

        let tick = derivatives.tick().expect("the three prices have arrived");
        assert!(
            (tick.taker_buy_volume - 6.0).abs() < 1e-9,
            "three buys of two should be six, got {}",
            tick.taker_buy_volume
        );
        assert!(
            (tick.taker_sell_volume - 2.0).abs() < 1e-9,
            "one sell of two should be two, got {}",
            tick.taker_sell_volume
        );
    }

    #[test]
    fn a_derivatives_tick_needs_its_prices_first() {
        let mut derivatives = DerivativesState::default();
        // Funding alone is not a tick: `DerivativesTick::new` rejects a
        // non-positive mark, index or futures price, so a host feeding only
        // the funding channel must not produce one.
        derivatives.apply(&DerivativesUpdate {
            funding_rate: Some(0.0001),
            timestamp: 1,
            ..DerivativesUpdate::default()
        });
        assert!(derivatives.tick().is_none());
    }

    #[test]
    fn liquidations_accumulate_and_other_channels_replace() {
        let mut derivatives = DerivativesState::default();
        let priced = DerivativesUpdate {
            mark_price: Some(20_000.0),
            index_price: Some(20_000.0),
            futures_price: Some(20_050.0),
            timestamp: 1,
            ..DerivativesUpdate::default()
        };
        derivatives.apply(&priced);
        // A liquidation is a flow over the interval: two prints inside one
        // interval are two liquidations, not the second one only.
        for _ in 0..3 {
            derivatives.apply(&DerivativesUpdate {
                long_liquidation: Some(1_000.0),
                open_interest: Some(500.0),
                ..DerivativesUpdate::default()
            });
        }
        let tick = derivatives.tick().expect("priced");
        assert!((tick.long_liquidation - 3_000.0).abs() < 1e-9);
        // Open interest is a level, not a flow, so it replaces.
        assert!((tick.open_interest - 500.0).abs() < 1e-9);
    }

    /// The mid is read before the print is folded, not after.
    ///
    /// The whole microstructure measurement is how far a print landed from the
    /// mid that was STANDING when it arrived. A trade does not move the book,
    /// so reading it in the fold is free -- but reading it from a book the
    /// print had already been applied to would measure nothing at all.
    #[test]
    fn a_trade_quote_pairs_the_print_with_the_standing_mid() {
        let sym = Symbol::new("BTC", "USDT");
        let mut book = BookState::default();
        book.apply_snapshot(&OrderBookSnapshot {
            symbol: sym.clone(),
            bids: vec![BookLevel {
                price: dec!(99),
                quantity: dec!(5),
            }],
            asks: vec![BookLevel {
                price: dec!(101),
                quantity: dec!(5),
            }],
            last_update_id: 1,
        });
        assert_eq!(book.mid(), Some(dec!(100)));
    }

    #[test]
    fn a_one_sided_book_has_no_mid_to_measure_against() {
        let sym = Symbol::new("BTC", "USDT");
        let mut book = BookState::default();
        book.apply_snapshot(&OrderBookSnapshot {
            symbol: sym,
            bids: vec![BookLevel {
                price: dec!(99),
                quantity: dec!(5),
            }],
            asks: Vec::new(),
            last_update_id: 1,
        });
        // No ask means no mid, and a TradeQuote without one is not a quote:
        // `TradeQuote::new` rejects a non-positive mid, and half a book has no
        // defensible one to offer.
        assert!(book.mid().is_none());
    }

    #[test]
    fn a_microstructure_indicator_reads_prints_against_the_book() {
        let sym = Symbol::new("BTC", "USDT");
        let mut state = AppState {
            indicators: vec![IndicatorSpec {
                kind: "EffectiveSpread".to_string(),
                params: Vec::new(),
                reference: None,
            }],
            ..Default::default()
        };
        // A book on both sides, then prints that cross it. Without the book
        // there is no mid and the family stays silent, which is the case the
        // wiring has to get right.
        for step in 0..40 {
            let mid = Decimal::from(100 + step % 3);
            state.fold(
                0,
                &sym,
                &Event::BookSnapshot(OrderBookSnapshot {
                    symbol: sym.clone(),
                    bids: vec![BookLevel {
                        price: mid - dec!(1),
                        quantity: dec!(5),
                    }],
                    asks: vec![BookLevel {
                        price: mid + dec!(1),
                        quantity: dec!(5),
                    }],
                    last_update_id: u64::try_from(step).unwrap_or(0),
                }),
            );
            state.fold(0, &sym, &trade(&sym, mid + dec!(1), OrderSide::Buy));
        }
        let reading = state
            .get(&(0, sym))
            .expect("BTC is tracked")
            .indicators
            .values()
            .first()
            .and_then(|(_, reading)| *reading);
        assert!(
            reading.is_some(),
            "EffectiveSpread produced no reading after 40 prints against a two-sided book"
        );
    }
}
