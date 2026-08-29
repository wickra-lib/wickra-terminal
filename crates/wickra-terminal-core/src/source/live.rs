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
use std::time::{Duration, Instant};
use wickra_exchange::{connect, Credentials, Event, Exchange, ExchangeOptions, MarketType};

/// The first wait after a live socket reports a drop.
const RECONNECT_MIN_WAIT: Duration = Duration::from_millis(250);

/// The longest wait between reconnect attempts.
const RECONNECT_MAX_WAIT: Duration = Duration::from_secs(30);

/// Paces how often a dropped live source is polled, and so how often it tries to
/// reconnect.
///
/// The exchange layer reconnects from inside `poll_events`: a poll that finds
/// the socket dropped calls the transport's `connect`, which spawns an OS thread
/// with its own current-thread runtime and returns `Ok` before `connect_async`
/// has been attempted. The connection is therefore installed as if live, the
/// next poll finds it dead, and the next one after that repeats -- once per
/// render frame, because the terminal is what drives `poll`. `connect_async`
/// carries no connect timeout, so against a black-holed endpoint each of those
/// threads sits for the operating system's TCP timeout -- roughly 21 seconds on
/// Windows, up to 130 on Linux -- and at a 10 Hz render loop that is hundreds of
/// threads outstanding at once.
///
/// The root cause is in the exchange crate, but the rate is set here: the
/// terminal is the only consumer driving that loop at render rate, so this is
/// where a backoff belongs. A drop starts at a quarter of a second and doubles
/// to half a minute, so a transient blip still recovers almost immediately while
/// an endpoint that is simply gone is retried about twice a minute.
#[derive(Debug)]
struct ReconnectBackoff {
    /// When polling may resume; `None` while the source is behaving.
    retry_at: Option<Instant>,
    delay: Duration,
}

impl ReconnectBackoff {
    const fn new() -> Self {
        Self {
            retry_at: None,
            delay: RECONNECT_MIN_WAIT,
        }
    }

    /// Whether the source may be polled at `now`.
    fn ready(&self, now: Instant) -> bool {
        self.retry_at.is_none_or(|at| now >= at)
    }

    /// Fold what a poll returned into the schedule.
    ///
    /// Reads the raw events rather than what `poll` forwards: `Disconnected` and
    /// `Reconnected` carry no symbol, so the source's own filter drops them
    /// before anything else could see them.
    fn observe(&mut self, events: &[Event], now: Instant) {
        let dropped = events.iter().any(|e| matches!(e, Event::Disconnected));
        let recovered = events
            .iter()
            .any(|e| matches!(e, Event::Reconnected) || event_symbol(e).is_some());

        if recovered {
            self.retry_at = None;
            self.delay = RECONNECT_MIN_WAIT;
        } else if dropped {
            self.retry_at = Some(now + self.delay);
            self.delay = (self.delay * 2).min(RECONNECT_MAX_WAIT);
        }
    }
}

/// A live feed from one venue.
pub struct LiveSource {
    id: SourceId,
    client: Box<dyn Exchange>,
    backoff: ReconnectBackoff,
}

impl std::fmt::Debug for LiveSource {
    /// `client` is a `Box<dyn Exchange>` from wickra-exchange, which carries no
    /// `Debug` bound, so the source is identified by its id.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LiveSource")
            .field("id", &self.id)
            .finish_non_exhaustive()
    }
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
        Ok(Self {
            id,
            client,
            backoff: ReconnectBackoff::new(),
        })
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
        let now = Instant::now();
        if !self.backoff.ready(now) {
            return Vec::new();
        }
        let events = self.client.poll_events();
        self.backoff.observe(&events, now);
        events
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

    /// A `Disconnected` with no reconnect in the same poll.
    fn dropped() -> Vec<Event> {
        vec![Event::Disconnected]
    }

    #[test]
    fn a_drop_stops_the_next_poll_and_the_wait_grows() {
        let start = Instant::now();
        let mut backoff = ReconnectBackoff::new();
        assert!(backoff.ready(start), "a fresh source polls immediately");

        backoff.observe(&dropped(), start);
        assert!(
            !backoff.ready(start),
            "a drop must not be retried in the same instant"
        );
        assert!(!backoff.ready(start + Duration::from_millis(249)));
        assert!(backoff.ready(start + RECONNECT_MIN_WAIT));

        // Each further drop doubles the wait: a quarter second, a half, one.
        let second = start + RECONNECT_MIN_WAIT;
        backoff.observe(&dropped(), second);
        assert!(!backoff.ready(second + Duration::from_millis(499)));
        assert!(backoff.ready(second + Duration::from_millis(500)));

        let third = second + Duration::from_millis(500);
        backoff.observe(&dropped(), third);
        assert!(!backoff.ready(third + Duration::from_millis(999)));
        assert!(backoff.ready(third + Duration::from_secs(1)));
    }

    #[test]
    fn the_wait_is_capped() {
        // Without a cap the doubling runs to hours and then overflows.
        let start = Instant::now();
        let mut backoff = ReconnectBackoff::new();
        let mut at = start;
        for _ in 0..40 {
            backoff.observe(&dropped(), at);
            at += RECONNECT_MAX_WAIT;
        }
        backoff.observe(&dropped(), at);
        let just_short = RECONNECT_MAX_WAIT.saturating_sub(Duration::from_millis(1));
        assert!(!backoff.ready(at + just_short));
        assert!(backoff.ready(at + RECONNECT_MAX_WAIT));
    }

    #[test]
    fn a_reconnect_clears_the_wait_entirely() {
        let start = Instant::now();
        let mut backoff = ReconnectBackoff::new();
        backoff.observe(&dropped(), start);
        backoff.observe(&dropped(), start + RECONNECT_MIN_WAIT);
        assert!(!backoff.ready(start + RECONNECT_MIN_WAIT));

        backoff.observe(&[Event::Reconnected], start + Duration::from_secs(1));
        assert!(
            backoff.ready(start + Duration::from_secs(1)),
            "a recovered source polls at once"
        );

        // And the next drop starts from the minimum again rather than from where
        // the previous outage left off.
        let later = start + Duration::from_secs(2);
        backoff.observe(&dropped(), later);
        assert!(backoff.ready(later + RECONNECT_MIN_WAIT));
    }

    #[test]
    fn data_counts_as_recovery_even_without_a_reconnect_event() {
        // A poll can carry a drop and then real prints from a fresh connection;
        // treating that as an outage would throttle a working feed.
        let start = Instant::now();
        let mut backoff = ReconnectBackoff::new();
        let symbol = Symbol::new("BTC", "USDT");
        let trade = Event::Trade(wickra_exchange::TradePrint {
            symbol,
            price: rust_decimal::Decimal::from(100),
            quantity: rust_decimal::Decimal::from(1),
            aggressor: wickra_exchange::OrderSide::Buy,
            timestamp: 0,
        });
        backoff.observe(&[Event::Disconnected, trade], start);
        assert!(backoff.ready(start));
    }

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
