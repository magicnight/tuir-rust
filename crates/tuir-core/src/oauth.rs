//! OAuth2 authentication for Reddit
//!
//! Implements Reddit's installed app OAuth2 flow with localhost callback.
//!
//! ## Setup
//!
//! To use this with real Reddit API:
//! 1. Go to https://www.reddit.com/prefs/apps
//! 2. Create an "installed app"
//! 3. Use the client ID in your config
//!
//! ## Note
//!
//! For testing without credentials, use `MockRedditClient` instead.

use serde::{Deserialize, Serialize};

/// OAuth configuration
#[derive(Debug, Clone)]
pub struct OAuth {
    /// OAuth client ID from Reddit app
    pub client_id: String,
    /// Redirect port (default: 65000)
    pub port: u16,
    /// OAuth scopes to request
    pub scopes: Vec<String>,
}

impl OAuth {
    /// Create OAuth config
    pub fn new(client_id: String, port: u16, scopes: Vec<String>) -> Self {
        Self {
            client_id,
            port,
            scopes,
        }
    }

    /// Get the authorization URL
    ///
    /// In a real implementation, this would build the OAuth URL using the oauth2 crate.
    /// For now, this is a placeholder.
    pub fn auth_url(&self) -> String {
        format!(
            "https://www.reddit.com/api/v1/authorize?client_id={}&response_type=code&state=mock&redirect_uri=http://localhost:{}/&duration=permanent&scope={}",
            self.client_id,
            self.port,
            self.scopes.join(",")
        )
    }

    /// Check if configured (has client_id)
    pub fn is_configured(&self) -> bool {
        !self.client_id.is_empty()
    }
}

/// Auth state (placeholder for token data)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthState {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_at: Option<i64>,
}

/// Token data stored to disk
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredToken {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_at: Option<i64>,
}

impl std::fmt::Display for OAuth {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "OAuth(client_id={}, port={})", self.client_id, self.port)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_oauth_auth_url() {
        let oauth = OAuth::new("test_client".to_string(), 65000, vec!["read".to_string()]);
        let url = oauth.auth_url();
        assert!(url.contains("reddit.com"));
        assert!(url.contains("test_client"));
    }

    #[test]
    fn test_oauth_configured() {
        let oauth = OAuth::new("".to_string(), 65000, vec![]);
        assert!(!oauth.is_configured());

        let oauth = OAuth::new("real_id".to_string(), 65000, vec![]);
        assert!(oauth.is_configured());
    }
}
