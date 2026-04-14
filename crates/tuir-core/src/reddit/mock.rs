//! Mock Reddit client for testing without real API credentials
//!
//! Provides fake data that mirrors Reddit's API response structure.

use crate::reddit::api::{RedditApi, Sort, SubmissionPayload};
use crate::reddit::models::{Comment, Listing, ListingData, Message, Submission, Subreddit, Thing};
use anyhow::Result;
use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Mock Reddit client that returns fake data
pub struct MockRedditClient {
    state: Arc<RwLock<MockState>>,
}

#[derive(Default)]
pub struct MockState {
    pub authenticated: bool,
    pub username: String,
    pub voted_posts: std::collections::HashMap<String, i8>,
    pub read_messages: std::collections::HashSet<String>,
}

impl MockRedditClient {
    /// Create a new mock client
    pub fn new() -> Self {
        Self {
            state: Arc::new(RwLock::new(MockState::default())),
        }
    }

    /// Simulate authentication
    pub async fn authenticate(&self, username: &str) {
        let mut state = self.state.write().await;
        state.authenticated = true;
        state.username = username.to_string();
    }

    /// Get hot posts (mock)
    pub async fn hot(&self, subreddit: Option<&str>, limit: usize) -> Listing<Submission> {
        let sub = subreddit.unwrap_or("popular");
        let state = self.state.read().await;
        let items: Vec<Submission> = (1..=limit.min(25))
            .map(|i| {
                let id = format!("{}_{}", sub, i);
                let title = format!("Sample Post {} from r/{}", i, sub);
                self.mock_submission(&state, &id, &title)
            })
            .collect();

        Listing {
            kind: "Listing".to_string(),
            data: ListingData {
                modhash: Some("mock_modhash".to_string()),
                dist: Some(items.len() as i64),
                children: items
                    .into_iter()
                    .map(|s| Thing {
                        kind: "t3".to_string(),
                        data: s,
                    })
                    .collect(),
                after: Some("t3_mockafter".to_string()),
                before: None,
            },
        }
    }

    /// Get new posts (mock)
    pub async fn newest(&self, subreddit: Option<&str>, limit: usize) -> Listing<Submission> {
        self.hot(subreddit, limit).await
    }

    /// Get a single submission (mock)
    pub async fn submission(&self, id: &str) -> MockSubmissionResponse {
        let state = self.state.read().await;
        MockSubmissionResponse {
            submission: self.mock_submission(&state, id, "Mock Submission Title"),
            comments: self.mock_comments(id, 5),
        }
    }

    /// Vote (mock - just records it)
    pub async fn vote(&self, id: &str, direction: i8) {
        let mut state = self.state.write().await;
        if direction == 0 {
            state.voted_posts.remove(id);
        } else {
            state.voted_posts.insert(id.to_string(), direction);
        }
    }

    pub async fn mark_read(&self, id: &str) {
        let mut state = self.state.write().await;
        state.read_messages.insert(id.to_string());
    }

    pub async fn mark_unread(&self, id: &str) {
        let mut state = self.state.write().await;
        state.read_messages.remove(id);
    }

    /// Get inbox (mock)
    pub async fn inbox(&self) -> Listing<Message> {
        let state = self.state.read().await;
        let message_name = "t4_mock_msg_1".to_string();
        let messages = vec![Message {
            id: "mock_msg_1".to_string(),
            name: message_name.clone(),
            subject: "Welcome to Reddit!".to_string(),
            author: "reddit".to_string(),
            body: "Thanks for joining Reddit!".to_string(),
            body_html: Some("<p>Thanks for joining Reddit!</p>".to_string()),
            created_utc: 1700000000.0,
            dest: "mock_user".to_string(),
            new: !state.read_messages.contains(&message_name),
            reply_to: Some("t3_rust_1".to_string()),
            subreddit: Some("rust".to_string()),
            author_flair_text: None,
            was_comment: true,
        }];

        Listing {
            kind: "Listing".to_string(),
            data: ListingData {
                modhash: None,
                dist: Some(messages.len() as i64),
                children: messages
                    .into_iter()
                    .map(|m| Thing {
                        kind: "t4".to_string(),
                        data: m,
                    })
                    .collect(),
                after: None,
                before: None,
            },
        }
    }

