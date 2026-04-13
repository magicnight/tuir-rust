//! Subreddit listing page

use crate::keymap::KeyAction;
use crate::widgets::{render_submission_list, SortOrder, VoteState};
use crate::PageAction;
use crossterm::event::KeyEvent;
use ratatui::{
    layout::{Constraint, Direction, Layout},
    style::Style,
    widgets::{Block, BorderType, Borders, ListState},
    Frame,
};
use std::sync::Arc;
use tuir_core::reddit::models::Submission;
use tuir_core::reddit::MockRedditClient;

/// Subreddit page state
pub struct SubredditPage {
    pub name: String,
    pub sort: SortOrder,
    pub submissions: Vec<Submission>,
    pub vote_states: Vec<VoteState>,
    pub list_state: ListState,
    pub loading: bool,
    pub client: Arc<MockRedditClient>,
}

impl SubredditPage {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            sort: SortOrder::Hot,
            submissions: Vec::new(),
            vote_states: Vec::new(),
            list_state: ListState::default(),
            loading: false,
            client: Arc::new(MockRedditClient::new()),
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

        let listing = self.client.hot(sub, 50).await;
        self.submissions = listing.data.children.into_iter().map(|c| c.data).collect();
        self.vote_states = vec![VoteState::None; self.submissions.len()];

        if !self.submissions.is_empty() {
            self.list_state.select(Some(0));
        }

        self.loading = false;
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
                tokio::spawn(async move {
                    client.vote(&name, direction).await;
                });
            }
        }
    }

    /// Refresh listing
    pub fn refresh(&mut self) {
        let sub = self.name.clone();
        let sort = self.sort;

        tokio::spawn(async move {
            let mut page = SubredditPage::new(&sub);
            page.sort = sort;
            page.load().await;
        });
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
        let footer_text = " ↑/↓ or j/k: Navigate | a: Upvote | z: Downvote | r: Refresh | q: Quit ";
        let footer = Block::default()
            .title(footer_text)
            .borders(Borders::ALL)
            .border_type(BorderType::Plain)
            .style(Style::default().bg(ratatui::style::Color::Rgb(30, 30, 20)));

        frame.render_widget(footer, chunks[2]);
    }

    fn handle_key(&mut self, key: KeyEvent) -> PageAction {
        use crate::pages::PageAction;

        if let Some(action) = KeyAction::from_key(key.code, key.modifiers) {
            match action {
                KeyAction::Quit => return PageAction::Quit,
                KeyAction::Refresh => {
                    self.refresh();
                }
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
