//! Media URL classification.
//!
//! Reddit submissions point at all kinds of external resources — direct
//! image hosts (i.redd.it, i.imgur.com), preview CDNs (preview.redd.it
//! with signed query strings), animated content (gifv, gif), video
//! (v.redd.it), gallery/multi-image posts, and plain web links. Before
//! the TUI tries to fetch and render anything it asks this module
//! "what kind of thing is this URL pointing at?" so it can decide
//! whether to:
//!
//! - render inline via the `image` + `ratatui-image` pipeline (still or
//!   first-frame image)
//! - hand off to mailcap / system viewer (video, animated gif when the
//!   user opted out of inline animation, gallery)
//! - just treat it as a regular link the user can copy / open in a
//!   browser
//!
//! The classification only inspects the URL string. Network probing
//! (HEAD request, content-type sniffing) lives in the future media
//! download layer; keeping this module pure means it stays trivially
//! testable.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

/// What sort of media a Reddit submission is pointing at.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MediaKind {
    /// Static image we can decode and render inline (jpg/png/webp).
    Image,
    /// Animated GIF — first frame is renderable inline; full animation
    /// requires the external viewer path.
    AnimatedGif,
    /// Video stream (v.redd.it, *.mp4, *.webm). Inline rendering is out
    /// of scope; mailcap / external viewer handles it.
    Video,
    /// Multi-image Reddit gallery. Needs API expansion to enumerate
    /// children — out of scope for the first inline preview pass.
    Gallery,
    /// Looks like a normal external web link (article, GitHub repo, …).
    External,
}

/// A classified media reference attached to a submission.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediaRef {
    pub kind: MediaKind,
    pub url: String,
}

impl MediaRef {
    pub fn new(kind: MediaKind, url: impl Into<String>) -> Self {
        Self {
            kind,
            url: url.into(),
        }
    }

    /// Whether the inline-image renderer (image + ratatui-image) can
    /// actually display this ref. Animated gifs qualify because we render
    /// the first frame; videos and galleries do not.
    pub fn is_inline_renderable(&self) -> bool {
        matches!(self.kind, MediaKind::Image | MediaKind::AnimatedGif)
    }
}

/// Inspect a Reddit submission URL and classify the resource.
///
/// Returns `None` when the URL is empty or otherwise unrecognizable so
/// callers can short-circuit instead of having to compare against a
/// catch-all variant.
pub fn detect_media(url: &str) -> Option<MediaRef> {
    let trimmed = url.trim();
    if trimmed.is_empty() {
        return None;
    }

    let lowered = trimmed.to_lowercase();
    // Drop any query string before extension matching so signed CDN
    // URLs like preview.redd.it/abc.jpg?width=640&s=xyz still resolve.
    let path_only = match lowered.split_once('?') {
        Some((before, _)) => before.to_string(),
        None => lowered.clone(),
    };

    // ── Reddit-hosted video ────────────────────────────────────
    if host_contains(&lowered, "v.redd.it") {
        return Some(MediaRef::new(MediaKind::Video, trimmed));
    }

    // ── Reddit galleries ───────────────────────────────────────
    if path_only.contains("/gallery/") {
        return Some(MediaRef::new(MediaKind::Gallery, trimmed));
    }

    // ── Imgur quirks ───────────────────────────────────────────
    // imgur .gifv is browser-only (HTML5 video); rewrite to .mp4 and
    // classify as video. The mailcap viewer can take it from there.
    if host_contains(&lowered, "imgur.com") && path_only.ends_with(".gifv") {
        let mp4 = lowered.trim_end_matches(".gifv").to_string() + ".mp4";
        return Some(MediaRef::new(MediaKind::Video, mp4));
    }
    // imgur album pages are galleries.
    if host_contains(&lowered, "imgur.com")
        && (path_only.contains("/a/") || path_only.contains("/gallery/"))
    {
        return Some(MediaRef::new(MediaKind::Gallery, trimmed));
    }

    // ── Animated GIF ───────────────────────────────────────────
    if path_only.ends_with(".gif") {
        return Some(MediaRef::new(MediaKind::AnimatedGif, trimmed));
    }

    // ── Static image extensions ────────────────────────────────
    const IMAGE_EXTS: &[&str] = &[".jpg", ".jpeg", ".png", ".webp", ".bmp"];
    if IMAGE_EXTS.iter().any(|ext| path_only.ends_with(ext)) {
        return Some(MediaRef::new(MediaKind::Image, trimmed));
    }

    // ── Image hosts without extensions ─────────────────────────
    // i.redd.it and i.imgur.com sometimes serve URLs without a file
    // extension when the user landed on a thumbnail/page. Still treat
    // them as images and let the decoder figure out the type from the
    // payload.
    if host_contains(&lowered, "i.redd.it")
        || host_contains(&lowered, "preview.redd.it")
        || host_contains(&lowered, "i.imgur.com")
    {
        return Some(MediaRef::new(MediaKind::Image, trimmed));
    }

    // ── Video extensions ───────────────────────────────────────
    const VIDEO_EXTS: &[&str] = &[".mp4", ".webm", ".mov", ".mkv"];
    if VIDEO_EXTS.iter().any(|ext| path_only.ends_with(ext)) {
        return Some(MediaRef::new(MediaKind::Video, trimmed));
    }

    // ── Anything else with a scheme is just a web link ─────────
    if lowered.starts_with("http://") || lowered.starts_with("https://") {
        return Some(MediaRef::new(MediaKind::External, trimmed));
    }

    None
}

