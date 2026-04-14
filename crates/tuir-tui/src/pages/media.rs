//! Media preview page (M9.3 — actual inline rendering).
//!
//! Renders Reddit submission media inline in the terminal via the
//! `image` + `ratatui-image` pipeline. The first time the user presses
//! `i` on a submission, this page:
//!
//! 1. classifies the URL via [`tuir_core::media::detect_media`]
//! 2. downloads the bytes to `$XDG_CACHE_HOME/tuir/media/<sha256>.bin`
//!    (cached forever — repeat opens are zero-network)
//! 3. decodes the bytes into a [`image::DynamicImage`] (jpeg / png /
//!    webp / gif first frame are all supported by default features)
//! 4. constructs a [`ratatui_image::picker::Picker`] for the active
//!    terminal protocol, optionally forcing half-blocks when the user
//!    chose [`tuir_core::media::MediaStyle::Retro`]
//! 5. wraps the result in a [`ratatui_image::protocol::StatefulProtocol`]
//!    and renders it via [`ratatui_image::StatefulImage`]
//!
//! Failures at any stage are captured into a small status enum so the
//! page can render a useful error message instead of crashing the TUI.

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
use ratatui_image::picker::{Picker, ProtocolType};
use ratatui_image::protocol::StatefulProtocol;
use ratatui_image::StatefulImage;
use std::path::PathBuf;
use std::sync::Arc;
use tuir_core::media::{download_to_cache, MediaKind, MediaRef, MediaStyle};

/// Render-side status for an inline media preview.
pub enum MediaStatus {
    /// Page is fresh — `ensure_loaded` has not been called yet, or the
    /// page is in [`MediaStyle::Off`] mode and intentionally never loads.
    Idle,
    /// The download/decode pipeline succeeded; we hold a stateful
    /// protocol the renderer feeds frames to on every draw.
    Ready(StatefulProtocol),
    /// Some stage of the pipeline failed; show the message to the user.
    Failed(String),
    /// The classified MediaRef is not something we render inline (video,
    /// gallery, external link). Tells the user to use mailcap instead.
    NotInline,
}

/// Inline media preview page.
pub struct MediaPage {
    pub media: MediaRef,
    pub style: MediaStyle,
    pub theme: Arc<AppTheme>,
    pub status: MediaStatus,
    pub cached_path: Option<PathBuf>,
}

impl MediaPage {
    pub fn new(media: MediaRef) -> Self {
        let initial_status = if media.is_inline_renderable() {
            MediaStatus::Idle
        } else {
            MediaStatus::NotInline
        };
        Self {
            media,
            style: MediaStyle::Auto,
            theme: Arc::new(AppTheme::default()),
            status: initial_status,
            cached_path: None,
        }
    }

    pub fn set_theme(&mut self, theme: Arc<AppTheme>) {
        self.theme = theme;
    }

    pub fn set_style(&mut self, style: MediaStyle) {
        self.style = style;
        if matches!(style, MediaStyle::Off) {
            self.status = MediaStatus::Idle;
        }
    }

    /// Run the download + decode + protocol-construction pipeline.
    ///
    /// Synchronous (block_on) by design — the rest of the page stack uses
    /// the same pattern, and it keeps the navigation transition as a
    /// single user-perceived step rather than introducing a spinner /
    /// background-task path that the TUI doesn't have a concept for yet.
    pub fn ensure_loaded(&mut self, http: &reqwest::Client, cache_dir: &std::path::Path) {
        if !matches!(self.status, MediaStatus::Idle) {
            return;
        }
        if matches!(self.style, MediaStyle::Off) {
            return;
        }
        if !self.media.is_inline_renderable() {
            self.status = MediaStatus::NotInline;
            return;
        }

        let url = self.media.url.clone();
        let cache_dir = cache_dir.to_path_buf();
        let download_result = crate::pages::block_on(async move {
            download_to_cache(http, &url, &cache_dir).await
        });

        let path = match download_result {
            Ok(p) => p,
            Err(err) => {
                tracing::error!("media download failed: {err}");
                self.status = MediaStatus::Failed(format!("download: {err}"));
                return;
            }
        };
        self.cached_path = Some(path.clone());

        let dyn_image = match image::ImageReader::open(&path)
            .and_then(|r| r.with_guessed_format())
            .map_err(anyhow::Error::from)
            .and_then(|r| r.decode().map_err(anyhow::Error::from))
        {
            Ok(img) => img,
            Err(err) => {
                tracing::error!("media decode failed for {}: {err}", path.display());
                self.status = MediaStatus::Failed(format!("decode: {err}"));
                return;
            }
        };

        let mut picker = match Picker::from_query_stdio() {
            Ok(p) => p,
            Err(_) => {
                // Headless / non-tty fallback: pick a reasonable font cell
                // size so at least the half-blocks renderer has something
                // to compute against.
                Picker::from_fontsize((8, 16))
            }
        };
        if matches!(self.style, MediaStyle::Retro) {
            picker.set_protocol_type(ProtocolType::Halfblocks);
        }

        let protocol = picker.new_resize_protocol(dyn_image);
        self.status = MediaStatus::Ready(protocol);
    }

    fn kind_label(kind: MediaKind) -> &'static str {
        match kind {
            MediaKind::Image => "Static image",
            MediaKind::AnimatedGif => "Animated GIF (first frame)",
            MediaKind::Video => "Video",
            MediaKind::Gallery => "Gallery (multi-image)",
            MediaKind::External => "External link",
        }
    }

    fn status_label(&self) -> &'static str {
        match self.status {
            MediaStatus::Ready(_) => "rendering inline",
            MediaStatus::Idle => "loading…",
            MediaStatus::Failed(_) => "failed",
            MediaStatus::NotInline => "not inline-renderable; use mailcap to open externally",
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
                " ▎ MEDIA • {} • {:?} ",
                Self::kind_label(self.media.kind),
                self.style
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

        match &mut self.status {
            MediaStatus::Ready(protocol) => {
                let widget = StatefulImage::default();
                frame.render_stateful_widget(widget, inner, protocol);
            }
            MediaStatus::Idle => {
                let lines = vec![
                    Line::from(Span::styled("Loading media…", self.theme.muted)),
                    Line::from(""),
                    Line::from(Span::raw(self.media.url.clone())),
                ];
                frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), inner);
            }
            MediaStatus::Failed(err) => {
                let lines = vec![
                    Line::from(Span::styled(
                        format!("Failed: {err}"),
                        self.theme.downvote,
                    )),
                    Line::from(""),
                    Line::from(Span::raw(self.media.url.clone())),
                ];
                frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), inner);
            }
            MediaStatus::NotInline => {
                let lines = vec![
                    Line::from(vec![
                        Span::styled("Kind ", self.theme.author),
                        Span::raw(Self::kind_label(self.media.kind)),
                    ]),
                    Line::from(vec![
                        Span::styled("URL  ", self.theme.author),
                        Span::raw(self.media.url.clone()),
                    ]),
                    Line::from(""),
                    Line::from(Span::styled(self.status_label(), self.theme.muted)),
                ];
                frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), inner);
            }
        }

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
    fn non_inline_kind_starts_in_not_inline_status() {
        let page = page_with(MediaKind::Video);
        assert!(matches!(page.status, MediaStatus::NotInline));
    }

    #[test]
    fn image_kind_starts_idle() {
        let page = page_with(MediaKind::Image);
        assert!(matches!(page.status, MediaStatus::Idle));
    }

    #[test]
    fn status_label_for_not_inline_mentions_mailcap() {
        let page = page_with(MediaKind::Gallery);
        assert!(page.status_label().contains("mailcap"));
    }
}
