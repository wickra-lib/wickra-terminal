#![no_main]
//! Fuzz the data-driven command boundary: a synth terminal is driven with
//! arbitrary command JSON. Every command — known or malformed — must return a
//! typed result without panicking, and the produced frame must serialize.

use libfuzzer_sys::fuzz_target;
use terminal_core::{Config, SourceSpec, Symbol, Terminal};

fuzz_target!(|data: &[u8]| {
    let Ok(text) = std::str::from_utf8(data) else {
        return;
    };
    let mut config = Config::default_layout();
    config.sources = vec![SourceSpec::Synth { seed: 1 }];
    let Ok(mut terminal) = Terminal::new(&config) else {
        return;
    };
    let _ = terminal.subscribe(0, &Symbol::new("BTC", "USDT"));
    let _ = terminal.command_json(text);
});