    /// Get subscribed subreddits (mock)
    pub async fn subscribed(&self, limit: usize) -> Listing<Subreddit> {
        let subs = vec![
            ("t5_1", "rust", "r/rust", "Rust programming language"),
            (
                "t5_2",
                "technology",
                "r/technology",
                "Tech news and discussion",
            ),
            (
                "t5_3",
                "programming",
                "r/programming",
                "Programming discussions",
            ),
            ("t5_4", "linux", "r/linux", "Linux community"),
            ("t5_5", "vim", "r/vim", "Vim text editor"),
        ];

        let subreddits: Vec<Subreddit> = subs
            .into_iter()
            .take(limit.min(25))
            .map(|(id, name, url, title)| Subreddit {
                id: id.to_string(),
                name: name.to_string(),
                display_name: name.to_string(),
                title: title.to_string(),
                description: format!("A community about {}", title),
                public_description: format!("Community for {}", title),
                subscribers: 10000 + stable_offset(name, 1000),
                active_user_count: Some(500 + stable_offset(name, 200)),
                over18: false,
                url: url.to_string(),
                created_utc: 1600000000.0,
                submission_type: Some("any".to_string()),
                user_is_subscriber: Some(true),
                user_has_favorited: Some(false),
                user_is_banned: Some(false),
                user_is_moderator: Some(false),
                icon_img: None,
                banner_img: None,
                header_title: None,
                description_html: None,
            })
            .collect();

        Listing {
            kind: "Listing".to_string(),
            data: ListingData {
                modhash: None,
                dist: Some(subreddits.len() as i64),
                children: subreddits
                    .into_iter()
                    .map(|s| Thing {
                        kind: "t5".to_string(),
                        data: s,
                    })
                    .collect(),
                after: None,
                before: None,
            },
        }
    }

    fn mock_submission(&self, state: &MockState, id: &str, title: &str) -> Submission {
        let fullname = format!("t3_{}", id);
        let vote = state.voted_posts.get(&fullname).copied().unwrap_or(0);
        let vote_delta = i64::from(vote);
        let (subreddit, derived_title) = submission_metadata(id, title);

        Submission {
            id: id.to_string(),
            name: fullname,
            title: derived_title,
            author: "mock_user".to_string(),
            subreddit: subreddit.clone(),
            score: 1000 + stable_offset(id, 250) + vote_delta,
            num_comments: 50 + stable_offset(id, 75),
            permalink: format!("/r/{subreddit}/comments/{id}/mock_title"),
            url: format!("https://reddit.com/r/{subreddit}/comments/{id}"),
            selftext: "This is mock content for testing purposes.".to_string(),
            created_utc: 1700000000.0,
            distinguished: None,
            edited: super::submission::EditedField::Bool(false),
            link_flair_text: None,
            author_flair_text: None,
            over_18: false,
            pinned: false,
            spoiler: false,
            stickied: false,
            saved: false,
            hidden: false,
            likes: if vote == 0 { None } else { Some(vote.into()) },
            url_full: None,
        }
    }

    fn mock_comments(&self, _submission_id: &str, count: usize) -> Vec<Comment> {
        (1..=count)
            .map(|i| Comment {
                id: format!("mock_comment_{}", i),
                name: format!("t1_mock_comment_{}", i),
                author: format!("commenter_{}", i),
                body: format!("This is mock comment #{} for testing.", i),
                body_html: Some(format!("<p>This is mock comment #{} for testing.</p>", i)),
                link_id: format!("t3_{}", _submission_id),
                parent_id: format!("t3_{}", _submission_id),
                score: 100 + stable_offset(&format!("{_submission_id}_{i}"), 50),
                created_utc: 1700000100.0 + (i as f64 * 100.0),
                distinguished: None,
                edited: super::submission::EditedField::Bool(false),
                depth: Some(0),
                replies: None,
                is_submitter: i == 1,
                saved: false,
                likes: None,
                author_flair_text: None,
            })
            .collect()
    }
}

impl Default for MockRedditClient {
    fn default() -> Self {
        Self::new()
    }
}

/// Response type for submission with comments
pub struct MockSubmissionResponse {
    pub submission: Submission,
    pub comments: Vec<Comment>,
}

#[async_trait]
impl RedditApi for MockRedditClient {
    async fn listing(
        &self,
        sort: Sort,
        subreddit: Option<&str>,
        limit: usize,
    ) -> Result<Listing<Submission>> {
        let mut listing = MockRedditClient::hot(self, subreddit, limit).await;
        // Stamp the sort into the post titles so the UI (and tests) can
        // observe that switching sort actually went through the trait method
        // even though the mock returns the same shape regardless.
        let label = sort.as_str();
        for child in listing.data.children.iter_mut() {
            child.data.title = format!("[{label}] {}", child.data.title);
        }
        Ok(listing)
    }

