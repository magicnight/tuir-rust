//! Listing model (subreddit feeds, comment trees)

use serde::{Deserialize, Serialize};

/// Reddit listing wrapper (e.g. /r/rust/hot.json)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Listing<T> {
    pub kind: String,
    pub data: ListingData<T>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListingData<T> {
    pub modhash: Option<String>,
    pub dist: Option<i64>,
    pub children: Vec<T>,
    pub after: Option<String>,
    pub before: Option<String>,
}
