//! Subreddit model

use serde::{Deserialize, Serialize};

/// Reddit subreddit
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Subreddit {
    pub id: String,
    pub name: String,
    pub display_name: String,
    pub title: String,
    pub description: String,
    pub public_description: String,
    pub subscribers: i64,
    pub over18: bool,
    pub url: String,
    pub created_utc: f64,
}

impl Subreddit {
    /// Get the full URL to the subreddit
    pub fn url_full(&self) -> String {
        format!("https://www.reddit.com{}", self.url)
    }
}
