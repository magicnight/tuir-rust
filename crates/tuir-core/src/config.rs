//! Configuration management
//!
//! Handles INI config file parsing, XDG paths, and CLI argument merging.

use configparser::ini::Ini;
use serde::Deserialize;
use std::path::PathBuf;

/// Configuration struct representing tuir.cfg
#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub general: GeneralConfig,
    pub reddit: RedditConfig,
    pub appearance: AppearanceConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GeneralConfig {
    pub data_dir: Option<PathBuf>,
    pub log_dir: Option<PathBuf>,
    pub editor: Option<String>,
    pub open_link_in_browser: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RedditConfig {
    pub refresh_token: Option<String>,
    pub client_id: Option<String>,
    pub poll_rate: u64,
    pub max_items: usize,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AppearanceConfig {
    pub theme: Option<String>,
    pub enable_media: bool,
    pub link_numbers: bool,
}

impl Config {
    /// Load configuration from default XDG locations
    pub fn load() -> anyhow::Result<Self> {
        let config_paths = Self::config_paths();
        for path in config_paths {
            if path.exists() {
                return Self::from_file(&path);
            }
        }
        Ok(Config::default())
    }

    /// Load configuration from a specific file
    pub fn from_file(path: impl Into<PathBuf>) -> anyhow::Result<Self> {
        let path_str = path.into().to_string_lossy().into_owned();
        let mut ini = Ini::new();
        ini.read(path_str)
            .map_err(|e| anyhow::anyhow!("Failed to parse config: {e}"))?;
        // TODO: parse sections into Config struct
        Ok(Config::default())
    }

    /// Return priority-ordered config file locations
    fn config_paths() -> Vec<PathBuf> {
        vec![
            dirs::config_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join("tuir")
                .join("tuir.cfg"),
            dirs::config_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join("tuir")
                .join("config.ini"),
        ]
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            general: GeneralConfig {
                data_dir: dirs::data_dir().map(|p: PathBuf| p.join("tuir")),
                log_dir: dirs::data_dir().map(|p: PathBuf| p.join("tuir").join("logs")),
                editor: std::env::var("EDITOR").ok(),
                open_link_in_browser: false,
            },
            reddit: RedditConfig {
                refresh_token: None,
                client_id: None,
                poll_rate: 60,
                max_items: 100,
            },
            appearance: AppearanceConfig {
                theme: Some("solarized".to_string()),
                enable_media: true,
                link_numbers: true,
            },
        }
    }
}
