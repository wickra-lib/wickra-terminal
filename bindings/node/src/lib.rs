//! Node.js bindings for `wickra-terminal` (napi-rs).
//!
//! Thin glue over the terminal core's data-driven surface: build a `Terminal`
//! from a JSON config, drive it with a command JSON and read back the frame
//! JSON. The same command protocol crosses every binding, so a Node front-end
//! drives the exact same core as the native TUI.

#![allow(missing_debug_implementations)]
// napi exposes owned `String` arguments; the bodies only need to borrow them.
#![allow(clippy::needless_pass_by_value)]

use napi::Result;
use napi_derive::napi;

use terminal_core::Terminal as CoreTerminal;

/// Build a napi error from a message.
fn err(message: impl Into<String>) -> napi::Error {
    napi::Error::from_reason(message.into())
}

/// The library version.
#[napi]
pub fn version() -> String {
    CoreTerminal::version().to_string()
}

/// A trading terminal instance driven by JSON commands.
#[napi]
pub struct Terminal {
    inner: CoreTerminal,
}

#[napi]
impl Terminal {
    /// Build a terminal from a JSON config string.
    #[napi(constructor)]
    pub fn new(config_json: String) -> Result<Self> {
        CoreTerminal::from_json(&config_json)
            .map(|inner| Self { inner })
            .map_err(|e| err(e.to_string()))
    }

    /// Apply a command JSON and return the resulting frame JSON.
    #[napi]
    pub fn command(&mut self, cmd_json: String) -> Result<String> {
        self.inner
            .command_json(&cmd_json)
            .map_err(|e| err(e.to_string()))
    }

    /// The library version.
    #[napi]
    pub fn version(&self) -> String {
        CoreTerminal::version().to_string()
    }
}
