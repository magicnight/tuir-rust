//! Page trait and implementations

pub mod inbox;
pub mod message;
pub mod submission;
pub mod subreddit;
pub mod subscription;

use crossterm::event::KeyEvent;
use ratatui::Frame;
use std::future::Future;

/// Page action returned by key handlers
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PageAction {
    /// Continue rendering this page
    None,
    /// Navigate back to previous page
    Back,
    /// Switch to a different page
    Switch(PageKind),
    /// Exit the application
    Quit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PageKind {
    Subreddit,
    Submission,
    Message,
    Inbox,
    Subscription,
    Help,
}

/// Trait for all application pages
pub trait Page {
    /// Render the page onto the frame
    fn render(&mut self, frame: &mut Frame);

    /// Handle a key press, returning the resulting action
    fn handle_key(&mut self, key: KeyEvent) -> PageAction;

    /// Title shown in the status bar
    fn title(&self) -> &str;
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
