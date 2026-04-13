//! Application state machine and event loop

/// Main application state
pub struct App {
    running: bool,
}

impl App {
    /// Create a new application instance
    pub fn new() -> Self {
        Self { running: true }
    }

    /// Main event loop tick — returns false when the app should exit
    pub fn tick(&mut self) -> bool {
        self.running
    }

    /// Signal the app to exit
    pub fn quit(&mut self) {
        self.running = false;
    }

    /// Check if the app is running
    pub fn is_running(&self) -> bool {
        self.running
    }
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}
