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
use crate::config::{IndicatorSpec, MAX_RECORDING};
use crate::error::{Error, Result};
use crate::registry::{self, TickIndicator, TickInput};
use crate::source::{DataSource, SourceId, Symbol};

/// Box size for the point-and-figure column state, as a fraction of price.
///
/// A percentage box rather than a fixed increment, because this terminal tracks
/// whatever markets a config names: one box of $1 is noise on an index and a
/// whole trend on a small cap. One percent is the conventional default for a
/// percentage-box chart.
const PNF_BOX_FRACTION: f64 = 0.01;

/// Boxes of counter-move needed to start a new column. Three is the standard.
const PNF_REVERSAL_BOXES: f64 = 3.0;

/// Point-and-figure column state for one market.
///
/// Exists for exactly one reading: `BullishPercentIndex` counts what share of a
/// universe sits on a P&F BUY signal, and that signal is not a price level — it
/// is a property of the column history. A market is on a buy signal from the
/// moment a rising column exceeds the previous rising column's high (a
/// double-top breakout) until a falling column undercuts the previous falling
/// column's low.
///
/// Price is folded, not sampled: the column only advances when the close moves a
/// whole box, and only reverses on a counter-move of `PNF_REVERSAL_BOXES`. That
/// filtering is the point of the chart — it is why a P&F signal is not the same
/// thing as "the price went up".
#[derive(Debug, Default)]
struct PointAndFigure {
    /// True while the current column is rising (an X column).
    rising: bool,
    /// The high of the current X column, or the low of the current O column.
    extreme: f64,
    /// The high of the last completed X column, which a breakout must exceed.
    previous_high: Option<f64>,
    /// The low of the last completed O column, which a breakdown must undercut.
    previous_low: Option<f64>,
    /// Whether the market is currently on a buy signal.
    on_buy_signal: bool,
    /// False until the first close has seeded a column.
    started: bool,
}

impl PointAndFigure {
    /// Fold one close.
    fn update(&mut self, close: f64) {
        if !close.is_finite() || close <= 0.0 {
            return;
        }
        if !self.started {
            self.started = true;
            self.rising = true;
            self.extreme = close;
            return;
        }
        let box_size = close * PNF_BOX_FRACTION;
        let reversal = box_size * PNF_REVERSAL_BOXES;

        if self.rising {
            if close >= self.extreme + box_size {
                self.extreme = close;
                // A rising column that clears the previous rising column's high
                // is the double-top breakout: the signal turns to buy and stays
                // there until a breakdown takes it away.
                if self.previous_high.is_some_and(|high| close > high) {
                    self.on_buy_signal = true;
                }
            } else if close <= self.extreme - reversal {
                self.previous_high = Some(self.extreme);
                self.rising = false;
                self.extreme = close;
            }
        } else if close <= self.extreme - box_size {
            self.extreme = close;
            if self.previous_low.is_some_and(|low| close < low) {
                self.on_buy_signal = false;
            }
        } else if close >= self.extreme + reversal {
            self.previous_low = Some(self.extreme);
            self.rising = true;
            self.extreme = close;
        }
    }
}

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
    /// Point-and-figure column state, for the buy-signal breadth reading.
    point_and_figure: PointAndFigure,
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
            point_and_figure: PointAndFigure::default(),
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
        self.point_and_figure.update(close);
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
            self.point_and_figure.on_buy_signal,
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

/// One profile tracked for a symbol: its label and its latest histogram.
struct ProfileEntry {
    label: String,
    profile: Box<dyn registry::ProfileIndicator>,
    last: Option<registry::ProfileReading>,
}

/// A profile is a trait object, so the label and the histogram are what a
/// reader can be shown -- the same shape `IndicatorEntry` prints.
impl std::fmt::Debug for ProfileEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProfileEntry")
            .field("label", &self.label)
            .field("last", &self.last)
            .finish_non_exhaustive()
    }
}

/// The set of profiles tracked for a symbol.
///
/// Separate from [`IndicatorSet`] because a profile answers with a histogram
/// rather than a reading. Driving them from one set would mean a reading type
/// that is sometimes a number and sometimes a distribution, and every consumer
/// learning which.
#[derive(Debug, Default)]
pub struct ProfileSet {
    entries: Vec<ProfileEntry>,
}

