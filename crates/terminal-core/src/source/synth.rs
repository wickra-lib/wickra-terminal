//! A deterministic synthetic feed.
//!
//! [`SynthSource`] needs no network and no RNG entropy: given the same seed and
//! the same sequence of `subscribe`/`poll` calls it produces the exact same
//! events every run, on every platform. That makes it the zero-dependency demo
//! source and a reproducible fixture for tests. The price path is a seeded
//! integer walk (a linear congruential generator), so there are no transcendental
//! functions and no float non-determinism.

use std::collections::BTreeMap;

use rust_decimal::Decimal;

use super::{DataSource, SourceId, SourceKind, Symbol};
use crate::error::Result;
use wickra_exchange::{BookLevel, Event, OrderBookSnapshot, OrderSide, TradePrint};

/// Per-symbol synthetic state: the market, its current price and the walk's RNG
/// state. Keyed in an ordered map by the symbol's string form so iteration —
/// and therefore the event stream — is deterministic across runs.
struct SynthSym {
    symbol: Symbol,
    price: Decimal,
    lcg: u64,
    seq: u64,
}

/// A deterministic synthetic feed.
pub struct SynthSource {
    id: SourceId,
    seed: u64,
    /// Global tick counter (the synthetic timestamp), advanced once per `poll`.
    t: i64,
    symbols: BTreeMap<String, SynthSym>,
}

impl SynthSource {
    /// Construct a synthetic source seeded by `seed`.
    #[must_use]
    pub fn new(id: SourceId, seed: u64) -> Self {
        Self {
            id,
            seed,
            t: 0,
            symbols: BTreeMap::new(),
        }
    }

    /// A stable per-symbol starting price derived from the seed and the symbol,
    /// so different markets diverge but each run is reproducible.
    fn start_price(&self, sym: &Symbol) -> Decimal {
        let mut h = self.seed;
        for byte in sym.to_string().bytes() {
            h = h
                .wrapping_mul(1_099_511_628_211)
                .wrapping_add(u64::from(byte));
        }
        // Base in the 100.00 .. 1100.00 range. Built via `From<u64>` + division
        // to keep two-decimal precision without a `u64 as i64` cast.
        let cents = 10_000 + (h % 100_000);
        Decimal::from(cents) / Decimal::from(100u64)
    }
}

/// Advance an LCG and return the new state (Numerical Recipes constants).
fn lcg_step(state: u64) -> u64 {
    state
        .wrapping_mul(6_364_136_223_846_793_005)
        .wrapping_add(1_442_695_040_888_963_407)
}

impl DataSource for SynthSource {
    fn id(&self) -> SourceId {
        self.id
    }

    fn kind(&self) -> SourceKind {
        SourceKind::Synth
    }

    fn subscribe(&mut self, sym: &Symbol) -> Result<()> {
        let key = sym.to_string();
        if !self.symbols.contains_key(&key) {
            let price = self.start_price(sym);
            let lcg = lcg_step(self.seed ^ u64::from(key.len() as u32));
            self.symbols.insert(
                key,
                SynthSym {
                    symbol: sym.clone(),
                    price,
                    lcg,
                    seq: 0,
                },
            );
        }
        Ok(())
    }

    fn unsubscribe(&mut self, sym: &Symbol) {
        self.symbols.remove(&sym.to_string());
    }

    fn poll(&mut self) -> Vec<(Symbol, Event)> {
        self.t = self.t.wrapping_add(1);
        let mut out = Vec::new();
        for state in self.symbols.values_mut() {
            let sym = state.symbol.clone();
            state.lcg = lcg_step(state.lcg);
            state.seq += 1;
            // A bounded step in [-10, +10] half-cents, keeping the price positive.
            let step = i64::from((state.lcg >> 40) as u32 % 21) - 10;
            let mut price = state.price + Decimal::new(step, 2);
            if price < Decimal::ONE {
                price = Decimal::ONE;
            }
            state.price = price;

            let aggressor = if state.lcg & 1 == 0 {
                OrderSide::Buy
            } else {
                OrderSide::Sell
            };
            let quantity = Decimal::new(1 + i64::from((state.lcg >> 20) as u32 % 50), 2);
            out.push((
                sym.clone(),
                Event::Trade(TradePrint {
                    symbol: sym.clone(),
                    price,
                    quantity,
                    aggressor,
                    timestamp: self.t,
                }),
            ));

            // A shallow book snapshot every eight ticks keeps the book panel live.
            if state.seq % 8 == 0 {
                let bids = (1..=5)
                    .map(|i| BookLevel::new(price - Decimal::new(i, 2), Decimal::new(i, 1)))
                    .collect();
                let asks = (1..=5)
                    .map(|i| BookLevel::new(price + Decimal::new(i, 2), Decimal::new(i, 1)))
                    .collect();
                out.push((
                    sym.clone(),
                    Event::BookSnapshot(OrderBookSnapshot {
                        symbol: sym.clone(),
                        last_update_id: state.seq,
                        bids,
                        asks,
                    }),
                ));
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn synth_is_deterministic_for_a_seed() {
        let sym = Symbol::new("BTC", "USDT");
        let mut a = SynthSource::new(0, 42);
        let mut b = SynthSource::new(0, 42);
        a.subscribe(&sym).unwrap();
        b.subscribe(&sym).unwrap();
        for _ in 0..25 {
            assert_eq!(a.poll(), b.poll());
        }
    }

    #[test]
    fn different_seeds_diverge() {
        let sym = Symbol::new("BTC", "USDT");
        let mut a = SynthSource::new(0, 1);
        let mut b = SynthSource::new(0, 2);
        a.subscribe(&sym).unwrap();
        b.subscribe(&sym).unwrap();
        // Over a handful of ticks the two seeds produce different event streams.
        let sa: Vec<_> = (0..10).map(|_| a.poll()).collect();
        let sb: Vec<_> = (0..10).map(|_| b.poll()).collect();
        assert_ne!(sa, sb);
    }

    #[test]
    fn unsubscribe_stops_the_symbol() {
        let sym = Symbol::new("ETH", "USDT");
        let mut s = SynthSource::new(1, 7);
        s.subscribe(&sym).unwrap();
        assert!(!s.poll().is_empty());
        s.unsubscribe(&sym);
        assert!(s.poll().is_empty());
    }

    #[test]
    fn price_never_goes_below_one() {
        let sym = Symbol::new("A", "B");
        let mut s = SynthSource::new(0, 3);
        s.subscribe(&sym).unwrap();
        for _ in 0..500 {
            for (_, ev) in s.poll() {
                if let Event::Trade(t) = ev {
                    assert!(t.price >= Decimal::ONE);
                }
            }
        }
    }
}
