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
    #[napi(constructor, catch_unwind)]
    pub fn new(config_json: String) -> Result<Self> {
        CoreTerminal::from_json(&config_json)
            .map(|inner| Self { inner })
            .map_err(|e| err(e.to_string()))
    }

    /// Apply a command JSON and return the resulting frame JSON.
    #[napi(catch_unwind)]
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

#[cfg(test)]
mod tests {
    /// napi emits `catch_unwind` only where the attribute asks for it.
    ///
    /// Unlike pyo3, whose trampolines catch unconditionally, napi-rs gates the
    /// wrapper on `#[napi(catch_unwind)]`: without it a panic in the core
    /// unwinds out of the addon and takes the Node process with it, measured as
    /// an exit code of 127 with no JavaScript catch ever reached. The workspace
    /// release profile builds with `panic = "unwind"` precisely so a boundary can
    /// catch, which does nothing on a boundary that does not.
    ///
    /// Reading the source rather than provoking a panic, because nothing a caller
    /// can send makes the core panic on purpose -- which is also why the gap went
    /// unnoticed long enough for a comment to claim it was covered.
    #[test]
    fn every_fallible_entry_point_catches_a_panic() {
        let source = include_str!("lib.rs");
        let mut attribute = None;
        let mut checked = 0;
        for line in source.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("#[napi") {
                attribute = Some(trimmed);
                continue;
            }
            if trimmed.starts_with("pub fn ") && trimmed.contains("-> Result<") {
                let attr = attribute.expect("a napi entry point carries an attribute");
                assert!(
                    attr.contains("catch_unwind"),
                    "{trimmed} can fail but {attr} does not catch a panic"
                );
                checked += 1;
            }
            if !trimmed.is_empty() && !trimmed.starts_with("///") {
                attribute = None;
            }
        }
        assert_eq!(checked, 2, "expected the constructor and command");
    }
}
