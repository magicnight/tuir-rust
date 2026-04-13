//! OAuth2 authentication for Reddit
//!
//! Implements Reddit's installed app OAuth2 flow with localhost callback.

use oauth2::basic::BasicClient;
use oauth2::{AuthUrl, ClientId, RedirectUrl};
use std::sync::Arc;
use tokio::sync::RwLock;

/// OAuth state for Reddit authentication
pub struct OAuth {
    /// Stored access token
    access_token: Arc<RwLock<Option<String>>>,
}

impl OAuth {
    /// Create a new OAuth client for Reddit installed app flow
    #[allow(dead_code)]
    pub fn new(_client_id: String) -> Self {
        Self {
            access_token: Arc::new(RwLock::new(None)),
        }
    }

    /// Get the current access token
    pub async fn get_token(&self) -> Option<String> {
        self.access_token.read().await.clone()
    }

    /// Set the access token after successful authentication
    pub async fn set_token(&self, token: String) {
        let mut t = self.access_token.write().await;
        *t = Some(token);
    }

    /// Build authorization URL for Reddit OAuth
    #[allow(dead_code)]
    pub fn auth_url(&self, client_id: &str) -> String {
        let client = BasicClient::new(ClientId::new(client_id.to_string()))
            .set_auth_uri(
                AuthUrl::new("https://www.reddit.com/api/v1/authorize".to_string()).unwrap(),
            )
            .set_redirect_uri(RedirectUrl::new("http://localhost:65000".to_string()).unwrap());
        let (url, _csrf) = client.authorize_url(oauth2::CsrfToken::new_random).url();
        url.to_string()
    }
}
