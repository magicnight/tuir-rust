//! Submission detail page with comment tree

use crate::keymap::KeyAction;
use crate::widgets::VoteState;
use crate::PageAction;
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind};
use ratatui::{
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, List, ListItem, ListState, Paragraph, Wrap},
    Frame,
};
use std::sync::Arc;
use tuir_core::reddit::models::{Comment, CommentReplies, Submission};
use tuir_core::reddit::{MockRedditClient, RedditApi};

/// Flattened comment node for display
#[derive(Debug, Clone)]
pub struct CommentNode {
    pub comment: Comment,
    pub collapsed: bool,
    pub visible: bool,
    pub reply_count: usize,
}

impl CommentNode {
    /// Create from a comment, computing reply count
    fn from_comment(comment: Comment) -> Self {
        let reply_count = Self::count_replies(&comment.replies);
        Self {
            comment,
            collapsed: false,
            visible: true,
            reply_count,
        }
    }

    fn count_replies(replies: &Option<Box<CommentReplies>>) -> usize {
        replies.as_ref().map_or(0, |b| match &**b {
            CommentReplies::Listing(listing) => listing.data.children.len(),
            CommentReplies::Empty => 0,
        })
    }

    /// Check if this is a "load more" type comment
    pub fn is_more(&self) -> bool {
        self.comment.body.is_empty() || self.comment.id.starts_with("more_")
    }
}

/// Submission page state
pub struct SubmissionPage {
    pub submission: Submission,
    pub comments: Vec<CommentNode>,
    pub flattened: Vec<usize>, // indices into comments that are currently visible
    pub list_state: ListState,
    pub vote_state: VoteState,
    pub client: Arc<dyn RedditApi>,
    pub loading: bool,
}

impl SubmissionPage {
    /// Create a new submission page
    pub fn new(submission: Submission) -> Self {
        Self::with_client(submission, Arc::new(MockRedditClient::new()))
    }

    pub fn with_client(submission: Submission, client: Arc<dyn RedditApi>) -> Self {
        Self {
            submission,
            comments: Vec::new(),
            flattened: Vec::new(),
            list_state: ListState::default(),
            vote_state: VoteState::None,
            client,
            loading: false,
        }
    }

