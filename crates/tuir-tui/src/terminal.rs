//! Terminal setup and teardown

use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use std::io;

/// Terminal wrapper type
pub type TerminalType = Terminal<CrosstermBackend<io::Stderr>>;

/// Initialize the terminal with alternate screen and raw mode
pub fn init() -> anyhow::Result<TerminalType> {
    crossterm::terminal::enable_raw_mode()?;

    let mut stderr = io::stderr();
    crossterm::execute!(
        stderr,
        crossterm::terminal::EnterAlternateScreen,
        crossterm::cursor::Hide,
    )?;

    let backend = CrosstermBackend::new(stderr);
    let terminal = Terminal::new(backend)?;

    Ok(terminal)
}

/// Restore the terminal to its original state
pub fn restore() -> anyhow::Result<()> {
    crossterm::execute!(
        io::stderr(),
        crossterm::cursor::Show,
        crossterm::terminal::LeaveAlternateScreen,
    )?;

    crossterm::terminal::disable_raw_mode()?;

    Ok(())
}
