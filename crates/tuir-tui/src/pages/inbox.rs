//! Inbox page (messages and replies)

use super::{Page, PageAction};
use crossterm::event::KeyEvent;
use ratatui::Frame;

pub struct InboxPage;

impl InboxPage {
    pub fn new() -> Self {
        Self
    }
}

impl Default for InboxPage {
    fn default() -> Self {
        Self::new()
    }
}

impl Page for InboxPage {
    fn render(&mut self, frame: &mut Frame) {
        let area = frame.area();
        frame.render_widget(
            ratatui::widgets::Paragraph::new("Inbox — TODO: implement messages"),
            area,
        );
    }

    fn handle_key(&mut self, _key: KeyEvent) -> PageAction {
        PageAction::None
    }

    fn title(&self) -> &str {
        "inbox"
    }
}
