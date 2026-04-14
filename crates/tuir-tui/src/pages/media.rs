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
use ratatui_image::picker::Picker;
use ratatui_image::protocol::StatefulProtocol;
use ratatui_image::StatefulImage;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tuir_core::media::{download_to_cache, MediaKind, MediaRef, MediaStyle};

/// Per-GIF floor on frame delay. Some GIFs ship `0ms` or `10ms` delays
/// that would pin a CPU; browsers clamp to 100ms for the same reason.
/// We match that, and it also aligns with the event-loop poll cadence
/// so ticks can actually keep up.
const MIN_FRAME_DELAY: Duration = Duration::from_millis(100);

/// One pre-built frame of an animated GIF: the stateful protocol that
/// feeds ratatui-image plus the post-clamp delay until the next frame.
pub struct AnimatedFrame {
    pub protocol: Box<StatefulProtocol>,
    pub delay: Duration,
}

/// State for an animated GIF preview. Frames are pre-built once and
/// swapped in on `tick` based on wall-clock elapsed time.
pub struct AnimatedMedia {
    pub frames: Vec<AnimatedFrame>,
    pub current: usize,
    pub last_advance: Instant,
}

/// Render-side status for an inline media preview.
pub enum MediaStatus {
    /// Page is fresh — `ensure_loaded` has not been called yet, or the
    /// page is in [`MediaStyle::Off`] mode and intentionally never loads.
    Idle,
    /// The download/decode pipeline succeeded; we hold a stateful
    /// protocol the renderer feeds frames to on every draw. Boxed so
    /// the enum variants stay close in size — `StatefulProtocol` is
    /// noticeably larger than the other variants on ratatui-image v10.
    Ready(Box<StatefulProtocol>),
    /// Pipeline succeeded on an animated GIF: we hold the full frame
    /// vector and advance through it on `tick`.
    Animated(Box<AnimatedMedia>),
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

        // Picker selection:
        //   Retro  → always use the explicit halfblocks constructor so
        //            even kitty/iTerm2/sixel terminals get the
        //            deliberate browsh aesthetic.
        //   Auto   → query the terminal for its best protocol; fall
        //            back to halfblocks on headless / non-tty.
        let picker = if matches!(self.style, MediaStyle::Retro) {
            Picker::halfblocks()
        } else {
            Picker::from_query_stdio().unwrap_or_else(|_| Picker::halfblocks())
        };

        if matches!(self.media.kind, MediaKind::AnimatedGif) {
            match load_animated_gif(&path, &picker) {
                Ok(anim) => self.status = MediaStatus::Animated(Box::new(anim)),
                Err(err) => {
                    tracing::warn!(
                        "gif multi-frame decode failed for {}: {err}; falling back to static first frame",
                        path.display()
                    );
                    // Fall through to still-image path so the user still
                    // sees something instead of a hard error.
                    self.load_still(&path, &picker);
                }
            }
            return;
        }

        self.load_still(&path, &picker);
    }

    /// Single-frame decode path used for static images and as the GIF
    /// fallback. Sets `status` to `Ready` or `Failed`.
    fn load_still(&mut self, path: &std::path::Path, picker: &Picker) {
        let dyn_image = match image::ImageReader::open(path)
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
        let protocol = picker.new_resize_protocol(dyn_image);
        self.status = MediaStatus::Ready(Box::new(protocol));
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

    fn status_label(&self) -> &'static str {
        match self.status {
            MediaStatus::Ready(_) => "rendering inline",
            MediaStatus::Animated(_) => "animating inline",
            MediaStatus::Idle => "loading…",
            MediaStatus::Failed(_) => "failed",
            MediaStatus::NotInline => "not inline-renderable; use mailcap to open externally",
        }
    }
}

/// Walk the frame index forward based on `elapsed`, consuming per-frame
/// delays. Returns the new index if it changed, or `None` when the
/// animation has not yet crossed the next-frame boundary. Factored out
/// of `MediaPage::tick` so it is testable without building real
/// ratatui-image `StatefulProtocol` instances.
fn advance_gif_index(delays: &[Duration], current: usize, mut elapsed: Duration) -> Option<usize> {
    if delays.is_empty() {
        return None;
    }
    let mut idx = current.min(delays.len() - 1);
    let mut advanced = false;
    loop {
        let d = delays[idx];
        if elapsed < d {
            break;
        }
        elapsed -= d;
        idx = (idx + 1) % delays.len();
        advanced = true;
    }
    advanced.then_some(idx)
}

