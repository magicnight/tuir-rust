//! Subreddit listing page

use crate::keymap::KeyAction;
use crate::widgets::{render_submission_list, SortOrder, VoteState};
use crate::PageAction;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    layout::{Constraint, Direction, Layout},
    style::Style,
    widgets::{Block, BorderType, Borders, ListState},
    Frame,
};
use std::sync::Arc;
use tuir_core::reddit::models::Submission;
use tuir_core::reddit::{MockRedditClient, RedditApi};

/// Subreddit page state
pub struct SubredditPage {
    pub name: String,
    pub sort: SortOrder,
    pub submissions: Vec<Submission>,
    pub vote_states: Vec<VoteState>,
    pub list_state: ListState,
    pub loading: bool,
    pub client: Arc<dyn RedditApi>,
}

impl SubredditPage {
    pub fn new(name: &str) -> Self {
        Self::with_client(name, Arc::new(MockRedditClient::new()))
    }

    pub fn with_client(name: &str, client: Arc<dyn RedditApi>) -> Self {
        Self {
            name: name.to_string(),
            sort: SortOrder::Hot,
            submissions: Vec::new(),
            vote_states: Vec::new(),
            list_state: ListState::default(),
            loading: false,
            client,
        }
    }

    pub fn subreddit_display(&self) -> &str {
        if self.name.is_empty() {
            "front"
        } else {
            &self.name
        }
    }

    /// Load submissions
    pub async fn load(&mut self) {
        self.loading = true;

        let sub = if self.name.is_empty() {
            None
        } else {
            Some(self.name.as_str())
        };

        let sort = self.sort.to_core();
        let listing = match self.client.listing(sort, sub, 50).await {
            Ok(listing) => listing,
            Err(err) => {
                tracing::error!("failed to load subreddit listing: {err}");
                self.loading = false;
                return;
            }
        };
        self.submissions = listing.data.children.into_iter().map(|c| c.data).collect();
        self.vote_states = self
            .submissions
            .iter()
            .map(|submission| match submission.likes.map(i8::from).unwrap_or(0) {
                1 => VoteState::Up,
                -1 => VoteState::Down,
                _ => VoteState::None,
            })
            .collect();

        if !self.submissions.is_empty() {
            self.list_state.select(Some(0));
        }

        self.loading = false;
    }

    pub fn load_sync(&mut self) {
        crate::pages::block_on(self.load());
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
            if idx < self.submissions.len().saturating_sub(1) {
                self.list_state.select(Some(idx + 1));
            }
        }
    }

    /// Move to top
    pub fn move_to_top(&mut self) {
        if !self.submissions.is_empty() {
            self.list_state.select(Some(0));
        }
    }

    /// Move to bottom
    pub fn move_to_bottom(&mut self) {
        if !self.submissions.is_empty() {
            self.list_state.select(Some(self.submissions.len() - 1));
        }
    }

    /// Vote on selected submission
    pub fn vote(&mut self, direction: i8) {
        if let Some(idx) = self.list_state.selected() {
            if idx < self.submissions.len() {
                let sub = &mut self.submissions[idx];
                let old_vote: i8 = match self.vote_states[idx] {
                    VoteState::Up => 1,
                    VoteState::Down => -1,
                    VoteState::None => 0,
                };

                // Update local state
                self.vote_states[idx] = match direction {
                    1 => VoteState::Up,
                    -1 => VoteState::Down,
                    _ => VoteState::None,
                };

                // Update score
                let diff = direction - old_vote;
                sub.score += diff as i64;

                // Send to client
                let client = Arc::clone(&self.client);
                let name = sub.name.clone();
                crate::pages::block_on(async move {
                    if let Err(err) = client.vote(&name, direction).await {
                        tracing::error!("vote failed: {err}");
                    }
                });
            }
        }
    }

    /// Refresh listing
    pub fn refresh(&mut self) {
        self.load_sync();
    }

    /// Switch the active sort order and reload immediately.
    pub fn set_sort(&mut self, sort: SortOrder) {
        if self.sort == sort {
            return;
        }
        self.sort = sort;
        self.load_sync();
    }
}

