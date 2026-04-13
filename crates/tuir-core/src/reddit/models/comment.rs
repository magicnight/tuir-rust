//! Comment model

use super::submission::EditedField;
use serde::{Deserialize, Serialize};

/// Reddit comment
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Comment {
    /// Fullname like "t1_abc123"
    pub id: String,
    pub name: String,
    pub author: String,
    /// The body text (markdown)
    pub body: String,
    /// HTML body
    pub body_html: Option<String>,
    /// Link to the parent submission
    pub link_id: String,
    /// Parent comment/post fullname
    pub parent_id: String,
    pub score: i64,
    /// Unix timestamp
    pub created_utc: f64,
    /// Whether distinguished (moderator/admin)
    pub distinguished: Option<String>,
    /// Whether edited
    pub edited: EditedField,
    /// Nesting depth
    pub depth: Option<i64>,
    /// Replies (can be MoreComments or listing)
    pub replies: Option<Box<CommentReplies>>,
    /// Whether author is the submission poster
    pub is_submitter: bool,
    /// Whether saved by current user
    pub saved: bool,
    /// Vote status
    pub likes: Option<i8>,
    /// User flair
    pub author_flair_text: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(untagged)]
pub enum CommentReplies {
    /// A listing of more comments
    Listing(super::listing::Listing<Comment>),
    /// Empty or deleted
    #[default]
    Empty,
}

/// Wrapper for comment in listing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommentWrapper {
    pub kind: String,
    pub data: Comment,
}

/// Represents a "load more comments" or "continue thread" node
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MoreComments {
    pub id: Option<String>,
    pub name: String,
    pub parent_id: String,
    pub children: Vec<String>,
    pub count: i64,
    pub depth: Option<i64>,
}
