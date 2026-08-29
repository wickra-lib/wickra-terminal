//! Gated live-integration smoke test.
//!
//! Builds a real `Live` source over the exchange connectivity layer, connects to
//! a public venue, subscribes a liquid market and folds a few streamed events
//! end-to-end through the terminal — the one path the offline suite cannot cover.
//! It hits **public** endpoints only (no API keys).
//!
//! `#[ignore]`d so it never runs on a normal push (network flakiness would flake
//! every PR); the `testnet.yml` workflow runs it nightly and on demand to surface
//! upstream/API drift — the same pattern as `wickra-exchange`.
//!
//! Reaching the deadline without data is a failure by default. Set
//! `WICKRA_LIVE_ALLOW_SKIP=1` -- as `testnet.yml` does -- to degrade that to a
//! logged skip, for networks the venue geo-restricts.

#![cfg(feature = "live")]

use std::thread::sleep;
use std::time::{Duration, Instant};

use rust_decimal::Decimal;
use wickra_terminal_core::{Config, SourceSpec, Symbol, Terminal};

#[test]
#[ignore = "hits live Binance public endpoints; run via testnet.yml"]
fn live_binance_streams_public_market_data() {
    let sym = Symbol::new("BTC", "USDT");
    let mut config = Config::default_layout();
    config.sources = vec![SourceSpec::Live {
        venue: "binance".to_string(),
        symbol: "BTC/USDT".to_string(),
        testnet: false,
    }];

    // `Terminal::new` connects the read-only client and auto-subscribes the live
    // market — a real HTTP/WebSocket handshake.
    let mut terminal = Terminal::new(&config).expect("connect + subscribe binance BTC/USDT");

    // BTC/USDT is highly liquid, so a trade or ticker arrives within seconds.
    // Poll for up to ~20s before giving up.
    let deadline = Instant::now() + Duration::from_secs(20);
    let mut saw_price = false;
    while Instant::now() < deadline {
        terminal.tick();
        if terminal
            .state()
            .get(&(0, sym.clone()))
            .is_some_and(|state| state.last > Decimal::ZERO)
        {
            saw_price = true;
            break;
        }
        sleep(Duration::from_millis(250));
    }

    if saw_price {
        return;
    }

    // No price within the deadline has two very different causes, and the test
    // cannot tell them apart from here: the venue is unreachable from this network
    // (Binance geo-restricts data-centre and CI-runner IP ranges, and the block
    // shows up as silence rather than an error, because `connect` only builds the
    // HTTP client and the subscribe handshake is asynchronous), or the live path is
    // genuinely broken.
    //
    // So the caller decides which it is. Unset -- a developer on an ordinary
    // network -- the deadline is a failure, which is what a test is for. Set by
    // `testnet.yml`, where a hosted runner may well be blocked, it degrades to a
    // loud skip so a geo-block does not paint the nightly red forever. Either way
    // it stays visible: passing silently on no data is what left this test unable
    // to report anything at all.
    let message = "no live BTC/USDT price within 20s";
    let restricted = "(venue likely restricted from this runner)";
    if std::env::var_os("WICKRA_LIVE_ALLOW_SKIP").is_some() {
        if std::env::var_os("GITHUB_ACTIONS").is_some() {
            println!("::warning::live integration skipped: {message} {restricted}");
        }
        eprintln!("skipping live_binance_streams_public_market_data: {message} {restricted}");
        return;
    }
    panic!("{message}; set WICKRA_LIVE_ALLOW_SKIP=1 if this network cannot reach the venue");
}
