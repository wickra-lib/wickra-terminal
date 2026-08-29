//! A host-fed source.
//!
//! [`ManualSource`] opens no connection of its own: the host pushes events into
//! it through the terminal's `Feed` command, and `poll()` drains them into the
//! state fold on the next tick. It is how the browser renderer bridges an
//! exchange WebSocket into the WASM core — which cannot open native sockets — and
//! how any embedder drives the terminal from a feed it already has.

use std::collections::{HashSet, VecDeque};

use super::{DataSource, Fed, SourceId, SourceKind, Symbol};
use crate::error::Result;
use wickra_exchange_core::Event;

/// The most events a [`ManualSource`] holds between ticks.
///
/// The queue was the one collection in the feed path with no limit, and its
/// shape is the web renderer's: a backgrounded tab stops firing rAF, so nothing
/// ticks while a socket keeps feeding, and the queue grows for as long as the
/// tab stays hidden. Everything else the fold touches is capped.
///
/// Four thousand is minutes of a busy market at a hundred prints a second, so a
/// host that ticks at all never meets it, and a host that has stopped ticking is
/// told rather than allowed to grow.
pub(crate) const MAX_PENDING_EVENTS: usize = 4096;

/// A source whose events are pushed in by the host rather than pulled from a
/// connection.
#[derive(Debug)]
pub struct ManualSource {
    id: SourceId,
    subscribed: HashSet<Symbol>,
    /// Events fed since the last poll, oldest first.
    queue: VecDeque<(Symbol, Event)>,
}

impl ManualSource {
    /// A host-fed source with the given id and an empty queue.
    #[must_use]
    pub fn new(id: SourceId) -> Self {
        Self {
            id,
            subscribed: HashSet::new(),
            queue: VecDeque::new(),
        }
    }

    /// The number of events waiting to be drained on the next poll.
    #[must_use]
    pub fn pending(&self) -> usize {
        self.queue.len()
    }
}

impl DataSource for ManualSource {
    fn id(&self) -> SourceId {
        self.id
    }

    fn kind(&self) -> SourceKind {
        SourceKind::Manual
    }

    fn subscribe(&mut self, sym: &Symbol) -> Result<()> {
        self.subscribed.insert(sym.clone());
        Ok(())
    }

    fn unsubscribe(&mut self, sym: &Symbol) {
        self.subscribed.remove(sym);
        // Drop any still-queued events for the dropped market.
        self.queue.retain(|(queued, _)| queued != sym);
    }

    fn poll(&mut self) -> Vec<(Symbol, Event)> {
        self.queue.drain(..).collect()
    }

