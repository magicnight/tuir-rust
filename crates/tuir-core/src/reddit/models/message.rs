//! Message model (inbox)

use serde::{Deserialize, Serialize};

/// Reddit private message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub id: String,
    pub name: String,
    pub subject: String,
    pub author: String,
    pub body: String,
    pub body_html: Option<String>,
    pub created_utc: f64,
    pub dest: String,
    pub new: bool,
    pub replies: Option<Box<serde_json::Value>>,
}
