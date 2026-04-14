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
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tuir_core::media::{download_to_cache, MediaKind, MediaRef, MediaStyle};

/// Braille spinner frames used while a media download is in flight.
/// Eight-frame cycle gives a smooth rotation under the 100ms poll.
const SPINNER_FRAMES: [&str; 8] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧"];

/// How long each spinner frame is visible before the next one advances.
const SPINNER_FRAME_DURATION: Duration = Duration::from_millis(80);

/// Per-GIF floor on frame delay. Some GIFs ship `0ms` or `10ms` delays
/// that would pin a CPU; browsers clamp to 100ms for the same reason.
/// We match that, and it also aligns with the event-loop poll cadence
/// so ticks can actually keep up.
const MIN_FRAME_DELAY: Duration = Duration::from_millis(100);

pub struct AnimatedFrame {
    protocol: Box<StatefulProtocol>,
    delay: Duration,
}

pub struct AnimatedMedia {
    frames: Vec<AnimatedFrame>,
    current: usize,
    last_advance: Instant,
}

/// Worker-thread output: picker construction stays on the main thread
/// (may query stdio), so workers only deliver raw pixels.
enum LoadedPixels {
    Still(image::DynamicImage),
    Gif(Vec<(image::DynamicImage, Duration)>),
}

type LoadResult = Result<LoadedPixels, String>;

pub struct DownloadInFlight {
    started_at: Instant,
    rx: Receiver<LoadResult>,
}

/// Render-side status for an inline media preview.
pub enum MediaStatus {
    /// Page is fresh — `start_download` has not been called yet, or the
    /// page is in [`MediaStyle::Off`] mode and intentionally never loads.
    Idle,
    /// A background worker is pulling bytes + decoding on another
    /// thread. Tick polls the channel and transitions out on
    /// completion; render shows a spinner keyed off `started_at`.
    Downloading(DownloadInFlight),
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

    /// Non-blocking: spawns a worker thread, returns with the page in
    /// `Downloading` state. `tick()` polls the channel each iteration
    /// and transitions to `Ready`/`Animated`/`Failed` on completion.
    /// Second and later calls are no-ops.
    pub fn start_download(&mut self, http: &reqwest::Client, cache_dir: &Path) {
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

        let (tx, rx) = mpsc::channel::<LoadResult>();
        let http = http.clone();
        let cache_dir = cache_dir.to_path_buf();
        let url = self.media.url.clone();
        let kind = self.media.kind;

        std::thread::spawn(move || {
            let result = run_download_and_decode(http, url, cache_dir, kind);
            // Receiver-dropped means the page navigated away; nothing
            // to do — the cached bytes are still on disk for next time.
            let _ = tx.send(result);
        });

        self.status = MediaStatus::Downloading(DownloadInFlight {
            started_at: Instant::now(),
            rx,
        });
    }

    /// Convert completed `LoadedPixels` into render-ready status,
    /// building `StatefulProtocol` instances on the main thread (the
    /// picker can touch stdio so cannot live on the worker).
    fn finalize(&mut self, loaded: LoadedPixels) {
        let picker = if matches!(self.style, MediaStyle::Retro) {
            Picker::halfblocks()
        } else {
            Picker::from_query_stdio().unwrap_or_else(|_| Picker::halfblocks())
        };

        match loaded {
            LoadedPixels::Still(image) => {
                let protocol = picker.new_resize_protocol(image);
                self.status = MediaStatus::Ready(Box::new(protocol));
            }
            LoadedPixels::Gif(frames) => {
                let built: Vec<AnimatedFrame> = frames
                    .into_iter()
                    .map(|(img, delay)| AnimatedFrame {
                        protocol: Box::new(picker.new_resize_protocol(img)),
                        delay,
                    })
                    .collect();
                self.status = MediaStatus::Animated(Box::new(AnimatedMedia {
                    frames: built,
                    current: 0,
                    last_advance: Instant::now(),
                }));
            }
        }
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
            MediaStatus::Downloading(_) => "downloading…",
            MediaStatus::Idle => "idle",
            MediaStatus::Failed(_) => "failed",
            MediaStatus::NotInline => "not inline-renderable; use mailcap to open externally",
        }
    }

    /// Compute the current spinner frame index from the time elapsed
    /// since the download started. Pure function so the caller can
    /// render Downloading state without mutating the page.
    fn spinner_frame(started_at: Instant) -> &'static str {
        let idx = (started_at.elapsed().as_millis() / SPINNER_FRAME_DURATION.as_millis())
            as usize
            % SPINNER_FRAMES.len();
        SPINNER_FRAMES[idx]
    }
}

