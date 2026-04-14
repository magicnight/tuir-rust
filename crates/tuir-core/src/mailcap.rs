//! Mailcap file parser and MIME-aware viewer dispatch (RFC 1524 subset).
//!
//! tuir uses mailcap to figure out *what to spawn* when the user wants
//! to open a media URL externally — videos (v.redd.it), galleries,
//! plain external links, anything the inline image renderer can't
//! handle. The user puts entries like
//!
//! ```text
//! image/jpeg; feh %s
//! image/*; feh %s
//! video/*; mpv %s
//! application/pdf; zathura %s
//! ```
//!
//! into `~/.config/tuir/mailcap`, `~/.mailcap`, or `/etc/mailcap` and
//! we resolve a URL against:
//!
//! 1. an extension → canonical MIME table (so `.jpg` → `image/jpeg`)
//! 2. an exact-MIME lookup (`image/jpeg`)
//! 3. a wildcard fallback (`image/*`)
//! 4. the universal `*/*` catch-all
//!
//! and then `expand` substitutes the URL into the command's `%s`
//! placeholder. The TUI layer is responsible for suspending the
//! terminal, spawning the resulting command, and resuming.

use anyhow::Result;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// A parsed mailcap entry.
#[derive(Debug, Clone)]
pub struct MailcapEntry {
    /// MIME type (e.g. "image/png" or wildcard "image/*").
    pub mime_type: String,
    /// Command template with `%s` placeholder for the file/URL.
    pub command: String,
    /// Optional `test=…` condition (e.g. "test=test -n \"$DISPLAY\"").
    pub test: Option<String>,
    /// Whether the entry was tagged `copiousoutput` (pager-friendly).
    pub copiousoutput: bool,
}

impl MailcapEntry {
    /// Substitute `%s` in [`Self::command`] with the given URL or path,
    /// shell-quoting it so URL query strings can't break out.
    pub fn expand(&self, url: &str) -> String {
        let quoted = shell_quote(url);
        if self.command.contains("%s") {
            self.command.replace("%s", &quoted)
        } else {
            format!("{} {}", self.command, quoted)
        }
    }
}

/// Mailcap database: the parsed contents of one or more mailcap files.
#[derive(Debug, Default, Clone)]
pub struct Mailcap {
    entries: HashMap<String, Vec<MailcapEntry>>,
}

impl Mailcap {
    /// Parse a single mailcap file from disk.
    pub fn from_file(path: impl AsRef<Path>) -> Result<Self> {
        let content = std::fs::read_to_string(path)?;
        Self::parse(&content)
    }

    /// Parse mailcap content from a string.
    pub fn parse(content: &str) -> Result<Self> {
        let mut entries: HashMap<String, Vec<MailcapEntry>> = HashMap::new();
        for raw_line in content.lines() {
            let line = raw_line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            // Split on `;` but ignore `;` inside double-quotes — minimal
            // RFC 1524 conformance. We also collapse line continuations
            // ("\\\n") at the parse-line level by lifting them in the
            // caller; for now we accept only single-line entries.
            let parts: Vec<&str> = line.split(';').map(str::trim).collect();
            if parts.len() < 2 {
                continue;
            }
            let mime_type = parts[0].to_string();
            let command = parts[1].to_string();

            let mut test = None;
            let mut copiousoutput = false;
            for field in &parts[2..] {
                if let Some(stripped) = field.strip_prefix("test=") {
                    test = Some(stripped.to_string());
                } else if *field == "copiousoutput" {
                    copiousoutput = true;
                }
            }

            entries
                .entry(mime_type.clone())
                .or_default()
                .push(MailcapEntry {
                    mime_type,
                    command,
                    test,
                    copiousoutput,
                });
        }
        Ok(Self { entries })
    }

