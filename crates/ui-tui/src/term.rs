//! Terminal lifecycle guard.
//!
//! [`TermGuard`] owns the raw-mode + alternate-screen state with RAII: it enters
//! on construction and restores on `Drop`, and installs a panic hook that also
//! restores — so a panic never leaves the user's terminal in raw mode with a
//! hidden cursor. This is the one piece the renderer must get right regardless of
//! how the event loop exits.

use std::io::{self, Stdout};

use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};

/// Restores the terminal to a cooked, main-screen state on drop or panic.
pub struct TermGuard;

impl TermGuard {
    /// Enter raw mode + the alternate screen and arm the panic-restore hook.
    ///
    /// # Errors
    ///
    /// Returns an I/O error if the terminal mode cannot be changed.
    pub fn new() -> io::Result<Self> {
        enable_raw_mode()?;
        execute!(io::stdout(), EnterAlternateScreen)?;

        let previous = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            let _ = restore();
            previous(info);
        }));
        Ok(Self)
    }
}

impl Drop for TermGuard {
    fn drop(&mut self) {
        let _ = restore();
    }
}

/// Leave the alternate screen and disable raw mode (best effort).
fn restore() -> io::Result<()> {
    let mut out: Stdout = io::stdout();
    execute!(out, LeaveAlternateScreen)?;
    disable_raw_mode()
}