    fn feed(&mut self, sym: Symbol, event: Event) -> Fed {
        // Only accept events for subscribed markets, mirroring how a pull source
        // only yields events for what it streams.
        if !self.subscribed.contains(&sym) {
            return Fed::Refused;
        }
        // Refuse rather than evict. Dropping the oldest would keep the terminal
        // current at the cost of a silent hole in the sequence, and a book delta
        // is only meaningful in order -- a missing one leaves a local book that
        // is wrong rather than stale. What is queued stays contiguous, and the
        // host learns it has fallen behind instead of reading a book that
        // quietly stopped matching the venue.

        if self.queue.len() >= MAX_PENDING_EVENTS {
            return Fed::Full;
        }
        self.queue.push_back((sym, event));
        Fed::Accepted
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;
    use wickra_exchange_core::{OrderSide, TradePrint};

    fn trade(sym: &Symbol, price: rust_decimal::Decimal) -> Event {
        Event::Trade(TradePrint {
            symbol: sym.clone(),
            price,
            quantity: dec!(1),
            aggressor: OrderSide::Buy,
            timestamp: 0,
        })
    }

    #[test]
    fn feeds_only_subscribed_markets_then_drains_on_poll() {
        let btc = Symbol::new("BTC", "USDT");
        let eth = Symbol::new("ETH", "USDT");
        let mut src = ManualSource::new(4);
        assert_eq!(src.id(), 4);
        assert_eq!(src.kind(), SourceKind::Manual);

        // Unsubscribed markets are rejected.
        assert_eq!(src.feed(btc.clone(), trade(&btc, dec!(100))), Fed::Refused);
        assert_eq!(src.pending(), 0);

        src.subscribe(&btc).unwrap();
        assert_eq!(src.feed(btc.clone(), trade(&btc, dec!(100))), Fed::Accepted);
        assert_eq!(src.feed(btc.clone(), trade(&btc, dec!(101))), Fed::Accepted);
        assert_eq!(src.feed(eth.clone(), trade(&eth, dec!(2000))), Fed::Refused);
        assert_eq!(src.pending(), 2);

        // Poll drains everything queued, oldest first, and leaves the source empty.
        let drained = src.poll();
        assert_eq!(drained.len(), 2);
        assert_eq!(drained[0].0, btc);
        assert!(src.poll().is_empty());
    }

    #[test]
    fn a_host_that_stops_ticking_is_refused_rather_than_growing() {
        // The web renderer's shape: a backgrounded tab stops firing rAF, so
        // nothing ticks while a socket keeps feeding.
        let sym = Symbol::new("BTC", "USDT");
        let mut src = ManualSource::new(1);
        src.subscribe(&sym).unwrap();
        for _ in 0..MAX_PENDING_EVENTS {
            assert_eq!(src.feed(sym.clone(), trade(&sym, dec!(100))), Fed::Accepted);
        }
        assert_eq!(src.pending(), MAX_PENDING_EVENTS);
        assert_eq!(src.feed(sym.clone(), trade(&sym, dec!(101))), Fed::Full);
        assert_eq!(
            src.pending(),
            MAX_PENDING_EVENTS,
            "a refused event was queued anyway"
        );
    }

    #[test]
    fn a_full_queue_keeps_what_it_has_in_order() {
        // Refusing rather than evicting is the point: a book delta only means
        // anything in sequence, so what is queued has to stay contiguous.
        let sym = Symbol::new("BTC", "USDT");
        let mut src = ManualSource::new(1);
        src.subscribe(&sym).unwrap();
        for tick in 0..MAX_PENDING_EVENTS {
            let price = rust_decimal::Decimal::from(u32::try_from(tick).unwrap_or(0));
            assert_eq!(src.feed(sym.clone(), trade(&sym, price)), Fed::Accepted);
        }
        assert_eq!(src.feed(sym.clone(), trade(&sym, dec!(999_999))), Fed::Full);

        let drained = src.poll();
        assert_eq!(drained.len(), MAX_PENDING_EVENTS);
        let Event::Trade(first) = &drained[0].1 else {
            panic!("the queue holds trades");
        };
        assert_eq!(
            first.price,
            rust_decimal::Decimal::ZERO,
            "the oldest event was evicted"
        );
    }

    #[test]
    fn draining_makes_room_again() {
        let sym = Symbol::new("BTC", "USDT");
        let mut src = ManualSource::new(1);
        src.subscribe(&sym).unwrap();
        for _ in 0..MAX_PENDING_EVENTS {
            src.feed(sym.clone(), trade(&sym, dec!(100)));
        }
        assert_eq!(src.feed(sym.clone(), trade(&sym, dec!(100))), Fed::Full);
        let _ = src.poll();
        assert_eq!(src.feed(sym.clone(), trade(&sym, dec!(100))), Fed::Accepted);
    }

    #[test]
    fn unsubscribe_drops_the_market_and_its_queued_events() {
        let sym = Symbol::new("BTC", "USDT");
        let mut src = ManualSource::new(1);
        src.subscribe(&sym).unwrap();
        src.feed(sym.clone(), trade(&sym, dec!(100)));
        assert_eq!(src.pending(), 1);
        src.unsubscribe(&sym);
        assert_eq!(src.pending(), 0);
        assert_eq!(src.feed(sym.clone(), trade(&sym, dec!(100))), Fed::Refused);
    }
}
