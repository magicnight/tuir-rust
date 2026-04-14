//! Bridge from [`tuir_core::theme::Theme`] (terminal-agnostic) to ratatui
//! [`Style`]s consumed by the page render code.
//!
//! The core crate parses theme files into a generic `ThemeStyle { fg, bg,
//! attrs }` where the colors are raw ANSI 256 indices (0 = default). This
//! module materializes the handful of styles the TUI actually uses into a
//! single [`AppTheme`] struct that pages clone via `Arc` and read from at
//! render time.
//!
//! When no theme is configured, [`AppTheme::default`] returns a built-in
//! palette that matches the hard-coded colors the pages used before the
//! theme system was wired up — so unconfigured users see no visual change.

use ratatui::style::{Color, Modifier, Style};
use tuir_core::theme::{Theme, ThemeElement, ThemeStyle};

/// Resolved styles for the subset of theme elements the TUI currently uses.
///
/// More fields can be added incrementally as pages are migrated off
/// hard-coded colors.
#[derive(Debug, Clone)]
pub struct AppTheme {
    pub name: String,
    pub header: Style,
    pub footer: Style,
    pub selected: Style,
    pub upvote: Style,
    pub downvote: Style,
    pub link: Style,
    pub nsfw: Style,
    pub stickied: Style,
    pub author: Style,
    pub muted: Style,
    /// 4-cycle of foreground styles for comment body text, indexed by
    /// `depth % 4`. Maps to the `CursorBar1..4` theme elements — the
    /// same slot the original urwid tuir used for depth-indent bars.
    pub comment_depth: [Style; 4],
}

impl AppTheme {
    /// Materialize an [`AppTheme`] from a parsed core [`Theme`].
    pub fn from_core(theme: &Theme) -> Self {
        Self {
            name: theme.name.clone(),
            header: to_style(&theme.get(ThemeElement::TitleBar)),
            footer: to_style(&theme.get(ThemeElement::HelpBar)),
            selected: to_style(&theme.get(ThemeElement::Selected)),
            upvote: to_style(&theme.get(ThemeElement::Upvote)),
            downvote: to_style(&theme.get(ThemeElement::Downvote)),
            link: to_style(&theme.get(ThemeElement::Link)),
            nsfw: to_style(&theme.get(ThemeElement::NSFW)),
            stickied: to_style(&theme.get(ThemeElement::Stickied)),
            author: to_style(&theme.get(ThemeElement::SubmissionAuthor)),
            muted: to_style(&theme.get(ThemeElement::Created)),
            comment_depth: [
                to_style(&theme.get(ThemeElement::CursorBar1)),
                to_style(&theme.get(ThemeElement::CursorBar2)),
                to_style(&theme.get(ThemeElement::CursorBar3)),
                to_style(&theme.get(ThemeElement::CursorBar4)),
            ],
        }
    }
}

impl Default for AppTheme {
    /// Replicates the legacy hard-coded look so pages render identically
    /// when no `appearance.theme` entry is present in the config.
    fn default() -> Self {
        Self {
            name: "default".to_string(),
            header: Style::default()
                .bg(Color::Rgb(20, 20, 30))
                .add_modifier(Modifier::BOLD),
            footer: Style::default().bg(Color::Rgb(30, 30, 20)),
            selected: Style::default()
                .bg(Color::Rgb(40, 40, 40))
                .add_modifier(Modifier::BOLD),
            upvote: Style::default().fg(Color::Green),
            downvote: Style::default().fg(Color::Red),
            link: Style::default().fg(Color::Blue),
            nsfw: Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            stickied: Style::default().fg(Color::Green),
            author: Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
            muted: Style::default().fg(Color::DarkGray),
            comment_depth: [
                Style::default().fg(Color::Yellow),
                Style::default().fg(Color::Green),
                Style::default().fg(Color::Cyan),
                Style::default().fg(Color::Magenta),
            ],
        }
    }
}

/// Convert a core [`ThemeStyle`] into a ratatui [`Style`].
///
/// The fg/bg fields use a raw ANSI 256-color index where `0` means
/// "inherit / terminal default". Built-in themes always use `ansi_NNN` for
/// colors, so the indices line up with [`Color::Indexed`] directly.
pub fn to_style(s: &ThemeStyle) -> Style {
    let mut style = Style::default();
    if let Some(fg) = s.fg.and_then(non_default) {
        style = style.fg(Color::Indexed(fg));
    }
    if let Some(bg) = s.bg.and_then(non_default) {
        style = style.bg(Color::Indexed(bg));
    }
    let mut modifier = Modifier::empty();
    if s.attrs.is_bold() {
        modifier |= Modifier::BOLD;
    }
    if s.attrs.is_underline() {
        modifier |= Modifier::UNDERLINED;
    }
    if s.attrs.is_reverse() || s.attrs.is_standout() {
        modifier |= Modifier::REVERSED;
    }
    style.add_modifier(modifier)
}

fn non_default(value: u8) -> Option<u8> {
    if value == 0 {
        None
    } else {
        Some(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_theme_matches_legacy_header_bg() {
        let theme = AppTheme::default();
        assert_eq!(theme.header.bg, Some(Color::Rgb(20, 20, 30)));
        assert!(theme.header.add_modifier.contains(Modifier::BOLD));
    }

    #[test]
    fn from_core_loads_solarized_dark_title_bar() {
        // Solarized Dark TitleBar = ansi_37 - bold+reverse
        let core = Theme::solarized_dark();
        let app = AppTheme::from_core(&core);
        assert_eq!(app.header.fg, Some(Color::Indexed(37)));
        assert_eq!(app.header.bg, None);
        assert!(app.header.add_modifier.contains(Modifier::BOLD));
        assert!(app.header.add_modifier.contains(Modifier::REVERSED));
    }

    #[test]
    fn from_core_loads_molokai_normal() {
        // Molokai Normal = ansi_252 ansi_234 normal
        let core = Theme::molokai();
        let app = AppTheme::from_core(&core);
        // Selected uses ansi_252/ansi_236 — pick that one to verify bg index.
        assert_eq!(app.selected.fg, Some(Color::Indexed(252)));
        assert_eq!(app.selected.bg, Some(Color::Indexed(236)));
    }

    #[test]
    fn default_theme_has_four_distinct_comment_depth_colors() {
        let theme = AppTheme::default();
        let fgs: Vec<_> = theme.comment_depth.iter().map(|s| s.fg).collect();
        assert_eq!(
            fgs,
            vec![
                Some(Color::Yellow),
                Some(Color::Green),
                Some(Color::Cyan),
                Some(Color::Magenta),
            ]
        );
    }

    #[test]
    fn from_core_pulls_comment_depth_from_cursor_bars() {
        // Molokai CursorBar1..4 = ansi_141 / 197 / 154 / 208
        let core = Theme::molokai();
        let app = AppTheme::from_core(&core);
        assert_eq!(app.comment_depth[0].fg, Some(Color::Indexed(141)));
        assert_eq!(app.comment_depth[1].fg, Some(Color::Indexed(197)));
        assert_eq!(app.comment_depth[2].fg, Some(Color::Indexed(154)));
        assert_eq!(app.comment_depth[3].fg, Some(Color::Indexed(208)));
    }

    #[test]
    fn to_style_treats_zero_fg_as_inherit() {
        use tuir_core::theme::StyleAttributes;
        let style = to_style(&ThemeStyle {
            fg: Some(0),
            bg: Some(45),
            attrs: StyleAttributes::default(),
        });
        assert_eq!(style.fg, None);
        assert_eq!(style.bg, Some(Color::Indexed(45)));
    }
}
