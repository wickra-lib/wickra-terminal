#![no_main]
//! Fuzz the event-parsing path: arbitrary bytes are deserialized as the public
//! `Event` type and as a feed (a `Vec<Event>`). Neither must panic; malformed
//! input must surface as a clean `Err`.

use libfuzzer_sys::fuzz_target;
use terminal_core::Event;

fuzz_target!(|data: &[u8]| {
    let Ok(text) = std::str::from_utf8(data) else {
        return;
    };
    let _ = serde_json::from_str::<Event>(text);
    let _ = serde_json::from_str::<Vec<Event>>(text);
});
