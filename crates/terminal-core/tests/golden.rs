//! Golden-fixture parity: a committed recorded feed (`golden/replay/basic.json`)
//! drives the terminal and must produce the byte-identical frame view-models in
//! `golden/expected/basic.json`, so the deterministic feed-to-frame pipeline can
//! never drift silently — and every language binding can be checked against the
//! same fixture. Regenerate the fixtures with `WICKRA_REGEN=1 cargo test`.
//!
//! Three artifacts back the cross-language parity check (`golden/`):
//! - `replay/basic.json` — the recorded feed (human-readable, pretty).
//! - `expected/basic.json` — the frame view-models (human-readable, pretty).
//! - `config.json` — the complete `Terminal::new` config (a `Replay` source with
//!   the feed embedded + the default layout), so every binding constructs the
//!   identical terminal from one committed file with no JSON assembly.
//! - `expected/basic.min.json` — the frame exactly as `command_json` emits it
//!   (compact `serde_json::to_string`). Because every binding returns that string
//!   verbatim, a binding is at parity iff its frame equals this file byte-for-byte
//!   — no per-language JSON deep-equal needed. See `docs/STREAMING.md`.

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
    config.sources = vec![SourceSpec::Replay {
        dataset: dataset.clone(),
    }];
    let mut terminal = Terminal::new(&config).unwrap();
    terminal.subscribe(0, &sym).unwrap();
    for _ in 0..feed.len() {
        terminal.tick();
    }
    let frame_pretty = format!(
        "{}\n",
        serde_json::to_string_pretty(&terminal.frame()).unwrap()
    );
    // The compact frame exactly as `command_json` emits it — the byte-for-byte
    // artifact every binding compares against.
    let frame_min = serde_json::to_string(&terminal.frame()).unwrap();

    // Over-ticking past the exhausted feed must not change the frame: the
    // bindings tick a fixed count (> the feed length) and rely on the frame being
    // stable once the replay is drained.
    for _ in 0..16 {
        terminal.tick();
    }
    assert_eq!(
        frame_min,
        serde_json::to_string(&terminal.frame()).unwrap(),
        "frame changed after the replay was exhausted; bindings over-tick and rely on stability"
    );

    // The full binding config: a Replay source with the feed embedded plus the
    // default layout. Keybinds are omitted (they carry a non-deterministic map
    // order and never affect the frame); `Terminal::new` fills the defaults.
    let config_json = serde_json::json!({
        "sources": config.sources,
        "layout": { "panels": config.layout.panels },
    });
    let config_pretty = format!("{}\n", serde_json::to_string_pretty(&config_json).unwrap());

    let replay_path = format!("{dir}/replay/basic.json");
    let expected_path = format!("{dir}/expected/basic.json");
    let expected_min_path = format!("{dir}/expected/basic.min.json");
    let config_path = format!("{dir}/config.json");

    if std::env::var("WICKRA_REGEN").is_ok() {
        fs::create_dir_all(format!("{dir}/replay")).unwrap();
        fs::create_dir_all(format!("{dir}/expected")).unwrap();
        fs::write(&replay_path, &feed_pretty).unwrap();
        fs::write(&expected_path, &frame_pretty).unwrap();
        fs::write(&expected_min_path, &frame_min).unwrap();
        fs::write(&config_path, &config_pretty).unwrap();
    }

    let committed_feed = fs::read_to_string(&replay_path)
        .unwrap_or_else(|_| panic!("missing {replay_path}; regenerate with WICKRA_REGEN=1"));
    let committed_frame = fs::read_to_string(&expected_path)
        .unwrap_or_else(|_| panic!("missing {expected_path}; regenerate with WICKRA_REGEN=1"));
    let committed_frame_min = fs::read_to_string(&expected_min_path)
        .unwrap_or_else(|_| panic!("missing {expected_min_path}; regenerate with WICKRA_REGEN=1"));
    let committed_config = fs::read_to_string(&config_path)
        .unwrap_or_else(|_| panic!("missing {config_path}; regenerate with WICKRA_REGEN=1"));

    assert_eq!(
        feed_pretty, committed_feed,
        "recorded feed drifted from golden/replay/basic.json"
    );
    assert_eq!(
        frame_pretty, committed_frame,
        "frame drifted from golden/expected/basic.json — regenerate with WICKRA_REGEN=1"
    );
    assert_eq!(
        frame_min,
        committed_frame_min.trim_end(),
        "compact frame drifted from golden/expected/basic.min.json — regenerate with WICKRA_REGEN=1"
    );
    assert_eq!(
        config_pretty, committed_config,
        "binding config drifted from golden/config.json — regenerate with WICKRA_REGEN=1"
    );

    // The committed config must actually reproduce the golden frame, exactly as a
    // binding will drive it from `golden/config.json`.
    let mut from_file = Terminal::new(&Config::from_json(committed_config.trim_end()).unwrap())
        .expect("golden/config.json is a valid terminal config");
    from_file.subscribe(0, &sym).unwrap();
    for _ in 0..feed.len() {
        from_file.tick();
    }
    assert_eq!(
        frame_min,
        serde_json::to_string(&from_file.frame()).unwrap(),
        "golden/config.json does not reproduce the golden frame"
    );
}