impl crate::pages::Page for SubredditPage {
    fn render(&mut self, frame: &mut Frame) {
        let area = frame.area();

        // Main layout: header, content, footer
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1), // Header
                Constraint::Min(1),    // Content
                Constraint::Length(1), // Footer
            ])
            .split(area);

        // Header
        let header_text = if self.loading {
            format!(
                " r/{} | {} | Loading... ",
                self.subreddit_display(),
                self.sort.label().to_uppercase()
            )
        } else {
            format!(
                " r/{} | {} | {} posts ",
                self.subreddit_display(),
                self.sort.label().to_uppercase(),
                self.submissions.len()
            )
        };

        let header = Block::default()
            .title(header_text)
            .borders(Borders::ALL)
            .border_type(BorderType::Plain)
            .style(Style::default().bg(ratatui::style::Color::Rgb(20, 20, 30)));

        frame.render_widget(header, chunks[0]);

        // Content
        render_submission_list(
            frame,
            chunks[1],
            &self.submissions,
            &self.vote_states,
            &mut self.list_state,
        );

        // Footer
        let footer_text = " j/k:Nav | 1-5:Sort hot/new/top/contro/rising | a/z:Vote | r:Refresh | q:Quit ";
        let footer = Block::default()
            .title(footer_text)
            .borders(Borders::ALL)
            .border_type(BorderType::Plain)
            .style(Style::default().bg(ratatui::style::Color::Rgb(30, 30, 20)));

        frame.render_widget(footer, chunks[2]);
    }

    fn handle_key(&mut self, key: KeyEvent) -> PageAction {
        use crate::pages::PageAction;

        // Digit shortcuts cycle the sort order (mirrors classic tuir/rtv).
        if let KeyCode::Char(ch) = key.code {
            if let Some(sort) = match ch {
                '1' => Some(SortOrder::Hot),
                '2' => Some(SortOrder::New),
                '3' => Some(SortOrder::Top),
                '4' => Some(SortOrder::Controversial),
                '5' => Some(SortOrder::Rising),
                _ => None,
            } {
                self.set_sort(sort);
                return PageAction::None;
            }
        }

        if let Some(action) = KeyAction::from_key(key.code, key.modifiers) {
            match action {
                KeyAction::Quit => return PageAction::Quit,
                KeyAction::Refresh => {
                    self.refresh();
                }
                KeyAction::Open => return PageAction::Switch(crate::pages::PageKind::Submission),
                KeyAction::NextItem => self.move_down(),
                KeyAction::PrevItem => self.move_up(),
                KeyAction::Top => self.move_to_top(),
                KeyAction::Bottom => self.move_to_bottom(),
                KeyAction::VoteUp => self.vote(1),
                KeyAction::VoteDown => self.vote(-1),
                _ => {}
            }
        }

        PageAction::None
    }

    fn title(&self) -> &str {
        self.subreddit_display()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use crate::pages::{Page, PageAction, PageKind};

    #[test]
    fn refresh_reloads_current_page_state() {
        let mut page = SubredditPage::new("rust");
        assert!(page.submissions.is_empty());

        page.refresh();

        assert!(!page.submissions.is_empty());
        assert_eq!(page.list_state.selected(), Some(0));
    }

    #[test]
    fn refresh_preserves_mock_vote_state() {
        let mut page = SubredditPage::new("rust");
        page.load_sync();

        page.vote(1);
        page.refresh();

        assert_eq!(page.vote_states.first(), Some(&VoteState::Up));
    }

    #[test]
    fn pressing_two_switches_to_new_and_reloads() {
        let mut page = SubredditPage::new("rust");
        page.load_sync();
        assert_eq!(page.sort, SortOrder::Hot);

        let action =
            page.handle_key(KeyEvent::new(KeyCode::Char('2'), KeyModifiers::empty()));
        assert_eq!(action, PageAction::None);
        assert_eq!(page.sort, SortOrder::New);
        // Mock stamps the sort label into each title; verify reload happened.
        assert!(
            page.submissions
                .first()
                .map(|s| s.title.starts_with("[new]"))
                .unwrap_or(false),
            "expected mock submissions to be re-stamped with [new] prefix; got {:?}",
            page.submissions.first().map(|s| s.title.as_str())
        );
    }

    #[test]
    fn pressing_five_switches_to_rising() {
        let mut page = SubredditPage::new("rust");
        page.load_sync();
        page.handle_key(KeyEvent::new(KeyCode::Char('5'), KeyModifiers::empty()));
        assert_eq!(page.sort, SortOrder::Rising);
    }

    #[test]
    fn pressing_same_sort_is_a_noop() {
        let mut page = SubredditPage::new("rust");
        page.load_sync();
        let before = page.submissions.len();
        page.handle_key(KeyEvent::new(KeyCode::Char('1'), KeyModifiers::empty()));
        assert_eq!(page.sort, SortOrder::Hot);
        assert_eq!(page.submissions.len(), before);
    }

    #[test]
    fn open_returns_submission_switch() {
        let mut page = SubredditPage::new("rust");
        page.load_sync();

        let action = page.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::empty()));

        assert_eq!(action, PageAction::Switch(PageKind::Submission));
    }
}
