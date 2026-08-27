//! Criterion benchmarks for the terminal core's per-tick hot paths: folding one
//! trade, applying an order-book diff, building the frame, a full `tick`, and a
//! `command_json` round-trip.

use std::hint::black_box;

use criterion::{criterion_group, criterion_main, Criterion};
use rust_decimal::Decimal;
use terminal_core::{AppState, Config, SourceSpec, Symbol, Terminal};
use wickra_exchange_core::{BookDelta, BookLevel, Event, OrderSide, TradePrint};

fn synth_terminal() -> Terminal {
    let mut config = Config::default_layout();
    config.sources = vec![SourceSpec::Synth { seed: 1 }];
    let mut terminal = Terminal::new(&config).unwrap();
    terminal.subscribe(0, &Symbol::new("BTC", "USDT")).unwrap();
    terminal
}

fn trade(sym: &Symbol) -> Event {
    Event::Trade(TradePrint {
        symbol: sym.clone(),
        price: Decimal::new(20_000, 0),
        quantity: Decimal::new(1, 0),
        aggressor: OrderSide::Buy,
        timestamp: 0,
    })
}

/// A depth diff of the shape a venue actually sends: a handful of changed
/// levels per side, some of them removals.
fn book_delta(sym: &Symbol, seq: u64) -> Event {
    let level = |price: i64, qty: i64| BookLevel {
        price: Decimal::new(price, 2),
        quantity: Decimal::new(qty, 3),
    };
    Event::BookDelta(BookDelta {
        symbol: sym.clone(),
        first_update_id: seq,
        final_update_id: seq,
        bids: vec![
            level(1_999_900, 1_500),
            level(1_999_800, 0), // a removal
            level(1_999_700, 2_250),
        ],
        asks: vec![
            level(2_000_100, 1_200),
            level(2_000_200, 0), // a removal
            level(2_000_300, 3_000),
        ],
    })
}

fn benchmarks(c: &mut Criterion) {
    c.bench_function("fold_trade", |b| {
        let sym = Symbol::new("BTC", "USDT");
        let mut state = AppState::default();
        let event = trade(&sym);
        b.iter(|| state.fold(0, black_box(&sym), black_box(&event)));
    });

    // The highest-rate message on a live feed: a venue sends depth diffs far
    // more often than trades, so this is the fold path that decides whether a
    // busy market keeps up.
    c.bench_function("book_delta", |b| {
        let sym = Symbol::new("BTC", "USDT");
        let mut state = AppState::default();
        let event = book_delta(&sym, 1);
        b.iter(|| state.fold(0, black_box(&sym), black_box(&event)));
    });

    // Building every panel's view-model without polling, which is what a
    // renderer pays on a redraw that has no new data behind it.
    c.bench_function("frame_build", |b| {
        let mut terminal = synth_terminal();
        for _ in 0..200 {
            terminal.tick();
        }
        b.iter(|| black_box(terminal.frame()));
    });

    c.bench_function("tick_synth", |b| {
        let mut terminal = synth_terminal();
        b.iter(|| black_box(terminal.tick()));
    });

    c.bench_function("command_json_tick", |b| {
        let mut terminal = synth_terminal();
        b.iter(|| {
            black_box(
                terminal
                    .command_json(black_box("{\"type\":\"Tick\"}"))
                    .unwrap(),
            )
        });
    });
}

criterion_group!(benches, benchmarks);
criterion_main!(benches);