fn host_contains(lowered_url: &str, needle: &str) -> bool {
    // We do not parse the URL; checking the substring is sufficient
    // because the host appears between "://" and the next "/".
    if let Some(after_scheme) = lowered_url.split_once("://").map(|(_, rest)| rest) {
        let host = after_scheme.split('/').next().unwrap_or("");
        return host.contains(needle);
    }
    false
}

/// Map a URL to a deterministic on-disk cache filename.
///
/// `<cache_dir>/<sha256-of-url>.bin` — extension is intentionally `.bin`
/// rather than the source URL's extension because the decoder sniffs the
/// payload type from the magic bytes, and we don't want a `.gif` URL
/// that turned out to be a 404 HTML page to look like a gif on disk.
pub fn cache_path_for(cache_dir: &Path, url: &str) -> PathBuf {
    let mut hasher = Sha256::new();
    hasher.update(url.as_bytes());
    let digest = hasher.finalize();
    cache_dir.join(format!("{}.bin", hex::encode(digest)))
}

/// Download a media URL to the on-disk cache.
///
/// Returns the path to the cached file. If the file is already present
/// (a previous successful download) the network round-trip is skipped
/// entirely, so repeat previews of the same image are instant. Creates
/// the cache directory on demand.
pub async fn download_to_cache(
    http: &reqwest::Client,
    url: &str,
    cache_dir: &Path,
) -> Result<PathBuf> {
    let path = cache_path_for(cache_dir, url);
    if path.exists() {
        return Ok(path);
    }

    std::fs::create_dir_all(cache_dir)
        .with_context(|| format!("create media cache dir {}", cache_dir.display()))?;

    let response = http
        .get(url)
        .send()
        .await
        .with_context(|| format!("GET {url}"))?
        .error_for_status()
        .with_context(|| format!("media URL {url} returned non-2xx"))?;

    let bytes = response
        .bytes()
        .await
        .with_context(|| format!("read body for {url}"))?;

    std::fs::write(&path, &bytes)
        .with_context(|| format!("write cache file {}", path.display()))?;

    Ok(path)
}

/// How the TUI should render media when it knows how to.
///
/// `Auto` uses the best protocol available in the user's terminal
/// (kitty, iTerm2, sixel, half-blocks). `Retro` deliberately forces the
/// half-block pipeline even on capable terminals — it's a stylistic
/// opt-in that channels browsh / 90s-era screen captures. `Off`
/// suppresses inline rendering entirely; submissions just show the URL.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum MediaStyle {
    #[default]
    Auto,
    Retro,
    Off,
}

