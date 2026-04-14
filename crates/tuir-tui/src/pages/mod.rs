//! Page trait and implementations

pub mod help;
pub mod inbox;
pub mod media;
pub mod message;
pub mod submission;
pub mod subreddit;
pub mod subscription;

use crossterm::event::KeyEvent;
use ratatui::Frame;
use std::future::Future;

/// Page action returned by key handlers
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PageAction {
    /// Continue rendering this page
    None,
    /// Navigate back to previous page
    Back,
    /// Switch to a different page
    Switch(PageKind),
    /// Exit the application
    Quit,
    /// Suspend the TUI, hand the terminal off to an external program
    /// resolved via mailcap for the given URL, then resume. The CLI
    /// event loop is responsible for the mailcap lookup and the
    /// suspend/spawn/resume dance — pages just signal intent.
    OpenExternal(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PageKind {
    Subreddit,
    Submission,
    Message,
    Inbox,
    Subscription,
    Help,
    Media,
}

/// Trait for all application pages
pub trait Page {
    /// Render the page onto the frame
    fn render(&mut self, frame: &mut Frame);

    /// Handle a key press, returning the resulting action
    fn handle_key(&mut self, key: KeyEvent) -> PageAction;

    /// Title shown in the status bar
    fn title(&self) -> &str;

    /// Time-based update hook called once per event-loop iteration,
    /// regardless of whether a key event fired. Default is a no-op;
    /// pages that drive animations or background work override this.
    fn tick(&mut self) {}
}

pub fn block_on<F>(future: F) -> F::Output
where
    F: Future,
{
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("page runtime should build")
        .block_on(future)
}
