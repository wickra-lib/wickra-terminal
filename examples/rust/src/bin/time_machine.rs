//! A runnable Rust example: rewind a recorded feed and watch state re-fold.
//!
//! The time-machine is what makes a recording more than a slow synthetic feed:
//! `Seek` throws the folded state away and rebuilds it from the recording, so a
//! rewind is deterministic rather than approximate.
//!
//! Rust reaches the same commands twice over -- through `command_json` like
//! every other language, and through typed methods. Both are shown, because a
//! Rust embedder should not have to assemble JSON to reach its own core.
//!
//! ```bash
//! cargo run -p wickra-terminal-example --bin time_machine
//! ```

use wickra_terminal_core::{Config, PanelView, SourceSpec, Symbol, Terminal};

const TRADES: usize = 6;

/// The recorded feed, as the JSON array a `Replay` source takes.
fn feed() -> String {
    let events: Vec<String> = (0..TRADES)
        .map(|i| {
            format!(
                r#"{{"type":"trade","symbol":{{"base":"BTC","quote":"USDT"}},"price":"{}","quantity":"1","aggressor":"Buy","timestamp":{}}}"#,
                100 + i,
                i + 1
            )
        })
        .collect();
    format!("[{}]", events.join(","))
}

/// The chart panel's last price, out of a frame.
fn last(terminal: &Terminal) -> f64 {
    terminal
        .frame()
        .panels
        .into_iter()
        .find_map(|panel| match panel {
            PanelView::Chart(chart) => Some(chart.last),
            _ => None,
        })
        .unwrap_or(0.0)
}

fn main() {
    let mut config = Config::default_layout();
    config.sources = vec![SourceSpec::Replay { dataset: feed() }];

    let mut terminal = Terminal::new(&config).expect("valid config");
    terminal
        .subscribe(0, &Symbol::new("BTC", "USDT"))
        .expect("subscribe");

    for _ in 0..TRADES {
        terminal.tick();
    }
    println!("played to the end:   last = {}", last(&terminal));

    // Typed, because this is Rust. Every other binding asks the same question
    // with {"type":"ReplayPosition","source":0}.
    if let Some((cursor, length)) = terminal.replay_position(0) {
        println!("position:            {cursor}/{length}");
    }

    // Rewind to just after the second trade. The state is rebuilt from the
    // recording rather than restored from a snapshot, which is why a rewind
    // lands on exactly the frame the forward pass had at that point.
    terminal.seek(0, 2).expect("seek");
    println!("rewound to index 2:  last = {}", last(&terminal));

    // And forward again from there, over the same events -- this time through
    // the JSON boundary, which is the one every language shares.
    terminal
        .command_json(r#"{"type":"Tick"}"#)
        .expect("tick command");
    println!("one tick later:      last = {}", last(&terminal));
}
