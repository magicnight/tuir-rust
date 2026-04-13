//! Reddit REST API client
//!
//! Thin REST client wrapping Reddit's JSON API with automatic token management.
//!
//! ## Note
//!
//! For testing without real API credentials, use `MockRedditClient` from `reddit::mock`.

use anyhow::{anyhow, Result};
use reqwest::Client;
use serde::de::DeserializeOwned;
use serde_json::Value;
use std::time::Duration;
use tracing::{debug, error};

const USER_AGENT: &str = "tuir-rust:v0.1.0 (by /u/tuir-dev)";

/// Reddit API client with bearer token authentication
pub struct RedditClient {
    http: Client,
    base_url: &'static str,
    token: Option<String>,
}

impl RedditClient {
    /// Create a new Reddit client
    pub fn new() -> Self {
        let http = Client::builder()
            .user_agent(USER_AGENT)
            .timeout(Duration::from_secs(30))
            .build()
            .expect("HTTP client should build");

        Self {
            http,
            base_url: "https://www.reddit.com",
            token: None,
        }
    }

    /// Set the bearer token
    pub fn set_token(&mut self, token: String) {
        self.token = Some(token);
    }

    /// Get current bearer token
    pub fn token(&self) -> Option<&str> {
        self.token.as_deref()
    }

    /// Make an authenticated GET request
    pub async fn get<T: DeserializeOwned>(&self, path: &str) -> Result<T> {
        let token = self
            .token
            .as_ref()
            .ok_or_else(|| anyhow!("Not authenticated - call set_token first"))?;

        let url = format!("{}{}.json", self.base_url, path);
        debug!("GET {}", url);

        let response = self
            .http
            .get(&url)
            .header("Authorization", format!("Bearer {}", token))
            .header("User-Agent", USER_AGENT)
            .send()
            .await?;

        if response.status() == reqwest::StatusCode::UNAUTHORIZED {
            error!("401 Unauthorized - token may be expired");
            return Err(anyhow!("Authentication expired"));
        }

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            error!("API error {}: {}", status, body);
            return Err(anyhow!("API error {}: {}", status, body));
        }

        let json: T = response.json().await?;
        Ok(json)
    }

    /// Make an authenticated POST request
    pub async fn post(&self, path: &str, body: &[(&str, &str)]) -> Result<Value> {
        let token = self
            .token
            .as_ref()
            .ok_or_else(|| anyhow!("Not authenticated - call set_token first"))?;

        let url = format!("{}{}", self.base_url, path);
        debug!("POST {}", url);

        let response = self
            .http
            .post(&url)
            .header("Authorization", format!("Bearer {}", token))
            .header("User-Agent", USER_AGENT)
            .form(body)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            error!("API error {}: {}", status, body);
            return Err(anyhow!("API error {}: {}", status, body));
        }

        let json: Value = response.json().await?;
        Ok(json)
    }

    /// Check if authenticated
    pub fn is_authenticated(&self) -> bool {
        self.token.is_some()
    }
}

impl Default for RedditClient {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_requires_auth() {
        let client = RedditClient::new();
        assert!(!client.is_authenticated());

        let mut client = RedditClient::new();
        client.set_token("fake_token".to_string());
        assert!(client.is_authenticated());
    }
}
