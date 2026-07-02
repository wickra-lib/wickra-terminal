//! Golden-fixture parity: a committed recorded feed (`golden/replay/basic.json`)
//! drives the terminal and must produce the byte-identical frame view-models in
//! `golden/expected/basic.json`, so the deterministic feed-to-frame pipeline can
//! never drift silently — and every language binding can be checked against the
//! same fixture. Regenerate the fixtures with `WICKRA_REGEN=1 cargo test`.

use std::fs;

use rust_decimal::Decimal;
use terminal_core::{Config, SourceSpec, Symbol, Terminal};
use wickra_exchange_core::{BookLevel, Event, OrderBookSnapshot, OrderSide, TradePrint};

fn golden_dir() -> String {
    format!("{}/../../golden", env!("CARGO_MANIFEST_DIR"))
}

fn trade(sym: &Symbol, price: i64, qty: i64, buy: bool, ts: i64) -> Event {
    Event::Trade(TradePrint {
        symbol: sym.clone(),
        price: Decimal::new(price, 0),
        quantity: Decimal::new(qty, 2),
        aggressor: if buy { OrderSide::Buy } else { OrderSide::Sell },
        timestamp: ts,
    })
}

/// The canonical recorded feed: a handful of prints plus a book snapshot.
fn canonical_feed(sym: &Symbol) -> Vec<Event> {
    vec![
        trade(sym, 20_000, 50, true, 1),
        trade(sym, 20_001, 30, true, 2),
        trade(sym, 19_999, 40, false, 3),
        Event::BookSnapshot(OrderBookSnapshot {
            symbol: sym.clone(),
            last_update_id: 10,
            bids: vec![
                BookLevel::new(Decimal::new(19_999, 0), Decimal::new(15, 1)),
                BookLevel::new(Decimal::new(19_998, 0), Decimal::new(25, 1)),
            ],
            asks: vec![
                BookLevel::new(Decimal::new(20_001, 0), Decimal::new(12, 1)),
                BookLevel::new(Decimal::new(20_002, 0), Decimal::new(30, 1)),
            ],
        }),
        trade(sym, 20_002, 20, true, 4),
        trade(sym, 20_000, 10, false, 5),
    ]
}

#[test]
fn golden_basic_frame_is_byte_exact() {
    let dir = golden_dir();
    let sym = Symbol::new("BTC", "USDT");
    let feed = canonical_feed(&sym);

    let feed_pretty = format!("{}\n", serde_json::to_string_pretty(&feed).unwrap());
    let dataset = serde_json::to_string(&feed).unwrap();

    let mut config = Config::default_layout();
    config.sources = vec![SourceSpec::Replay { dataset }];
    let mut terminal = Terminal::new(&config).unwrap();
    terminal.subscribe(0, &sym).unwrap();
    for _ in 0..feed.len() {
        terminal.tick();
    }
    let frame_pretty = format!(
        "{}\n",
        serde_json::to_string_pretty(&terminal.frame()).unwrap()
    );

    let replay_path = format!("{dir}/replay/basic.json");
    let expected_path = format!("{dir}/expected/basic.json");

    if std::env::var("WICKRA_REGEN").is_ok() {
        fs::create_dir_all(format!("{dir}/replay")).unwrap();
        fs::create_dir_all(format!("{dir}/expected")).unwrap();
        fs::write(&replay_path, &feed_pretty).unwrap();
        fs::write(&expected_path, &frame_pretty).unwrap();
    }

    let committed_feed = fs::read_to_string(&replay_path)
        .unwrap_or_else(|_| panic!("missing {replay_path}; regenerate with WICKRA_REGEN=1"));
    let committed_frame = fs::read_to_string(&expected_path)
        .unwrap_or_else(|_| panic!("missing {expected_path}; regenerate with WICKRA_REGEN=1"));

    assert_eq!(
        feed_pretty, committed_feed,
        "recorded feed drifted from golden/replay/basic.json"
    );
    assert_eq!(
        frame_pretty, committed_frame,
        "frame drifted from golden/expected/basic.json — regenerate with WICKRA_REGEN=1"
    );
}
