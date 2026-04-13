//! Submission (post) model

use serde::{Deserialize, Serialize};

/// Reddit submission (link or self-post)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Submission {
    /// Fullname like "t3_abc123"
    pub id: String,
    pub name: String,
    pub title: String,
    pub author: String,
    pub subreddit: String,
    pub score: i64,
    pub num_comments: i64,
    /// Permanent link to the submission
    pub permalink: String,
    /// The URL of the submission (may be self.reddit or external)
    pub url: String,
    /// Self text for self-posts
    pub selftext: String,
    /// Unix timestamp
    pub created_utc: f64,
    /// Whether the submission is distinguished
    pub distinguished: Option<String>,
    /// Whether edited (timestamp or false)
    #[serde(deserialize_with = "deserialize_edited")]
    pub edited: EditedField,
    /// Post flair
    pub link_flair_text: Option<String>,
    /// User flair
    pub author_flair_text: Option<String>,
    /// NSFW marker
    pub over_18: bool,
    /// Pinned status
    pub pinned: bool,
    /// Spoiler marker
    pub spoiler: bool,
    /// Stickied status
    pub stickied: bool,
    /// Whether the current user has saved this
    pub saved: bool,
    /// Whether the current user has hidden this
    pub hidden: bool,
    /// Vote status: 1 = upvoted, 0 = none, -1 = downvoted
    pub likes: Option<VoteState>,
    /// Full URL to the submission
    pub url_full: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum EditedField {
    Bool(bool),
    Float(f64),
}

impl Default for EditedField {
    fn default() -> Self {
        EditedField::Bool(false)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(from = "i8", into = "i8")]
pub struct VoteState(i8);

impl VoteState {
    pub const UP: i8 = 1;
    pub const NONE: i8 = 0;
    pub const DOWN: i8 = -1;
}

impl From<i8> for VoteState {
    fn from(v: i8) -> Self {
        VoteState(v)
    }
}

impl From<VoteState> for i8 {
    fn from(v: VoteState) -> Self {
        v.0
    }
}

/// Wrapper for Reddit's listing response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubmissionWrapper {
    pub kind: String,
    pub data: Submission,
}

/// Get full URL to the submission
impl Submission {
    pub fn url_full(&self) -> String {
        if self.url.starts_with('/') {
            format!("https://www.reddit.com{}", self.url)
        } else {
            self.url.clone()
        }
    }

    /// Get the Reddit shortlink
    pub fn shortlink(&self) -> String {
        format!("https://redd.it/{}", self.id)
    }
}

/// Deserialize edited field
fn deserialize_edited<'de, D>(deserializer: D) -> Result<EditedField, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = serde_json::Value::deserialize(deserializer)?;
    match value {
        serde_json::Value::Bool(b) => Ok(EditedField::Bool(b)),
        serde_json::Value::Number(n) => {
            if let Some(f) = n.as_f64() {
                Ok(EditedField::Float(f))
            } else {
                Ok(EditedField::Bool(false))
            }
        }
        _ => Ok(EditedField::Bool(false)),
    }
}
