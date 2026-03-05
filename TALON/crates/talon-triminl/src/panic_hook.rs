//! Terminal panic hook — restores terminal state before printing panic info.
//!
//! Must be called before entering raw mode. Without this, a panic inside
//! the TUI event loop leaves the terminal in raw mode with the alternate
//! screen still active, making the shell unusable.

use std::io::stdout;

use crossterm::cursor::Show;
use crossterm::execute;
use crossterm::terminal::{LeaveAlternateScreen, disable_raw_mode};

/// Install a panic hook that restores the terminal before delegating
/// to the original hook. Call once at startup, before entering raw mode.
pub fn install_panic_hook() {
    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        // Best-effort terminal restoration — ignore errors since we are
        // already panicking and cannot meaningfully handle them.
        let _ = disable_raw_mode();
        let _ = execute!(stdout(), LeaveAlternateScreen, Show);
        original_hook(panic_info);
    }));
}