/// Load every frame of a GIF via `image::codecs::gif::GifDecoder`, then
/// pre-build a `StatefulProtocol` per frame so `tick` is a pointer
/// swap instead of a decode.
///
/// Frames come out of `AnimationDecoder` already composited — the
/// image crate handles disposal methods internally — so each frame is
/// a full-canvas RGBA buffer we can hand straight to the picker.
fn load_animated_gif(
    path: &std::path::Path,
    picker: &Picker,
) -> anyhow::Result<AnimatedMedia> {
    use image::codecs::gif::GifDecoder;
    use image::{AnimationDecoder, DynamicImage};

    let file = std::fs::File::open(path)?;
    let decoder = GifDecoder::new(std::io::BufReader::new(file))?;
    let raw_frames = decoder.into_frames().collect_frames()?;
    if raw_frames.is_empty() {
        anyhow::bail!("gif has zero frames");
    }

    let mut frames: Vec<AnimatedFrame> = Vec::with_capacity(raw_frames.len());
    for frame in raw_frames {
        let delay_ms = {
            let (numer, denom) = frame.delay().numer_denom_ms();
            if denom == 0 {
                100
            } else {
                numer / denom
            }
        };
        let delay = Duration::from_millis(delay_ms as u64).max(MIN_FRAME_DELAY);
        let buffer = frame.into_buffer();
        let dyn_image = DynamicImage::ImageRgba8(buffer);
        let protocol = picker.new_resize_protocol(dyn_image);
        frames.push(AnimatedFrame {
            protocol: Box::new(protocol),
            delay,
        });
    }

    Ok(AnimatedMedia {
        frames,
        current: 0,
        last_advance: Instant::now(),
    })
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
                frame.render_stateful_widget(widget, inner, protocol.as_mut());
            }
            MediaStatus::Animated(anim) => {
                let idx = anim.current.min(anim.frames.len().saturating_sub(1));
                let current_frame = &mut anim.frames[idx];
                let widget = StatefulImage::default();
                frame.render_stateful_widget(widget, inner, current_frame.protocol.as_mut());
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
            .title(" o:External viewer | ?:Help | q:Back ")
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
                // Open / o → hand the URL to the external mailcap viewer
                // regardless of whether the inline renderer succeeded.
                // Useful for galleries, videos, and "I want this in my
                // real image viewer" cases.
                KeyAction::Open => PageAction::OpenExternal(self.media.url.clone()),
                _ => PageAction::None,
            };
        }
        PageAction::None
    }

    fn title(&self) -> &str {
        "media"
    }

    /// Advance the animated-GIF frame cursor if enough wall-clock time
    /// has elapsed since the last advance. Walks multiple frames in
    /// one call when the loop is behind (e.g. a slow redraw held us up
    /// past several 100ms slots) so the animation's speed reflects
    /// real time rather than tick count.
    fn tick(&mut self) {
        if let MediaStatus::Animated(anim) = &mut self.status {
            let delays: Vec<Duration> = anim.frames.iter().map(|f| f.delay).collect();
            if let Some(next) =
                advance_gif_index(&delays, anim.current, anim.last_advance.elapsed())
            {
                anim.current = next;
                anim.last_advance = Instant::now();
            }
        }
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

    #[test]
    fn advance_gif_index_returns_none_when_elapsed_below_delay() {
        let delays = vec![Duration::from_millis(100), Duration::from_millis(100)];
        assert_eq!(advance_gif_index(&delays, 0, Duration::from_millis(50)), None);
    }

    #[test]
    fn advance_gif_index_steps_single_frame() {
        let delays = vec![Duration::from_millis(100), Duration::from_millis(100)];
        assert_eq!(
            advance_gif_index(&delays, 0, Duration::from_millis(120)),
            Some(1)
        );
    }

    #[test]
    fn advance_gif_index_wraps_and_skips_when_behind() {
        // Three frames of 100ms each; 350ms elapsed from index 0 should
        // skip past frames 1 and 2 and land back on 0 with leftover.
        let delays = vec![
            Duration::from_millis(100),
            Duration::from_millis(100),
            Duration::from_millis(100),
        ];
        assert_eq!(
            advance_gif_index(&delays, 0, Duration::from_millis(350)),
            Some(0)
        );
    }

    #[test]
    fn advance_gif_index_empty_delays_is_none() {
        assert_eq!(advance_gif_index(&[], 0, Duration::from_millis(999)), None);
    }

    /// End-to-end: build a 2-frame GIF in a tempfile via `image`'s
    /// `GifEncoder`, decode it through `load_animated_gif`, and assert
    /// the frame count and clamped delays. Verifies the whole pipeline
    /// without reaching the network.
    #[test]
    fn load_animated_gif_decodes_two_frame_synthetic_gif() {
        use image::codecs::gif::GifEncoder;
        use image::{Delay, Frame, ImageBuffer, Rgba};

        let tmp = std::env::temp_dir().join(format!(
            "tuir-gif-test-{}.gif",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&tmp);

        // Two distinct solid 4×4 frames, 10ms each (below the 100ms
        // floor so we also exercise the clamp).
        let red = ImageBuffer::<Rgba<u8>, Vec<u8>>::from_fn(4, 4, |_, _| Rgba([255, 0, 0, 255]));
        let blue = ImageBuffer::<Rgba<u8>, Vec<u8>>::from_fn(4, 4, |_, _| Rgba([0, 0, 255, 255]));
        let delay = Delay::from_numer_denom_ms(10, 1);
        let frames = vec![
            Frame::from_parts(red, 0, 0, delay),
            Frame::from_parts(blue, 0, 0, delay),
        ];

        {
            let file = std::fs::File::create(&tmp).expect("create tmp gif");
            let mut encoder = GifEncoder::new(file);
            encoder.encode_frames(frames).expect("encode gif");
        }

        let picker = Picker::halfblocks();
        let anim = load_animated_gif(&tmp, &picker).expect("decode gif");
        assert_eq!(anim.frames.len(), 2);
        // Encoded delay was 10ms — should be clamped up to MIN_FRAME_DELAY.
        assert_eq!(anim.frames[0].delay, MIN_FRAME_DELAY);
        assert_eq!(anim.frames[1].delay, MIN_FRAME_DELAY);

        let _ = std::fs::remove_file(&tmp);
    }
}