impl ProfileSet {
    /// Build the set a config asks for.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Config`] naming the profile if a spec's kind is not a
    /// profile or its parameters are rejected.
    pub fn from_specs(specs: &[IndicatorSpec]) -> Result<Self> {
        let entries = specs
            .iter()
            .map(|spec| {
                Ok(ProfileEntry {
                    label: spec.label(),
                    profile: registry::build_profile(&spec.kind, &spec.params)?,
                    last: None,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        Ok(Self { entries })
    }

    /// Feed one tick to every profile.
    pub fn update(&mut self, input: &TickInput) {
        for entry in &mut self.entries {
            if let Some(reading) = entry.profile.update(input) {
                entry.last = Some(reading);
            }
        }
    }

    /// Every profile's label and latest histogram, in configured order.
    ///
    /// A profile that has not produced one yet is listed with `None` rather
    /// than left out, so a panel's rows do not reorder as the session warms up.
    #[must_use]
    pub fn readings(&self) -> Vec<(&str, Option<&registry::ProfileReading>)> {
        self.entries
            .iter()
            .map(|entry| (entry.label.as_str(), entry.last.as_ref()))
            .collect()
    }

    /// Whether any profile is tracked.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// How many completed bars each alternative chart keeps.
///
/// Bounded, like everything else the fold writes into: a fast session can
/// complete several bars from one candle, and an unbounded ring would grow
/// with the session rather than with the screen.
const ALT_BARS_KEPT: usize = 256;

/// How many closed bars a market keeps for the chart.
///
/// Bounded for the same reason everything else here is: a terminal left running
/// overnight must not grow without limit. 256 one-minute bars is four hours,
/// which is more than any renderer draws at once and enough for the widest
/// window a chart panel asks for.
const OHLC_HISTORY: usize = 256;

/// How many recent prices a market keeps for the chart's tick series.
///
/// Named rather than written three times: the bound appeared as a bare `512` at
/// each of the two places that enforce it and once more at the allocation, so
/// raising it meant finding all three -- and `docs/PANELS.md` states it as a
/// ceiling a reader is entitled to trust.
const PRICE_HISTORY: usize = 512;

/// One named alternative bar stream and the bars it has completed.
struct BarEntry {
    label: String,
    stream: Box<dyn registry::BarStream>,
    bars: VecDeque<registry::AltBar>,
}

/// A bar stream is a trait object; the label and the bars are what a reader
/// can be shown.
impl std::fmt::Debug for BarEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BarEntry")
            .field("label", &self.label)
            .field("bars", &self.bars.len())
            .finish_non_exhaustive()
    }
}

/// The set of alternative bar streams tracked for a symbol.
#[derive(Debug, Default)]
pub struct BarSet {
    entries: Vec<BarEntry>,
}

impl BarSet {
    /// Build the set a config asks for.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Config`] naming the entry if a spec's kind is not a
    /// bar type or its parameters are rejected.
    pub fn from_specs(specs: &[IndicatorSpec]) -> Result<Self> {
        let entries = specs
            .iter()
            .map(|spec| {
                Ok(BarEntry {
                    label: spec.label(),
                    stream: registry::build_bars(&spec.kind, &spec.params)?,
                    bars: VecDeque::with_capacity(ALT_BARS_KEPT),
                })
            })
            .collect::<Result<Vec<_>>>()?;
        Ok(Self { entries })
    }

    /// Feed one tick to every stream, keeping what it completes.
    pub fn update(&mut self, input: &TickInput) {
        for entry in &mut self.entries {
            for bar in entry.stream.update(input) {
                if entry.bars.len() == ALT_BARS_KEPT {
                    entry.bars.pop_front();
                }
                entry.bars.push_back(bar);
            }
        }
    }

    /// Every stream's label and its most recent bars, oldest first.
    #[must_use]
    pub fn streams(&self, depth: usize) -> Vec<(&str, Vec<registry::AltBar>)> {
        self.entries
            .iter()
            .map(|entry| {
                let take = entry.bars.len().saturating_sub(depth);
                (
                    entry.label.as_str(),
                    entry.bars.iter().skip(take).copied().collect(),
                )
            })
            .collect()
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
    /// The profile set, for the profile panel.
    pub profiles: ProfileSet,
    /// The alternative bar streams, for the bars panel.
    pub bars: BarSet,
    /// The last traded price seen.
    pub last: Decimal,
    /// The venue's best bid, from the ticker stream.
    ///
    /// Zero until a ticker arrives. The book carries a best bid too, and it is
    /// not the same number: the book's is whatever the depth stream has
    /// delivered so far, and a venue that publishes a truncated book publishes a
    /// ticker that is not truncated.
    pub bid: Decimal,
    /// The venue's best ask, from the ticker stream. Zero until one arrives.
    pub ask: Decimal,
    /// The venue's rolling base-asset volume, from the ticker stream.
    ///
    /// The venue's own window, not a total of the prints this terminal has seen
    /// -- a terminal opened five minutes ago has seen five minutes of them.
    pub volume: Decimal,
    /// The first price this market was ever folded at, and what a change is
    /// measured from.
    ///
    /// Not the venue's session open: a venue's day boundary is its own, and the
    /// terminal is not told where it falls. This is the open of the window the
    /// terminal has actually watched -- the first backfilled bar's open when a
    /// subscription is seeded, and otherwise the first price to arrive. Naming
    /// it a session open would claim a boundary nothing here knows.
    pub open: Decimal,
    /// A bounded recent price history for the chart series.
    pub history: VecDeque<Decimal>,
    /// Aggregates the trade stream into the bars the candle indicators read.
    pub candles: CandleBuilder,
    /// The closed bars the chart draws, oldest first.
    ///
    /// Kept because the builder does not: it holds the bar in progress and
    /// hands each closed one to the indicators, which read it and keep only
    /// their own state. Without this ring a renderer could draw the last price
    /// and nothing else -- which is what every renderer here did.
    ohlc: VecDeque<wc::Candle>,
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
    pub fn new(
        specs: &[IndicatorSpec],
        profiles: &[IndicatorSpec],
        bars: &[IndicatorSpec],
        timeframe: Timeframe,
    ) -> Result<Self> {
        Ok(Self {
            book: BookState::default(),
            tape: TapeRing::default(),
            footprint: Footprint::default(),
            indicators: IndicatorSet::from_specs(specs)?,
            profiles: ProfileSet::from_specs(profiles)?,
            bars: BarSet::from_specs(bars)?,
            last: Decimal::ZERO,
            bid: Decimal::ZERO,
            ask: Decimal::ZERO,
            volume: Decimal::ZERO,
            open: Decimal::ZERO,
            history: VecDeque::with_capacity(PRICE_HISTORY),
            candles: CandleBuilder::new(timeframe),
            ohlc: VecDeque::with_capacity(OHLC_HISTORY),
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

impl SymbolState {
    /// Seed this market from historical bars, oldest first.
    ///
    /// What a fresh subscription to a live venue starts with, and what the
    /// terminal had no way to use: every bar was built from ticks it saw itself,
    /// so a bar indicator was silent for its whole warmup in wall-clock time --
    /// fourteen hours for `Atr(14)` at an hourly timeframe -- and the chart
    /// opened empty on a market that has traded for years.
    ///
    /// The bars drive the same tick the live fold drives, with the close as the
    /// price: that is all a bar carries, and it is what every charting platform
    /// warms an indicator on. Only the bar-derived state is seeded. The book,
    /// the tape and the footprint are not: a bar records that trading happened,
    /// not the prints it was made of, and inventing those would put numbers on
    /// screen that no venue ever published.
    /// Remember the first price this market was folded at.
    ///
    /// Idempotent after the first non-zero price, so a change is measured from
    /// where the terminal started watching and does not walk forward with the
    /// market. A zero is refused rather than recorded: it is what an
    /// unparseable price folds to, and an open of zero would report every
    /// later price as an infinite gain.
    pub(crate) fn open_at(&mut self, price: Decimal) {
        if self.open.is_zero() && !price.is_zero() {
            self.open = price;
        }
    }

    pub(crate) fn seed_bars(&mut self, bars: &[wc::Candle]) {
        // History comes before anything live, so the open is the oldest bar's,
        // not the first tick that happens to arrive after it.
        if let Some(first) = bars.first() {
            if let Some(open) = Decimal::from_f64_retain(first.open) {
                self.open_at(open);
            }
        }
        for bar in bars {
            if self.ohlc.len() == OHLC_HISTORY {
                self.ohlc.pop_front();
            }
            self.ohlc.push_back(*bar);

            if self.history.len() == PRICE_HISTORY {
                self.history.pop_front();
            }
            if let Some(close) = Decimal::from_f64_retain(bar.close) {
                self.history.push_back(close);
                self.last = close;
            }

            self.breadth.update(bar);
            let mut tick = TickInput::price(bar.close);
            tick.candle = Some(*bar);
            self.indicators.update(&tick);
            self.profiles.update(&tick);
            self.bars.update(&tick);
        }
    }

    /// The most recent closed bars, oldest first, at most `n` of them.
    ///
    /// Closed only: the bar in progress is [`forming`](Self::forming), because
    /// it is the one that will still change and a renderer draws it
    /// differently.
    #[must_use]
    pub fn ohlc(&self, n: usize) -> Vec<wc::Candle> {
        let skip = self.ohlc.len().saturating_sub(n);
        self.ohlc.iter().skip(skip).copied().collect()
    }

    /// The bar still accumulating, or `None` before this market's first trade.
    #[must_use]
    pub fn forming(&self) -> Option<wc::Candle> {
        self.candles.partial()
    }

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
    /// The profile specs every market is tracked with, validated the same way.
    pub profiles: Vec<IndicatorSpec>,
    /// The alternative bar specs, validated the same way.
    pub bars: Vec<IndicatorSpec>,
    /// The bar size the candle-input indicators are fed at.
    pub timeframe: Timeframe,
    /// How many recent events to keep for export, or `None` to record nothing.
    ///
    /// Crate-visible rather than public: a caller sets it through
    /// [`set_recording`](Self::set_recording), which clamps the capacity and
    /// clears what is held, so the two can never disagree.
    pub(crate) record_capacity: Option<usize>,
    /// The recorded events, oldest first, in the shape `Replay` takes.
    pub(crate) recorded: VecDeque<Event>,
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
            .field("profiles", &self.profiles)
            .field("bars", &self.bars)
            .field("timeframe", &self.timeframe)
            .field("record_capacity", &self.record_capacity)
            // By count, like `sources`: a reader wants to know a recording is
            // running and how long it is, not to have thousands of trades
            // printed into a panic message.
            .field("recorded", &self.recorded.len())
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
            SymbolState::new(&self.indicators, &self.profiles, &self.bars, self.timeframe)
                .expect("indicator specs are validated before they reach the state")
        });
        match event {
            Event::Trade(print) => {
                let price = print.price.to_f64().unwrap_or(0.0);
                state.open_at(print.price);
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
                    if state.ohlc.len() == OHLC_HISTORY {
                        state.ohlc.pop_front();
                    }
                    state.ohlc.push_back(*bar);
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
                state.profiles.update(&tick);
                state.bars.update(&tick);
                if state.history.len() == PRICE_HISTORY {
                    state.history.pop_front();
                }
                state.history.push_back(print.price);
            }
            Event::Ticker(ticker) => {
                // Every field, not just the last price. The other three were
                // dropped on the floor, which is why a watchlist could show a
                // price and never a spread, a change or a volume: the numbers
                // arrived on every ticker message and were discarded here.
                state.open_at(ticker.last);
                state.last = ticker.last;
                state.bid = ticker.bid;
                state.ask = ticker.ask;
                state.volume = ticker.volume;
            }
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
        SymbolState::new(&self.indicators, &self.profiles, &self.bars, self.timeframe)
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
            // The kept bars go too. They are bars of the *old* size, and a chart
            // that drew them beside the new ones would show one series measured
            // two ways -- the same reason the indicator set is rebuilt rather
            // than continued.
            state.ohlc.clear();
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
            self.record(&ev);
            self.fold(id, &sym, &ev);
        }
        folded
    }

    /// Keep an event for export, if recording is on.
    ///
    /// Recorded here rather than inside `fold`, because `fold` is also how a
    /// seek re-folds a recording: recording there would append the replayed
    /// events back onto the recording, and every rewind would double it.
    fn record(&mut self, event: &Event) {
        let Some(capacity) = self.record_capacity else {
            return;
        };
        if self.recorded.len() == capacity {
            self.recorded.pop_front();
        }
        self.recorded.push_back(event.clone());
    }

    /// The recorded events, oldest first, in the shape `Replay` takes.
    #[must_use]
    pub fn recording(&self) -> Vec<Event> {
        self.recorded.iter().cloned().collect()
    }

    /// Turn recording on with a capacity, or off with `None`. Clears what is
    /// already held either way, so a capacity change never leaves a recording
    /// that is part one size and part another.
    pub fn set_recording(&mut self, capacity: Option<usize>) {
        self.record_capacity = capacity.map(|c| c.clamp(1, MAX_RECORDING));
        self.recorded = VecDeque::new();
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

    /// A ticker carries four numbers and all four are kept.
    ///
    /// The fold used to take `last` and drop the bid, the ask and the volume on
    /// the floor -- which is why a watchlist could show a price and never a
    /// spread or a turnover, though every ticker message had carried them all
    /// along.
    #[test]
    fn a_ticker_keeps_every_field_it_carries() {
        let sym = Symbol::new("BTC", "USDT");
        let mut state = AppState::default();
        state.fold(
            0,
            &sym,
            &Event::Ticker(Ticker {
                symbol: sym.clone(),
                last: dec!(20010),
                bid: dec!(20009),
                ask: dec!(20011),
                volume: dec!(1234),
            }),
        );

        let market = state.get(&(0, sym)).expect("the ticker created the market");
        assert_eq!(market.last, dec!(20010));
        assert_eq!(market.bid, dec!(20009));
        assert_eq!(market.ask, dec!(20011));
        assert_eq!(market.volume, dec!(1234));
    }

    /// The open is the first price folded and does not move with the market.
    ///
    /// If it walked forward the change would read zero on every frame, which is
    /// the shape this is easiest to get wrong in.
    #[test]
    fn the_open_is_the_first_price_and_stays_there() {
        let sym = Symbol::new("BTC", "USDT");
        let mut state = AppState::default();
        for price in [dec!(100), dec!(110), dec!(90)] {
            state.fold(0, &sym, &trade(&sym, price, OrderSide::Buy));
        }
        let market = state.get(&(0, sym)).expect("the trades created the market");
        assert_eq!(market.open, dec!(100));
        assert_eq!(market.last, dec!(90));
    }

    /// A zero price does not become the open.
    ///
    /// Zero is what an unparseable price folds to, and an open of zero would
    /// report every later price as an infinite gain.
    #[test]
    fn a_zero_price_does_not_open_the_window() {
        let sym = Symbol::new("BTC", "USDT");
        let mut state = AppState::default();
        state.fold(0, &sym, &trade(&sym, Decimal::ZERO, OrderSide::Buy));
        state.fold(0, &sym, &trade(&sym, dec!(100), OrderSide::Buy));
        let market = state.get(&(0, sym)).expect("the trades created the market");
        assert_eq!(market.open, dec!(100));
    }

    /// Backfilled history opens the window, not the first live tick after it.
    ///
    /// A subscription that seeds two hundred bars and then measures its change
    /// from the next print would report a flat market on a day that had moved.
    #[test]
    fn seeded_history_sets_the_open_from_its_oldest_bar() {
        let bars: Vec<wickra_core::Candle> = [(100.0, 0), (140.0, 1)]
            .into_iter()
            .map(|(close, ts)| {
                wickra_core::Candle::new(close, close, close, close, 1.0, ts)
                    .expect("an ordered candle")
            })
            .collect();
        let mut state = SymbolState::new(&[], &[], &[], Timeframe::default())
            .expect("an empty indicator set is constructible");
        state.seed_bars(&bars);
        assert!((state.open.to_f64().unwrap_or(0.0) - 100.0).abs() < 1e-9);
    }

    /// The rings answer whether they hold anything, and nothing asked.
    ///
    /// `is_empty` beside a `len` is not decoration -- it is what a caller reads
    /// to decide whether to draw a panel at all, and a `len() == 0` written at
    /// each call site is the same question answered four ways.
    #[test]
    fn the_rings_report_emptiness_before_and_after_a_print() {
        let sym = Symbol::new("BTC", "USDT");
        let mut state = AppState::default();

        let market = SymbolState::new(&[], &[], &[], Timeframe::default())
            .expect("an empty indicator set is constructible");
        assert!(market.tape.is_empty());
        assert!(market.footprint.is_empty());

        state.fold(0, &sym, &trade(&sym, dec!(100), OrderSide::Buy));
        let market = state.get(&(0, sym)).expect("the trade created the market");
        assert!(!market.tape.is_empty());
        assert!(!market.footprint.is_empty());
    }

    /// A set says whether it needs a reference market, and a set with none says
    /// so.
    ///
    /// The fold reads this to decide whether to gather the other markets'
    /// prices at all, so a set that answered wrongly would either do the work
    /// for nothing or leave a pairwise indicator without its second input.
    #[test]
    fn an_indicator_set_says_whether_it_needs_a_reference() {
        let plain = IndicatorSet::from_specs(&[IndicatorSpec::new("Sma", vec![3.0])])
            .expect("Sma is registered");
        assert!(!plain.wants_references());

        let paired = IndicatorSet::from_specs(&[IndicatorSpec::paired(
            "RollingCorrelation",
            vec![20.0],
            "ETH/USDT",
        )])
        .expect("RollingCorrelation is registered");
        assert!(paired.wants_references());
    }

    /// A bar with a close that is not a number is dropped, not folded.
    ///
    /// A NaN close would make the change NaN and stay there, and every breadth
    /// reading taken afterwards with it -- so the guard is the difference
    /// between one bad bar and a session of them.
    #[test]
    fn a_breadth_bar_with_no_usable_close_is_ignored() {
        let mut breadth = BreadthState::new();
        let good =
            wickra_core::Candle::new(100.0, 101.0, 99.0, 100.0, 5.0, 0).expect("an ordered candle");
        breadth.update(&good);
        let after_good = breadth.previous_close;

        // `Candle::new` refuses a NaN, so the bad bar is assembled past it --
        // which is exactly how one reaches the fold: from a feed, not from a
        // constructor.
        let mut bad = good;
        bad.close = f64::NAN;
        breadth.update(&bad);
        assert_eq!(
            breadth.previous_close, after_good,
            "a NaN close moved the previous close"
        );
        assert!(breadth.change.is_finite(), "the change went to NaN");
    }

    /// A bar with an unusable volume folds as zero rather than poisoning it.
    #[test]
    fn a_breadth_bar_with_no_usable_volume_reads_as_zero() {
        let mut breadth = BreadthState::new();
        let mut bar =
            wickra_core::Candle::new(100.0, 101.0, 99.0, 100.0, 5.0, 0).expect("an ordered candle");
        bar.volume = f64::NEG_INFINITY;
        breadth.update(&bar);
        assert!(
            breadth.volume.abs() < f64::EPSILON,
            "volume: {}",
            breadth.volume
        );
    }

    /// `IndicatorEntry` prints its label and the length of its series.
    ///
    /// The indicator is a trait object with no `Debug` bound, so the label
    /// stands in for it; dumping the series would bury the fields a reader of a
    /// panic message is actually after.
    #[test]
    fn an_indicator_entry_prints_its_label_and_not_its_series() {
        let sym = Symbol::new("BTC", "USDT");
        let mut state = AppState {
            indicators: vec![IndicatorSpec::new("Sma", vec![2.0])],
            ..AppState::default()
        };
        for price in [dec!(100), dec!(101), dec!(102)] {
            state.fold(0, &sym, &trade(&sym, price, OrderSide::Buy));
        }

        let shown = format!("{:?}", state.get(&(0, sym)).expect("the market exists"));
        assert!(shown.contains("IndicatorEntry"), "{shown}");
        assert!(shown.contains("Sma(2)"), "the label is missing: {shown}");
        // Two, not three: a two-period average has no reading until its second
        // price, so the series is one shorter than the prints that fed it. The
        // number is what matters -- a length rather than the values themselves.
        assert!(
            shown.contains("series: 2"),
            "the series was not counted: {shown}"
        );
    }

    /// Seeding more history than the rings hold keeps the newest of it.
    ///
    /// A live subscription asks for as many bars as the config's `backfill`
    /// says, and nothing clamps that to what a `SymbolState` carries -- so a
    /// config asking for a thousand bars on a venue that has them walks both
    /// rings past their bound. They evict rather than grow, and the bar that
    /// survives has to be the newest one, because that is the one the chart
    /// draws and the one the next tick continues from.
    #[test]
    fn seeding_past_the_rings_evicts_the_oldest_bars() {
        let bars: Vec<wickra_core::Candle> = (0..600)
            .map(|i| {
                let close = 100.0 + f64::from(i);
                wickra_core::Candle::new(close, close, close, close, 1.0, i64::from(i))
                    .expect("an ordered candle")
            })
            .collect();

        let mut state = SymbolState::new(&[], &[], &[], Timeframe::default())
            .expect("an empty indicator set is constructible");
        state.seed_bars(&bars);

        assert_eq!(
            state.ohlc.len(),
            OHLC_HISTORY,
            "the bar ring grew past its bound"
        );
        assert_eq!(
            state.history.len(),
            512,
            "the price ring grew past its bound"
        );
        let newest = state.ohlc.back().expect("the ring is not empty");
        assert!(
            (newest.close - 699.0).abs() < 1e-9,
            "the newest bar was evicted"
        );
        let oldest = state.ohlc.front().expect("the ring is not empty");
        let first_kept = 700.0 - f64::from(u16::try_from(OHLC_HISTORY).expect("the ring is small"));
        assert!((oldest.close - first_kept).abs() < 1e-9);
    }

    /// `AppState` is what a panicking test prints, and it holds a recording that
    /// can run to thousands of events. Printing the ring itself would bury the
    /// fields a reader is actually after, so the recorder is reported by count
    /// -- and that only stays true if something reads it.
    #[test]
    fn the_debug_view_reports_the_recording_by_count() {
        let mut state = AppState::default();
        state.set_recording(Some(16));
        let sym = Symbol::new("BTC", "USDT");
        let event = trade(&sym, dec!(100), OrderSide::Buy);
        state.fold(0, &sym, &event);
        state.record(&event);

        let shown = format!("{state:?}");
        assert!(shown.contains("record_capacity: Some(16)"), "{shown}");
        assert!(shown.contains("recorded: 1"), "{shown}");
        assert!(shown.contains("sources: 0"), "{shown}");
    }

    /// A tick carrying one closed candle, which is all the bar streams read.
    fn candle_tick(open: f64, high: f64, low: f64, close: f64) -> TickInput {
        let mut input = TickInput::price(close);
        input.candle = Some(
            wickra_core::Candle::new(open, high, low, close, 1.0, 0).expect("an ordered candle"),
        );
        input
    }

    #[test]
    fn a_point_and_figure_column_ignores_a_price_that_is_not_one() {
        // The close comes from a fold that has already seen whatever the feed
        // sent, so a zero or a NaN reaches here rather than being filtered
        // upstream. Seeding a column from one would set the extreme to it and
        // every later box would be measured against nothing.
        let mut pnf = PointAndFigure::default();
        for bad in [f64::NAN, f64::INFINITY, 0.0, -5.0] {
            pnf.update(bad);
            assert!(!pnf.started, "{bad} started a column");
        }
        pnf.update(100.0);
        assert!(pnf.started);
    }

    #[test]
    fn taker_flow_ignores_a_quantity_that_is_not_one() {
        // Adding a NaN size makes the running total NaN for the rest of the
        // session, and every derivatives indicator reading with it.
        let mut state = DerivativesState::default();
        state.add_trade(f64::NAN, OrderSide::Buy);
        state.add_trade(-1.0, OrderSide::Sell);
        assert!(state.taker_buy_volume.abs() < 1e-9);
        assert!(state.taker_sell_volume.abs() < 1e-9);
        state.add_trade(2.5, OrderSide::Buy);
        assert!((state.taker_buy_volume - 2.5).abs() < 1e-9);
    }

    #[test]
    fn a_profile_set_knows_whether_it_tracks_anything() {
        assert!(ProfileSet::from_specs(&[]).unwrap().is_empty());
        let one =
            ProfileSet::from_specs(&[IndicatorSpec::new("VolumeProfile", vec![4.0, 8.0])]).unwrap();
        assert!(!one.is_empty());
    }

    #[test]
    fn a_profile_entry_debugs_without_printing_its_whole_histogram() {
        // The entry holds an indicator that has no Debug and a reading that can
        // be hundreds of bins, so the impl is written by hand; a reader wants
        // to know which profile it is looking at.
        let set =
            ProfileSet::from_specs(&[IndicatorSpec::new("VolumeProfile", vec![4.0, 8.0])]).unwrap();
        let shown = format!("{:?}", set.entries[0]);
        assert!(shown.contains("ProfileEntry"), "{shown}");
        assert!(shown.contains("VolumeProfile(4,8)"), "{shown}");
    }

    #[test]
    fn a_bar_entry_debugs_its_bar_count_rather_than_every_bar() {
        let set = BarSet::from_specs(&[IndicatorSpec::new("RenkoBars", vec![3.0])]).unwrap();
        let shown = format!("{:?}", set.entries[0]);
        assert!(shown.contains("BarEntry"), "{shown}");
        assert!(shown.contains("RenkoBars(3)"), "{shown}");
        assert!(shown.contains("bars: 0"), "{shown}");
    }

    #[test]
    fn a_bar_stream_keeps_the_most_recent_bars_and_drops_the_oldest() {
        // These streams run for as long as the terminal does, so the buffer is
        // bounded. Without the eviction it grows for the whole session.
        let mut set = BarSet::from_specs(&[IndicatorSpec::new("RenkoBars", vec![1.0])]).unwrap();
        let mut price = 100.0;
        for _ in 0..(ALT_BARS_KEPT * 2) {
            price += 2.0;
            set.update(&candle_tick(price - 2.0, price, price - 2.0, price));
        }
        let kept = set.entries[0].bars.len();
        assert_eq!(kept, ALT_BARS_KEPT, "kept {kept}");
        // The survivors are the newest: the last brick closes at the last price.
        let last = set.entries[0].bars.back().expect("a brick");
        assert!(last.close >= price - 2.0, "{last:?} against {price}");
    }
    use rust_decimal_macros::dec;
    use wickra_exchange_core::{Symbol, Ticker};

    fn trade(sym: &Symbol, price: Decimal, side: OrderSide) -> Event {
        Event::Trade(stamped(sym, price, side, 0))
    }

    /// A print with a chosen timestamp.
    ///
    /// Built directly rather than by destructuring what `trade` returns: taking
    /// the variant apart needs an `else` arm for a shape the constructor above
    /// cannot produce, and a branch no run can take is one no test can cover.
    fn stamped(sym: &Symbol, price: Decimal, side: OrderSide, timestamp: i64) -> TradePrint {
        TradePrint {
            symbol: sym.clone(),
            price,
            quantity: dec!(2),
            aggressor: side,
            timestamp,
        }
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
        let levels = st.footprint.len();
        assert!(
            levels <= MAX_FOOTPRINT_LEVELS,
            "the footprint holds {levels} levels"
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
        let kept = series.len();
        let bars = ticks / 4;
        assert!(
            kept > bars,
            "series of {kept} is barely longer than the {bars} bars, so it is not carrying forward"
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
            let print = stamped(&sym, dec!(100), OrderSide::Buy, step * 1_000);
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

    /// Drive a point-and-figure column through a sequence of closes.
    fn pnf(closes: &[f64]) -> PointAndFigure {
        let mut chart = PointAndFigure::default();
        for close in closes {
            chart.update(*close);
        }
        chart
    }

    /// The signal is a column property, not a price level.
    ///
    /// Around 100 the box is ~1.0 and the reversal ~3.0, so: seed at 100, run
    /// up to 102, reverse down to 98 (which completes an X column at 102),
    /// reverse back up, then clear 102. That last step is the double-top
    /// breakout and the only thing that turns the signal on.
    #[test]
    fn a_rising_column_clearing_the_previous_one_is_a_buy_signal() {
        assert!(
            !pnf(&[100.0, 102.0]).on_buy_signal,
            "no previous column to clear yet"
        );
        assert!(pnf(&[100.0, 102.0, 98.0, 102.0, 104.0]).on_buy_signal);
    }

    #[test]
    fn a_falling_column_undercutting_the_previous_one_takes_it_away() {
        let chart = pnf(&[100.0, 102.0, 98.0, 102.0, 104.0]);
        assert!(chart.on_buy_signal, "the setup should be on a buy signal");
        // Down to 100 (completing an X at 104), back up to 104, down again to
        // 100, then through it: the breakdown below the previous O low.
        let chart = pnf(&[100.0, 102.0, 98.0, 102.0, 104.0, 100.0, 104.0, 100.0, 96.0]);
        assert!(!chart.on_buy_signal);
    }

    #[test]
    fn a_move_smaller_than_a_box_does_not_advance_the_column() {
        // Half a box, repeatedly. A P&F chart is a filter: this is the whole
        // reason its signal is not the same thing as "the price went up".
        let chart = pnf(&[100.0, 100.5, 100.9, 100.4, 100.8]);
        assert!(
            (chart.extreme - 100.0).abs() < 1e-9,
            "the column did not move"
        );
        assert!(chart.rising);
    }

    #[test]
    fn a_counter_move_smaller_than_the_reversal_does_not_start_a_column() {
        // Two boxes down from a rising column is not three, so the column
        // stands: without this a P&F chart would be an ordinary line chart.
        let chart = pnf(&[100.0, 104.0, 102.0]);
        assert!(
            chart.rising,
            "a two-box pullback must not reverse the column"
        );
        assert!((chart.extreme - 104.0).abs() < 1e-9);
    }

    #[test]
    fn the_breadth_member_reports_the_point_and_figure_signal() {
        // The reason all of this exists: BullishPercentIndex reads this flag,
        // and it was hard-coded false until the column state existed.
        let mut breadth = BreadthState::new();
        let bar = |close: f64| {
            wc::Candle::new(close, close, close, close, 10.0, 0)
                .expect("a flat synthetic bar is valid")
        };
        for close in [100.0, 102.0, 98.0, 102.0, 104.0] {
            breadth.update(&bar(close));
        }
        let member = breadth.member().expect("bars have closed");
        assert!(member.on_buy_signal);
    }
}
