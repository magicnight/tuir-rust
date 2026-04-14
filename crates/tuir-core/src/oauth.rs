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
use url::form_urlencoded;

/// OAuth configuration
#[derive(Debug, Clone)]
pub struct OAuth {
    /// OAuth client ID from Reddit app
    pub client_id: String,
    /// Redirect URI
    pub redirect_uri: String,
    /// OAuth scopes to request
    pub scopes: Vec<String>,
}

impl OAuth {
    /// Create OAuth config
    pub fn new(client_id: String, redirect_uri: String, scopes: Vec<String>) -> Self {
        Self {
            client_id,
            redirect_uri,
            scopes,
        }
    }

    /// Get the authorization URL
    ///
    /// In a real implementation, this would build the OAuth URL using the oauth2 crate.
    /// For now, this is a placeholder.
    pub fn auth_url(&self) -> String {
        let query = form_urlencoded::Serializer::new(String::new())
            .append_pair("client_id", &self.client_id)
            .append_pair("response_type", "code")
            .append_pair("state", "mock")
            .append_pair("redirect_uri", &self.redirect_uri)
            .append_pair("duration", "permanent")
            .append_pair("scope", &self.scopes.join(","))
            .finish();

        format!("https://www.reddit.com/api/v1/authorize?{query}")
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
        write!(
            f,
            "OAuth(client_id={}, redirect_uri={})",
            self.client_id, self.redirect_uri
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_oauth_auth_url() {
        let oauth = OAuth::new(
            "test_client".to_string(),
            "http://127.0.0.1:65000/".to_string(),
            vec!["read".to_string()],
        );
        let url = oauth.auth_url();
        assert!(url.contains("reddit.com"));
        assert!(url.contains("test_client"));
        assert!(url.contains("redirect_uri=http%3A%2F%2F127.0.0.1%3A65000%2F"));
    }

    #[test]
    fn test_oauth_configured() {
        let oauth = OAuth::new("".to_string(), "http://127.0.0.1:65000/".to_string(), vec![]);
        assert!(!oauth.is_configured());

        let oauth = OAuth::new(
            "real_id".to_string(),
            "http://127.0.0.1:65000/".to_string(),
            vec![],
        );
        assert!(oauth.is_configured());
    }
}
