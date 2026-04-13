//! tuir-tui — Terminal UI frontend for tuir
//!
//! Built on ratatui + crossterm.

pub mod app;
pub mod keymap;
pub mod pages;
pub mod terminal;
pub mod tui;
pub mod widgets;

pub use app::App;
pub use keymap::KeyAction;
pub use pages::{Page, PageAction, PageKind};
pub use terminal::{init, restore};
