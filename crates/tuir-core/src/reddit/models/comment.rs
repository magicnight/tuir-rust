//! Comment model

use serde::{Deserialize, Serialize};

/// Reddit comment
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Comment {
    pub id: String,
    pub name: String,
    pub author: String,
    pub body: String,
    pub body_html: Option<String>,
    pub link_id: String,
    pub parent_id: String,
    pub score: i64,
    pub created_utc: f64,
    pub distinguished: Option<String>,
    pub edited: Option<serde_json::Value>,
    pub depth: Option<i64>,
    pub replies: Option<Box<serde_json::Value>>, // Can be MoreComments or Listing
    pub is_submitter: bool,
}
