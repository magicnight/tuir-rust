//! Configuration management
//!
//! Handles INI config file parsing, XDG paths, and CLI argument merging.
//! Compatible with the original tuir.cfg format.

use configparser::ini::Ini;
use std::collections::HashMap;
use std::path::PathBuf;

/// Application configuration
#[derive(Debug, Clone, Default)]
pub struct Config {
    /// General settings
    pub general: GeneralConfig,
    /// Reddit API settings
    pub reddit: RedditConfig,
    /// Appearance settings
    pub appearance: AppearanceConfig,
    /// Key bindings
    pub bindings: HashMap<String, String>,
}

#[derive(Debug, Clone)]
pub struct GeneralConfig {
    /// Data directory for tuir (token, history, etc.)
    pub data_dir: PathBuf,
    /// Log file path
    pub log_file: Option<String>,
    /// Editor for composing posts/comments
    pub editor: Option<String>,
    /// ASCII-only mode (disable unicode)
    pub ascii: bool,
    /// Monochrome mode (disable colors)
    pub monochrome: bool,
    /// Flash on invalid action
    pub flash: bool,
    /// Default subreddit on startup
    pub subreddit: String,
    /// Persistent authentication (store token between sessions)
    pub persistent: bool,
    /// Auto-login on startup
    pub autologin: bool,
    /// Clear stored credentials on startup
    pub clear_auth: bool,
    /// Max history entries
    pub history_size: usize,
    /// Open external links via mailcap
    pub enable_media: bool,
    /// Max columns for comments
    pub max_comment_cols: usize,
    /// Max columns for pager
    pub max_pager_cols: Option<usize>,
    /// Hide username when logged in
    pub hide_username: bool,
    /// Open links in new browser window
    pub force_new_browser_window: bool,
    /// Clipboard command
    pub clipboard_cmd: Option<String>,
}

#[derive(Debug, Clone)]
pub struct RedditConfig {
    /// OAuth client ID (reddit app)
    pub oauth_client_id: Option<String>,
    /// OAuth client secret
    pub oauth_client_secret: Option<String>,
    /// OAuth redirect URI
    pub oauth_redirect_uri: String,
    /// OAuth redirect port
    pub oauth_redirect_port: u16,
    /// OAuth scope (comma-separated)
    pub oauth_scope: String,
    /// Imgur API client ID for image extraction
    pub imgur_client_id: Option<String>,
}

#[derive(Debug, Clone)]
pub struct AppearanceConfig {
    /// Theme name or path
    pub theme: Option<String>,
    /// Link formatting (default, compact, etc.)
    pub look_and_feel: String,
    /// Compact submission format string
    pub subreddit_format: Option<String>,
}

impl Default for GeneralConfig {
    fn default() -> Self {
        Self {
            data_dir: dirs::data_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join("tuir"),
            log_file: None,
            editor: std::env::var("EDITOR").ok(),
            ascii: false,
            monochrome: false,
            flash: true,
            subreddit: "front".to_string(),
            persistent: true,
            autologin: true,
            clear_auth: false,
            history_size: 200,
            enable_media: false,
            max_comment_cols: 120,
            max_pager_cols: None,
            hide_username: false,
            force_new_browser_window: false,
            clipboard_cmd: None,
        }
    }
}

impl Default for RedditConfig {
    fn default() -> Self {
        Self {
            oauth_client_id: None,
            oauth_client_secret: None,
            oauth_redirect_uri: "http://127.0.0.1:65000/".to_string(),
            oauth_redirect_port: 65000,
            oauth_scope: "edit,history,identity,mysubreddits,privatemessages,read,report,save,submit,subscribe,vote"
                .to_string(),
            imgur_client_id: None,
        }
    }
}

impl Default for AppearanceConfig {
    fn default() -> Self {
        Self {
            theme: Some("solarized".to_string()),
            look_and_feel: "default".to_string(),
            subreddit_format: None,
        }
    }
}

impl Config {
    /// Load configuration from default XDG locations
    pub fn load() -> anyhow::Result<Self> {
        let config_paths = Self::config_paths();
        for path in &config_paths {
            if path.exists() {
                return Self::from_file(path);
            }
        }
        // Return defaults if no config file found
        Ok(Config::default())
    }

