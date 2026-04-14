//! Reddit API trait — unified interface for mock and real clients.

use crate::reddit::models::{Comment, Listing, Message, Submission, Subreddit};
use anyhow::Result;
use async_trait::async_trait;

/// Combined submission detail payload (submission + comment list).
#[derive(Debug, Clone)]
pub struct SubmissionPayload {
    pub submission: Submission,
    pub comments: Vec<Comment>,
}

/// Listing sort order for subreddit feeds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Sort {
    #[default]
    Hot,
    New,
    Top,
    Controversial,
    Rising,
}

impl Sort {
    /// URL path segment used by Reddit (`/r/<sub>/<segment>`).
    pub fn as_str(self) -> &'static str {
        match self {
            Sort::Hot => "hot",
            Sort::New => "new",
            Sort::Top => "top",
            Sort::Controversial => "controversial",
            Sort::Rising => "rising",
        }
    }
}

/// Common Reddit API interface consumed by the TUI layer.
///
/// Both [`crate::reddit::MockRedditClient`] and [`crate::reddit::RedditClient`]
/// implement this trait so pages can hold `Arc<dyn RedditApi>` and swap between
/// mock and real backends at runtime.
#[async_trait]
pub trait RedditApi: Send + Sync {
    /// Fetch a listing of submissions for the given sort order.
    async fn listing(
        &self,
        sort: Sort,
        subreddit: Option<&str>,
        limit: usize,
    ) -> Result<Listing<Submission>>;

    /// Convenience shortcut for [`Sort::Hot`]; trait callers can use either.
    async fn hot(&self, subreddit: Option<&str>, limit: usize) -> Result<Listing<Submission>> {
        self.listing(Sort::Hot, subreddit, limit).await
    }

    async fn submission(&self, id: &str) -> Result<SubmissionPayload>;

    async fn vote(&self, id: &str, direction: i8) -> Result<()>;

    async fn inbox(&self) -> Result<Listing<Message>>;

    async fn mark_read(&self, id: &str) -> Result<()>;

    async fn mark_unread(&self, id: &str) -> Result<()>;

    async fn subscribed(&self, limit: usize) -> Result<Listing<Subreddit>>;
}
