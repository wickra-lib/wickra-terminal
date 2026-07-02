//! Python bindings for `wickra-terminal`, exposed under the `wickra_terminal`
//! package.
//!
//! Thin glue over the terminal core's data-driven surface: build a [`Terminal`]
//! from a JSON config, drive it with a command JSON and read back the frame
//! JSON. The same command protocol crosses every binding, so a Python front-end
//! drives the exact same core as the native TUI.

// PyO3 protocol methods take `self` by value/ref regardless of use.
#![allow(clippy::needless_pass_by_value)]

use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;

use terminal_core::Terminal;

/// A trading terminal instance driven by JSON commands.
///
/// `unsendable`: the terminal owns non-`Send` feed sources (trait objects, and a
/// live exchange client), so a handle is bound to the thread that created it.
#[pyclass(name = "Terminal", unsendable)]
struct PyTerminal {
    inner: Terminal,
}

#[pymethods]
impl PyTerminal {
    /// Build a terminal from a JSON config string.
    #[new]
    fn new(config_json: &str) -> PyResult<Self> {
        Terminal::from_json(config_json)
            .map(|inner| Self { inner })
            .map_err(|err| PyValueError::new_err(err.to_string()))
    }

    /// Apply a command JSON and return the resulting frame JSON.
    fn command(&mut self, cmd_json: &str) -> PyResult<String> {
        self.inner
            .command_json(cmd_json)
            .map_err(|err| PyValueError::new_err(err.to_string()))
    }

    /// The library version.
    #[staticmethod]
    fn version() -> &'static str {
        Terminal::version()
    }
}

/// The native module (`wickra_terminal._wickra_terminal`).
#[pymodule]
fn _wickra_terminal(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add("__version__", env!("CARGO_PKG_VERSION"))?;
    module.add_class::<PyTerminal>()?;
    Ok(())
}
