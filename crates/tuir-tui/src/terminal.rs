//! Terminal setup and teardown
//!
//! Handles crossterm initialization, alternate screen, and suspend/resume.

use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use std::io::{self, BufWriter};

/// Terminal wrapper type
pub type RatatuiTerminal = Terminal<CrosstermBackend<BufWriter<io::Stdout>>>;

/// Initialize the terminal with alternate screen and raw mode
pub fn init() -> io::Result<RatatuiTerminal> {
    crossterm::terminal::enable_raw_mode()?;
    let mut stdout = BufWriter::new(io::stdout());
    crossterm::execute!(
        stdout,
        crossterm::terminal::EnterAlternateScreen,
        crossterm::cursor::Hide,
    )?;
    let backend = CrosstermBackend::new(stdout);
    let terminal = Terminal::new(backend)?;
    Ok(terminal)
}

/// Restore the terminal to its original state
pub fn restore() -> io::Result<()> {
    crossterm::execute!(
        io::stdout(),
        crossterm::cursor::Show,
        crossterm::terminal::LeaveAlternateScreen,
    )?;
    crossterm::terminal::disable_raw_mode()
}

/// Suspend the terminal (for external viewers like feh)
pub fn suspend() -> io::Result<()> {
    restore()
}

/// Resume the terminal after suspend
pub fn resume() -> io::Result<RatatuiTerminal> {
    init()
}
