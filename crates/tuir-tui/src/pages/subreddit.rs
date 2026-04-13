//! Subreddit listing page

use super::{Page, PageAction};
use crossterm::event::KeyEvent;
use ratatui::Frame;

pub struct SubredditPage {
    pub name: String,
}

impl SubredditPage {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
        }
    }
}

impl Page for SubredditPage {
    fn render(&mut self, frame: &mut Frame) {
        // TODO: render subreddit listing
        let area = frame.area();
        frame.render_widget(
            ratatui::widgets::Paragraph::new(format!(
                "Subreddit: r/{} — TODO: implement listing",
                self.name
            )),
            area,
        );
    }

    fn handle_key(&mut self, key: KeyEvent) -> PageAction {
        use crate::keymap::KeyAction;
        if let Some(action) = KeyAction::from_key(key.code, key.modifiers) {
            match action {
                KeyAction::Quit => PageAction::Quit,
                _ => PageAction::None,
            }
        } else {
            PageAction::None
        }
    }

    fn title(&self) -> &str {
        &self.name
    }
}