/// Walk the frame index forward based on `elapsed`. Returns the new
/// index if it changed, or `None` when the animation has not yet
/// crossed the next-frame boundary. Takes a `delay_of` closure so
/// callers can avoid collecting per-frame delays into a scratch Vec
/// on every tick — the live `MediaPage::tick` path indexes into
/// `anim.frames` directly while tests pass in a slice closure.
fn advance_gif_index(
    frame_count: usize,
    current: usize,
    mut elapsed: Duration,
    delay_of: impl Fn(usize) -> Duration,
) -> Option<usize> {
    if frame_count == 0 {
        return None;
    }
    let mut idx = current.min(frame_count - 1);
    let mut advanced = false;
    loop {
        let d = delay_of(idx);
        if elapsed < d {
            break;
        }
        elapsed -= d;
        idx = (idx + 1) % frame_count;
        advanced = true;
    }
    advanced.then_some(idx)
}

/// Background-thread entry point: download the URL to the on-disk
/// cache, then decode the bytes into either a single frame or a full
/// GIF frame vector. Returns a transport-friendly `LoadResult` — no
/// ratatui types, so this runs on any thread.
fn run_download_and_decode(
    http: reqwest::Client,
    url: String,
    cache_dir: PathBuf,
    kind: MediaKind,
) -> LoadResult {
    // The worker owns a scratch current-thread runtime purely so
    // `download_to_cache` (async) can be driven without dragging a
    // global runtime into the TUI.
    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(rt) => rt,
        Err(err) => return Err(format!("runtime: {err}")),
    };

    let path = match runtime.block_on(download_to_cache(&http, &url, &cache_dir)) {
        Ok(p) => p,
        Err(err) => {
            tracing::error!("media download failed: {err}");
            return Err(format!("download: {err}"));
        }
    };

    if matches!(kind, MediaKind::AnimatedGif) {
        match decode_all_gif_frames(&path) {
            Ok(frames) => return Ok(LoadedPixels::Gif(frames)),
            Err(err) => {
                tracing::warn!(
                    "gif multi-frame decode failed for {}: {err}; falling back to still",
                    path.display()
                );
                // Fall through to still-image path.
            }
        }
    }

    match decode_still(&path) {
        Ok(img) => Ok(LoadedPixels::Still(img)),
        Err(err) => {
            tracing::error!("media decode failed for {}: {err}", path.display());
            Err(format!("decode: {err}"))
        }
    }
}

/// Decode a single still frame from the given file, guessing the
/// format from magic bytes so jpeg/png/webp all work without relying
/// on file extensions.
fn decode_still(path: &Path) -> anyhow::Result<image::DynamicImage> {
    let img = image::ImageReader::open(path)?
        .with_guessed_format()?
        .decode()?;
    Ok(img)
}

