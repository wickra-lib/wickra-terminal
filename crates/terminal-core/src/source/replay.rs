//! A recorded-feed replay source.
//!
//! [`ReplaySource`] drives a fixed, recorded list of events with a movable
//! cursor, so a session can be re-played deterministically and seeked (the
//! time-machine). It reads no files itself — a renderer loads a dataset and
//! hands the events (or their JSON) in — which keeps the core filesystem-free and
//! usable from WebAssembly. This is exactly the source the byte-exact golden
//! corpus drives.

use std::collections::HashSet;

use super::{event_symbol, DataSource, SourceId, SourceKind, Symbol};
use crate::error::{Error, Result};
use wickra_exchange_core::Event;

/// A deterministic replay of a recorded event list.
pub struct ReplaySource {
    id: SourceId,
    /// The recorded feed, each event pre-tagged with its market. Lifecycle
    /// events without a symbol are dropped at load time.
    events: Vec<(Symbol, Event)>,
    /// The next event to emit.
    cursor: usize,
    subscribed: HashSet<Symbol>,
}

impl ReplaySource {
    /// Build a replay from an in-memory event list (the primary constructor;
    /// used by tests and the golden corpus).
    #[must_use]
    pub fn from_events(id: SourceId, events: Vec<Event>) -> Self {
        let events = events
            .into_iter()
            .filter_map(|ev| event_symbol(&ev).map(|sym| (sym, ev)))
            .collect();
        Self {
            id,
            events,
            cursor: 0,
            subscribed: HashSet::new(),
        }
    }

    /// Build a replay from a dataset given as a JSON array of events.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Source`] if `dataset` is not a JSON array of events.
    pub fn from_dataset(id: SourceId, dataset: &str) -> Result<Self> {
        let events: Vec<Event> = serde_json::from_str(dataset)
            .map_err(|e| Error::Source(format!("replay dataset is not a JSON event array: {e}")))?;
        Ok(Self::from_events(id, events))
    }

    /// The number of recorded (symbol-bearing) events.
    #[must_use]
    pub fn len(&self) -> usize {
        self.events.len()
    }

    /// Whether the recorded feed is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    /// Move the cursor to `index` (clamped to the feed length) — the time-machine
    /// seek. The caller re-folds state from a fresh `AppState` after seeking
    /// backward; this source only owns the cursor.
    pub fn seek(&mut self, index: usize) {
        self.cursor = index.min(self.events.len());
    }

    /// The current cursor position.
    #[must_use]
    pub fn cursor(&self) -> usize {
        self.cursor
    }
}

impl DataSource for ReplaySource {
    fn id(&self) -> SourceId {
        self.id
    }

    fn kind(&self) -> SourceKind {
        SourceKind::Replay
    }

    fn subscribe(&mut self, sym: &Symbol) -> Result<()> {
        self.subscribed.insert(sym.clone());
        Ok(())
    }

    fn unsubscribe(&mut self, sym: &Symbol) {
        self.subscribed.remove(sym);
    }

    fn poll(&mut self) -> Vec<(Symbol, Event)> {
        // Step: advance past unsubscribed events and emit the next subscribed one.
        while self.cursor < self.events.len() {
            let (sym, ev) = &self.events[self.cursor];
            self.cursor += 1;
            if self.subscribed.contains(sym) {
                return vec![(sym.clone(), ev.clone())];
            }
        }
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;
    use wickra_exchange_core::{OrderSide, TradePrint};

    fn trade(sym: &Symbol, price: rust_decimal::Decimal, ts: i64) -> Event {
        Event::Trade(TradePrint {
            symbol: sym.clone(),
            price,
            quantity: dec!(1),
            aggressor: OrderSide::Buy,
            timestamp: ts,
        })
    }

    fn feed(sym: &Symbol) -> Vec<Event> {
        vec![
            trade(sym, dec!(100), 1),
            trade(sym, dec!(101), 2),
            trade(sym, dec!(102), 3),
        ]
    }

    #[test]
    fn replays_subscribed_events_in_order_then_exhausts() {
        let sym = Symbol::new("BTC", "USDT");
        let mut r = ReplaySource::from_events(7, feed(&sym));
        r.subscribe(&sym).unwrap();
        assert_eq!(r.len(), 3);
        let prices: Vec<_> = std::iter::from_fn(|| r.poll().into_iter().next())
            .map(|(_, ev)| match ev {
                Event::Trade(t) => t.price,
                _ => panic!("expected trade"),
            })
            .collect();
        assert_eq!(prices, vec![dec!(100), dec!(101), dec!(102)]);
        assert!(r.poll().is_empty());
    }

    #[test]
    fn seek_rewinds_the_cursor() {
        let sym = Symbol::new("BTC", "USDT");
        let mut r = ReplaySource::from_events(7, feed(&sym));
        r.subscribe(&sym).unwrap();
        r.poll();
        r.poll();
        assert_eq!(r.cursor(), 2);
        r.seek(0);
        assert_eq!(r.cursor(), 0);
        assert_eq!(r.poll().len(), 1);
    }

    #[test]
    fn unsubscribed_symbols_are_skipped() {
        let btc = Symbol::new("BTC", "USDT");
        let eth = Symbol::new("ETH", "USDT");
        let mut events = feed(&btc);
        events.push(trade(&eth, dec!(2000), 4));
        let mut r = ReplaySource::from_events(1, events);
        r.subscribe(&eth).unwrap();
        // Only the ETH print is emitted; the BTC prints are skipped.
        let (sym, _) = r.poll().into_iter().next().unwrap();
        assert_eq!(sym, eth);
        assert!(r.poll().is_empty());
    }

    #[test]
    fn from_dataset_parses_json_events() {
        let sym = Symbol::new("BTC", "USDT");
        let json = serde_json::to_string(&feed(&sym)).unwrap();
        let r = ReplaySource::from_dataset(2, &json).unwrap();
        assert_eq!(r.len(), 3);
    }

    #[test]
    fn from_dataset_rejects_non_event_json() {
        assert!(matches!(
            ReplaySource::from_dataset(2, "not json"),
            Err(Error::Source(_))
        ));
    }
}
