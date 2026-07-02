//! Data sources — the activatable feed modules behind one trait.
//!
//! Every feed (a live venue, a recorded replay, a synthetic generator) is a
//! [`DataSource`]: it is subscribed to symbols and drained with `poll()`, which
//! yields symbol-tagged [`Event`]s. Connection-lifecycle events that carry no
//! symbol (disconnect/reconnect/ack) are handled at the source boundary and not
//! forwarded to the state fold, so `poll()` only ever yields market events for a
//! concrete market.

// The Live source pulls the native exchange facade (tokio/reqwest), so it is
// gated behind the `live` feature and excluded from wasm builds.
#[cfg(feature = "live")]
pub mod live;
pub mod manual;
pub mod replay;
pub mod synth;

use crate::config::SourceSpec;
use crate::error::Result;

// The market vocabulary is the exchange layer's — re-exported so the whole core
// (and every binding) speaks one set of types.
pub use wickra_exchange_core::{Event, Symbol};

#[cfg(feature = "live")]
pub use live::LiveSource;
pub use manual::ManualSource;
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
    /// A host-fed source driven by the `Feed` command.
    Manual,
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

    /// Rewind a replayable source to `index` (clamped to its length) and return
    /// every *subscribed* event up to that point, so the terminal can re-fold
    /// deterministic state — the time-machine. After the seek, `poll()` resumes
    /// from `index`. Sources that cannot be replayed (live, synthetic) return
    /// `None` and are left untouched.
    ///
    /// Re-folding from the recorded feed — rather than restoring a cloned state
    /// snapshot — is deliberate: a `SymbolState` owns boxed streaming indicators
    /// that are not `Clone`, so a snapshot ring is not viable; a deterministic
    /// re-fold rebuilds identical state (the fold is O(1) per event).
    fn seek(&mut self, index: usize) -> Option<Vec<(Symbol, Event)>> {
        let _ = index;
        None
    }

    /// A replayable source's current cursor position (0 for non-replayable
    /// sources), so a renderer can show a time-machine scrubber.
    fn cursor(&self) -> usize {
        0
    }

    /// The number of recorded events in a replayable source (0 if not
    /// replayable).
    fn event_count(&self) -> usize {
        0
    }

    /// Push an externally sourced event into a host-fed source, to be folded on
    /// the next tick. Returns `true` if the source accepted it — a manual source
    /// accepts events for its subscribed markets. Sources that own their feed
    /// (live, replay, synthetic) return `false` and ignore it.
    fn feed(&mut self, sym: Symbol, event: Event) -> bool {
        let _ = (sym, event);
        false
    }
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
        } => build_live(id, venue, symbol, *testnet),
        SourceSpec::Replay { dataset } => Ok(Box::new(ReplaySource::from_dataset(id, dataset)?)),
        SourceSpec::Synth { seed } => Ok(Box::new(SynthSource::new(id, *seed))),
        SourceSpec::Manual => Ok(Box::new(ManualSource::new(id))),
    }
}

/// Build a Live source (native builds with the `live` feature).
#[cfg(feature = "live")]
fn build_live(
    id: SourceId,
    venue: &str,
    symbol: &str,
    testnet: bool,
) -> Result<Box<dyn DataSource>> {
    Ok(Box::new(LiveSource::connect(id, venue, symbol, testnet)?))
}

/// Without the `live` feature (e.g. wasm), a Live spec is an error: the browser
/// renderer feeds live data over its own WebSocket, not this native source.
#[cfg(not(feature = "live"))]
fn build_live(
    _id: SourceId,
    _venue: &str,
    _symbol: &str,
    _testnet: bool,
) -> Result<Box<dyn DataSource>> {
    Err(crate::error::Error::Source(
        "live sources require the `live` feature (native builds only)".to_string(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;
    use wickra_exchange_core::{OrderSide, TradePrint};

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
