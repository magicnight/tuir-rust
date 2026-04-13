//! Theme system for tuir
//!
//! Parses theme configuration files. Outputs raw color/attribute values
//! that can be converted to ratatui Style in the tuir-tui crate.

use std::collections::HashMap;
use std::path::Path;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ThemeError {
    #[error("Failed to parse theme file: {0}")]
    ParseError(String),
    #[error("Missing [theme] section")]
    MissingSection,
    #[error("Invalid color: {0}")]
    InvalidColor(String),
    #[error("Invalid attribute: {0}")]
    InvalidAttribute(String),
    #[error("Unknown element: {0}")]
    UnknownElement(String),
}

/// Theme element identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ThemeElement {
    // Modifiers
    Normal,
    Selected,
    SelectedCursor,
    // Page elements
    TitleBar,
    OrderBar,
    OrderBarHighlight,
    HelpBar,
    Prompt,
    NoticeInfo,
    NoticeLoading,
    NoticeError,
    NoticeSuccess,
    // Cursor elements
    CursorBlock,
    CursorBar1,
    CursorBar2,
    CursorBar3,
    CursorBar4,
    // Submission/Comment elements
    CommentAuthor,
    CommentAuthorSelf,
    CommentCount,
    CommentText,
    Created,
    Downvote,
    Gold,
    HiddenCommentExpand,
    HiddenCommentText,
    MultiredditName,
    MultiredditText,
    NeutralVote,
    NSFW,
    Saved,
    Hidden,
    Score,
    Separator,
    Stickied,
    SubscriptionName,
    SubscriptionText,
    SubmissionAuthor,
    SubmissionFlair,
    SubmissionSubreddit,
    SubmissionText,
    SubmissionTitle,
    SubmissionTitleSeen,
    Upvote,
    Link,
    LinkSeen,
    UserFlair,
    New,
    Distinguished,
    // Message elements
    MessageSubject,
    MessageLink,
    MessageAuthor,
    MessageSubreddit,
    MessageText,
}

impl ThemeElement {
    /// Parse element name from string
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "Normal" => Some(Self::Normal),
            "Selected" => Some(Self::Selected),
            "SelectedCursor" => Some(Self::SelectedCursor),
            "TitleBar" => Some(Self::TitleBar),
            "OrderBar" => Some(Self::OrderBar),
            "OrderBarHighlight" => Some(Self::OrderBarHighlight),
            "HelpBar" => Some(Self::HelpBar),
            "Prompt" => Some(Self::Prompt),
            "NoticeInfo" => Some(Self::NoticeInfo),
            "NoticeLoading" => Some(Self::NoticeLoading),
            "NoticeError" => Some(Self::NoticeError),
            "NoticeSuccess" => Some(Self::NoticeSuccess),
            "CursorBlock" => Some(Self::CursorBlock),
            "CursorBar1" => Some(Self::CursorBar1),
            "CursorBar2" => Some(Self::CursorBar2),
            "CursorBar3" => Some(Self::CursorBar3),
            "CursorBar4" => Some(Self::CursorBar4),
            "CommentAuthor" => Some(Self::CommentAuthor),
            "CommentAuthorSelf" => Some(Self::CommentAuthorSelf),
            "CommentCount" => Some(Self::CommentCount),
            "CommentText" => Some(Self::CommentText),
            "Created" => Some(Self::Created),
            "Downvote" => Some(Self::Downvote),
            "Gold" => Some(Self::Gold),
            "HiddenCommentExpand" => Some(Self::HiddenCommentExpand),
            "HiddenCommentText" => Some(Self::HiddenCommentText),
            "MultiredditName" => Some(Self::MultiredditName),
            "MultiredditText" => Some(Self::MultiredditText),
            "NeutralVote" => Some(Self::NeutralVote),
            "NSFW" => Some(Self::NSFW),
            "Saved" => Some(Self::Saved),
            "Hidden" => Some(Self::Hidden),
            "Score" => Some(Self::Score),
            "Separator" => Some(Self::Separator),
            "Stickied" => Some(Self::Stickied),
            "SubscriptionName" => Some(Self::SubscriptionName),
            "SubscriptionText" => Some(Self::SubscriptionText),
            "SubmissionAuthor" => Some(Self::SubmissionAuthor),
            "SubmissionFlair" => Some(Self::SubmissionFlair),
            "SubmissionSubreddit" => Some(Self::SubmissionSubreddit),
            "SubmissionText" => Some(Self::SubmissionText),
            "SubmissionTitle" => Some(Self::SubmissionTitle),
            "SubmissionTitleSeen" => Some(Self::SubmissionTitleSeen),
            "Upvote" => Some(Self::Upvote),
            "Link" => Some(Self::Link),
            "LinkSeen" => Some(Self::LinkSeen),
            "UserFlair" => Some(Self::UserFlair),
            "New" => Some(Self::New),
            "Distinguished" => Some(Self::Distinguished),
            "MessageSubject" => Some(Self::MessageSubject),
            "MessageLink" => Some(Self::MessageLink),
            "MessageAuthor" => Some(Self::MessageAuthor),
            "MessageSubreddit" => Some(Self::MessageSubreddit),
            "MessageText" => Some(Self::MessageText),
            _ => None,
        }
    }
}