    /// Load configuration from a specific file
    pub fn from_file(path: impl Into<PathBuf>) -> anyhow::Result<Self> {
        let path_str = path.into().to_string_lossy().into_owned();
        let mut ini = Ini::new();
        ini.read(path_str)
            .map_err(|e| anyhow::anyhow!("Failed to parse config: {e}"))?;

        Self::parse_ini(&ini)
    }

    /// Parse INI into Config struct
    fn parse_ini(ini: &Ini) -> anyhow::Result<Self> {
        // Determine which section to use (tuir or rtv for backwards compat)
        // Try 'tuir' first, then 'rtv'
        let section = if ini.get("tuir", "subreddit").is_some() {
            "tuir"
        } else if ini.get("rtv", "subreddit").is_some() {
            "rtv"
        } else {
            // No known section found, use defaults
            return Ok(Config::default());
        };

        // Helper to get string option
        let get_str = |ini: &Ini, sec: &str, key: &str, default: &str| -> String {
            ini.get(sec, key).unwrap_or_else(|| default.to_string())
        };

        // Helper to get bool option
        let get_bool = |ini: &Ini, sec: &str, key: &str, default: bool| -> bool {
            ini.getbool(sec, key).ok().flatten().unwrap_or(default)
        };

        // Helper to get usize option
        let get_usize = |ini: &Ini, sec: &str, key: &str, default: usize| -> usize {
            ini.getint(sec, key)
                .unwrap_or(Some(default as i64))
                .unwrap_or(default as i64) as usize
        };

        // Helper to get optional usize
        let get_opt_usize = |ini: &Ini, sec: &str, key: &str| -> Option<usize> {
            ini.getint(sec, key).unwrap_or(None).map(|v| v as usize)
        };

        // Helper to get u16 option
        let get_u16 = |ini: &Ini, sec: &str, key: &str, default: u16| -> u16 {
            ini.getint(sec, key)
                .unwrap_or(Some(default as i64))
                .unwrap_or(default as i64) as u16
        };

        // Helper to get optional string
        let get_opt_str = |ini: &Ini, sec: &str, key: &str| -> Option<String> { ini.get(sec, key) };

        // Parse general settings
        let general = GeneralConfig {
            data_dir: dirs::data_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join("tuir"),
            log_file: get_opt_str(ini, section, "log"),
            editor: get_opt_str(ini, section, "clipboard_cmd"),
            ascii: get_bool(ini, section, "ascii", false),
            monochrome: get_bool(ini, section, "monochrome", false),
            flash: get_bool(ini, section, "flash", true),
            subreddit: get_str(ini, section, "subreddit", "front"),
            persistent: get_bool(ini, section, "persistent", true),
            autologin: get_bool(ini, section, "autologin", true),
            clear_auth: get_bool(ini, section, "clear_auth", false),
            history_size: get_usize(ini, section, "history_size", 200),
            enable_media: get_bool(ini, section, "enable_media", false),
            max_comment_cols: get_usize(ini, section, "max_comment_cols", 120),
            max_pager_cols: get_opt_usize(ini, section, "max_pager_cols"),
            hide_username: get_bool(ini, section, "hide_username", false),
            force_new_browser_window: get_bool(ini, section, "force_new_browser_window", false),
            clipboard_cmd: get_opt_str(ini, section, "clipboard_cmd"),
        };

        // Parse Reddit/OAuth settings
        let reddit = RedditConfig {
            oauth_client_id: get_opt_str(ini, section, "oauth_client_id"),
            oauth_client_secret: get_opt_str(ini, section, "oauth_client_secret"),
            oauth_redirect_uri: get_str(
                ini,
                section,
                "oauth_redirect_uri",
                "http://127.0.0.1:65000/",
            ),
            oauth_redirect_port: get_u16(ini, section, "oauth_redirect_port", 65000),
            oauth_scope: get_str(
                ini,
                section,
                "oauth_scope",
                "edit,history,identity,mysubreddits,privatemessages,read,report,save,submit,subscribe,vote",
            ),
            imgur_client_id: get_opt_str(ini, section, "imgur_client_id"),
        };

        // Parse appearance settings
        let appearance = AppearanceConfig {
            theme: get_opt_str(ini, section, "theme"),
            look_and_feel: get_str(ini, section, "look_and_feel", "default"),
            subreddit_format: get_opt_str(ini, section, "subreddit_format"),
        };

        // Parse bindings if present
        let mut bindings = HashMap::new();
        // Try to get bindings section (configparser may return empty if section doesn't exist)
        let binding_keys = ini
            .get("bindings", "")
            .map(|_| {
                // Section exists, get all keys
                let mut keys = Vec::new();
                // We need to iterate - configparser's get returns Option<String>
                // So we try known binding keys
                for key in &[
                    "UPVOTE",
                    "DOWNVOTE",
                    "MOVE_UP",
                    "MOVE_DOWN",
                    "EXIT",
                    "FORCE_EXIT",
                    "HELP",
                    "REFRESH",
                    "SAVE",
                    "OPEN_SUBREDDIT",
                    "SUBMISSION_OPEN_IN_BROWSER",
                ] {
                    if let Some(val) = ini.get("bindings", key) {
                        keys.push((key.to_string(), val));
                    }
                }
                keys
            })
            .unwrap_or_default();

        for (key, value) in binding_keys {
            bindings.insert(key, value);
        }

        Ok(Self {
            general,
            reddit,
            appearance,
            bindings,
        })
    }

