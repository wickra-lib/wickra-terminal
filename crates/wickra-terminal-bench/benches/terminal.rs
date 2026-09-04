//! Criterion benchmarks for the terminal core's per-tick hot paths: folding one
//! trade, applying an order-book diff, building the frame, a full `tick`, and a
//! `command_json` round-trip.

use std::hint::black_box;

use criterion::{criterion_group, criterion_main, Criterion};
use rust_decimal::Decimal;
use wickra_exchange_core::{BookDelta, BookLevel, Event, OrderSide, TradePrint};
use wickra_terminal_core::{AppState, Config, SourceSpec, Symbol, Terminal};

fn synth_terminal() -> Terminal {
    let mut config = Config::default_layout();
    config.sources = vec![SourceSpec::Synth { seed: 1 }];
    let mut terminal = Terminal::new(&config).unwrap();
    terminal.subscribe(0, &Symbol::new("BTC", "USDT")).unwrap();
    terminal
}

/// The exact config the nine per-binding throughput benchmarks build, so the
/// `command_json_bench_config` row below is the same operation they time with a
/// language boundary in the way. `command_json_tick` above uses the default
/// five-panel layout with its default indicators, which is a heavier tick and
/// not comparable to them.
const BENCH_CONFIG: &str = concat!(
    "{\"sources\":[{\"Synth\":{\"seed\":1}}],",
    "\"layout\":{\"panels\":[",
    "{\"kind\":\"Chart\",\"rect\":{\"x\":0,\"y\":0,\"w\":100,\"h\":40}},",
    "{\"kind\":\"Book\",\"rect\":{\"x\":0,\"y\":40,\"w\":50,\"h\":30}},",
    "{\"kind\":\"Tape\",\"rect\":{\"x\":50,\"y\":40,\"w\":50,\"h\":30}}]}}",
);

fn bench_terminal() -> Terminal {
    let mut terminal = Terminal::from_json(BENCH_CONFIG).unwrap();
    terminal
        .command_json("{\"type\":\"Subscribe\",\"source\":0,\"symbol\":\"BTC/USDT\"}")
        .unwrap();
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
        // Zero, like the trade fixture above: the fold being measured never
        // reads it, and a clock in a benchmark input is a number that has to be
        // held steady for the measurement to mean anything.
        timestamp: 0,
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

    // The no-boundary baseline for the per-binding throughput benchmarks: same
    // config, same command, no language boundary in the way.
    c.bench_function("command_json_bench_config", |b| {
        let mut terminal = bench_terminal();
        b.iter(|| {
            black_box(
                terminal
                    .command_json(black_box("{\"type\":\"Tick\"}"))
                    .unwrap(),
            );
        });
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
