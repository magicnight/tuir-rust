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

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};
use url::form_urlencoded;

const DEFAULT_AUTH_BASE: &str = "https://www.reddit.com/api/v1/authorize";
const DEFAULT_TOKEN_BASE: &str = "https://www.reddit.com/api/v1/access_token";
const USER_AGENT: &str = "tuir-rust:v0.1.0 (by /u/tuir-dev)";

/// OAuth configuration
#[derive(Debug, Clone)]
pub struct OAuth {
    pub client_id: String,
    pub redirect_uri: String,
    pub scopes: Vec<String>,
    /// Overridable for tests (wiremock). Defaults to reddit.com.
    pub auth_base: String,
    /// Overridable for tests (wiremock). Defaults to reddit.com.
    pub token_base: String,
}

impl OAuth {
    pub fn new(client_id: String, redirect_uri: String, scopes: Vec<String>) -> Self {
        Self {
            client_id,
            redirect_uri,
            scopes,
            auth_base: DEFAULT_AUTH_BASE.to_string(),
            token_base: DEFAULT_TOKEN_BASE.to_string(),
        }
    }

    /// Override auth + token endpoints (used for wiremock-based tests).
    pub fn with_endpoints(mut self, auth_base: String, token_base: String) -> Self {
        self.auth_base = auth_base;
        self.token_base = token_base;
        self
    }

    /// Build the authorization URL shown to the user.
    pub fn auth_url(&self) -> String {
        self.auth_url_with_state("tuir")
    }

    /// Same as [`auth_url`] but lets callers inject their own state nonce to
    /// defend against CSRF.
    pub fn auth_url_with_state(&self, state: &str) -> String {
        let query = form_urlencoded::Serializer::new(String::new())
            .append_pair("client_id", &self.client_id)
            .append_pair("response_type", "code")
            .append_pair("state", state)
            .append_pair("redirect_uri", &self.redirect_uri)
            .append_pair("duration", "permanent")
            .append_pair("scope", &self.scopes.join(","))
            .finish();
        format!("{}?{query}", self.auth_base)
    }

    pub fn is_configured(&self) -> bool {
        !self.client_id.is_empty()
    }

    /// Exchange an authorization `code` for an access + refresh token pair.
    ///
    /// Uses Reddit's installed-app convention: HTTP Basic auth with the
    /// client_id as username and an empty password.
    pub async fn exchange_code(&self, http: &reqwest::Client, code: &str) -> Result<StoredToken> {
        let form = [
            ("grant_type", "authorization_code"),
            ("code", code),
            ("redirect_uri", self.redirect_uri.as_str()),
        ];
        let resp: TokenResponse = http
            .post(&self.token_base)
            .header("User-Agent", USER_AGENT)
            .basic_auth(&self.client_id, Some(""))
            .form(&form)
            .send()
            .await
            .context("token request failed")?
            .error_for_status()
            .context("token endpoint returned non-2xx")?
            .json()
            .await
            .context("failed to parse token response")?;

        Ok(resp.into_stored(None))
    }

    /// Refresh an expired access token. Reddit may omit `refresh_token` in the
    /// response; we keep the previous value in that case.
    pub async fn refresh(
        &self,
        http: &reqwest::Client,
        refresh_token: &str,
    ) -> Result<StoredToken> {
        let form = [
            ("grant_type", "refresh_token"),
            ("refresh_token", refresh_token),
        ];
        let resp: TokenResponse = http
            .post(&self.token_base)
            .header("User-Agent", USER_AGENT)
            .basic_auth(&self.client_id, Some(""))
            .form(&form)
            .send()
            .await
            .context("refresh request failed")?
            .error_for_status()
            .context("refresh endpoint returned non-2xx")?
            .json()
            .await
            .context("failed to parse refresh response")?;

        Ok(resp.into_stored(Some(refresh_token.to_string())))
    }
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

#[derive(Debug, Deserialize)]
struct TokenResponse {
    access_token: String,
    expires_in: Option<i64>,
    refresh_token: Option<String>,
    scope: Option<String>,
}

impl TokenResponse {
    fn into_stored(self, fallback_refresh: Option<String>) -> StoredToken {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        StoredToken {
            access_token: self.access_token,
            refresh_token: self
                .refresh_token
                .or(fallback_refresh)
                .unwrap_or_default(),
            expires_at: self.expires_in.map(|secs| now + secs),
            scope: self.scope,
        }
    }
}

/// Token data persisted to disk between sessions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredToken {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_at: Option<i64>,
    pub scope: Option<String>,
}