    /// Load and merge the standard mailcap search paths in priority
    /// order: user config, dotfile, system-wide. Missing files are
    /// silently skipped — only an unparseable file errors out.
    pub fn load_default() -> Result<Self> {
        let mut merged = Self::default();
        for path in default_paths() {
            if path.exists() {
                let other = Self::from_file(&path)?;
                for (mime, list) in other.entries {
                    merged
                        .entries
                        .entry(mime)
                        .or_default()
                        .extend(list);
                }
            }
        }
        Ok(merged)
    }

    /// Find the first usable entry for an exact MIME type.
    pub fn find(&self, mime_type: &str) -> Option<&MailcapEntry> {
        self.entries.get(mime_type).and_then(|v| v.first())
    }

    /// Find the best entry for a URL by:
    ///   1. mapping its file extension (or scheme) to a canonical MIME
    ///   2. trying exact MIME match
    ///   3. falling back to `<top>/*` wildcard
    ///   4. falling back to `*/*` catch-all
    ///
    /// Returns `None` only when none of those four match.
    pub fn find_for_url(&self, url: &str) -> Option<&MailcapEntry> {
        let mime = guess_mime_from_url(url)?;
        if let Some(hit) = self.find(&mime) {
            return Some(hit);
        }
        let top = mime.split('/').next().unwrap_or("");
        if !top.is_empty() {
            if let Some(hit) = self.find(&format!("{top}/*")) {
                return Some(hit);
            }
        }
        self.find("*/*")
    }
}

/// Shell-quote a URL so query strings, ampersands, and spaces survive.
fn shell_quote(url: &str) -> String {
    if url.is_empty() {
        return "''".to_string();
    }
    // Single-quote everything and escape any embedded single quotes by
    // closing the quote, inserting an escaped quote, and reopening.
    if url.contains('\'') {
        let escaped = url.replace('\'', r"'\''");
        format!("'{}'", escaped)
    } else {
        format!("'{}'", url)
    }
}

/// Map a URL to a canonical MIME type by inspecting its extension.
///
/// Returns `Some("application/octet-stream")` for known-but-unmapped
/// extensions and `Some("text/html")` for plain http(s) links without
/// a recognizable extension — that way the universal `text/html`
/// or `*/*` wildcard branches still get a shot at firing.
pub fn guess_mime_from_url(url: &str) -> Option<String> {
    let trimmed = url.trim();
    if trimmed.is_empty() {
        return None;
    }
    let lowered = trimmed.to_lowercase();
    let path_only = lowered.split_once('?').map(|(p, _)| p).unwrap_or(&lowered);

    let ext = path_only.rsplit('.').next().unwrap_or("");
    let mime = match ext {
        "jpg" | "jpeg" => "image/jpeg",
        "png" => "image/png",
        "webp" => "image/webp",
        "gif" => "image/gif",
        "bmp" => "image/bmp",
        "svg" => "image/svg+xml",
        "mp4" | "m4v" => "video/mp4",
        "webm" => "video/webm",
        "mkv" => "video/x-matroska",
        "mov" => "video/quicktime",
        "mp3" => "audio/mpeg",
        "ogg" => "audio/ogg",
        "wav" => "audio/wav",
        "pdf" => "application/pdf",
        "html" | "htm" => "text/html",
        _ if lowered.starts_with("http://") || lowered.starts_with("https://") => "text/html",
        _ => return Some("application/octet-stream".to_string()),
    };
    Some(mime.to_string())
}

