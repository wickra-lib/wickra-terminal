#![no_main]
//! Fuzz the config-parsing path: arbitrary bytes are parsed as a TOML config, a
//! JSON config, and used to construct a terminal. None must panic; malformed
//! input must surface as a clean `Err`.

use libfuzzer_sys::fuzz_target;
use terminal_core::{Config, Terminal};

fuzz_target!(|data: &[u8]| {
    let Ok(text) = std::str::from_utf8(data) else {
        return;
    };
    let _ = Config::from_toml(text);
    let _ = Config::from_json(text);
    let _ = Terminal::from_json(text);
});
