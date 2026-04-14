//! Subscription / multi-reddit listing page

use crate::keymap::KeyAction;
use crate::theme::AppTheme;
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
use tuir_core::reddit::models::Subreddit;
use tuir_core::reddit::{MockRedditClient, RedditApi};

/// Subscription page state
pub struct SubscriptionPage {
    pub subreddits: Vec<Subreddit>,
    pub list_state: ListState,
    pub loading: bool,
    pub client: Arc<dyn RedditApi>,
    pub theme: Arc<AppTheme>,
}

impl SubscriptionPage {
    /// Create a new subscription page
    pub fn new() -> Self {
        Self::with_client(Arc::new(MockRedditClient::new()))
    }

    pub fn with_client(client: Arc<dyn RedditApi>) -> Self {
        Self {
            subreddits: Vec::new(),
            list_state: ListState::default(),
            loading: false,
            client,
            theme: Arc::new(AppTheme::default()),
        }
    }

    pub fn set_theme(&mut self, theme: Arc<AppTheme>) {
        self.theme = theme;
    }

    /// Load subscribed subreddits
    pub async fn load(&mut self) {
        self.loading = true;

        let listing = match self.client.subscribed(50).await {
            Ok(listing) => listing,
            Err(err) => {
                tracing::error!("failed to load subscriptions: {err}");
                self.loading = false;
                return;
            }
        };
        self.subreddits = listing.data.children.into_iter().map(|c| c.data).collect();

        if !self.subreddits.is_empty() {
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
            if idx < self.subreddits.len().saturating_sub(1) {
                self.list_state.select(Some(idx + 1));
            }
        }
    }

    /// Move to top
    pub fn move_to_top(&mut self) {
        if !self.subreddits.is_empty() {
            self.list_state.select(Some(0));
        }
    }

    /// Move to bottom
    pub fn move_to_bottom(&mut self) {
        if !self.subreddits.is_empty() {
            self.list_state.select(Some(self.subreddits.len() - 1));
        }
    }

    /// Format subscriber count
    fn format_subscribers(count: i64) -> String {
        if count >= 1_000_000 {
            format!("{:.1}M", count as f64 / 1_000_000.0)
        } else if count >= 1_000 {
            format!("{:.1}K", count as f64 / 1_000.0)
        } else {
            format!("{}", count)
        }
    }

    /// Format a single subreddit for display
    fn format_subreddit<'a>(sub: &'a Subreddit, idx: usize, theme: &AppTheme) -> Line<'a> {
        let icon = if sub.over18 { "🔞 " } else { "📚 " };
        let subscriber_str = Self::format_subscribers(sub.subscribers);
        let active_str = sub.active_user_count.map(|c| {
            if c >= 1_000 {
                format!("{:.1}K", c as f64 / 1_000.0)
            } else {
                format!("{}", c)
            }
        });

        Line::from(vec![
            Span::raw(format!("{:<3} ", idx + 1)),
            Span::raw(icon),
            Span::styled(sub.display_name.clone(), theme.author),
            Span::raw(format!(" • {} subscribers", subscriber_str)),
            if let Some(active) = active_str {
                Span::raw(format!(" • {} active", active))
            } else {
                Span::raw("")
            },
            if sub.user_is_subscriber.unwrap_or(false) {
                Span::styled(" ★", theme.stickied)
            } else {
                Span::raw("")
            },
            Span::raw(format!(
                "\n    {}",
                if sub.public_description.len() > 70 {
                    format!("{}...", &sub.public_description[..67])
                } else {
                    sub.public_description.clone()
                }
            )),
        ])
    }
}

impl Default for SubscriptionPage {
    fn default() -> Self {
        Self::new()
    }
}

impl crate::pages::Page for SubscriptionPage {
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
        let subscribed_count = self
            .subreddits
            .iter()
            .filter(|s| s.user_is_subscriber.unwrap_or(false))
            .count();

        let header_text = format!(
            " SUBSCRIPTIONS • {} total • {} subscribed ",
            self.subreddits.len(),
            subscribed_count
        );

        let header = Block::default()
            .title(header_text)
            .borders(Borders::ALL)
            .border_type(BorderType::Plain)
            .style(self.theme.header);

        frame.render_widget(header, chunks[0]);

        // ── Content ──────────────────────────────────────────────
        let content_block = Block::default()
            .title(" Subreddits ")
            .borders(Borders::ALL)
            .border_type(BorderType::Plain);

        let inner = content_block.inner(chunks[1]);
        frame.render_widget(content_block, chunks[1]);

        if self.loading {
            let para = Paragraph::new("Loading subscriptions...").style(self.theme.muted);
            frame.render_widget(para, inner);
        } else if self.subreddits.is_empty() {
            let para =
                Paragraph::new("No subscriptions found.\nUse ' subreddits --sync' to update.")
                    .style(self.theme.muted);
            frame.render_widget(para, inner);
        } else {
            let items: Vec<ListItem> = {
                let theme = &*self.theme;
                self.subreddits
                    .iter()
                    .enumerate()
                    .map(|(i, sub)| ListItem::new(Self::format_subreddit(sub, i, theme)))
                    .collect()
            };

            let list = List::new(items)
                .block(Block::default())
                .highlight_style(self.theme.selected);

            frame.render_stateful_widget(list, inner, &mut self.list_state);
        }

        // ── Footer ───────────────────────────────────────────────
        let footer_text = " j/k:Navigate | Enter:Open r/sub | q:Back | r:Refresh ";
        let footer = Block::default()
            .title(footer_text)
            .borders(Borders::ALL)
            .border_type(BorderType::Plain)
            .style(self.theme.footer);

        frame.render_widget(footer, chunks[2]);
    }

    fn handle_key(&mut self, key: KeyEvent) -> PageAction {
        use crate::pages::PageAction;

        if key.kind != KeyEventKind::Press {
            return PageAction::None;
        }

        if let Some(action) = KeyAction::from_key(key.code, key.modifiers) {
            match action {
                KeyAction::Quit => return PageAction::Quit,
                KeyAction::Back => return PageAction::Back,
                KeyAction::Refresh => self.refresh(),
                KeyAction::Open => return PageAction::Switch(crate::pages::PageKind::Subreddit),
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
        "subscriptions"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pages::{Page, PageAction, PageKind};
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    #[test]
    fn open_returns_subreddit_switch() {
        let mut page = SubscriptionPage::new();
        page.load_sync();

        let action = page.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::empty()));

        assert_eq!(action, PageAction::Switch(PageKind::Subreddit));
    }
}
