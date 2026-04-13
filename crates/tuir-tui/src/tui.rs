//! TUI utilities

pub use crossterm::event::{Event, KeyEvent, KeyEventKind};

use std::time::Duration;

/// Poll for events with timeout
pub fn poll_event(timeout: Duration) -> Option<Event> {
    crossterm::event::poll(timeout).ok().and_then(|ready| {
        if ready {
            crossterm::event::read().ok()
        } else {
            None
        }
    })
}
