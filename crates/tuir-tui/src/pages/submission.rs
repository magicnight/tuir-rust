//! Submission detail page with comment tree

use crate::keymap::KeyAction;
use crate::theme::AppTheme;
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
use tuir_core::content::render_plain_string;
use tuir_core::reddit::models::{Comment, CommentReplies, Submission};
use tuir_core::reddit::{MockRedditClient, RedditApi};

/// Flattened comment node for display
#[derive(Debug, Clone)]
pub struct CommentNode {
    pub comment: Comment,
    pub collapsed: bool,
    pub reply_count: usize,
}

impl CommentNode {
    fn from_comment(comment: Comment) -> Self {
        Self {
            comment,
            collapsed: false,
            reply_count: 0,
        }
    }

    /// Check if this is a "load more" type comment
    pub fn is_more(&self) -> bool {
        self.comment.body.is_empty() || self.comment.id.starts_with("more_")
    }
}

/// Depth-first flatten of a Reddit comment tree into a pre-order list.
///
/// Reddit returns comments as a nested tree (each comment may carry a
/// `replies` listing). The view layer wants a flat index sequence with
/// depth information so it can indent and skip collapsed subtrees. This
/// helper walks the tree once, consumes children into top-level nodes,
/// fills in `depth` when the API omitted it, and records each node's
/// direct-reply count before the children are moved out.
pub fn flatten_tree(comments: Vec<Comment>, depth: i64) -> Vec<CommentNode> {
    let mut out = Vec::new();
    for mut comment in comments {
        let replies = comment.replies.take();
        if comment.depth.is_none() {
            comment.depth = Some(depth);
        }

        let (direct_count, child_comments) = match replies {
            Some(boxed) => match *boxed {
                CommentReplies::Listing(listing) => {
                    let children: Vec<Comment> = listing
                        .data
                        .children
                        .into_iter()
                        .map(|thing| thing.data)
                        .collect();
                    (children.len(), children)
                }
                CommentReplies::Empty => (0, Vec::new()),
            },
            None => (0, Vec::new()),
        };

        let mut node = CommentNode::from_comment(comment);
        node.reply_count = direct_count;
        out.push(node);
        out.extend(flatten_tree(child_comments, depth + 1));
    }
    out
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
    pub theme: Arc<AppTheme>,
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
            theme: Arc::new(AppTheme::default()),
        }
    }

    /// Inject a shared theme. See [`crate::pages::subreddit::SubredditPage::set_theme`].
    pub fn set_theme(&mut self, theme: Arc<AppTheme>) {
        self.theme = theme;
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
        self.comments = flatten_tree(resp.comments, 0);

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

    /// Rebuild the visible-comment index list from `self.comments`, honoring
    /// per-node `collapsed` state.
    ///
    /// Because [`flatten_tree`] produces a depth-first pre-order list,
    /// every descendant of a collapsed comment sits at a strictly greater
    /// `depth` and is adjacent in the vector — so a single linear pass with
    /// a "hide below depth N" threshold is enough.
    fn rebuild_flattened(&mut self) {
        self.flattened.clear();
        let mut hidden_below: Option<i64> = None;

        for (idx, node) in self.comments.iter().enumerate() {
            let depth = node.comment.depth.unwrap_or(0);
            if let Some(threshold) = hidden_below {
                if depth > threshold {
                    continue;
                }
                hidden_below = None;
            }
            self.flattened.push(idx);
            if node.collapsed {
                hidden_below = Some(depth);
            }
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
            .style(self.theme.header);

        frame.render_widget(header, chunks[0]);

        // ── Submission Content ────────────────────────────────────
        let submission_block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Plain);

        let inner = submission_block.inner(chunks[1]);
        frame.render_widget(submission_block, chunks[1]);

        // Title
        let title_style = if self.submission.over_18 {
            self.theme.nsfw
        } else {
            Style::default().add_modifier(Modifier::BOLD)
        };

        let title_line = Line::from(vec![Span::styled(
            format!(" {} ", self.submission.title),
            title_style,
        )]);

        // Self-text — prefer Reddit's HTML rendering when available so
        // links/lists/code survive; fall back to raw markdown otherwise.
        let body_text = self
            .submission
            .selftext_html
            .as_deref()
            .map(render_plain_string)
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| self.submission.selftext.clone());
        let selftext = if body_text.is_empty() {
            Line::from(Span::styled(
                format!(" [link] {}", self.submission.url),
                self.theme.link,
            ))
        } else {
            let preview: String = body_text.chars().take(200).collect();
            Line::from(Span::styled(format!(" {preview} "), self.theme.muted))
        };

        let meta = Line::from(vec![
            Span::raw(" "),
            Span::styled("▲ ", self.theme.upvote),
            Span::raw(format!("{} ", self.submission.score)),
            Span::styled("💬 ", self.theme.author),
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
            .border_type(BorderType::Plain);

        let comment_area = chunks[2];
        frame.render_widget(&comment_header, comment_area);

        let inner = comment_header.inner(comment_area);

        if self.loading {
            let para = Paragraph::new("Loading comments...").style(self.theme.muted);
            frame.render_widget(para, inner);
        } else if self.flattened.is_empty() {
            let para = Paragraph::new("No comments yet.").style(self.theme.muted);
            frame.render_widget(para, inner);
        } else {
            // Build the list items first to drop the immutable borrow on
            // self before we hand list_state out as &mut.
            let items: Vec<ListItem> = {
                let comments = &self.comments;
                let theme = &*self.theme;
                self.flattened
                    .iter()
                    .filter_map(|&comment_idx| comments.get(comment_idx))
                    .map(|node| ListItem::new(format_comment_line(node, theme)))
                    .collect()
            };

            let list = List::new(items)
                .block(Block::default())
                .highlight_style(self.theme.selected);

            frame.render_stateful_widget(list, inner, &mut self.list_state);
        }

        // ── Footer ───────────────────────────────────────────────
        let footer_text = " j/k:Nav | c:Collapse | a/z:Vote | r:Refresh | ?:Help | q:Back ";
        let footer = Block::default()
            .title(footer_text)
            .borders(Borders::ALL)
            .border_type(BorderType::Plain)
            .style(self.theme.footer);

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

}

/// Standalone comment formatter — extracted so it can run while
/// `&mut self.list_state` is held by the caller.
fn format_comment_line<'a>(node: &'a CommentNode, theme: &AppTheme) -> Line<'a> {
    let depth = node.comment.depth.unwrap_or(0) as u32;

    let body_text = if node.collapsed {
        format!("[–] {} ({} hidden)", node.comment.author, node.reply_count)
    } else if node.is_more() {
        "[+] Load more comments".to_string()
    } else {
        let rendered = node
            .comment
            .body_html
            .as_deref()
            .map(render_plain_string)
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| node.comment.body.clone());
        rendered
            .split('\n')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join(" · ")
            .chars()
            .take(160)
            .collect::<String>()
    };

    let depth_color = match depth % 4 {
        0 => Color::Yellow,
        1 => Color::Green,
        2 => Color::Cyan,
        3 => Color::Magenta,
        _ => Color::White,
    };

    let time = SubmissionPage::format_timestamp(node.comment.created_utc);

    let score_str = match node.comment.likes.unwrap_or(0) {
        1 => format!("▲{}", node.comment.score),
        -1 => format!("▼{}", node.comment.score),
        _ => format!(" {}", node.comment.score),
    };

    let mut spans: Vec<Span<'_>> = Vec::new();
    spans.push(Span::raw(SubmissionPage::format_depth(depth)));
    spans.push(Span::styled(node.comment.author.as_str(), theme.author));
    spans.push(Span::raw(format!(" • {} • {} ", score_str, time)));

    if node.collapsed {
        spans.push(Span::styled(
            format!("[–] {} replies", node.reply_count),
            theme.muted,
        ));
    } else {
        spans.push(Span::styled(body_text, Style::default().fg(depth_color)));
    }

    Line::from(spans)
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
            selftext_html: None,
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

    fn make_comment(id: &str, depth: Option<i64>, replies: Vec<Comment>) -> Comment {
        use tuir_core::reddit::models::{
            CommentReplies, EditedField, Listing, ListingData, Thing,
        };
        let reply_box = if replies.is_empty() {
            Some(Box::new(CommentReplies::Empty))
        } else {
            let children: Vec<Thing<Comment>> = replies
                .into_iter()
                .map(|c| Thing {
                    kind: "t1".to_string(),
                    data: c,
                })
                .collect();
            let listing = Listing {
                kind: "Listing".to_string(),
                data: ListingData {
                    modhash: None,
                    dist: None,
                    children,
                    after: None,
                    before: None,
                },
            };
            Some(Box::new(CommentReplies::Listing(listing)))
        };
        Comment {
            id: id.to_string(),
            name: format!("t1_{id}"),
            author: "tester".to_string(),
            body: format!("body of {id}"),
            body_html: None,
            link_id: "t3_abc".to_string(),
            parent_id: "t3_abc".to_string(),
            score: 0,
            created_utc: 0.0,
            distinguished: None,
            edited: EditedField::Bool(false),
            depth,
            replies: reply_box,
            is_submitter: false,
            saved: false,
            likes: None,
            author_flair_text: None,
        }
    }

    #[test]
    fn flatten_tree_preorder_walks_nested_replies() {
        let tree = vec![
            make_comment(
                "a",
                Some(0),
                vec![
                    make_comment("a1", Some(1), vec![make_comment("a1a", Some(2), vec![])]),
                    make_comment("a2", Some(1), vec![]),
                ],
            ),
            make_comment("b", Some(0), vec![]),
        ];
        let flat = super::flatten_tree(tree, 0);
        let ids: Vec<&str> = flat.iter().map(|n| n.comment.id.as_str()).collect();
        assert_eq!(ids, ["a", "a1", "a1a", "a2", "b"]);
    }

    #[test]
    fn flatten_tree_fills_missing_depth() {
        let tree = vec![make_comment(
            "root",
            None,
            vec![make_comment("child", None, vec![])],
        )];
        let flat = super::flatten_tree(tree, 0);
        assert_eq!(flat[0].comment.depth, Some(0));
        assert_eq!(flat[1].comment.depth, Some(1));
    }

    #[test]
    fn flatten_tree_records_direct_reply_counts() {
        let tree = vec![make_comment(
            "root",
            Some(0),
            vec![
                make_comment("c1", Some(1), vec![]),
                make_comment("c2", Some(1), vec![]),
            ],
        )];
        let flat = super::flatten_tree(tree, 0);
        assert_eq!(flat[0].reply_count, 2);
        assert_eq!(flat[1].reply_count, 0);
    }

    #[test]
    fn rebuild_flattened_skips_collapsed_subtree() {
        let mut page = SubmissionPage::new(sample_submission(0));
        page.comments = super::flatten_tree(
            vec![
                make_comment(
                    "a",
                    Some(0),
                    vec![make_comment("a1", Some(1), vec![])],
                ),
                make_comment("b", Some(0), vec![]),
            ],
            0,
        );
        page.comments[0].collapsed = true;
        page.rebuild_flattened();
        let ids: Vec<&str> = page
            .flattened
            .iter()
            .map(|&idx| page.comments[idx].comment.id.as_str())
            .collect();
        assert_eq!(ids, ["a", "b"]);
    }
}
