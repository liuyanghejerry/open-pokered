use std::io;

use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use crossterm::ExecutableCommand;

/// RAII guard that puts the terminal into raw mode with an alternate screen
/// on construction, and restores normal mode on drop.
///
/// # Panics
///
/// The [`Drop`] implementation ignores errors during cleanup (they would
/// be unrecoverable anyway).
pub struct TerminalGuard;

impl TerminalGuard {
    /// Enter raw mode and switch to the alternate screen.
    pub fn new() -> io::Result<Self> {
        enable_raw_mode()?;
        io::stdout().execute(EnterAlternateScreen)?;
        Ok(Self)
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        // Ignore errors during cleanup — there's nothing we can do about them.
        let _ = io::stdout().execute(LeaveAlternateScreen);
        let _ = disable_raw_mode();
    }
}
