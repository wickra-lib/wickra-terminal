//! The crate error type.
//!
//! Everything fallible in the core — parsing a config, decoding a command,
//! building a source — funnels into [`Error`], so the data-driven FFI boundary
//! (`Terminal::command_json`) can turn any failure into a typed message instead
//! of a panic across the C ABI.

use thiserror::Error;

/// An error raised by the terminal core.
#[derive(Debug, Error)]
pub enum Error {
    /// The config could not be parsed (TOML or JSON).
    #[error("invalid config: {0}")]
    Config(String),

    /// A command JSON string could not be decoded into a known command.
    #[error("invalid command: {0}")]
    Command(String),

    /// A source spec referred to something that could not be built (e.g. an
    /// unknown venue, or a replay dataset that failed to load).
    #[error("invalid source: {0}")]
    Source(String),

    /// No source is registered under the given id.
    #[error("unknown source id: {0}")]
    UnknownSource(u32),

    /// A JSON (de)serialization error on the command/frame boundary.
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),

    /// An error surfaced by the underlying exchange connectivity layer.
    #[error("exchange error: {0}")]
    Exchange(String),
}

/// The crate result alias.
pub type Result<T> = core::result::Result<T, Error>;
