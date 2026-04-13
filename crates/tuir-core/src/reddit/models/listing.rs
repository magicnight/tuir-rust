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
    #[serde(rename = "children")]
    pub children: Vec<Thing<T>>,
    pub after: Option<String>,
    pub before: Option<String>,
}

/// A thing wrapper (t1_, t3_, t5_, etc.)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Thing<T> {
    #[serde(rename = "kind")]
    pub kind: String,
    #[serde(rename = "data")]
    pub data: T,
}
