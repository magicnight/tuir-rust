//! Mock Reddit client for testing without real API credentials
//!
//! Provides fake data that mirrors Reddit's API response structure.

use crate::reddit::models::{Comment, Listing, ListingData, Message, Submission, Subreddit, Thing};
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
        let items: Vec<Submission> = (1..=limit.min(25))
            .map(|i| {
                self.mock_submission(
                    &format!("{}_{}", sub, i),
                    &format!("Sample Post {} from r/{}", i, sub),
                )
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
        MockSubmissionResponse {
            submission: self.mock_submission(id, "Mock Submission Title"),
            comments: self.mock_comments(id, 5),
        }
    }

    /// Vote (mock - just records it)
    pub async fn vote(&self, id: &str, direction: i8) {
        let mut state = self.state.write().await;
        state.voted_posts.insert(id.to_string(), direction);
    }

    /// Get inbox (mock)
    pub async fn inbox(&self) -> Listing<Message> {
        let messages = vec![Message {
            id: "mock_msg_1".to_string(),
            name: "t4_mock_msg_1".to_string(),
            subject: "Welcome to Reddit!".to_string(),
            author: "reddit".to_string(),
            body: "Thanks for joining Reddit!".to_string(),
            body_html: Some("<p>Thanks for joining Reddit!</p>".to_string()),
            created_utc: 1700000000.0,
            dest: "mock_user".to_string(),
            new: true,
            reply_to: None,
            subreddit: None,
            author_flair_text: None,
            was_comment: false,
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
                subscribers: 10000 + rand_simple(),
                active_user_count: Some(500 + rand_simple()),
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

    fn mock_submission(&self, id: &str, title: &str) -> Submission {
        Submission {
            id: id.to_string(),
            name: format!("t3_{}", id),
            title: title.to_string(),
            author: "mock_user".to_string(),
            subreddit: "mock_sub".to_string(),
            score: 1000 + rand_simple(),
            num_comments: 50 + rand_simple(),
            permalink: format!("/r/mock_sub/comments/{}/mock_title", id),
            url: format!("https://reddit.com/r/mock_sub/comments/{}", id),
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
            likes: None,
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
                score: 100 + rand_simple(),
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

/// Simple pseudo-random number generator (for mock data determinism)
fn rand_simple() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .subsec_nanos();
    (nanos % 1000) as i64
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
}