impl MediaStyle {
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_lowercase().as_str() {
            "auto" | "" => Some(Self::Auto),
            "retro" | "halfblocks" | "halfblock" => Some(Self::Retro),
            "off" | "none" | "false" | "0" => Some(Self::Off),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn classify(url: &str) -> MediaKind {
        detect_media(url).expect("expected a classification").kind
    }

    #[test]
    fn detect_returns_none_for_empty_url() {
        assert!(detect_media("").is_none());
        assert!(detect_media("   ").is_none());
    }

    #[test]
    fn classifies_direct_jpeg_links() {
        assert_eq!(classify("https://i.redd.it/abcdef.jpg"), MediaKind::Image);
        assert_eq!(
            classify("https://i.imgur.com/abc.jpeg"),
            MediaKind::Image
        );
        assert_eq!(classify("https://example.com/foo.PNG"), MediaKind::Image);
    }

    #[test]
    fn classifies_preview_redd_it_with_query_string() {
        let url = "https://preview.redd.it/abc.jpg?width=640&format=pjpg&s=xyz";
        assert_eq!(classify(url), MediaKind::Image);
    }

    #[test]
    fn classifies_animated_gif_separately_from_static() {
        assert_eq!(classify("https://i.redd.it/x.gif"), MediaKind::AnimatedGif);
        assert_eq!(classify("https://i.redd.it/x.png"), MediaKind::Image);
    }

    #[test]
    fn classifies_v_redd_it_as_video() {
        assert_eq!(
            classify("https://v.redd.it/abcdef/HLSPlaylist.m3u8"),
            MediaKind::Video
        );
    }

    #[test]
    fn classifies_imgur_gifv_as_video_and_rewrites_to_mp4() {
        let media = detect_media("https://i.imgur.com/abcdef.gifv").unwrap();
        assert_eq!(media.kind, MediaKind::Video);
        assert!(media.url.ends_with(".mp4"));
    }

    #[test]
    fn classifies_imgur_album_as_gallery() {
        assert_eq!(
            classify("https://imgur.com/a/AbCdEf"),
            MediaKind::Gallery
        );
        assert_eq!(
            classify("https://imgur.com/gallery/XyZ"),
            MediaKind::Gallery
        );
    }

    #[test]
    fn classifies_reddit_gallery_path() {
        assert_eq!(
            classify("https://www.reddit.com/gallery/abc123"),
            MediaKind::Gallery
        );
    }

    #[test]
    fn classifies_extensionless_image_host() {
        assert_eq!(
            classify("https://i.redd.it/long-id-without-ext"),
            MediaKind::Image
        );
    }

    #[test]
    fn classifies_video_extensions() {
        assert_eq!(
            classify("https://example.com/clip.mp4"),
            MediaKind::Video
        );
        assert_eq!(
            classify("https://example.com/clip.webm"),
            MediaKind::Video
        );
    }

    #[test]
    fn falls_back_to_external_for_unrecognized_https_link() {
        assert_eq!(
            classify("https://github.com/rust-lang/rust"),
            MediaKind::External
        );
    }

    #[test]
    fn is_inline_renderable_only_for_images_and_gifs() {
        assert!(MediaRef::new(MediaKind::Image, "x").is_inline_renderable());
        assert!(MediaRef::new(MediaKind::AnimatedGif, "x").is_inline_renderable());
        assert!(!MediaRef::new(MediaKind::Video, "x").is_inline_renderable());
        assert!(!MediaRef::new(MediaKind::Gallery, "x").is_inline_renderable());
        assert!(!MediaRef::new(MediaKind::External, "x").is_inline_renderable());
    }

    #[test]
    fn media_style_parses_known_aliases() {
        assert_eq!(MediaStyle::parse("auto"), Some(MediaStyle::Auto));
        assert_eq!(MediaStyle::parse(""), Some(MediaStyle::Auto));
        assert_eq!(MediaStyle::parse("retro"), Some(MediaStyle::Retro));
        assert_eq!(MediaStyle::parse("halfblocks"), Some(MediaStyle::Retro));
        assert_eq!(MediaStyle::parse("off"), Some(MediaStyle::Off));
        assert_eq!(MediaStyle::parse("none"), Some(MediaStyle::Off));
    }

    #[test]
    fn media_style_rejects_unknown_value() {
        assert_eq!(MediaStyle::parse("kitty"), None);
        assert_eq!(MediaStyle::parse("crt"), None);
    }

    #[test]
    fn cache_path_is_deterministic_per_url() {
        let dir = std::path::PathBuf::from("/tmp/tuir/cache");
        let a = cache_path_for(&dir, "https://i.redd.it/x.jpg");
        let b = cache_path_for(&dir, "https://i.redd.it/x.jpg");
        let c = cache_path_for(&dir, "https://i.redd.it/y.jpg");
        assert_eq!(a, b);
        assert_ne!(a, c);
        assert!(a.starts_with(&dir));
        assert_eq!(a.extension().unwrap(), "bin");
    }
}
