#![no_main]
//! Fuzz the state fold: arbitrary bytes are parsed as a feed and folded into a
//! fresh `AppState`. No sequence of events — however adversarial (huge volumes,
//! crossed books) — may panic.

use libfuzzer_sys::fuzz_target;
use terminal_core::{AppState, Event, Symbol};

fuzz_target!(|data: &[u8]| {
    let Ok(text) = std::str::from_utf8(data) else {
        return;
    };
    let Ok(events) = serde_json::from_str::<Vec<Event>>(text) else {
        return;
    };
    let symbol = Symbol::new("BTC", "USDT");
    let mut state = AppState::default();
    for event in &events {
        state.fold(0, &symbol, event);
    }
});
