//! Reddit REST API client
//!
//! Thin REST client wrapping Reddit's JSON API with automatic token management.
//!
//! ## Note
//!
//! For testing without real API credentials, use `MockRedditClient` from `reddit::mock`.

use crate::reddit::api::{RedditApi, SubmissionPayload};
use crate::reddit::endpoints;
use crate::reddit::models::{Listing, Message, Submission, Subreddit};
use anyhow::{anyhow, Result};
use async_trait::async_trait;
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

#[async_trait]
impl RedditApi for RedditClient {
    async fn hot(&self, subreddit: Option<&str>, limit: usize) -> Result<Listing<Submission>> {
        endpoints::hot(self, subreddit, limit).await
    }

    async fn submission(&self, id: &str) -> Result<SubmissionPayload> {
        let resp = endpoints::submission(self, id).await?;
        let submission = resp
            .submission_listing
            .data
            .children
            .into_iter()
            .next()
            .map(|thing| thing.data)
            .ok_or_else(|| anyhow!("submission listing is empty"))?;
        let comments = resp
            .comment_listing
            .data
            .children
            .into_iter()
            .map(|thing| thing.data)
            .collect();
        Ok(SubmissionPayload { submission, comments })
    }

    async fn vote(&self, id: &str, direction: i8) -> Result<()> {
        endpoints::vote(self, id, direction).await
    }

    async fn inbox(&self) -> Result<Listing<Message>> {
        endpoints::inbox(self).await
    }

    async fn mark_read(&self, id: &str) -> Result<()> {
        endpoints::read_message(self, id).await
    }

    async fn mark_unread(&self, _id: &str) -> Result<()> {
        // Reddit's /api/unread_message is not yet wired in endpoints.rs.
        Err(anyhow!("mark_unread not implemented for real client"))
    }

    async fn subscribed(&self, limit: usize) -> Result<Listing<Subreddit>> {
        endpoints::subscribed(self, limit).await
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
