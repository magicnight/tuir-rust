//! Vim-style keybindings

use crossterm::event::KeyCode;

/// Key action mapping
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyAction {
    Quit,
    Refresh,
    Open,
    VoteUp,
    VoteDown,
    NextItem,
    PrevItem,
    Top,
    Bottom,
    GoTo,
    Back,
    Help,
    ScrollUp,
    ScrollDown,
}

impl KeyAction {
    /// Convert a key code to an action using vim-style bindings
    pub fn from_key(code: KeyCode, modifiers: crossterm::event::KeyModifiers) -> Option<Self> {
        let ctrl = modifiers.contains(crossterm::event::KeyModifiers::CONTROL);
        match (code, ctrl) {
            (KeyCode::Char('q'), false) => Some(Self::Quit),
            (KeyCode::Char('r'), false) => Some(Self::Refresh),
            (KeyCode::Char('o'), false) => Some(Self::Open),
            (KeyCode::Char('a'), false) => Some(Self::VoteUp),
            (KeyCode::Char('z'), false) => Some(Self::VoteDown),
            (KeyCode::Char('j'), false) => Some(Self::NextItem),
            (KeyCode::Char('k'), false) => Some(Self::PrevItem),
            (KeyCode::Char('g'), false) => Some(Self::Top),
            (KeyCode::Char('G'), false) => Some(Self::Bottom),
            (KeyCode::Char('/'), false) => Some(Self::GoTo),
            (KeyCode::Esc, _) => Some(Self::Back),
            (KeyCode::Char('?'), false) => Some(Self::Help),
            (KeyCode::Char('h'), false) => Some(Self::ScrollUp),
            (KeyCode::Char('l'), false) => Some(Self::ScrollDown),
            _ => None,
        }
    }
}
