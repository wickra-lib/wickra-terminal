//! Gated live-integration smoke test.
//!
//! Builds real `Live` sources over the exchange connectivity layer, connects them
//! to public venues, subscribes a liquid market on each and waits for streamed
//! events to fold end-to-end through the terminal — the one path the offline
//! suite cannot cover. It hits **public** endpoints only (no API keys).
//!
//! `#[ignore]`d so it never runs on a normal push (network flakiness would flake
//! every PR); the `testnet.yml` workflow runs it nightly and on demand to surface
//! upstream/API drift — the same pattern as `wickra-exchange`.
//!
//! Three venues rather than one, because a hosted CI runner cannot reach every
//! venue. Binance answers a data-centre address with HTTP 451, so a test pinned
//! to Binance alone skipped on every nightly run and verified nothing. OKX and
//! Kraken answer the same runners — `wickra-exchange`'s own nightly reads both —
//! so the nightly proves the live path on at least one venue while a developer on
//! an ordinary network still exercises all three.
//!
//! Each venue ends in one of three states:
//!
//! * **streamed** — a trade, a ticker or a book update folded into its market.
//!   Only the socket delivers those. The REST backfill seeds bars and the last
//!   price and nothing else, which is why the last price is not the signal: it is
//!   set before the first event arrives.
//! * **silent** — the backfill answered, so the venue is reachable, yet nothing
//!   streamed before the deadline. That is the live path broken, and it fails
//!   whatever the environment says.
//! * **unreachable** — neither answered, which is what a geo-block looks like.
//!   It fails by default. With `WICKRA_LIVE_ALLOW_SKIP=1` — as `testnet.yml` sets
//!   it — it is reported and skipped, provided another venue streamed; if none
//!   did, nothing was verified and the job carries a warning that says so.

#![cfg(feature = "live")]

use std::thread::sleep;
use std::time::{Duration, Instant};

use rust_decimal::Decimal;
use wickra_terminal_core::{Config, Market, SourceSpec, Symbol, Terminal};

/// The venues the test opens, in source order.
const VENUES: [&str; 3] = ["binance", "okx", "kraken"];

/// How long a venue gets to deliver its first streamed event.
const DEADLINE: Duration = Duration::from_secs(30);

/// What one venue showed by the deadline.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Outcome {
    Streamed,
    Silent,
    Unreachable,
}

#[test]
#[ignore = "hits live public venue endpoints; run via testnet.yml"]
fn live_venues_stream_public_market_data() {
    let sym = Symbol::new("BTC", "USDT");
    let mut config = Config::default_layout();
    config.sources = VENUES
        .iter()
        .map(|venue| SourceSpec::Live {
            venue: (*venue).to_string(),
            symbol: "BTC/USDT".to_string(),
            testnet: false,
            market: Market::Spot,
        })
        .collect();

    // `Terminal::new` connects a read-only client per venue and auto-subscribes
    // its market — real HTTP/WebSocket handshakes — and backfills each market's
    // bars over REST. Sources are numbered from zero in config order.
    let mut terminal = Terminal::new(&config).expect("connect + subscribe BTC/USDT on every venue");
    let keys: Vec<_> = (0_u32..)
        .zip(VENUES)
        .map(|(id, _)| (id, sym.clone()))
        .collect();

    // Whether the venue answered the REST backfill: the history is empty until a
    // bar is seeded or an event streams, and nothing has streamed yet.
    let answered: Vec<bool> = keys
        .iter()
        .map(|key| {
            terminal
                .state()
                .get(key)
                .is_some_and(|market| !market.history.is_empty())
        })
        .collect();

    let mut streamed = [false; VENUES.len()];
    let deadline = Instant::now() + DEADLINE;
    while Instant::now() < deadline && !streamed.iter().all(|seen| *seen) {
        terminal.tick();
        for (seen, key) in streamed.iter_mut().zip(&keys) {
            *seen = *seen
                || terminal.state().get(key).is_some_and(|market| {
                    !market.tape.is_empty()
                        || market.bid > Decimal::ZERO
                        || market.book.mid().is_some()
                });
        }
        sleep(Duration::from_millis(250));
    }

    let outcomes: Vec<(&str, Outcome)> = VENUES
        .iter()
        .zip(streamed.iter().zip(&answered))
        .map(|(venue, (seen, reached))| {
            let outcome = if *seen {
                Outcome::Streamed
            } else if *reached {
                Outcome::Silent
            } else {
                Outcome::Unreachable
            };
            (*venue, outcome)
        })
        .collect();
    for (venue, outcome) in &outcomes {
        eprintln!("  {venue}: {outcome:?}");
    }
    let named = |wanted: Outcome| -> Vec<&str> {
        outcomes
            .iter()
            .filter(|(_, outcome)| *outcome == wanted)
            .map(|(venue, _)| *venue)
            .collect()
    };

    let silent = named(Outcome::Silent);
    assert!(
        silent.is_empty(),
        "answered over REST but streamed nothing within {DEADLINE:?}: {silent:?}"
    );

    let unreachable = named(Outcome::Unreachable);
    if unreachable.is_empty() {
        return;
    }
    let unreachable = unreachable.join(", ");
    assert!(
        std::env::var_os("WICKRA_LIVE_ALLOW_SKIP").is_some(),
        "unreachable from this network: {unreachable}; set WICKRA_LIVE_ALLOW_SKIP=1 if it cannot reach them"
    );

    let on_actions = std::env::var_os("GITHUB_ACTIONS").is_some();
    let verified = named(Outcome::Streamed);
    if verified.is_empty() {
        // Not one venue answered, so this run proved nothing about the live path.
        // That stays loud: a runner that can reach no venue at all is worth
        // seeing, where a single geo-blocked venue is not.
        let message = format!(
            "live integration skipped: no venue reachable from this runner ({unreachable})"
        );
        if on_actions {
            println!("::warning::{message}");
        }
        eprintln!("{message}");
        return;
    }
    let message = format!(
        "live integration verified on {}; unreachable from this runner: {unreachable}",
        verified.join(", ")
    );
    if on_actions {
        println!("::notice::{message}");
    }
    eprintln!("{message}");
}
