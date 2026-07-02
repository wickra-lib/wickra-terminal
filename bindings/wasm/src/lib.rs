//! WebAssembly bindings for `wickra-terminal` (wasm-bindgen).
//!
//! The data-driven core, compiled to WebAssembly for the browser: build a
//! `Terminal` from a JSON config, drive it with a command JSON and read back the
//! frame JSON. This is the binding the web renderer runs on — the same command
//! protocol as the native TUI and every other binding.
//!
//! The `live` feature of the core is disabled here: the native exchange client
//! cannot run in a browser sandbox, so the web renderer feeds a `Live` source
//! over the browser's own WebSocket instead.

use wasm_bindgen::prelude::*;

use terminal_core::Terminal as CoreTerminal;

/// A trading terminal instance driven by JSON commands.
#[wasm_bindgen]
pub struct Terminal {
    inner: CoreTerminal,
}

#[wasm_bindgen]
impl Terminal {
    /// Build a terminal from a JSON config string.
    #[wasm_bindgen(constructor)]
    pub fn new(config_json: &str) -> Result<Terminal, JsError> {
        CoreTerminal::from_json(config_json)
            .map(|inner| Self { inner })
            .map_err(|e| JsError::new(&e.to_string()))
    }

    /// Apply a command JSON and return the resulting frame JSON.
    pub fn command(&mut self, cmd_json: &str) -> Result<String, JsError> {
        self.inner
            .command_json(cmd_json)
            .map_err(|e| JsError::new(&e.to_string()))
    }

    /// The library version.
    #[wasm_bindgen(js_name = version)]
    pub fn instance_version(&self) -> String {
        CoreTerminal::version().to_string()
    }
}

/// The library version.
#[wasm_bindgen]
pub fn version() -> String {
    CoreTerminal::version().to_string()
}