    /// Load submission and comments
    pub async fn load(&mut self) {
        self.loading = true;

        let resp = match self.client.submission(&self.submission.id).await {
            Ok(payload) => payload,
            Err(err) => {
                tracing::error!("failed to load submission: {err}");
                self.loading = false;
                return;
            }
        };
        self.submission = resp.submission;
        self.vote_state = match self.submission.likes.map(i8::from).unwrap_or(0) {
            1 => VoteState::Up,
            -1 => VoteState::Down,
            _ => VoteState::None,
        };
        self.comments = resp
            .comments
            .into_iter()
            .map(CommentNode::from_comment)
            .collect();

        self.rebuild_flattened();

        if !self.flattened.is_empty() {
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

    /// Rebuild the flattened view based on collapsed state
    fn rebuild_flattened(&mut self) {
        self.flattened.clear();

        fn walk(nodes: &[CommentNode], flattened: &mut Vec<usize>) {
            for (i, node) in nodes.iter().enumerate() {
                flattened.push(i);

                // Process replies if not collapsed
                if !node.collapsed {
                    if let Some(ref replies_box) = node.comment.replies {
                        match &**replies_box {
                            CommentReplies::Listing(listing) => {
                                for child in &listing.data.children {
                                    // Recursively walk children
                                    walk_child(&child.data, flattened, 1);
                                }
                            }
                            CommentReplies::Empty => {}
                        }
                    }
                }
            }
        }

        fn walk_child(_comment: &Comment, flattened: &mut Vec<usize>, _depth: usize) {
            flattened.push(flattened.len()); // placeholder - actual impl would need indices
        }

        walk(&self.comments, &mut self.flattened);

        // Fallback: if flatten failed, just show all
        if self.flattened.is_empty() {
            self.flattened = (0..self.comments.len()).collect();
        }
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
            if idx < self.flattened.len().saturating_sub(1) {
                self.list_state.select(Some(idx + 1));
            }
        }
    }

    /// Move to top
    pub fn move_to_top(&mut self) {
        if !self.flattened.is_empty() {
            self.list_state.select(Some(0));
        }
    }

    /// Move to bottom
    pub fn move_to_bottom(&mut self) {
        if !self.flattened.is_empty() {
            self.list_state.select(Some(self.flattened.len() - 1));
        }
    }

    /// Toggle collapse on selected comment
    pub fn toggle_collapse(&mut self) {
        if let Some(idx) = self.list_state.selected() {
            if idx < self.flattened.len() {
                let comment_idx = self.flattened[idx];
                if comment_idx < self.comments.len() {
                    self.comments[comment_idx].collapsed = !self.comments[comment_idx].collapsed;
                    self.rebuild_flattened();
                }
            }
        }
    }

    /// Vote on submission
    pub fn vote(&mut self, direction: i8) {
        let old_vote: i8 = self.vote_state.into();

        // Update local vote state
        self.vote_state = match direction {
            1 => VoteState::Up,
            -1 => VoteState::Down,
            _ => VoteState::None,
        };

        // Update score
        let diff = direction - old_vote;
        self.submission.score += diff as i64;

        // Send to client
        let client = Arc::clone(&self.client);
        let name = self.submission.name.clone();
        crate::pages::block_on(async move {
            if let Err(err) = client.vote(&name, direction).await {
                tracing::error!("vote failed: {err}");
            }
        });
    }

    fn format_timestamp(created_utc: f64) -> String {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as f64;
        let diff = now - created_utc;

        if diff < 60.0 {
            format!("{:.0}s", diff)
        } else if diff < 3600.0 {
            format!("{:.0}m", diff / 60.0)
        } else if diff < 86400.0 {
            format!("{:.0}h", diff / 3600.0)
        } else if diff < 2592000.0 {
            format!("{:.0}d", diff / 86400.0)
        } else {
            format!("{:.0}mo", diff / 2592000.0)
        }
    }

    fn format_depth(depth: u32) -> String {
        if depth == 0 {
            String::new()
        } else {
            let indent = "  │ "
                .repeat(depth as usize)
                .chars()
                .take(40)
                .collect::<String>();
            format!("{} ", indent)
        }
    }
}

impl crate::pages::Page for SubmissionPage {
    fn render(&mut self, frame: &mut Frame) {
        let area = frame.area();

        // Layout: header, submission, comments, footer
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1), // Header
                Constraint::Length(4), // Submission content
                Constraint::Min(1),    // Comments
                Constraint::Length(1), // Footer
            ])
            .split(area);

        // ── Header ──────────────────────────────────────────────
        let header_text = format!(
            " r/{} • {} • by {} • {} ",
            self.submission.subreddit,
            self.format_vote_display(),
            self.submission.author,
            Self::format_timestamp(self.submission.created_utc)
        );

        let header = Block::default()
            .title(header_text)
            .borders(Borders::ALL)
            .border_type(BorderType::Plain)
            .style(Style::default().bg(Color::Rgb(20, 20, 30)));

        frame.render_widget(header, chunks[0]);

        // ── Submission Content ────────────────────────────────────
        let submission_block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Plain)
            .style(Style::default().bg(Color::Rgb(30, 30, 30)));

        let inner = submission_block.inner(chunks[1]);
        frame.render_widget(submission_block, chunks[1]);

        // Title
        let title_style = if self.submission.over_18 {
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)
        } else {
            Style::default().add_modifier(Modifier::BOLD)
        };

        let title_line = Line::from(vec![Span::styled(
            format!(" {} ", self.submission.title),
            title_style,
        )]);

        // Self-text
        let selftext = if self.submission.selftext.is_empty() {
            Line::from(Span::styled(
                format!(" [link] {}", self.submission.url),
                Style::default().fg(Color::Blue),
            ))
        } else {
            let text = self
                .submission
                .selftext
                .chars()
                .take(200)
                .collect::<String>();
            Line::from(Span::styled(
                format!(" {} ", text),
                Style::default().fg(Color::Gray),
            ))
        };

        let meta = Line::from(vec![
            Span::raw(" "),
            Span::styled("▲ ", Style::default().fg(Color::Green)),
            Span::raw(format!("{} ", self.submission.score)),
            Span::styled("💬 ", Style::default().fg(Color::Cyan)),
            Span::raw(format!("{} ", self.submission.num_comments)),
        ]);

        let content = Paragraph::new(vec![title_line, selftext, meta])
            .wrap(Wrap { trim: true })
            .scroll((0, 0));

        frame.render_widget(content, inner);

        // ── Comments ─────────────────────────────────────────────
        let comment_header = Block::default()
            .title(format!(" Comments: {} ", self.flattened.len()))
            .borders(Borders::ALL)
            .border_type(BorderType::Plain)
            .style(Style::default().bg(Color::Rgb(25, 25, 35)));

        let comment_area = chunks[2];
        frame.render_widget(&comment_header, comment_area);

        let inner = comment_header.inner(comment_area);

        if self.loading {
            let para =
                Paragraph::new("Loading comments...").style(Style::default().fg(Color::Gray));
            frame.render_widget(para, inner);
        } else if self.flattened.is_empty() {
            let para =
                Paragraph::new("No comments yet.").style(Style::default().fg(Color::DarkGray));
            frame.render_widget(para, inner);
        } else {
            let items: Vec<ListItem> = self
                .flattened
                .iter()
                .filter_map(|&comment_idx| {
                    if comment_idx < self.comments.len() {
                        Some(ListItem::new(Self::format_comment(
                            &self.comments[comment_idx],
                        )))
                    } else {
                        None
                    }
                })
                .collect();

            let list = List::new(items).block(Block::default()).highlight_style(
                Style::default()
                    .bg(Color::Rgb(40, 40, 40))
                    .add_modifier(Modifier::BOLD),
            );

            frame.render_stateful_widget(list, inner, &mut self.list_state);
        }

        // ── Footer ───────────────────────────────────────────────
        let footer_text = " c:Collapse | a:Upvote | z:Downvote | j/k:Navigate | q:Back ";
        let footer = Block::default()
            .title(footer_text)
            .borders(Borders::ALL)
            .border_type(BorderType::Plain)
            .style(Style::default().bg(Color::Rgb(30, 30, 20)));

        frame.render_widget(footer, chunks[3]);
    }

    fn handle_key(&mut self, key: KeyEvent) -> PageAction {
        use crate::pages::PageAction;

        if key.kind != KeyEventKind::Press {
            return PageAction::None;
        }

        // Handle 'c' for collapse directly
        if key.code == KeyCode::Char('c') {
            self.toggle_collapse();
            return PageAction::None;
        }

        if let Some(action) = KeyAction::from_key(key.code, key.modifiers) {
            match action {
                KeyAction::Quit => return PageAction::Quit,
                KeyAction::Back => return PageAction::Back,
                KeyAction::Refresh => self.refresh(),
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
        &self.submission.title
    }
}

impl SubmissionPage {
    /// Format vote display for header
    fn format_vote_display(&self) -> String {
        match self.vote_state {
            VoteState::Up => format!("▲ {}", self.submission.score),
            VoteState::Down => format!("▼ {}", self.submission.score),
            VoteState::None => format!("  {}", self.submission.score),
        }
    }

    /// Format a single comment for display
    fn format_comment(node: &CommentNode) -> Line<'_> {
        let depth = node.comment.depth.unwrap_or(0) as u32;

        let body_text = if node.collapsed {
            format!("[–] {} ({} hidden)", node.comment.author, node.reply_count)
        } else if node.is_more() {
            "[+] Load more comments".to_string()
        } else {
            node.comment.body.chars().take(100).collect::<String>()
        };

        let depth_color = match depth % 4 {
            0 => Color::Yellow,
            1 => Color::Green,
            2 => Color::Cyan,
            3 => Color::Magenta,
            _ => Color::White,
        };

        let time = Self::format_timestamp(node.comment.created_utc);

        let score_str = match node.comment.likes.unwrap_or(0) {
            1 => format!("▲{}", node.comment.score),
            -1 => format!("▼{}", node.comment.score),
            _ => format!(" {}", node.comment.score),
        };

        // Build spans
        let mut spans: Vec<Span<'_>> = Vec::new();

        // Depth indicator
        spans.push(Span::raw(Self::format_depth(depth)));

        // Author
        spans.push(Span::styled(
            node.comment.author.as_str(),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ));

        // Score & time
        spans.push(Span::raw(format!(" • {} • {} ", score_str, time)));

        // Body
        if node.collapsed {
            spans.push(Span::styled(
                format!("[–] {} replies", node.reply_count),
                Style::default().fg(Color::DarkGray),
            ));
        } else {
            spans.push(Span::styled(body_text, Style::default().fg(depth_color)));
        }

        Line::from(spans)
    }
}

