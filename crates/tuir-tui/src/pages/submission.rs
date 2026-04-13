//! Submission detail + comment tree page

use super::{Page, PageAction};
use crossterm::event::KeyEvent;
use ratatui::Frame;

pub struct SubmissionPage {
    pub id: String,
}

impl SubmissionPage {
    pub fn new(id: &str) -> Self {
        Self { id: id.to_string() }
    }
}

impl Page for SubmissionPage {
    fn render(&mut self, frame: &mut Frame) {
        let area = frame.area();
        frame.render_widget(
            ratatui::widgets::Paragraph::new(format!(
                "Submission: {} — TODO: implement comments",
                self.id
            )),
            area,
        );
    }

    fn handle_key(&mut self, _key: KeyEvent) -> PageAction {
        PageAction::None
    }

    fn title(&self) -> &str {
        &self.id
    }
}
