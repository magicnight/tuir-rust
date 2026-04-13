//! Reddit REST API client

use std::sync::Arc;
use tokio::sync::RwLock;

/// Reddit API client with automatic token management
pub struct RedditClient {
    token: Arc<RwLock<Option<String>>>,
}

impl RedditClient {
    /// Create a new Reddit client
    pub fn new() -> Self {
        Self {
            token: Arc::new(RwLock::new(None)),
        }
    }

    /// Set the bearer token
    pub async fn set_token(&self, token: String) {
        let mut t = self.token.write().await;
        *t = Some(token);
    }

    /// Get current bearer token
    pub async fn token(&self) -> Option<String> {
        self.token.read().await.clone()
    }

    /// Check if authenticated
    pub async fn is_authenticated(&self) -> bool {
        self.token.read().await.is_some()
    }
}

impl Default for RedditClient {
    fn default() -> Self {
        Self::new()
    }
}
