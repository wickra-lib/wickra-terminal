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
use wickra_core::{Ema, Indicator, Sma};
use wickra_exchange_core::{BookDelta, BookLevel, Event, OrderBookSnapshot, OrderSide, TradePrint};

use crate::source::{DataSource, SourceId, Symbol};

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

    /// The bid/ask spread, or `None` if either side is empty.
    #[must_use]
    pub fn spread(&self) -> Option<Decimal> {
        match (self.best_bid(), self.best_ask()) {
            (Some((bid, _)), Some((ask, _))) => Some(ask - bid),
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

/// Volume traded at each price, split by aggressor side (a footprint / volume
/// profile).
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
    }

    /// The (buy, sell) volume at `price`, if any has traded there.
    #[must_use]
    pub fn at(&self, price: Decimal) -> Option<(Decimal, Decimal)> {
        self.levels.get(&price).copied()
    }

    /// The top `n` price levels, highest price first, as `(price, buy, sell)`.
    #[must_use]
    pub fn top(&self, n: usize) -> Vec<(Decimal, Decimal, Decimal)> {
        self.levels
            .iter()
            .rev()
            .take(n)
            .map(|(price, &(buy, sell))| (*price, buy, sell))
            .collect()
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

/// One named streaming indicator plus its latest output.
struct IndicatorEntry {
    name: String,
    indicator: Box<dyn Indicator<Input = f64, Output = f64>>,
    last: Option<f64>,
}

/// The set of streaming indicators tracked for a symbol's chart panel.
pub struct IndicatorSet {
    entries: Vec<IndicatorEntry>,
}

impl IndicatorSet {
    /// Feed one price into every indicator, advancing their state by one input.
    pub fn update(&mut self, price: f64) {
        for entry in &mut self.entries {
            entry.last = entry.indicator.update(price);
        }
    }

    /// The latest `(name, value)` of each indicator (`value` is `None` while
    /// still warming up).
    #[must_use]
    pub fn values(&self) -> Vec<(String, Option<f64>)> {
        self.entries
            .iter()
            .map(|entry| (entry.name.clone(), entry.last))
            .collect()
    }
}

impl Default for IndicatorSet {
    /// A short and a long moving average — the default chart overlay.
    fn default() -> Self {
        let entries = vec![
            IndicatorEntry {
                name: "SMA(20)".to_string(),
                indicator: Box::new(Sma::new(20).unwrap()),
                last: None,
            },
            IndicatorEntry {
                name: "EMA(50)".to_string(),
                indicator: Box::new(Ema::new(50).unwrap()),
                last: None,
            },
        ];
        Self { entries }
    }
}

/// All state for a single market on a single source.
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
}

impl AppState {
    /// Fold one event for `(src, sym)` into state, in O(1) per event (bounded by
    /// the event's own size, never by history).
    pub fn fold(&mut self, src: SourceId, sym: &Symbol, event: &Event) {
        let state = self.symbols.entry((src, sym.clone())).or_default();
        match event {
            Event::Trade(print) => {
                state.last = print.price;
                state.tape.push(print.clone());
                state.footprint.add(print);
                state.indicators.update(print.price.to_f64().unwrap_or(0.0));
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
        let mut set = IndicatorSet::default();
        for _ in 0..19 {
            set.update(100.0);
        }
        // SMA(20) is still warming up after 19 inputs.
        assert_eq!(set.values()[0].1, None);
        set.update(100.0);
        assert_eq!(set.values()[0].1, Some(100.0));
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
}
