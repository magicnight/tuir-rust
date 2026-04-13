//! Reddit API client
//!
//! Thin REST client wrapping Reddit's JSON API.

pub mod client;
pub mod endpoints;
pub mod models;

pub use client::RedditClient;