impl StoredToken {
    /// Return true when the access token is past its expiry (with a 60 s slack).
    pub fn is_expired(&self) -> bool {
        let Some(expires_at) = self.expires_at else {
            return false;
        };
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        now + 60 >= expires_at
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("create token dir {}", parent.display()))?;
        }
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(path, json)
            .with_context(|| format!("write token file {}", path.display()))?;
        Ok(())
    }

    pub fn load(path: &Path) -> Result<Self> {
        let bytes = std::fs::read(path)
            .with_context(|| format!("read token file {}", path.display()))?;
        let token: StoredToken = serde_json::from_slice(&bytes)
            .with_context(|| format!("parse token file {}", path.display()))?;
        Ok(token)
    }
}

/// Legacy alias (kept for callers that still import `AuthState`).
pub type AuthState = StoredToken;

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use wiremock::matchers::{body_string_contains, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[test]
    fn auth_url_contains_expected_fields() {
        let oauth = OAuth::new(
            "test_client".to_string(),
            "http://127.0.0.1:65000/".to_string(),
            vec!["read".to_string(), "vote".to_string()],
        );
        let url = oauth.auth_url();
        assert!(url.contains("client_id=test_client"));
        assert!(url.contains("redirect_uri=http%3A%2F%2F127.0.0.1%3A65000%2F"));
        assert!(url.contains("scope=read%2Cvote"));
    }

    #[test]
    fn is_configured_reflects_client_id() {
        assert!(!OAuth::new(String::new(), "x".to_string(), vec![]).is_configured());
        assert!(OAuth::new("abc".to_string(), "x".to_string(), vec![]).is_configured());
    }

    #[test]
    fn stored_token_round_trip_on_disk() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("refresh-token");
        let token = StoredToken {
            access_token: "at".to_string(),
            refresh_token: "rt".to_string(),
            expires_at: Some(1_700_000_000),
            scope: Some("read".to_string()),
        };
        token.save(&path).unwrap();
        let loaded = StoredToken::load(&path).unwrap();
        assert_eq!(loaded.access_token, "at");
        assert_eq!(loaded.refresh_token, "rt");
        assert_eq!(loaded.scope.as_deref(), Some("read"));
    }

    #[test]
    fn expired_when_expires_at_is_in_the_past() {
        let token = StoredToken {
            access_token: String::new(),
            refresh_token: String::new(),
            expires_at: Some(1),
            scope: None,
        };
        assert!(token.is_expired());
    }

    #[tokio::test]
    async fn exchange_code_posts_auth_code_grant() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/v1/access_token"))
            .and(body_string_contains("grant_type=authorization_code"))
            .and(body_string_contains("code=ABC"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "access_token": "new-at",
                "token_type": "bearer",
                "expires_in": 3600,
                "refresh_token": "new-rt",
                "scope": "read vote"
            })))
            .expect(1)
            .mount(&server)
            .await;

        let oauth = OAuth::new(
            "client".to_string(),
            "http://127.0.0.1:65000/".to_string(),
            vec!["read".to_string()],
        )
        .with_endpoints(
            format!("{}/api/v1/authorize", server.uri()),
            format!("{}/api/v1/access_token", server.uri()),
        );

        let http = reqwest::Client::new();
        let token = oauth.exchange_code(&http, "ABC").await.unwrap();
        assert_eq!(token.access_token, "new-at");
        assert_eq!(token.refresh_token, "new-rt");
        assert!(token.expires_at.is_some());
    }

    #[tokio::test]
    async fn refresh_reuses_previous_refresh_token_when_omitted() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/v1/access_token"))
            .and(body_string_contains("grant_type=refresh_token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "access_token": "refreshed-at",
                "token_type": "bearer",
                "expires_in": 3600,
                "scope": "read"
            })))
            .expect(1)
            .mount(&server)
            .await;

        let oauth = OAuth::new(
            "client".to_string(),
            "http://127.0.0.1:65000/".to_string(),
            vec![],
        )
        .with_endpoints(
            format!("{}/api/v1/authorize", server.uri()),
            format!("{}/api/v1/access_token", server.uri()),
        );

        let http = reqwest::Client::new();
        let token = oauth.refresh(&http, "keep-me").await.unwrap();
        assert_eq!(token.access_token, "refreshed-at");
        assert_eq!(token.refresh_token, "keep-me");
    }

    #[tokio::test]
    async fn exchange_code_propagates_http_error() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/v1/access_token"))
            .respond_with(ResponseTemplate::new(401))
            .mount(&server)
            .await;

        let oauth = OAuth::new(
            "client".to_string(),
            "http://127.0.0.1:65000/".to_string(),
            vec![],
        )
        .with_endpoints(
            format!("{}/api/v1/authorize", server.uri()),
            format!("{}/api/v1/access_token", server.uri()),
        );

        let http = reqwest::Client::new();
        let err = oauth.exchange_code(&http, "bad").await.unwrap_err();
        assert!(err.to_string().contains("non-2xx"));
    }

}
