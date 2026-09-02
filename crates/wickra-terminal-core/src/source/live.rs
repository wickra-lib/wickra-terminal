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
use wickra_exchange::{connect, Credentials, Event, ExchangeOptions, MarketData, MarketType};

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
    /// The venue's market-data half, and only that.
    ///
    /// `connect` hands back a `dyn Exchange`, which also carries order placement
    /// and balances. Narrowing it here says structurally what `THREAT_MODEL.md`
    /// says in prose: this source reads. It cannot place an order, because the
    /// type it holds has no method for one -- and a future edit that tried would
    /// not compile rather than needing to be noticed in review.
    client: Box<dyn MarketData>,
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
    /// `client` is a trait object from wickra-exchange, which carries no `Debug`
    /// bound, so the source is identified by its id.
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
        let kind = market_type(market);
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

/// The exchange layer's market kind for a config's.
///
/// Its own function rather than a `match` inside `connect`, because `connect`
/// needs a venue and this does not -- and a mapping that silently sent every
/// market to the spot book is exactly the bug this replaces.
fn market_type(market: Market) -> MarketType {
    match market {
        Market::Spot => MarketType::Spot,
        Market::UsdMFutures => MarketType::UsdMFutures,
        Market::CoinMFutures => MarketType::CoinMFutures,
        Market::Margin => MarketType::Margin,
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

    /// An exchange that answers from memory, so the source's own logic can be
    /// tested without a venue.
    ///
    /// The module header says the network round-trip is not unit-testable, and
    /// that stays true. What *is* testable is everything this source does around
    /// it — which markets it forwards, what it does with a backfill, what a
    /// subscribe records — and that is where the bugs were: `unsubscribe` did
    /// nothing at all, and a dropped market was folded for the rest of the
    /// session.
    ///
    /// It implements market data and nothing else, because the source holds a
    /// `dyn MarketData` and nothing else. That narrowing is the point: there is
    /// no execution half to stub, so there is none to get wrong.
    #[derive(Default)]
    struct StubState {
        bars: Vec<wickra_exchange::Candle>,
        events: Vec<Event>,
        subscribed: Vec<Symbol>,
        klines_asked: Option<(String, u32)>,
        klines_fail: bool,
        /// How many times the source asked for market data over REST rather
        /// than taking it off the stream. Backfill is not counted: that is one
        /// request per subscription and the reason `klines` exists here.
        rest_polls: usize,
    }

    /// The stub and the test share one state, so the test can queue events and
    /// read back what was asked for after the source has taken ownership of the
    /// client. A handle rather than an accessor on `LiveSource`: the source has
    /// no business growing a method that exists for a test.
    #[derive(Clone, Default)]
    struct StubExchange {
        state: std::rc::Rc<std::cell::RefCell<StubState>>,
    }

    impl StubExchange {
        fn new() -> Self {
            Self::default()
        }

        fn queue(&self, events: Vec<Event>) {
            self.state.borrow_mut().events = events;
        }

        fn klines_asked(&self) -> Option<(String, u32)> {
            self.state.borrow().klines_asked.clone()
        }

        fn rest_polls(&self) -> usize {
            self.state.borrow().rest_polls
        }
    }

    impl wickra_exchange::MarketData for StubExchange {
        fn ticker(&mut self, _symbol: &Symbol) -> wickra_exchange::Result<wickra_exchange::Ticker> {
            self.state.borrow_mut().rest_polls += 1;
            Err(wickra_exchange::Error::InvalidSymbol(
                "the stub answers market data over the stream".to_string(),
            ))
        }

        fn klines(
            &mut self,
            _symbol: &Symbol,
            interval: &str,
            limit: u32,
        ) -> wickra_exchange::Result<Vec<wickra_exchange::Candle>> {
            let mut state = self.state.borrow_mut();
            state.klines_asked = Some((interval.to_string(), limit));
            if state.klines_fail {
                return Err(wickra_exchange::Error::InvalidSymbol("stub".to_string()));
            }
            Ok(state.bars.clone())
        }

        fn order_book(
            &mut self,
            _symbol: &Symbol,
            _depth: u32,
        ) -> wickra_exchange::Result<wickra_exchange::OrderBookSnapshot> {
            self.state.borrow_mut().rest_polls += 1;
            Err(wickra_exchange::Error::InvalidSymbol(
                "the stub answers market data over the stream".to_string(),
            ))
        }

        fn subscribe_trades(&mut self, symbol: &Symbol) -> wickra_exchange::Result<()> {
            self.state.borrow_mut().subscribed.push(symbol.clone());
            Ok(())
        }

        fn subscribe_book(&mut self, _symbol: &Symbol) -> wickra_exchange::Result<()> {
            Ok(())
        }

        fn subscribe_ticker(&mut self, _symbol: &Symbol) -> wickra_exchange::Result<()> {
            Ok(())
        }

        fn poll_events(&mut self) -> Vec<Event> {
            std::mem::take(&mut self.state.borrow_mut().events)
        }
    }

    /// A source over a stub exchange, with no socket anywhere.
    fn stubbed(stub: &StubExchange) -> LiveSource {
        LiveSource {
            id: 0,
            client: Box::new(stub.clone()),
            backoff: ReconnectBackoff::new(),
            subscribed: HashSet::new(),
        }
    }

    #[test]
    fn subscribing_records_the_market_and_unsubscribing_drops_it() {
        // unsubscribe was a comment and an empty body. The fold creates state
        // for whatever market an event names, so the dropped market came back on
        // the next poll and was folded for the rest of the session.
        let btc = Symbol::new("BTC", "USDT");
        let stub = StubExchange::new();
        let mut source = stubbed(&stub);

        source.subscribe(&btc).expect("the stub accepts");
        assert_eq!(source.poll().len(), 0, "no events queued yet");

        stub.queue(vec![print(&btc, 100)]);
        assert_eq!(source.poll().len(), 1, "a subscribed market is forwarded");

        source.unsubscribe(&btc);
        stub.queue(vec![print(&btc, 101)]);
        assert!(
            source.poll().is_empty(),
            "an unsubscribed market is still being folded"
        );
    }

    #[test]
    fn a_backfill_asks_for_the_timeframe_and_limit_it_was_given() {
        // The interval is the timeframe in the venue's own notation, and a
        // source that asked for the wrong one would return bars of a size the
        // indicators are not being fed at -- which looks like data, not a bug.
        let stub = StubExchange::new();
        stub.state.borrow_mut().bars = vec![
            wickra_exchange::Candle::new(100.0, 110.0, 95.0, 105.0, 1.0, 0).expect("valid"),
            wickra_exchange::Candle::new(105.0, 115.0, 100.0, 110.0, 1.0, 60_000).expect("valid"),
        ];
        let mut source = stubbed(&stub);

        let bars = source.backfill(&Symbol::new("BTC", "USDT"), "4h", 200);
        assert_eq!(bars.len(), 2);
        assert_eq!(stub.klines_asked(), Some(("4h".to_string(), 200)));
    }

    #[test]
    fn a_failed_backfill_is_not_a_failed_subscription() {
        // The venue may not carry the interval, the request may time out, or the
        // market may be too new to have a history. In each the right outcome is a
        // terminal that starts with no history, not one that refuses the market.
        let stub = StubExchange::new();
        stub.state.borrow_mut().klines_fail = true;
        let mut source = stubbed(&stub);
        assert!(source
            .backfill(&Symbol::new("BTC", "USDT"), "1m", 200)
            .is_empty());
    }

    #[test]
    fn a_backfill_limit_past_a_u32_is_clamped_rather_than_wrapping() {
        let stub = StubExchange::new();
        let mut source = stubbed(&stub);
        source.backfill(&Symbol::new("BTC", "USDT"), "1m", usize::MAX);
        assert_eq!(stub.klines_asked(), Some(("1m".to_string(), u32::MAX)));
    }

    #[test]
    fn a_dropped_socket_stops_the_next_poll() {
        // The backoff reads the raw events, before the symbol filter drops the
        // lifecycle ones -- so a disconnect has to reach it through a real poll,
        // not only through the unit test above.
        let btc = Symbol::new("BTC", "USDT");
        let stub = StubExchange::new();
        let mut source = stubbed(&stub);
        source.subscribe(&btc).expect("the stub accepts");

        stub.queue(vec![Event::Disconnected]);
        assert!(source.poll().is_empty(), "a disconnect carries no market");

        stub.queue(vec![print(&btc, 100)]);
        assert!(
            source.poll().is_empty(),
            "the source polled again inside its own backoff"
        );
    }

    #[test]
    fn every_market_maps_to_its_own_book() {
        // The mapping was a hard-coded Spot, so a perpetual could not be opened
        // at all. A mapping that quietly sent two markets to the same book would
        // be the same bug wearing a config field, so each is named.
        assert_eq!(market_type(Market::Spot), MarketType::Spot);
        assert_eq!(market_type(Market::UsdMFutures), MarketType::UsdMFutures);
        assert_eq!(market_type(Market::CoinMFutures), MarketType::CoinMFutures);
        assert_eq!(market_type(Market::Margin), MarketType::Margin);
    }

    /// The source takes its ticker and its book off the stream, never by REST.
    ///
    /// `MarketData` also offers `ticker` and `order_book` as one-shot requests,
    /// and a source that reached for either per tick would be rate-limited off
    /// the venue within minutes -- quietly, because each call on its own
    /// succeeds. The only REST call a subscription makes is the one backfill,
    /// which is counted separately.
    #[test]
    fn a_subscription_polls_no_market_data_over_rest() {
        let stub = StubExchange::new();
        stub.state.borrow_mut().bars =
            vec![wickra_exchange::Candle::new(100.0, 110.0, 95.0, 105.0, 1.0, 0).expect("valid")];
        let mut source = stubbed(&stub);
        let btc = Symbol::new("BTC", "USDT");

        source.subscribe(&btc).expect("the stub accepts");
        source.backfill(&btc, "1m", 10);
        stub.queue(vec![print(&btc, 100)]);
        for _ in 0..5 {
            source.poll();
        }

        assert_eq!(
            stub.rest_polls(),
            0,
            "the source polled market data over REST"
        );
        assert!(
            stub.klines_asked().is_some(),
            "the one backfill did not happen"
        );
    }

    /// And when something does ask, the stub refuses rather than inventing a
    /// book -- so the assertion above is measuring a real refusal, not a method
    /// that quietly answers.
    #[test]
    fn the_stub_refuses_a_rest_request_and_counts_it() {
        let stub = StubExchange::new();
        let btc = Symbol::new("BTC", "USDT");
        let mut client = stub.clone();

        assert!(wickra_exchange::MarketData::ticker(&mut client, &btc).is_err());
        assert!(wickra_exchange::MarketData::order_book(&mut client, &btc, 10).is_err());
        assert_eq!(stub.rest_polls(), 2);
    }

    #[test]
    fn a_venue_candle_crosses_the_version_gap() {
        // The exchange pins wickra-core 0.9 and this crate builds against 1, so
        // the compiler sees two identical structs as unrelated types. The
        // conversion re-validates rather than trusting the shape.
        let bar = wickra_exchange::Candle::new(100.0, 110.0, 95.0, 105.0, 7.0, 42)
            .expect("a valid venue candle");
        let core = into_core(&bar).expect("a valid candle converts");
        // Bit-for-bit, not within a tolerance: the conversion copies fields, and
        // a copy that changed a value would be the bug worth catching.
        let same = |a: f64, b: f64| (a - b).abs() < f64::EPSILON;
        assert!(same(core.open, 100.0), "open: {}", core.open);
        assert!(same(core.high, 110.0), "high: {}", core.high);
        assert!(same(core.low, 95.0), "low: {}", core.low);
        assert!(same(core.close, 105.0), "close: {}", core.close);
        assert!(same(core.volume, 7.0), "volume: {}", core.volume);
        assert_eq!(core.timestamp, 42);
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
