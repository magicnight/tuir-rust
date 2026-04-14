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
    if let Err(err) = crossterm::execute!(
        stderr,
        crossterm::terminal::EnterAlternateScreen,
        crossterm::cursor::Hide,
    ) {
        let _ = crossterm::terminal::disable_raw_mode();
        return Err(err.into());
    }

    let backend = CrosstermBackend::new(stderr);
    let terminal = match Terminal::new(backend) {
        Ok(terminal) => terminal,
        Err(err) => {
            let _ = crossterm::execute!(
                io::stderr(),
                crossterm::cursor::Show,
                crossterm::terminal::LeaveAlternateScreen,
            );
            let _ = crossterm::terminal::disable_raw_mode();
            return Err(err.into());
        }
    };

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

/// Temporarily yield the terminal so an external program (image
/// viewer, video player, browser…) can write to it without the TUI
/// fighting over raw mode and the alternate screen.
///
/// Pair with [`resume`] in a balanced way:
///
/// ```ignore
/// suspend(&mut terminal)?;
/// std::process::Command::new("mpv").arg(url).status()?;
/// resume(&mut terminal)?;
/// ```
pub fn suspend(terminal: &mut TerminalType) -> anyhow::Result<()> {
    // Make sure ratatui flushes any pending diff before we drop the
    // alternate screen — otherwise the user briefly sees stale frame
    // residue underneath the spawned program.
    terminal.show_cursor().ok();
    crossterm::execute!(
        io::stderr(),
        crossterm::cursor::Show,
        crossterm::terminal::LeaveAlternateScreen,
    )?;
    crossterm::terminal::disable_raw_mode()?;
    Ok(())
}

/// Counterpart to [`suspend`]. Re-enters the alternate screen and
/// re-arms raw mode, then forces a full redraw so the next render
/// pass repaints from a clean slate instead of trying to diff against
/// whatever the spawned program left behind.
pub fn resume(terminal: &mut TerminalType) -> anyhow::Result<()> {
    crossterm::terminal::enable_raw_mode()?;
    crossterm::execute!(
        io::stderr(),
        crossterm::terminal::EnterAlternateScreen,
        crossterm::cursor::Hide,
    )?;
    terminal.clear()?;
    Ok(())
}