/// Text styling attributes (similar to ncurses A_* constants)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct StyleAttributes(u8);

impl StyleAttributes {
    pub const BOLD: u8 = 1 << 0;
    pub const REVERSE: u8 = 1 << 1;
    pub const UNDERLINE: u8 = 1 << 2;
    pub const STANDOUT: u8 = 1 << 3;

    pub fn is_bold(&self) -> bool {
        self.0 & Self::BOLD != 0
    }

    pub fn is_reverse(&self) -> bool {
        self.0 & Self::REVERSE != 0
    }

    pub fn is_underline(&self) -> bool {
        self.0 & Self::UNDERLINE != 0
    }

    pub fn is_standout(&self) -> bool {
        self.0 & Self::STANDOUT != 0
    }
}

/// Raw ANSI color code (0-255 for 256-color terminals)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Color {
    pub ansi: u8,
}

impl Color {
    pub const RESET: u8 = 0;
    pub const BLACK: u8 = 1;
    pub const RED: u8 = 2;
    pub const GREEN: u8 = 3;
    pub const YELLOW: u8 = 4;
    pub const BLUE: u8 = 5;
    pub const MAGENTA: u8 = 6;
    pub const CYAN: u8 = 7;
    pub const WHITE: u8 = 8;
}

/// A single theme style definition (foreground, background, attributes)
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ThemeStyle {
    pub fg: Option<u8>,
    pub bg: Option<u8>,
    pub attrs: StyleAttributes,
}

/// Parse ANSI color name to color code
pub fn parse_ansi_color(s: &str) -> Result<u8, ThemeError> {
    match s {
        "-" | "default" => Ok(0), // RESET/default
        "black" => Ok(1),
        "red" => Ok(2),
        "green" => Ok(3),
        "yellow" => Ok(4),
        "blue" => Ok(5),
        "magenta" => Ok(6),
        "cyan" => Ok(7),
        "white" | "light_gray" => Ok(8),
        "dark_gray" => Ok(9),
        "bright_red" => Ok(10),
        "bright_green" => Ok(11),
        "bright_yellow" => Ok(12),
        "bright_blue" => Ok(13),
        "bright_magenta" => Ok(14),
        "bright_cyan" => Ok(15),
        "bright_white" => Ok(16),
        _ if s.starts_with("ansi_") => {
            let num: u8 = s[5..]
                .parse()
                .map_err(|_| ThemeError::InvalidColor(s.to_string()))?;
            Ok(num)
        }
        _ => Err(ThemeError::InvalidColor(s.to_string())),
    }
}

/// Parse style attributes
pub fn parse_attributes(s: &str) -> Result<StyleAttributes, ThemeError> {
    let mut attrs = StyleAttributes::default();
    for part in s.split('+') {
        match part.trim() {
            "-" | "" | "normal" => {}
            "bold" => attrs.0 |= StyleAttributes::BOLD,
            "reverse" => attrs.0 |= StyleAttributes::REVERSE,
            "underline" => attrs.0 |= StyleAttributes::UNDERLINE,
            "standout" => attrs.0 |= StyleAttributes::STANDOUT,
            other => return Err(ThemeError::InvalidAttribute(other.to_string())),
        }
    }
    Ok(attrs)
}

/// Parse a theme line in the format: `<element> = <fg> <bg> <attrs>`
pub fn parse_theme_line(line: &str) -> Result<(ThemeElement, ThemeStyle), ThemeError> {
    let line = line.trim();
    if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
        return Err(ThemeError::ParseError("Empty or comment line".to_string()));
    }

    // Split on = to get element name and value
    let parts: Vec<&str> = line.splitn(2, '=').collect();
    if parts.len() != 2 {
        return Err(ThemeError::ParseError(format!(
            "Invalid line format: {line}"
        )));
    }

    let element_name = parts[0].trim();
    let element = ThemeElement::parse(element_name)
        .ok_or_else(|| ThemeError::UnknownElement(element_name.to_string()))?;

    // Parse the value: fg bg attrs
    let value = parts[1].trim();
    let tokens: Vec<&str> = value.split_whitespace().collect();

    if tokens.len() < 2 {
        return Err(ThemeError::ParseError(format!(
            "Not enough tokens: {value}"
        )));
    }

    let fg = parse_ansi_color(tokens[0])?;
    let bg = parse_ansi_color(tokens[1])?;
    let attrs = if tokens.len() >= 3 {
        parse_attributes(tokens[2])?
    } else {
        StyleAttributes::default()
    };

    Ok((
        element,
        ThemeStyle {
            fg: Some(fg),
            bg: Some(bg),
            attrs,
        },
    ))
}

