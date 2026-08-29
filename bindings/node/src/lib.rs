//! Node.js bindings for `wickra-terminal` (napi-rs).
//!
//! Thin glue over the terminal core's data-driven surface: build a `Terminal`
//! from a JSON config, drive it with a command JSON and read back the frame
//! JSON. The same command protocol crosses every binding, so a Node front-end
//! drives the exact same core as the native TUI.

// napi exposes owned `String` arguments; the bodies only need to borrow them.
#![allow(clippy::needless_pass_by_value)]

use napi::Result;
use napi_derive::napi;

use wickra_terminal_core::Terminal as CoreTerminal;

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
//
// CodeQL reports `rust/access-invalid-pointer` on this line, and it is a false
// positive worth writing down rather than re-triaging every time it resurfaces.
//
// This file has no `unsafe`, no raw pointer and no `from_raw`/`into_raw` -- zero
// occurrences in 98 lines. What CodeQL analyses is the napi-derive expansion,
// which this line is the anchor for: `cargo expand -p wickra-terminal-node`
// shows 29 generated `unsafe` blocks doing `Box::into_raw`, `cast()` and, at the
// reported site:
//
//     validate_type_tag(env, napi_val, <Terminal as TypeTag>::type_tag(), "Terminal")?;
//     register_native_borrow_with_value(env, napi_val, wrapped_val.cast::<Terminal>(), false)?;
//     Ok(&*(wrapped_val as *const Terminal))
//
// The dereference is two lines below the runtime asserting that the pointer the
// JS engine handed back is in fact a `Terminal`. CodeQL cannot follow that
// invariant across the FFI boundary, so it sees a bare deref of a pointer of
// unknown provenance.
//
// Dismissed as a false positive rather than excluded by a CodeQL config. The
// wickra library excludes its whole node binding for this rule, but that file is
// 22,280 lines and produces one finding per exported class; this one produces
// exactly one, so dismissing the single alert keeps the rule live for anything
// genuinely new.
#[napi]
#[derive(Debug)]
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