    async fn submission(&self, id: &str) -> Result<SubmissionPayload> {
        let resp = MockRedditClient::submission(self, id).await;
        Ok(SubmissionPayload {
            submission: resp.submission,
            comments: resp.comments,
        })
    }

    async fn vote(&self, id: &str, direction: i8) -> Result<()> {
        MockRedditClient::vote(self, id, direction).await;
        Ok(())
    }

    async fn inbox(&self) -> Result<Listing<Message>> {
        Ok(MockRedditClient::inbox(self).await)
    }

    async fn mark_read(&self, id: &str) -> Result<()> {
        MockRedditClient::mark_read(self, id).await;
        Ok(())
    }

    async fn mark_unread(&self, id: &str) -> Result<()> {
        MockRedditClient::mark_unread(self, id).await;
        Ok(())
    }

    async fn subscribed(&self, limit: usize) -> Result<Listing<Subreddit>> {
        Ok(MockRedditClient::subscribed(self, limit).await)
    }
}

fn stable_offset(seed: &str, modulo: i64) -> i64 {
    let hash = seed
        .bytes()
        .fold(0u64, |acc, byte| acc.wrapping_mul(131).wrapping_add(u64::from(byte)));
    (hash % modulo as u64) as i64
}

fn submission_metadata(id: &str, fallback_title: &str) -> (String, String) {
    if let Some((subreddit, post_no)) = id.rsplit_once('_') {
        if post_no.chars().all(|ch| ch.is_ascii_digit()) && !subreddit.is_empty() {
            return (
                subreddit.to_string(),
                format!("Sample Post {post_no} from r/{subreddit}"),
            );
        }
    }

    ("mock_sub".to_string(), fallback_title.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_mock_hot() {
        let client = MockRedditClient::new();
        let listing = client.hot(Some("rust"), 10).await;
        assert_eq!(listing.data.children.len(), 10);
    }

    #[tokio::test]
    async fn test_mock_inbox() {
        let client = MockRedditClient::new();
        let messages = client.inbox().await;
        assert!(!messages.data.children.is_empty());
    }

    #[tokio::test]
    async fn test_mock_vote() {
        let client = MockRedditClient::new();
        client.vote("t3_abc123", 1).await;
    }

    #[tokio::test]
    async fn test_mock_hot_reflects_vote_state() {
        let client = MockRedditClient::new();
        let before = client.hot(Some("rust"), 1).await;
        let before_score = before.data.children[0].data.score;
        let before_likes = before.data.children[0].data.likes;

        client.vote("t3_rust_1", 1).await;
        let after = client.hot(Some("rust"), 1).await;
        let after_submission = &after.data.children[0].data;

        assert_eq!(before_likes, None);
        assert_eq!(after_submission.score, before_score + 1);
        assert_eq!(after_submission.likes.map(i8::from), Some(1));
    }

    #[tokio::test]
    async fn test_mock_submission_reflects_vote_state() {
        let client = MockRedditClient::new();

        client.vote("t3_abc123", -1).await;
        let submission = client.submission("abc123").await;

        assert_eq!(submission.submission.likes.map(i8::from), Some(-1));
    }

    #[tokio::test]
    async fn test_mock_submission_uses_id_derived_metadata() {
        let client = MockRedditClient::new();

        let submission = client.submission("rust_7").await;

        assert_eq!(submission.submission.subreddit, "rust");
        assert_eq!(submission.submission.title, "Sample Post 7 from r/rust");
        assert!(submission.submission.permalink.contains("/r/rust/comments/rust_7/"));
    }

    #[tokio::test]
    async fn test_mock_inbox_preserves_read_state() {
        let client = MockRedditClient::new();

        let before = client.inbox().await;
        assert!(before.data.children[0].data.new);

        client.mark_read("t4_mock_msg_1").await;
        let after = client.inbox().await;

        assert!(!after.data.children[0].data.new);
    }

    #[tokio::test]
    async fn test_mock_inbox_can_mark_unread_again() {
        let client = MockRedditClient::new();

        client.mark_read("t4_mock_msg_1").await;
        client.mark_unread("t4_mock_msg_1").await;
        let inbox = client.inbox().await;

        assert!(inbox.data.children[0].data.new);
    }
}
