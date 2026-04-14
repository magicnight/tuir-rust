//! Reddit API client
//!
//! Thin REST client wrapping Reddit's JSON API.

pub mod api;
pub mod client;
pub mod endpoints;
pub mod mock;
pub mod models;

pub use api::{RedditApi, SubmissionPayload};
pub use client::RedditClient;
pub use endpoints::*;
pub use mock::MockRedditClient;
pub use models::*;
