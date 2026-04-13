//! Submission (post) model

use serde::{Deserialize, Serialize};

/// Reddit submission (link or self-post)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Submission {
    pub id: String,
    pub name: String,
    pub title: String,
    pub author: String,
    pub subreddit: String,
    pub score: i64,
    pub num_comments: i64,
    pub permalink: String,
    pub url: String,
    pub selftext: String,
    pub created_utc: f64,
    pub distinguished: Option<String>,
    pub edited: Option<EditedField>,
    pub flair_text: Option<String>,
    pub link_flair_text: Option<String>,
    pub over_18: bool,
    pub pinned: bool,
    pub spoiler: bool,
    pub stickied: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum EditedField {
    Bool(bool),
    Float(f64),
}

impl Submission {
    /// Get full URL to the submission
    pub fn url_full(&self) -> String {
        if self.url.starts_with('/') {
            format!("https://www.reddit.com{}", self.url)
        } else {
            self.url.clone()
        }
    }
}
