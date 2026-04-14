//! Media preview page (M9 stage 2 — info-only scaffold).
//!
//! Currently this page renders metadata about a classified media URL and
//! displays the URL. Stage 3 will plug in the `image` + `ratatui-image`
//! pipeline so static images and animated-gif first frames render
//! inline (with the [`tuir_core::media::MediaStyle::Retro`] half-block
//! pipeline as an opt-in stylistic choice).
//!
//! Splitting the milestone this way keeps every commit independently
//! verifiable: the page surface, navigation wiring, and key bindings
//! land here without any heavy image-decoding dependencies, then the
//! decoder lands on top with the rendering harness already in place.

use crate::keymap::KeyAction;
use crate::pages::{Page, PageAction};
use crate::theme::AppTheme;
use crossterm::event::{KeyEvent, KeyEventKind};
use ratatui::{
    layout::{Constraint, Direction, Layout},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Paragraph, Wrap},
    Frame,
};
use std::sync::Arc;
use tuir_core::media::{MediaKind, MediaRef, MediaStyle};

/// Inline media preview page.
pub struct MediaPage {
    pub media: MediaRef,
    pub style: MediaStyle,
    pub theme: Arc<AppTheme>,
}

impl MediaPage {
    pub fn new(media: MediaRef) -> Self {
        Self {
            media,
            style: MediaStyle::Auto,
            theme: Arc::new(AppTheme::default()),
        }
    }

    pub fn set_theme(&mut self, theme: Arc<AppTheme>) {
        self.theme = theme;
    }

    pub fn set_style(&mut self, style: MediaStyle) {
        self.style = style;
    }

    fn kind_label(kind: MediaKind) -> &'static str {
        match kind {
            MediaKind::Image => "Static image",
            MediaKind::AnimatedGif => "Animated GIF",
            MediaKind::Video => "Video",
            MediaKind::Gallery => "Gallery (multi-image)",
            MediaKind::External => "External link",
        }
    }

    fn renderability_label(&self) -> &'static str {
        if matches!(self.style, MediaStyle::Off) {
            return "inline rendering disabled (media_style = off)";
        }
        if self.media.is_inline_renderable() {
            "inline render pending — image decoder lands in M9.3"
        } else {
            "not inline-renderable; will hand off to mailcap viewer"
        }
    }
}

impl Page for MediaPage {
    fn render(&mut self, frame: &mut Frame) {
        let area = frame.area();
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1),
                Constraint::Min(1),
                Constraint::Length(1),
            ])
            .split(area);

        let header = Block::default()
            .title(format!(
                " ▎ MEDIA • {} ",
                Self::kind_label(self.media.kind)
            ))
            .borders(Borders::ALL)
            .border_type(BorderType::Plain)
            .style(self.theme.header);
        frame.render_widget(header, chunks[0]);

        let body_block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Plain);
        let inner = body_block.inner(chunks[1]);
        frame.render_widget(body_block, chunks[1]);

        let lines = vec![
            Line::from(vec![
                Span::styled("URL  ", self.theme.author),
                Span::raw(self.media.url.clone()),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("Kind ", self.theme.author),
                Span::raw(Self::kind_label(self.media.kind)),
            ]),
            Line::from(vec![
                Span::styled("Style", self.theme.author),
                Span::raw(format!(" {:?}", self.style)),
            ]),
            Line::from(""),
            Line::from(Span::styled(self.renderability_label(), self.theme.muted)),
        ];

        let body = Paragraph::new(lines).wrap(Wrap { trim: false });
        frame.render_widget(body, inner);

        let footer = Block::default()
            .title(" ?:Help | q:Back ")
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
            return match action {
                KeyAction::Quit | KeyAction::Back => PageAction::Back,
                _ => PageAction::None,
            };
        }
        PageAction::None
    }

    fn title(&self) -> &str {
        "media"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyCode, KeyModifiers};

    fn page_with(kind: MediaKind) -> MediaPage {
        MediaPage::new(MediaRef::new(kind, "https://i.redd.it/x.jpg"))
    }

    #[test]
    fn q_returns_back() {
        let mut page = page_with(MediaKind::Image);
        let action = page.handle_key(KeyEvent::new(KeyCode::Char('q'), KeyModifiers::empty()));
        assert_eq!(action, PageAction::Back);
    }

    #[test]
    fn esc_returns_back() {
        let mut page = page_with(MediaKind::Image);
        let action = page.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::empty()));
        assert_eq!(action, PageAction::Back);
    }

    #[test]
    fn renderability_label_reflects_style_off() {
        let mut page = page_with(MediaKind::Image);
        page.set_style(MediaStyle::Off);
        assert!(page.renderability_label().contains("disabled"));
    }

    #[test]
    fn renderability_label_for_video_says_mailcap() {
        let page = page_with(MediaKind::Video);
        assert!(page.renderability_label().contains("mailcap"));
    }

    #[test]
    fn renderability_label_for_image_under_auto_says_pending() {
        let page = page_with(MediaKind::Image);
        assert!(page.renderability_label().contains("pending"));
    }
}