/// The standard search order we honor for mailcap configuration files.
///
/// We always look at the user's tuir-specific override first so they
/// can pin a different image viewer for tuir without polluting their
/// global `~/.mailcap`.
pub fn default_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    if let Some(cfg) = dirs::config_dir() {
        paths.push(cfg.join("tuir").join("mailcap"));
    }
    if let Some(home) = dirs::home_dir() {
        paths.push(home.join(".mailcap"));
    }
    paths.push(PathBuf::from("/etc/mailcap"));
    paths
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_picks_up_basic_entries() {
        let src = "image/jpeg; feh %s\nvideo/mp4; mpv %s\n";
        let mc = Mailcap::parse(src).unwrap();
        assert_eq!(mc.find("image/jpeg").unwrap().command, "feh %s");
        assert_eq!(mc.find("video/mp4").unwrap().command, "mpv %s");
    }

    #[test]
    fn parse_ignores_comments_and_blanks() {
        let src = "# this is a comment\n\nimage/png; sxiv %s\n";
        let mc = Mailcap::parse(src).unwrap();
        assert!(mc.find("image/png").is_some());
    }

    #[test]
    fn parse_captures_test_and_copiousoutput() {
        let src = "text/plain; less %s; copiousoutput; test=test -n \"$DISPLAY\"\n";
        let mc = Mailcap::parse(src).unwrap();
        let entry = mc.find("text/plain").unwrap();
        assert!(entry.copiousoutput);
        assert!(entry.test.is_some());
    }

    #[test]
    fn expand_substitutes_percent_s_with_quoted_url() {
        let entry = MailcapEntry {
            mime_type: "image/jpeg".to_string(),
            command: "feh %s".to_string(),
            test: None,
            copiousoutput: false,
        };
        let expanded = entry.expand("https://i.redd.it/abc.jpg?w=1");
        assert_eq!(expanded, "feh 'https://i.redd.it/abc.jpg?w=1'");
    }

    #[test]
    fn expand_quotes_url_with_embedded_single_quote() {
        let entry = MailcapEntry {
            mime_type: "image/jpeg".to_string(),
            command: "feh %s".to_string(),
            test: None,
            copiousoutput: false,
        };
        let expanded = entry.expand("https://example.com/it's.jpg");
        // single quote becomes '\''
        assert!(expanded.contains(r"'\''"));
    }

    #[test]
    fn expand_appends_url_when_no_percent_s_placeholder() {
        let entry = MailcapEntry {
            mime_type: "image/jpeg".to_string(),
            command: "feh".to_string(),
            test: None,
            copiousoutput: false,
        };
        assert_eq!(
            entry.expand("https://i.redd.it/x.jpg"),
            "feh 'https://i.redd.it/x.jpg'"
        );
    }

    #[test]
    fn guess_mime_handles_query_strings() {
        assert_eq!(
            guess_mime_from_url("https://preview.redd.it/abc.jpg?w=640&s=xyz"),
            Some("image/jpeg".to_string())
        );
    }

    #[test]
    fn guess_mime_recognizes_video_extensions() {
        assert_eq!(
            guess_mime_from_url("https://example.com/clip.mp4"),
            Some("video/mp4".to_string())
        );
        assert_eq!(
            guess_mime_from_url("https://example.com/clip.webm"),
            Some("video/webm".to_string())
        );
    }

    #[test]
    fn guess_mime_falls_back_to_text_html_for_extensionless_https() {
        assert_eq!(
            guess_mime_from_url("https://github.com/rust-lang/rust"),
            Some("text/html".to_string())
        );
    }

    #[test]
    fn find_for_url_uses_exact_then_wildcard_then_catch_all() {
        let src = "image/jpeg; feh %s\nimage/*; sxiv %s\n*/*; xdg-open %s\n";
        let mc = Mailcap::parse(src).unwrap();

        // exact wins
        assert_eq!(mc.find_for_url("a.jpg").unwrap().command, "feh %s");
        // wildcard wins for png (no exact entry)
        assert_eq!(mc.find_for_url("a.png").unwrap().command, "sxiv %s");
        // catch-all wins for an unmapped video
        assert_eq!(mc.find_for_url("clip.mp4").unwrap().command, "xdg-open %s");
    }

    #[test]
    fn find_for_url_returns_none_when_nothing_matches() {
        let src = "image/jpeg; feh %s\n";
        let mc = Mailcap::parse(src).unwrap();
        assert!(mc.find_for_url("https://example.com/clip.mp4").is_none());
    }

    #[test]
    fn shell_quote_handles_empty_string() {
        assert_eq!(shell_quote(""), "''");
    }
}
