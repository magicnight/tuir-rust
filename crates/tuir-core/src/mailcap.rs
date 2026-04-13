//! Mailcap file parser and MIME type handler
//!
//! Parses ~/.config/tuir/mailcap (RFC 1524 subset) and executes
//! the appropriate viewer for a given MIME type.

use anyhow::Result;
use std::collections::HashMap;
use std::path::Path;

/// A parsed mailcap entry
#[derive(Debug, Clone)]
pub struct MailcapEntry {
    /// MIME type (e.g. "image/png")
    pub mime_type: String,
    /// Command template (e.g. "feh %s")
    pub command: String,
    /// Test field (e.g. "copiousoutput" for text viewers)
    pub test: Option<String>,
}

/// Mailcap database
pub struct Mailcap {
    entries: HashMap<String, Vec<MailcapEntry>>,
}

impl Mailcap {
    /// Parse a mailcap file
    pub fn from_file(path: impl AsRef<Path>) -> Result<Self> {
        let content = std::fs::read_to_string(path)?;
        Self::parse(&content)
    }

    /// Parse mailcap content
    pub fn parse(content: &str) -> Result<Self> {
        let mut entries: HashMap<String, Vec<MailcapEntry>> = HashMap::new();
        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            // RFC 1524 mailcap format: mime-type; command; field2; field3...
            let parts: Vec<&str> = line.split(';').collect();
            if parts.len() < 2 {
                continue;
            }
            let mime_type = parts[0].trim().to_string();
            let command = parts[1].trim().to_string();
            let test = parts.get(2).and_then(|f| {
                let f = f.trim();
                f.strip_prefix("test=").map(|stripped| stripped.to_string())
            });

            entries
                .entry(mime_type.clone())
                .or_default()
                .push(MailcapEntry {
                    mime_type,
                    command,
                    test,
                });
        }
        Ok(Self { entries })
    }

    /// Find the first usable entry for a MIME type
    pub fn find(&self, mime_type: &str) -> Option<&MailcapEntry> {
        self.entries.get(mime_type).and_then(|v| v.first())
    }
}
