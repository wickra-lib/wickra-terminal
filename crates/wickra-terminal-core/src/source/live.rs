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
use crate::config::Market;
use crate::error::{Error, Result};
use std::collections::HashSet;
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
    /// The markets this source forwards.
    ///
    /// The exchange client has no per-symbol unsubscribe in its public surface,
    /// so the socket keeps delivering a market after the terminal has dropped
    /// it. `unsubscribe` used to be a comment saying exactly that and doing
    /// nothing -- but the fold takes an event for any market and creates the
    /// state for it, so the dropped market came straight back and was folded
    /// forever, invisible because the watchlist no longer named it.
    ///
    /// Filtering here is what makes the drop mean something. The socket is still
    /// the venue's to close; the work is not.
    subscribed: HashSet<Symbol>,
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
    pub fn connect(
        id: SourceId,
        venue: &str,
        _symbol: &str,
        testnet: bool,
        market: Market,
    ) -> Result<Self> {
        // The market was hard-coded to Spot, so a perpetual could not be opened
        // at all -- which left the whole derivatives side of the catalogue with
        // no market to watch, before the question of a funding feed even
        // arises.
        let kind = match market {
            Market::Spot => MarketType::Spot,
            Market::UsdMFutures => MarketType::UsdMFutures,
            Market::CoinMFutures => MarketType::CoinMFutures,
            Market::Margin => MarketType::Margin,
        };
        let options = if testnet {
            ExchangeOptions::testnet(kind)
        } else {
            ExchangeOptions::mainnet(kind)
        };
        let client = connect(venue, Credentials::new("", ""), &options)
            .map_err(|e| Error::Exchange(e.to_string()))?;
        Ok(Self {
            id,
            client,
            backoff: ReconnectBackoff::new(),
            subscribed: HashSet::new(),
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
            .map_err(|e| Error::Exchange(e.to_string()))?;
        self.subscribed.insert(sym.clone());
        Ok(())
    }

    fn unsubscribe(&mut self, sym: &Symbol) {
        // The exchange client has no per-symbol unsubscribe, so the socket keeps
        // delivering this market. Dropping it from the filter is what stops the
        // terminal folding it -- without that the fold creates the state again
        // on the very next event.
        self.subscribed.remove(sym);
    }

    fn backfill(&mut self, sym: &Symbol, interval: &str, limit: usize) -> Vec<wickra_core::Candle> {
        let limit = u32::try_from(limit).unwrap_or(u32::MAX);
        // A failed backfill is not a failed subscription. The venue may not
        // carry this interval, the request may time out, or the market may be
        // too new to have a history -- and in every one of those the right
        // outcome is a terminal that starts with no history rather than one that
        // refuses to open the market at all.
        self.client
            .klines(sym, interval, limit)
            .map(|bars| bars.iter().filter_map(into_core).collect())
            .unwrap_or_default()
    }

    fn poll(&mut self) -> Vec<(Symbol, Event)> {
        let now = Instant::now();
        if !self.backoff.ready(now) {
            return Vec::new();
        }
        let events = self.client.poll_events();
        self.backoff.observe(&events, now);
        forwarded(events, &self.subscribed)
    }
}

/// The exchange's candle as this crate's core sees it.
///
/// They are the same struct from two versions of wickra-core: the exchange pins
/// 0.9 and this crate builds against 1, so the compiler sees two unrelated
/// types with identical fields. Copying them across is the whole of the
/// conversion, and `Candle::new` re-validates rather than trusting the shape --
/// cheap, and the one place a malformed bar from a venue would otherwise reach
/// an indicator.
///
/// When the exchange moves to wickra-core 1 this becomes an identity and can go.
/// Until then it is where the duplicate costs something, which is better than
/// having it cost a little everywhere.
fn into_core(bar: &wickra_exchange::Candle) -> Option<wickra_core::Candle> {
    wickra_core::Candle::new(
        bar.open,
        bar.high,
        bar.low,
        bar.close,
        bar.volume,
        bar.timestamp,
    )
    .ok()
}

/// The events a poll forwards: those that name a market, and only markets still
/// subscribed.
///
/// Split out from `poll` so it can be tested. Everything either side of it needs
/// a socket; this is the part that decides what the terminal folds, and it is
/// the part that was wrong -- `unsubscribe` did nothing, so a dropped market
/// kept arriving and the fold created its state again on the next event.
fn forwarded(events: Vec<Event>, subscribed: &HashSet<Symbol>) -> Vec<(Symbol, Event)> {
    events
        .into_iter()
        .filter_map(|ev| event_symbol(&ev).map(|sym| (sym, ev)))
        .filter(|(sym, _)| subscribed.contains(sym))
        .collect()
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

    fn print(symbol: &Symbol, price: i64) -> Event {
        Event::Trade(wickra_exchange::TradePrint {
            symbol: symbol.clone(),
            price: rust_decimal::Decimal::from(price),
            quantity: rust_decimal::Decimal::from(1),
            aggressor: wickra_exchange::OrderSide::Buy,
            timestamp: 0,
        })
    }

    #[test]
    fn a_poll_forwards_only_the_markets_still_subscribed() {
        // unsubscribe was a comment saying the client has no per-symbol
        // unsubscribe, and doing nothing. But the fold creates state for
        // whatever market an event names, so the dropped market came straight
        // back and was folded forever -- invisible, because the watchlist no
        // longer listed it. The socket is still the venue's to close; the work
        // is not.
        let btc = Symbol::new("BTC", "USDT");
        let eth = Symbol::new("ETH", "USDT");
        let mut subscribed = HashSet::new();
        subscribed.insert(btc.clone());

        let out = forwarded(vec![print(&btc, 100), print(&eth, 200)], &subscribed);
        assert_eq!(out.len(), 1, "an unsubscribed market was forwarded");
        assert_eq!(out[0].0, btc);
    }

    #[test]
    fn a_poll_with_nothing_subscribed_forwards_nothing() {
        let btc = Symbol::new("BTC", "USDT");
        assert!(forwarded(vec![print(&btc, 100)], &HashSet::new()).is_empty());
    }

    #[test]
    fn lifecycle_events_are_dropped_because_they_name_no_market() {
        // The backoff reads them from the raw list before this runs, which is
        // why it takes the raw events rather than what poll returns.
        let btc = Symbol::new("BTC", "USDT");
        let mut subscribed = HashSet::new();
        subscribed.insert(btc);
        let out = forwarded(vec![Event::Disconnected, Event::Reconnected], &subscribed);
        assert!(out.is_empty());
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
