//! tuir-core — Core library for tuir (Terminal UI for Reddit)
//!
//! Provides configuration, OAuth, Reddit API client, mailcap parsing,
//! clipboard, and content rendering.

pub mod config;
pub mod content;
pub mod mailcap;
pub mod media;
pub mod oauth;
pub mod reddit;
pub mod theme;

pub use anyhow::Result;
pub use config::Config;
pub use media::{detect_media, MediaKind, MediaRef, MediaStyle};
pub use oauth::OAuth;
pub use reddit::RedditClient;
pub use theme::{Theme, ThemeElement, ThemeError};
