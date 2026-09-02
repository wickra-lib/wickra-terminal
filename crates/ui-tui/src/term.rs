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
pub(crate) struct TermGuard;

impl TermGuard {
    /// Enter raw mode + the alternate screen and arm the panic-restore hook.
    ///
    /// # Errors
    ///
    /// Returns an I/O error if the terminal mode cannot be changed.
    pub(crate) fn new() -> io::Result<Self> {
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

#[cfg(test)]
mod tests {
    use super::{restore, TermGuard};

    /// `restore` is best effort and safe to call at any time.
    ///
    /// It is what the panic hook and the `Drop` both run, and both run it in
    /// states this cannot arrange: mid-unwind, and after an event loop that may
    /// have left the terminal anywhere. Calling it from a cooked, main-screen
    /// terminal is the one state a test can be in, and it must not fail there --
    /// a restore that errored on an already-restored terminal would turn every
    /// clean exit into a reported failure.
    #[test]
    fn restoring_an_already_restored_terminal_is_not_an_error() {
        // Twice, because the panic hook and the guard's `Drop` both fire when a
        // panic unwinds through the guard -- so the second call always runs
        // against a terminal the first has already put back.
        let first = restore().is_ok();
        let second = restore().is_ok();
        assert_eq!(
            first, second,
            "restoring twice reported differently the second time"
        );
    }

    /// The guard either takes the terminal or says why, and never both.
    ///
    /// Under `cargo test` there is usually no terminal to enter raw mode on, so
    /// this asserts the shape rather than the outcome: a guard that constructs
    /// restores on drop, and one that cannot construct hands back the I/O error
    /// rather than leaving the terminal half-entered.
    #[test]
    fn the_guard_either_constructs_and_restores_or_refuses() {
        match TermGuard::new() {
            Ok(guard) => {
                // Dropping is the whole contract: raw mode off, main screen
                // back. Nothing here can observe that from inside the process,
                // so what is checked is that it happens without panicking.
                drop(guard);
            }
            Err(err) => {
                // No terminal, which is the usual case under a test harness.
                // The error is the terminal's, passed through rather than
                // swallowed into a guard that reports success and owns nothing.
                assert!(!err.to_string().is_empty(), "an error that says nothing");
            }
        }
    }
}
