//! Reddit API endpoints
//!
//! Minimal endpoint implementations for the views used by tuir.

use super::client::RedditClient;
use anyhow::Result;

/// Hot posts from a subreddit or front page
pub async fn hot(_client: &RedditClient, subreddit: Option<&str>) -> Result<String> {
    let path = match subreddit {
        Some(sub) => format!("/r/{sub}/hot.json"),
        None => "/hot.json".to_string(),
    };
    Ok(path)
}

/// New posts from a subreddit
pub async fn newest(_client: &RedditClient, subreddit: Option<&str>) -> Result<String> {
    let path = match subreddit {
        Some(sub) => format!("/r/{sub}/new.json"),
        None => "/new.json".to_string(),
    };
    Ok(path)
}

/// Single submission with comments
pub async fn submission(_client: &RedditClient, id: &str) -> Result<String> {
    Ok(format!("/comments/{id}.json"))
}

/// Submit a vote
#[allow(dead_code)]
pub async fn vote(_client: &RedditClient, id: &str, direction: i8) -> Result<()> {
    let _ = (id, direction);
    Ok(())
}
