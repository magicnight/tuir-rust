//! Inbox page (messages and replies)

use crate::keymap::KeyAction;
use crate::PageAction;
use crossterm::event::{KeyEvent, KeyEventKind};
use ratatui::{
    layout::{Constraint, Direction, Layout},
    prelude::*,
    text::Line,
    widgets::{Block, BorderType, Borders, List, ListItem, ListState, Paragraph},
    Frame,
};
use std::sync::Arc;
use tuir_core::reddit::models::Message;
use tuir_core::reddit::{MockRedditClient, RedditApi};

/// Inbox page state
pub struct InboxPage {
    pub messages: Vec<Message>,
    pub list_state: ListState,
    pub loading: bool,
    pub client: Arc<dyn RedditApi>,
}

impl InboxPage {
    /// Create a new inbox page
    pub fn new() -> Self {
        Self::with_client(Arc::new(MockRedditClient::new()))
    }

    pub fn with_client(client: Arc<dyn RedditApi>) -> Self {
        Self {
            messages: Vec::new(),
            list_state: ListState::default(),
            loading: false,
            client,
        }
    }

    /// Load messages from inbox
    pub async fn load(&mut self) {
        self.loading = true;

        let listing = match self.client.inbox().await {
            Ok(listing) => listing,
            Err(err) => {
                tracing::error!("failed to load inbox: {err}");
                self.loading = false;
                return;
            }
        };
        self.messages = listing.data.children.into_iter().map(|c| c.data).collect();

        if !self.messages.is_empty() {
            self.list_state.select(Some(0));
        }

        self.loading = false;
    }

    pub fn load_sync(&mut self) {
        crate::pages::block_on(self.load());
    }

    pub fn refresh(&mut self) {
        self.load_sync();
    }

    fn open_selected_message(&mut self) -> PageAction {
        let Some(idx) = self.list_state.selected() else {
            return PageAction::None;
        };
        let Some(message) = self.messages.get_mut(idx) else {
            return PageAction::None;
        };

        message.new = false;
        let name = message.name.clone();
        let client = Arc::clone(&self.client);
        crate::pages::block_on(async move {
            if let Err(err) = client.mark_read(&name).await {
                tracing::error!("mark_read failed: {err}");
            }
        });

        PageAction::Switch(crate::pages::PageKind::Message)
    }

    fn toggle_selected_read_state(&mut self) {
        let Some(idx) = self.list_state.selected() else {
            return;
        };
        let Some(message) = self.messages.get_mut(idx) else {
            return;
        };

        message.new = !message.new;
        let name = message.name.clone();
        let mark_unread = message.new;
        let client = Arc::clone(&self.client);
        crate::pages::block_on(async move {
            let result = if mark_unread {
                client.mark_unread(&name).await
            } else {
                client.mark_read(&name).await
            };
            if let Err(err) = result {
                tracing::error!("toggle read-state failed: {err}");
            }
        });
    }

    /// Move cursor up
    pub fn move_up(&mut self) {
        if let Some(idx) = self.list_state.selected() {
            if idx > 0 {
                self.list_state.select(Some(idx - 1));
            }
        }
    }

    /// Move cursor down
    pub fn move_down(&mut self) {
        if let Some(idx) = self.list_state.selected() {
            if idx < self.messages.len().saturating_sub(1) {
                self.list_state.select(Some(idx + 1));
            }
        }
    }

    /// Move to top
    pub fn move_to_top(&mut self) {
        if !self.messages.is_empty() {
            self.list_state.select(Some(0));
        }
    }

    /// Move to bottom
    pub fn move_to_bottom(&mut self) {
        if !self.messages.is_empty() {
            self.list_state.select(Some(self.messages.len() - 1));
        }
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

    /// Format a single message for display
    fn format_message(msg: &Message) -> Line<'_> {
        let unread_marker = if msg.new { "● " } else { "  " };
        let subject = if msg.subject.len() > 50 {
            format!("{}...", &msg.subject[..47])
        } else {
            msg.subject.clone()
        };

        let body_preview = if msg.body.len() > 60 {
            format!("{}...", &msg.body[..57])
        } else {
            msg.body.clone()
        };

        let time = Self::format_timestamp(msg.created_utc);

        // Build spans with owned strings
        let mut spans: Vec<Span<'_>> = Vec::new();

        spans.push(Span::raw(unread_marker));
        spans.push(Span::styled(
            msg.author.as_str(),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ));
        spans.push(Span::raw(" • "));

        // Subject (owned)
        let subject_style = if msg.new {
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::White)
        };
        spans.push(Span::styled(subject, subject_style));

