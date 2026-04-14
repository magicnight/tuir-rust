//! Help page — keybinding reference

use crate::keymap::KeyAction;
use crate::pages::{Page, PageAction};
use crate::theme::AppTheme;
use std::sync::Arc;
use crossterm::event::{KeyEvent, KeyEventKind};
use ratatui::{
    layout::{Constraint, Direction, Layout},
    prelude::*,
    text::Line,
    widgets::{Block, BorderType, Borders, Paragraph, Wrap},
    Frame,
};

pub struct HelpPage {
    scroll: u16,
    pub theme: Arc<AppTheme>,
}

impl HelpPage {
    pub fn new() -> Self {
        Self {
            scroll: 0,
            theme: Arc::new(AppTheme::default()),
        }
    }

    pub fn set_theme(&mut self, theme: Arc<AppTheme>) {
        self.theme = theme;
    }

    fn sections() -> Vec<(&'static str, Vec<(&'static str, &'static str)>)> {
        vec![
            (
                "Navigation",
                vec![
                    ("j / ↓", "Move selection down"),
                    ("k / ↑", "Move selection up"),
                    ("g", "Jump to top"),
                    ("G", "Jump to bottom"),
                    ("h", "Scroll up"),
                    ("l", "Scroll down"),
                ],
            ),
            (
                "Actions",
                vec![
                    ("Enter / o", "Open selected item"),
                    ("a", "Upvote"),
                    ("z", "Downvote"),
                    ("c", "Collapse comment subtree"),
                    ("u", "Toggle inbox read/unread"),
                    ("r", "Refresh current page"),
                ],
            ),
            (
                "Sort (subreddit)",
                vec![
                    ("1", "Hot"),
                    ("2", "New"),
                    ("3", "Top"),
                    ("4", "Controversial"),
                    ("5", "Rising"),
                ],
            ),
            (
                "Global",
                vec![
                    ("?", "Toggle this help page"),
                    ("Esc", "Back to previous page"),
                    ("q", "Back, or Quit on the root page"),
                ],
            ),
        ]
    }

    fn rendered_lines(theme: &AppTheme) -> Vec<Line<'static>> {
        let mut lines: Vec<Line<'static>> = Vec::new();
        lines.push(Line::from(Span::styled(
            "TUIR-RUST — keybindings",
            theme.stickied,
        )));
        lines.push(Line::from(""));

        for (heading, entries) in Self::sections() {
            lines.push(Line::from(Span::styled(heading, theme.author)));
            for (key, desc) in entries {
                lines.push(Line::from(vec![
                    Span::raw("  "),
                    Span::styled(format!("{key:<12}"), theme.upvote),
                    Span::raw("  "),
                    Span::raw(desc.to_string()),
                ]));
            }
            lines.push(Line::from(""));
        }

        lines.push(Line::from(Span::styled("Esc or q — back", theme.muted)));
        lines
    }
}

impl Default for HelpPage {
    fn default() -> Self {
        Self::new()
    }
}

impl Page for HelpPage {
    fn render(&mut self, frame: &mut Frame) {
        let area = frame.area();
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(1), Constraint::Min(1), Constraint::Length(1)])
            .split(area);

        let header = Block::default()
            .title(" HELP ")
            .borders(Borders::ALL)
            .border_type(BorderType::Plain)
            .style(self.theme.header);
        frame.render_widget(header, chunks[0]);

        let body_block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Plain);
        let inner = body_block.inner(chunks[1]);
        frame.render_widget(body_block, chunks[1]);

        let paragraph = Paragraph::new(Self::rendered_lines(&self.theme))
            .wrap(Wrap { trim: false })
            .scroll((self.scroll, 0));
        frame.render_widget(paragraph, inner);

        let footer = Block::default()
            .title(" j/k:Scroll | ?:Close | q:Back ")
            .borders(Borders::ALL)
            .border_type(BorderType::Plain)
            .style(self.theme.footer);
        frame.render_widget(footer, chunks[2]);
    }

    fn handle_key(&mut self, key: KeyEvent) -> PageAction {
        if key.kind != KeyEventKind::Press {
            return PageAction::None;
        }

        if let Some(action) = KeyAction::from_key(key.code, key.modifiers) {
            match action {
                KeyAction::Quit | KeyAction::Back | KeyAction::Help => return PageAction::Back,
                KeyAction::NextItem | KeyAction::ScrollDown => {
                    self.scroll = self.scroll.saturating_add(1);
                }
                KeyAction::PrevItem | KeyAction::ScrollUp => {
                    self.scroll = self.scroll.saturating_sub(1);
                }
                KeyAction::Top => self.scroll = 0,
                _ => {}
            }
        }

        PageAction::None
    }

    fn title(&self) -> &str {
        "help"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyCode, KeyModifiers};

    #[test]
    fn question_mark_returns_back() {
        let mut page = HelpPage::new();
        let action = page.handle_key(KeyEvent::new(KeyCode::Char('?'), KeyModifiers::empty()));
        assert_eq!(action, PageAction::Back);
    }

    #[test]
    fn j_scrolls_down() {
        let mut page = HelpPage::new();
        let _ = page.handle_key(KeyEvent::new(KeyCode::Char('j'), KeyModifiers::empty()));
        assert_eq!(page.scroll, 1);
    }

    #[test]
    fn k_at_zero_stays_zero() {
        let mut page = HelpPage::new();
        let _ = page.handle_key(KeyEvent::new(KeyCode::Char('k'), KeyModifiers::empty()));
        assert_eq!(page.scroll, 0);
    }
}
