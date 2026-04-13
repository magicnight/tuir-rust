//! Subscription / multi-reddit listing page

use super::{Page, PageAction};
use crossterm::event::KeyEvent;
use ratatui::Frame;

pub struct SubscriptionPage;

impl SubscriptionPage {
    pub fn new() -> Self {
        Self
    }
}

impl Default for SubscriptionPage {
    fn default() -> Self {
        Self::new()
    }
}

impl Page for SubscriptionPage {
    fn render(&mut self, frame: &mut Frame) {
        let area = frame.area();
        frame.render_widget(
            ratatui::widgets::Paragraph::new("Subscriptions — TODO: implement listing"),
            area,
        );
    }

    fn handle_key(&mut self, _key: KeyEvent) -> PageAction {
        PageAction::None
    }

    fn title(&self) -> &str {
        "subscriptions"
    }
}
