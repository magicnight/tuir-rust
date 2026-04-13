//! Subreddit model

use serde::{Deserialize, Serialize};

/// Reddit subreddit
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Subreddit {
    /// Fullname like "t5_abc123"
    pub id: String,
    /// Display name (e.g. "rust")
    pub name: String,
    pub display_name: String,
    pub title: String,
    /// Description (markdown)
    pub description: String,
    /// Public description
    pub public_description: String,
    /// Subscriber count
    pub subscribers: i64,
    /// Active user count
    pub active_user_count: Option<i64>,
    /// Whether nsfw
    pub over18: bool,
    /// URL path (e.g. "/r/rust")
    pub url: String,
    /// Unix timestamp of creation
    pub created_utc: f64,
    /// Submission type (any, link, self)
    pub submission_type: Option<String>,
    /// User is subscribed
    pub user_is_subscriber: Option<bool>,
    /// User has favorited
    pub user_has_favorited: Option<bool>,
    /// User is banned
    pub user_is_banned: Option<bool>,
    /// User is a moderator
    pub user_is_moderator: Option<bool>,
    /// Icon image
    pub icon_img: Option<String>,
    /// Banner image
    pub banner_img: Option<String>,
    /// Header title
    pub header_title: Option<String>,
    /// Description HTML
    pub description_html: Option<String>,
}

impl Subreddit {
    /// Get the full URL to the subreddit
    pub fn url_full(&self) -> String {
        format!("https://www.reddit.com{}", self.url)
    }
}

/// Wrapper for subreddit in listing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubredditWrapper {
    pub kind: String,
    pub data: Subreddit,
}
