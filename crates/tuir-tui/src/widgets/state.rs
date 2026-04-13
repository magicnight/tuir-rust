//! Widget state types

use ratatui::widgets::ListState;

/// List state wrapper with selection tracking
#[derive(Default, Clone)]
pub struct SelectableListState {
    pub state: ListState,
    pub items_len: usize,
}

impl SelectableListState {
    pub fn new(items_len: usize) -> Self {
        Self {
            state: ListState::default().with_selected(Some(0)),
            items_len,
        }
    }

    pub fn selected(&self) -> Option<usize> {
        self.state.selected()
    }

    pub fn select(&mut self, idx: usize) {
        if idx < self.items_len {
            self.state.select(Some(idx));
        }
    }

    pub fn move_up(&mut self) {
        let current = self.state.selected().unwrap_or(0);
        if current > 0 {
            self.state.select(Some(current - 1));
        }
    }

    pub fn move_down(&mut self) {
        let current = self.state.selected().unwrap_or(0);
        if current < self.items_len.saturating_sub(1) {
            self.state.select(Some(current + 1));
        }
    }

    pub fn move_to_top(&mut self) {
        if self.items_len > 0 {
            self.state.select(Some(0));
        }
    }

    pub fn move_to_bottom(&mut self) {
        if self.items_len > 0 {
            self.state.select(Some(self.items_len - 1));
        }
    }
}
