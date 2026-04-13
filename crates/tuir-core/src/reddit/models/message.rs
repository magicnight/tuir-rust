//! Message model (inbox)

use serde::{Deserialize, Serialize};

/// Reddit private message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    /// Fullname like "t4_abc123"
    pub id: String,
    pub name: String,
    pub subject: String,
    pub author: String,
    /// Message body (markdown)
    pub body: String,
    /// HTML body
    pub body_html: Option<String>,
    /// Unix timestamp
    pub created_utc: f64,
    /// Destination (recipient)
    pub dest: String,
    /// Whether new/unread
    pub new: bool,
    /// Reply subject (for replies)
    pub reply_to: Option<String>,
    /// Subreddit (for subreddit messages)
    pub subreddit: Option<String>,
    /// Whether the current user has messaged this author
    pub author_flair_text: Option<String>,
    /// Was this message sent to the inbox (vs just created)
    pub was_comment: bool,
}

/// Wrapper for message in listing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageWrapper {
    pub kind: String,
    pub data: Message,
}
