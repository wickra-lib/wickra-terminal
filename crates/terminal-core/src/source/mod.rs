//! Data sources — the activatable feed modules behind one trait.
//!
//! Every feed (a live venue, a recorded replay, a synthetic generator) is a
//! [`DataSource`]: it is subscribed to symbols and drained with `poll()`, which
//! yields symbol-tagged [`Event`]s. Connection-lifecycle events that carry no
//! symbol (disconnect/reconnect/ack) are handled at the source boundary and not
//! forwarded to the state fold, so `poll()` only ever yields market events for a
//! concrete market.

pub mod live;
pub mod replay;
pub mod synth;

use crate::config::SourceSpec;
use crate::error::Result;

// The market vocabulary is the exchange layer's — re-exported so the whole core
// (and every binding) speaks one set of types.
pub use wickra_exchange::{Event, Symbol};

pub use live::LiveSource;
pub use replay::ReplaySource;
pub use synth::SynthSource;

/// A stable per-terminal identifier for one open source.
pub type SourceId = u32;

/// Which kind of feed a source is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceKind {
    /// A live venue feed over the exchange layer.
    Live,
    /// A recorded feed replayed from a dataset.
    Replay,
    /// A deterministic synthetic feed.
    Synth,
}

/// A feed the terminal can subscribe to and drain.
pub trait DataSource {
    /// This source's terminal-assigned id.
    fn id(&self) -> SourceId;

    /// Which kind of feed this is.
    fn kind(&self) -> SourceKind;

    /// Start streaming the given market.
    ///
    /// # Errors
    ///
    /// Returns an error if the underlying feed rejects the subscription.
    fn subscribe(&mut self, sym: &Symbol) -> Result<()>;

    /// Stop streaming the given market.
    fn unsubscribe(&mut self, sym: &Symbol);

    /// Drain the buffered events, each tagged with its market.
    fn poll(&mut self) -> Vec<(Symbol, Event)>;
}

/// The market an event concerns, if it is a market (not a lifecycle) event.
#[must_use]
pub fn event_symbol(event: &Event) -> Option<Symbol> {
    match event {
        Event::Trade(t) => Some(t.symbol.clone()),
        Event::Ticker(t) => Some(t.symbol.clone()),
        Event::BookSnapshot(s) => Some(s.symbol.clone()),
        Event::BookDelta(d) => Some(d.symbol.clone()),
        Event::OrderUpdate(o) => Some(o.symbol.clone()),
        Event::BalanceUpdate(_)
        | Event::Subscribed { .. }
        | Event::Disconnected
        | Event::Reconnected => None,
    }
}

/// Build a source of the kind described by `spec`, assigned the given `id`.
///
/// # Errors
///
/// Returns [`Error::Source`](crate::error::Error::Source) if the spec cannot be
/// realized — an unknown venue for `Live`, or a replay dataset that fails to
/// load.
pub fn build_source(id: SourceId, spec: &SourceSpec) -> Result<Box<dyn DataSource>> {
    match spec {
        SourceSpec::Live {
            venue,
            symbol,
            testnet,
        } => Ok(Box::new(LiveSource::connect(id, venue, symbol, *testnet)?)),
        SourceSpec::Replay { dataset } => Ok(Box::new(ReplaySource::from_dataset(id, dataset)?)),
        SourceSpec::Synth { seed } => Ok(Box::new(SynthSource::new(id, *seed))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;
    use wickra_exchange::{OrderSide, TradePrint};

    #[test]
    fn trade_event_has_a_symbol_tag() {
        let ev = Event::Trade(TradePrint {
            symbol: Symbol::new("BTC", "USDT"),
            price: dec!(20000),
            quantity: dec!(1),
            aggressor: OrderSide::Buy,
            timestamp: 0,
        });
        assert_eq!(event_symbol(&ev), Some(Symbol::new("BTC", "USDT")));
    }

    #[test]
    fn lifecycle_events_have_no_symbol_tag() {
        assert_eq!(event_symbol(&Event::Disconnected), None);
        assert_eq!(event_symbol(&Event::Reconnected), None);
    }

    #[test]
    fn build_source_makes_a_synth_source() {
        let src = build_source(3, &SourceSpec::Synth { seed: 1 }).unwrap();
        assert_eq!(src.id(), 3);
        assert_eq!(src.kind(), SourceKind::Synth);
    }
}