impl From<VoteState> for i8 {
    fn from(v: VoteState) -> Self {
        match v {
            VoteState::Up => 1,
            VoteState::Down => -1,
            VoteState::None => 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tuir_core::reddit::models::{EditedField, Submission};

    fn sample_submission(score: i64) -> Submission {
        Submission {
            id: "abc123".to_string(),
            name: "t3_abc123".to_string(),
            title: "Sample".to_string(),
            author: "tester".to_string(),
            subreddit: "rust".to_string(),
            score,
            num_comments: 3,
            permalink: "/r/rust/comments/abc123/sample".to_string(),
            url: "https://reddit.com/r/rust/comments/abc123/sample".to_string(),
            selftext: String::new(),
            created_utc: 1_700_000_000.0,
            distinguished: None,
            edited: EditedField::Bool(false),
            link_flair_text: None,
            author_flair_text: None,
            over_18: false,
            pinned: false,
            spoiler: false,
            stickied: false,
            saved: false,
            hidden: false,
            likes: None,
            url_full: None,
        }
    }

    #[test]
    fn vote_uses_previous_vote_state_when_adjusting_score() {
        let mut page = SubmissionPage::new(sample_submission(10));

        page.vote(1);
        assert_eq!(page.submission.score, 11);

        page.vote(-1);
        assert_eq!(page.submission.score, 9);
    }

    #[test]
    fn refresh_preserves_mock_vote_state() {
        let mut page = SubmissionPage::new(sample_submission(10));

        page.vote(1);
        page.refresh();

        assert_eq!(page.vote_state, VoteState::Up);
        assert_eq!(page.submission.likes.map(i8::from), Some(1));
    }
}
