//! A host-fed source.
//!
//! [`ManualSource`] opens no connection of its own: the host pushes events into
//! it through the terminal's `Feed` command, and `poll()` drains them into the
//! state fold on the next tick. It is how the browser renderer bridges an
//! exchange WebSocket into the WASM core — which cannot open native sockets — and
//! how any embedder drives the terminal from a feed it already has.

use std::collections::{HashSet, VecDeque};

use super::{DataSource, SourceId, SourceKind, Symbol};
use crate::error::Result;
use wickra_exchange_core::Event;

/// A source whose events are pushed in by the host rather than pulled from a
/// connection.
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

    fn feed(&mut self, sym: Symbol, event: Event) -> bool {
        // Only accept events for subscribed markets, mirroring how a pull source
        // only yields events for what it streams.
        if !self.subscribed.contains(&sym) {
            return false;
        }
        self.queue.push_back((sym, event));
        true
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
        assert!(!src.feed(btc.clone(), trade(&btc, dec!(100))));
        assert_eq!(src.pending(), 0);

        src.subscribe(&btc).unwrap();
        assert!(src.feed(btc.clone(), trade(&btc, dec!(100))));
        assert!(src.feed(btc.clone(), trade(&btc, dec!(101))));
        assert!(!src.feed(eth.clone(), trade(&eth, dec!(2000))));
        assert_eq!(src.pending(), 2);

        // Poll drains everything queued, oldest first, and leaves the source empty.
        let drained = src.poll();
        assert_eq!(drained.len(), 2);
        assert_eq!(drained[0].0, btc);
        assert!(src.poll().is_empty());
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
        assert!(!src.feed(sym.clone(), trade(&sym, dec!(100))));
    }
}
