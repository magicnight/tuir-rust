//! tuir-core — Core library for tuir (Terminal UI for Reddit)
//!
//! Provides configuration, OAuth, Reddit API client, mailcap parsing,
//! clipboard, and content rendering.

pub mod config;
pub mod content;
pub mod mailcap;
pub mod oauth;
pub mod reddit;

pub use anyhow::Result;
pub use config::Config;
pub use oauth::OAuth;
pub use reddit::RedditClient;
