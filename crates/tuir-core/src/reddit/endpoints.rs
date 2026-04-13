//! Reddit API endpoints
//!
//! Minimal endpoint implementations for the views used by tuir.

use super::client::RedditClient;
use crate::reddit::models::{Comment, Listing, Message, Submission, Subreddit};
use anyhow::Result;

/// Hot posts from a subreddit or front page
pub async fn hot(
    client: &RedditClient,
    subreddit: Option<&str>,
    limit: usize,
) -> Result<Listing<Submission>> {
    let path = match subreddit {
        Some(sub) => format!("/r/{sub}/hot?limit={}", limit),
        None => format!("/hot?limit={}", limit),
    };
    let listing: Listing<Submission> = client.get(&path).await?;
    Ok(listing)
}

/// New posts from a subreddit
pub async fn newest(
    client: &RedditClient,
    subreddit: Option<&str>,
    limit: usize,
) -> Result<Listing<Submission>> {
    let path = match subreddit {
        Some(sub) => format!("/r/{sub}/new?limit={}", limit),
        None => format!("/new?limit={}", limit),
    };
    let listing: Listing<Submission> = client.get(&path).await?;
    Ok(listing)
}

/// Top posts from a subreddit
pub async fn top(
    client: &RedditClient,
    subreddit: Option<&str>,
    limit: usize,
) -> Result<Listing<Submission>> {
    let path = match subreddit {
        Some(sub) => format!("/r/{sub}/top?limit={}", limit),
        None => format!("/top?limit={}", limit),
    };
    let listing: Listing<Submission> = client.get(&path).await?;
    Ok(listing)
}

/// Controversial posts
pub async fn controversial(
    client: &RedditClient,
    subreddit: Option<&str>,
    limit: usize,
) -> Result<Listing<Submission>> {
    let path = match subreddit {
        Some(sub) => format!("/r/{sub}/controversial?limit={}", limit),
        None => format!("/controversial?limit={}", limit),
    };
    let listing: Listing<Submission> = client.get(&path).await?;
    Ok(listing)
}

/// Single submission with comments
pub async fn submission(client: &RedditClient, id: &str) -> Result<SubmissionResponse> {
    let path = format!("/comments/{}", id);
    let response: SubmissionResponse = client.get(&path).await?;
    Ok(response)
}

/// Submit a vote
pub async fn vote(client: &RedditClient, id: &str, direction: i8) -> Result<()> {
    let path = "/api/vote";
    client
        .post(
            path,
            &[("id", id), ("dir", &direction.to_string()), ("rank", "2")],
        )
        .await?;
    Ok(())
}

/// Get inbox messages
pub async fn inbox(client: &RedditClient) -> Result<Listing<Message>> {
    let messages: Listing<Message> = client.get("/message/inbox").await?;
    Ok(messages)
}

/// Get sent messages
pub async fn sent(client: &RedditClient) -> Result<Listing<Message>> {
    let messages: Listing<Message> = client.get("/message/sent").await?;
    Ok(messages)
}

/// Get unread messages
pub async fn unread(client: &RedditClient) -> Result<Listing<Message>> {
    let messages: Listing<Message> = client.get("/message/unread").await?;
    Ok(messages)
}

/// Mark message as read
pub async fn read_message(client: &RedditClient, id: &str) -> Result<()> {
    let path = "/api/read_message";
    client.post(path, &[("id", id)]).await?;
    Ok(())
}

/// Get subscribed subreddits
pub async fn subscribed(client: &RedditClient, limit: usize) -> Result<Listing<Subreddit>> {
    let path = format!("/subreddits/mine/subscriber?limit={}", limit);
    let listing: Listing<Subreddit> = client.get(&path).await?;
    Ok(listing)
}

/// Get subreddit info
pub async fn subreddit_info(client: &RedditClient, subreddit: &str) -> Result<Subreddit> {
    let path = format!("/r/{}/about", subreddit);
    let data: SubredditData = client.get(&path).await?;
    Ok(data.data)
}

// Response types for Reddit API

/// Response from /r/{sub}/comments.json
#[derive(Debug, serde::Deserialize)]
pub struct SubmissionResponse {
    #[serde(rename = "0")]
    pub submission_listing: Listing<Submission>,
    #[serde(rename = "1")]
    pub comment_listing: Listing<Comment>,
}

/// Wrapper for subreddit data
#[derive(Debug, serde::Deserialize)]
pub struct SubredditData {
    pub data: Subreddit,
}
