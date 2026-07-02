//! Criterion benchmarks for the terminal core's per-tick hot paths: folding one
//! event into state, a full `tick`, and a `command_json` round-trip.

use std::hint::black_box;

use criterion::{criterion_group, criterion_main, Criterion};
use rust_decimal::Decimal;
use terminal_core::{AppState, Config, SourceSpec, Symbol, Terminal};
use wickra_exchange_core::{Event, OrderSide, TradePrint};

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

fn benchmarks(c: &mut Criterion) {
    c.bench_function("fold_trade", |b| {
        let sym = Symbol::new("BTC", "USDT");
        let mut state = AppState::default();
        let event = trade(&sym);
        b.iter(|| state.fold(0, black_box(&sym), black_box(&event)));
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