        spans.push(Span::raw("\n    "));

        // Body preview (owned)
        spans.push(Span::styled(
            body_preview,
            Style::default().fg(Color::DarkGray),
        ));

        spans.push(Span::raw(format!(" • {}", time)));

        Line::from(spans)
    }
}

impl Default for InboxPage {
    fn default() -> Self {
        Self::new()
    }
}

impl crate::pages::Page for InboxPage {
    fn render(&mut self, frame: &mut Frame) {
        let area = frame.area();

        // Layout: header, content, footer
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1), // Header
                Constraint::Min(1),    // Content
                Constraint::Length(1), // Footer
            ])
            .split(area);

        // ── Header ──────────────────────────────────────────────
        let header_text = format!(" INBOX • {} messages ", self.messages.len());

        let header = Block::default()
            .title(header_text)
            .borders(Borders::ALL)
            .border_type(BorderType::Plain)
            .style(Style::default().bg(Color::Rgb(20, 20, 30)));

        frame.render_widget(header, chunks[0]);

        // ── Content ──────────────────────────────────────────────
        let content_block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Plain);

        let inner = content_block.inner(chunks[1]);
        frame.render_widget(content_block, chunks[1]);

        if self.loading {
            let para = Paragraph::new("Loading inbox...").style(Style::default().fg(Color::Gray));
            frame.render_widget(para, inner);
        } else if self.messages.is_empty() {
            let para =
                Paragraph::new("No messages in inbox.").style(Style::default().fg(Color::DarkGray));
            frame.render_widget(para, inner);
        } else {
            let items: Vec<ListItem> = self
                .messages
                .iter()
                .map(|msg| ListItem::new(Self::format_message(msg)))
                .collect();

            let list = List::new(items).block(Block::default()).highlight_style(
                Style::default()
                    .bg(Color::Rgb(40, 40, 40))
                    .add_modifier(Modifier::BOLD),
            );

            frame.render_stateful_widget(list, inner, &mut self.list_state);
        }

        // ── Footer ───────────────────────────────────────────────
        let footer_text = " j/k:Navigate | Enter:Open | u:Toggle unread | q:Back | r:Refresh ";
        let footer = Block::default()
            .title(footer_text)
            .borders(Borders::ALL)
            .border_type(BorderType::Plain)
            .style(Style::default().bg(Color::Rgb(30, 30, 20)));

        frame.render_widget(footer, chunks[2]);
    }

    fn handle_key(&mut self, key: KeyEvent) -> PageAction {
        use crate::pages::PageAction;

        if key.kind != KeyEventKind::Press {
            return PageAction::None;
        }

        if key.code == crossterm::event::KeyCode::Char('u') {
            self.toggle_selected_read_state();
            return PageAction::None;
        }

        if let Some(action) = KeyAction::from_key(key.code, key.modifiers) {
            match action {
                KeyAction::Quit => return PageAction::Quit,
                KeyAction::Back => return PageAction::Back,
                KeyAction::Refresh => self.refresh(),
                KeyAction::Open => return self.open_selected_message(),
                KeyAction::NextItem => self.move_down(),
                KeyAction::PrevItem => self.move_up(),
                KeyAction::Top => self.move_to_top(),
                KeyAction::Bottom => self.move_to_bottom(),
                _ => {}
            }
        }

        PageAction::None
    }

    fn title(&self) -> &str {
        "inbox"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pages::{Page, PageAction, PageKind};
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use tuir_core::reddit::models::Message;

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
    fn open_returns_submission_switch() {
        let mut page = InboxPage::new();
        page.messages = vec![sample_message()];
        page.list_state.select(Some(0));

        let action = page.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::empty()));

        assert_eq!(action, PageAction::Switch(PageKind::Message));
    }

    #[test]
    fn open_marks_message_as_read() {
        let mut page = InboxPage::new();
        page.messages = vec![sample_message()];
        page.list_state.select(Some(0));

        let _ = page.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::empty()));

        assert!(!page.messages[0].new);
    }

    #[test]
    fn toggle_unread_marks_message_unread() {
        let mut page = InboxPage::new();
        let mut message = sample_message();
        message.new = false;
        page.messages = vec![message];
        page.list_state.select(Some(0));

        let _ = page.handle_key(KeyEvent::new(KeyCode::Char('u'), KeyModifiers::empty()));

        assert!(page.messages[0].new);
    }
}