/// A complete theme
#[derive(Debug, Clone)]
pub struct Theme {
    /// Theme name
    pub name: String,
    /// Source of the theme (built-in, preset, installed, custom)
    pub source: ThemeSource,
    /// Element styles
    pub elements: HashMap<ThemeElement, ThemeStyle>,
    /// Whether to use color
    pub use_color: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThemeSource {
    BuiltIn,
    Preset,
    Installed,
    Custom,
}

impl Theme {
    /// Load a theme from a file
    pub fn from_file(path: impl AsRef<Path>) -> Result<Self, ThemeError> {
        let content = std::fs::read_to_string(path.as_ref())
            .map_err(|e| ThemeError::ParseError(e.to_string()))?;

        let name = path
            .as_ref()
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("custom")
            .to_string();

        Self::parse(&content, name, ThemeSource::Custom)
    }

    /// Parse theme content
    pub fn parse(content: &str, name: String, source: ThemeSource) -> Result<Self, ThemeError> {
        let mut elements = HashMap::new();
        let mut in_theme_section = false;

        for line in content.lines() {
            let trimmed = line.trim();

            // Skip comments
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }

            // Check for section header
            if trimmed == "[theme]" {
                in_theme_section = true;
                continue;
            }

            if trimmed.starts_with('[') && trimmed.ends_with(']') {
                // New section
                in_theme_section = false;
                continue;
            }

            if in_theme_section {
                match parse_theme_line(line) {
                    Ok((element, style)) => {
                        elements.insert(element, style);
                    }
                    Err(ThemeError::UnknownElement(_)) => {
                        // Skip unknown elements
                        continue;
                    }
                    Err(e) => return Err(e),
                }
            }
        }

        if elements.is_empty() && source != ThemeSource::BuiltIn {
            return Err(ThemeError::MissingSection);
        }

        Ok(Self {
            name,
            source,
            elements,
            use_color: true,
        })
    }

    /// Get a theme element's style
    pub fn get(&self, element: ThemeElement) -> ThemeStyle {
        self.elements.get(&element).cloned().unwrap_or_default()
    }
}

/// Built-in themes
impl Theme {
    /// Solarized Dark theme
    pub fn solarized_dark() -> Self {
        Self::parse(
            include_str!("builtin_themes/solarized-dark.cfg"),
            "solarized-dark".to_string(),
            ThemeSource::BuiltIn,
        )
        .expect("Built-in solarized-dark theme should parse")
    }

    /// Solarized Light theme
    pub fn solarized_light() -> Self {
        Self::parse(
            include_str!("builtin_themes/solarized-light.cfg"),
            "solarized-light".to_string(),
            ThemeSource::BuiltIn,
        )
        .expect("Built-in solarized-light theme should parse")
    }

    /// Molokai theme
    pub fn molokai() -> Self {
        Self::parse(
            include_str!("builtin_themes/molokai.cfg"),
            "molokai".to_string(),
            ThemeSource::BuiltIn,
        )
        .expect("Built-in molokai theme should parse")
    }

    /// Papercolor theme
    pub fn papercolor() -> Self {
        Self::parse(
            include_str!("builtin_themes/papercolor.cfg"),
            "papercolor".to_string(),
            ThemeSource::BuiltIn,
        )
        .expect("Built-in papercolor theme should parse")
    }

    /// Get a built-in theme by name
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "solarized" | "solarized-dark" => Some(Self::solarized_dark()),
            "solarized-light" => Some(Self::solarized_light()),
            "molokai" => Some(Self::molokai()),
            "papercolor" => Some(Self::papercolor()),
            "monochrome" | "mono" => None, // TODO: implement monochrome
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_ansi_color() {
        assert_eq!(parse_ansi_color("red").unwrap(), 2);
        assert_eq!(parse_ansi_color("-").unwrap(), 0);
        assert_eq!(parse_ansi_color("ansi_196").unwrap(), 196);
    }

    #[test]
    fn test_parse_attributes() {
        assert!(!parse_attributes("").unwrap().is_bold());
        assert!(parse_attributes("bold").unwrap().is_bold());
        assert!(parse_attributes("bold+reverse").unwrap().is_bold());
        assert!(parse_attributes("bold+reverse").unwrap().is_reverse());
    }

    #[test]
    fn test_theme_element_parse() {
        assert_eq!(ThemeElement::parse("Normal"), Some(ThemeElement::Normal));
        assert_eq!(
            ThemeElement::parse("TitleBar"),
            Some(ThemeElement::TitleBar)
        );
        assert_eq!(ThemeElement::parse("Invalid"), None);
    }

    #[test]
    fn test_solarized_dark() {
        let theme = Theme::solarized_dark();
        let style = theme.get(ThemeElement::TitleBar);
        // TitleBar should have fg=37 (cyan) in solarized dark
        assert!(style.fg.is_some());
    }
}