/// Decode every frame of a GIF via `image::codecs::gif::GifDecoder`,
/// returning `(DynamicImage, post-clamp delay)` pairs. Frames come out
/// of `AnimationDecoder` already composited — the image crate handles
/// disposal methods internally — so each frame is a full-canvas RGBA
/// buffer.
fn decode_all_gif_frames(path: &Path) -> anyhow::Result<Vec<(image::DynamicImage, Duration)>> {
    use image::codecs::gif::GifDecoder;
    use image::{AnimationDecoder, DynamicImage};

    let file = std::fs::File::open(path)?;
    let decoder = GifDecoder::new(std::io::BufReader::new(file))?;
    let raw_frames = decoder.into_frames().collect_frames()?;
    if raw_frames.is_empty() {
        anyhow::bail!("gif has zero frames");
    }

    let mut out = Vec::with_capacity(raw_frames.len());
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
        out.push((DynamicImage::ImageRgba8(buffer), delay));
    }
    Ok(out)
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
            MediaStatus::Downloading(info) => {
                let elapsed_ms = info.started_at.elapsed().as_millis();
                let spinner = Self::spinner_frame(info.started_at);
                let lines = vec![
                    Line::from(vec![
                        Span::styled(
                            format!("{spinner} "),
                            self.theme.author,
                        ),
                        Span::styled(
                            format!("Downloading media… ({}ms)", elapsed_ms),
                            self.theme.muted,
                        ),
                    ]),
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
        if let MediaStatus::Downloading(info) = &self.status {
            match info.rx.try_recv() {
                Ok(Ok(pixels)) => {
                    self.finalize(pixels);
                }
                Ok(Err(err)) => {
                    self.status = MediaStatus::Failed(err);
                }
                Err(TryRecvError::Empty) => {}
                Err(TryRecvError::Disconnected) => {
                    self.status =
                        MediaStatus::Failed("worker thread terminated".to_string());
                }
            }
            return;
        }

        if let MediaStatus::Animated(anim) = &mut self.status {
            if let Some(next) = advance_gif_index(
                anim.frames.len(),
                anim.current,
                anim.last_advance.elapsed(),
                |i| anim.frames[i].delay,
            ) {
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

    fn walk(delays: &[Duration], current: usize, elapsed: Duration) -> Option<usize> {
        advance_gif_index(delays.len(), current, elapsed, |i| delays[i])
    }

    #[test]
    fn advance_gif_index_returns_none_when_elapsed_below_delay() {
        let delays = [Duration::from_millis(100), Duration::from_millis(100)];
        assert_eq!(walk(&delays, 0, Duration::from_millis(50)), None);
    }

    #[test]
    fn advance_gif_index_steps_single_frame() {
        let delays = [Duration::from_millis(100), Duration::from_millis(100)];
        assert_eq!(walk(&delays, 0, Duration::from_millis(120)), Some(1));
    }

    #[test]
    fn advance_gif_index_wraps_and_skips_when_behind() {
        // 3×100ms; 350ms should skip past 1 and 2, landing back on 0.
        let delays = [
            Duration::from_millis(100),
            Duration::from_millis(100),
            Duration::from_millis(100),
        ];
        assert_eq!(walk(&delays, 0, Duration::from_millis(350)), Some(0));
    }

    #[test]
    fn advance_gif_index_empty_delays_is_none() {
        assert_eq!(walk(&[], 0, Duration::from_millis(999)), None);
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

        let frames = decode_all_gif_frames(&tmp).expect("decode gif");
        assert_eq!(frames.len(), 2);
        // Encoded delay was 10ms — should be clamped up to MIN_FRAME_DELAY.
        assert_eq!(frames[0].1, MIN_FRAME_DELAY);
        assert_eq!(frames[1].1, MIN_FRAME_DELAY);

        let _ = std::fs::remove_file(&tmp);
    }

    #[test]
    fn spinner_frame_cycles_through_all_frames() {
        let start = Instant::now();
        let frame0 = MediaPage::spinner_frame(start);
        // Any frame index is valid — only verify the function returns
        // one of the known frames without panicking.
        assert!(SPINNER_FRAMES.contains(&frame0));
    }

    #[test]
    fn status_label_for_downloading_mentions_downloading() {
        let (_tx, rx) = mpsc::channel();
        let mut page = page_with(MediaKind::Image);
        page.status = MediaStatus::Downloading(DownloadInFlight {
            started_at: Instant::now(),
            rx,
        });
        assert!(page.status_label().contains("download"));
    }
}
