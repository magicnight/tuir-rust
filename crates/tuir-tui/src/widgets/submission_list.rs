//! Submission list widget

use ratatui::{
    prelude::*,
    widgets::{Block, BorderType, Borders, List, ListItem, ListState},
};
use tuir_core::reddit::models::Submission;

/// Vote state display
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum VoteState {
    #[default]
    None,
    Up,
    Down,
}

/// Sort order for subreddit listing
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SortOrder {
    #[default]
    Hot,
    New,
    Top,
    Controversial,
    Rising,
}

impl SortOrder {
    pub fn label(&self) -> &'static str {
        self.to_core().as_str()
    }

    /// Map the UI-side sort to the network-side [`tuir_core::reddit::api::Sort`].
    pub fn to_core(self) -> tuir_core::reddit::api::Sort {
        use tuir_core::reddit::api::Sort;
        match self {
            SortOrder::Hot => Sort::Hot,
            SortOrder::New => Sort::New,
            SortOrder::Top => Sort::Top,
            SortOrder::Controversial => Sort::Controversial,
            SortOrder::Rising => Sort::Rising,
        }
    }
}

/// Format a single submission as a Line
pub fn format_submission(idx: usize, sub: &Submission, vote: VoteState) -> Line<'static> {
    let mut spans = Vec::new();

    // Index
    spans.push(Span::raw(format!("{:<3} ", idx + 1)));

    // Vote indicator
    match vote {
        VoteState::Up => spans.push(Span::styled(
            format!("▲ {} ", sub.score),
            Style::default().fg(Color::Green),
        )),
        VoteState::Down => spans.push(Span::styled(
            format!("▼ {} ", sub.score),
            Style::default().fg(Color::Red),
        )),
        VoteState::None => spans.push(Span::raw(format!("  {} ", sub.score))),
    }

    // Title
    let title = if sub.over_18 {
        format!("[NSFW] {}", sub.title)
    } else if sub.stickied {
        format!("★ {}", sub.title)
    } else {
        sub.title.clone()
    };

    let title_style = if sub.over_18 {
        Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)
    } else if sub.stickied {
        Style::default().fg(Color::Green)
    } else {
        Style::default()
    };
    spans.push(Span::styled(title, title_style));

    // Metadata
    spans.push(Span::styled(
        format!("  r/{} • {} comments", sub.subreddit, sub.num_comments),
        Style::default().fg(Color::DarkGray),
    ));

    Line::from(spans)
}

/// Render submission list
pub fn render_submission_list(
    f: &mut Frame,
    area: Rect,
    submissions: &[Submission],
    vote_states: &[VoteState],
    state: &mut ListState,
) {
    if submissions.is_empty() {
        let block = Block::default()
            .title(" No submissions ")
            .borders(Borders::ALL);
        let para = ratatui::widgets::Paragraph::new("Loading...").block(block);
        f.render_widget(para, area);
        return;
    }

    let items: Vec<ListItem> = submissions
        .iter()
        .enumerate()
        .map(|(i, sub)| {
            let vote = vote_states.get(i).copied().unwrap_or_default();
            let line = format_submission(i, sub, vote);
            ListItem::new(line)
        })
        .collect();

    let list = List::new(items)
        .block(
            Block::default()
                .title(format!(" r/{} ", submissions.len()))
                .borders(Borders::ALL)
                .border_type(BorderType::Plain),
        )
        .highlight_style(
            Style::default()
                .bg(Color::Rgb(40, 40, 40))
                .add_modifier(Modifier::BOLD),
        );

    f.render_stateful_widget(list, area, state);
}
