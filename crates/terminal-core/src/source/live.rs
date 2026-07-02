//! A live venue source over the exchange connectivity layer.
//!
//! [`LiveSource`] wraps a boxed [`Exchange`](wickra_exchange::Exchange) built by
//! the exchange facade's `connect`, subscribes its public market-data channels,
//! and forwards `poll_events` as symbol-tagged events. It uses read-only
//! (empty) credentials — the terminal's default `Live` source streams public
//! data only; authenticated execution is a separate, opt-in, USER-GO path.
//!
//! Like the exchange facade's `connect`, this type only wires a real socket and
//! forwards; the machinery below the trait is covered by the exchange core's
//! offline suite, and the network round-trip here is not unit-testable, so it is
//! exercised through the runnable examples and gated live tests rather than the
//! offline unit tests.

use super::{event_symbol, DataSource, SourceId, SourceKind, Symbol};
use crate::error::{Error, Result};
use std::str::FromStr;
use wickra_exchange::{connect, Credentials, Event, Exchange, ExchangeOptions, MarketType};

/// A live feed from one venue.
pub struct LiveSource {
    id: SourceId,
    client: Box<dyn Exchange>,
}

impl LiveSource {
    /// Connect a read-only client to `venue` for public market data.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Exchange`] if the venue is unknown or the HTTP client
    /// cannot be constructed.
    pub fn connect(id: SourceId, venue: &str, _symbol: &str, testnet: bool) -> Result<Self> {
        let options = if testnet {
            ExchangeOptions::testnet(MarketType::Spot)
        } else {
            ExchangeOptions::mainnet(MarketType::Spot)
        };
        let client = connect(venue, Credentials::new("", ""), &options)
            .map_err(|e| Error::Exchange(e.to_string()))?;
        Ok(Self { id, client })
    }
}

impl DataSource for LiveSource {
    fn id(&self) -> SourceId {
        self.id
    }

    fn kind(&self) -> SourceKind {
        SourceKind::Live
    }

    fn subscribe(&mut self, sym: &Symbol) -> Result<()> {
        self.client
            .subscribe_trades(sym)
            .and_then(|()| self.client.subscribe_book(sym))
            .and_then(|()| self.client.subscribe_ticker(sym))
            .map_err(|e| Error::Exchange(e.to_string()))
    }

    fn unsubscribe(&mut self, _sym: &Symbol) {
        // The pull-based exchange client has no per-symbol unsubscribe in its
        // public surface; the terminal simply stops folding this symbol's state.
    }

    fn poll(&mut self) -> Vec<(Symbol, Event)> {
        self.client
            .poll_events()
            .into_iter()
            .filter_map(|ev| event_symbol(&ev).map(|sym| (sym, ev)))
            .collect()
    }
}

/// Parse a `venue:BASE/QUOTE` live source shorthand into its parts, validating
/// the symbol. Used by renderers turning a `--source live:…` flag into a
/// [`SourceSpec`](crate::config::SourceSpec).
///
/// # Errors
///
/// Returns [`Error::Source`] if the shorthand is not `venue:BASE/QUOTE`.
pub fn parse_live_shorthand(s: &str) -> Result<(String, String)> {
    let (venue, symbol) = s
        .split_once(':')
        .ok_or_else(|| Error::Source(format!("expected venue:SYMBOL, got {s:?}")))?;
    // Validate the symbol shape without keeping the parsed value.
    Symbol::from_str(symbol).map_err(|e| Error::Source(e.to_string()))?;
    Ok((venue.to_string(), symbol.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shorthand_splits_venue_and_symbol() {
        let (venue, symbol) = parse_live_shorthand("binance:BTC/USDT").unwrap();
        assert_eq!(venue, "binance");
        assert_eq!(symbol, "BTC/USDT");
    }

    #[test]
    fn shorthand_rejects_missing_colon() {
        assert!(matches!(
            parse_live_shorthand("binance").unwrap_err(),
            Error::Source(_)
        ));
    }

    #[test]
    fn shorthand_rejects_bad_symbol() {
        assert!(matches!(
            parse_live_shorthand("binance:BTCUSDT").unwrap_err(),
            Error::Source(_)
        ));
    }
}
