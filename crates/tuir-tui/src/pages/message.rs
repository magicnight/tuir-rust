//! Inbox message detail page

use crate::keymap::KeyAction;
use crate::pages::PageAction;
use crossterm::event::{KeyEvent, KeyEventKind};
use ratatui::{
    layout::{Constraint, Direction, Layout},
    prelude::*,
    text::Line,
    widgets::{Block, BorderType, Borders, Paragraph, Wrap},
    Frame,
};
use tuir_core::content::render_plain_string;
use tuir_core::reddit::models::Message;

pub struct MessagePage {
    pub message: Message,
}

impl MessagePage {
    pub fn new(message: Message) -> Self {
        Self { message }
    }

    fn format_timestamp(created_utc: f64) -> String {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as f64;
        let diff = now - created_utc;

        if diff < 60.0 {
            format!("{:.0}s ago", diff)
        } else if diff < 3600.0 {
            format!("{:.0}m ago", diff / 60.0)
        } else if diff < 86400.0 {
            format!("{:.0}h ago", diff / 3600.0)
        } else if diff < 2592000.0 {
            format!("{:.0}d ago", diff / 86400.0)
        } else {
            format!("{:.0}mo ago", diff / 2592000.0)
        }
    }
}

impl crate::pages::Page for MessagePage {
    fn render(&mut self, frame: &mut Frame) {
        let area = frame.area();
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1),
                Constraint::Length(4),
                Constraint::Min(1),
                Constraint::Length(1),
            ])
            .split(area);

        let header = Block::default()
            .title(format!(
                " MESSAGE • from {} • {} ",
                self.message.author,
                Self::format_timestamp(self.message.created_utc)
            ))
            .borders(Borders::ALL)
            .border_type(BorderType::Plain)
            .style(Style::default().bg(Color::Rgb(20, 20, 30)));
        frame.render_widget(header, chunks[0]);

        let meta_block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Plain)
            .style(Style::default().bg(Color::Rgb(30, 30, 30)));
        let meta_inner = meta_block.inner(chunks[1]);
        frame.render_widget(meta_block, chunks[1]);
        let target = self
            .message
            .subreddit
            .as_deref()
            .map(|sub| format!("r/{sub}"))
            .unwrap_or_else(|| self.message.dest.clone());
        let meta = Paragraph::new(vec![
            Line::from(format!(" Subject: {}", self.message.subject)),
            Line::from(format!(" To: {target}")),
            Line::from(format!(" Kind: {}", if self.message.was_comment { "comment reply" } else { "message" })),
        ]);
        frame.render_widget(meta, meta_inner);

        let body_block = Block::default()
            .title(" Body ")
            .borders(Borders::ALL)
            .border_type(BorderType::Plain)
            .style(Style::default().bg(Color::Rgb(25, 25, 35)));
        let body_inner = body_block.inner(chunks[2]);
        frame.render_widget(body_block, chunks[2]);
        let body_text = self
            .message
            .body_html
            .as_deref()
            .map(render_plain_string)
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| self.message.body.clone());
        let body = Paragraph::new(body_text).wrap(Wrap { trim: true });
        frame.render_widget(body, body_inner);

        let footer = Block::default()
            .title(" Enter:o/Open thread | Esc:Back | q:Quit ")
            .borders(Borders::ALL)
            .border_type(BorderType::Plain)
            .style(Style::default().bg(Color::Rgb(30, 30, 20)));
        frame.render_widget(footer, chunks[3]);
    }

    fn handle_key(&mut self, key: KeyEvent) -> PageAction {
        if key.kind != KeyEventKind::Press {
            return PageAction::None;
        }

        if let Some(action) = KeyAction::from_key(key.code, key.modifiers) {
            return match action {
                KeyAction::Open if self.message.reply_to.is_some() => {
                    PageAction::Switch(crate::pages::PageKind::Submission)
                }
                KeyAction::Back => PageAction::Back,
                KeyAction::Quit => PageAction::Quit,
                _ => PageAction::None,
            };
        }

        PageAction::None
    }

    fn title(&self) -> &str {
        &self.message.subject
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pages::{Page, PageAction, PageKind};
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    fn sample_message() -> Message {
        Message {
            id: "mock_msg_1".to_string(),
            name: "t4_mock_msg_1".to_string(),
            subject: "Inbox subject".to_string(),
            author: "reddit".to_string(),
            body: "Hello".to_string(),
            body_html: None,
            created_utc: 1_700_000_000.0,
            dest: "mock_user".to_string(),
            new: true,
            reply_to: Some("t3_rust_1".to_string()),
            subreddit: Some("rust".to_string()),
            author_flair_text: None,
            was_comment: true,
        }
    }

    #[test]
    fn open_returns_submission_switch_when_reply_target_exists() {
        let mut page = MessagePage::new(sample_message());

        let action = page.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::empty()));

        assert_eq!(action, PageAction::Switch(PageKind::Submission));
    }
}