    /// Return priority-ordered config file locations
    fn config_paths() -> Vec<PathBuf> {
        let xdg_config = std::env::var("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| dirs::config_dir().unwrap_or_else(|| PathBuf::from(".")));

        vec![
            xdg_config.join("tuir").join("tuir.cfg"),
            dirs::config_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join("tuir")
                .join("tuir.cfg"),
        ]
    }

    /// Get the config directory path
    pub fn config_dir() -> PathBuf {
        std::env::var("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| dirs::config_dir().unwrap_or_else(|| PathBuf::from(".")))
            .join("tuir")
    }

    /// Get the data directory path
    pub fn data_dir() -> PathBuf {
        dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("tuir")
    }

    /// Get the token file path
    pub fn token_file() -> PathBuf {
        Self::data_dir().join("refresh-token")
    }

    /// Get the history file path
    pub fn history_file() -> PathBuf {
        Self::data_dir().join("history.log")
    }

    /// Get the themes directory path
    pub fn themes_dir() -> PathBuf {
        Self::config_dir().join("themes")
    }

    /// Resolve the configured theme into a parsed [`crate::theme::Theme`].
    ///
    /// Lookup order:
    /// 1. `<themes_dir>/<name>.cfg` (user override)
    /// 2. The matching built-in theme by name
    ///
    /// Returns `None` when no theme is configured or the configured name
    /// matches neither a file nor a built-in.
    pub fn resolve_theme(&self) -> Option<crate::theme::Theme> {
        let name = self.appearance.theme.as_deref()?.trim();
        if name.is_empty() {
            return None;
        }

        let user_path = Self::themes_dir().join(format!("{name}.cfg"));
        if user_path.exists() {
            match crate::theme::Theme::from_file(&user_path) {
                Ok(theme) => return Some(theme),
                Err(err) => {
                    tracing::warn!("failed to load theme {}: {err}", user_path.display());
                }
            }
        }

        crate::theme::Theme::from_name(name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_default() {
        let config = Config::default();
        assert_eq!(config.general.subreddit, "front");
        assert_eq!(config.general.history_size, 200);
        assert!(config.general.persistent);
    }

    #[test]
    fn test_config_parse_ini() {
        let ini_content = r#"
[tuir]
subreddit = rust
ascii = true
history_size = 100
"#;
        let mut ini = Ini::new();
        ini.read(ini_content.to_string()).unwrap();
        let config = Config::parse_ini(&ini).unwrap();
        assert_eq!(config.general.subreddit, "rust");
        assert!(config.general.ascii);
        assert_eq!(config.general.history_size, 100);
    }
}
