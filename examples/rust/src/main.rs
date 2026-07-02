//! A runnable Rust example: drive a synthetic feed and print a frame.
//!
//! ```bash
//! cargo run -p wickra-terminal-example
//! ```

use terminal_core::{Config, SourceSpec, Symbol, Terminal};

fn main() {
    let mut config = Config::default_layout();
    config.sources = vec![SourceSpec::Synth { seed: 1 }];

    let mut terminal = Terminal::new(&config).expect("valid config");
    terminal
        .subscribe(0, &Symbol::new("BTC", "USDT"))
        .expect("subscribe");

    for _ in 0..20 {
        terminal.tick();
    }
    let frame_json = terminal
        .command_json("{\"type\":\"Tick\"}")
        .expect("tick command");

    println!("wickra-terminal {}", Terminal::version());
    println!("{frame_json}");
}
