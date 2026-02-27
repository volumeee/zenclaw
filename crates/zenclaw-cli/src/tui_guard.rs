//! RAII guard for terminal setup/teardown.
//!
//! Ensures `disable_raw_mode` and `LeaveAlternateScreen` always run,
//! even on panic. Use this instead of manual enable/disable pairs.

use ratatui::{
    backend::CrosstermBackend,
    crossterm::{
        execute,
        event::{EnableMouseCapture, DisableMouseCapture},
        terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    },
    Terminal,
};
use std::io::{self, Stdout};

/// RAII terminal guard.
///
/// # Example
/// ```rust,ignore
/// let guard = TuiGuard::new()?;
/// let terminal = &mut guard.terminal;
/// // ... draw, event loop ...
/// // cleanup happens automatically when `guard` is dropped
/// ```
pub struct TuiGuard {
    pub terminal: Terminal<CrosstermBackend<Stdout>>,
}

impl TuiGuard {
    /// Enter raw mode + alternate screen, return a ready terminal.
    pub fn new() -> io::Result<Self> {
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
        let backend = CrosstermBackend::new(stdout);
        let terminal = Terminal::new(backend)?;
        Ok(Self { terminal })
    }
}

impl Drop for TuiGuard {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(self.terminal.backend_mut(), LeaveAlternateScreen, DisableMouseCapture);
        let _ = self.terminal.show_cursor();
    }
}
