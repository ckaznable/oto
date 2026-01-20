use ratatui::widgets::ScrollbarState;

#[derive(Default)]
pub struct QueueState {
    pub cursor_index: usize,
    pub playing_index: usize,
    pub scroll_state: ScrollbarState,
}

impl QueueState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Move cursor by steps (positive = forward, negative = backward), returns true if cursor changed
    pub fn move_cursor(&mut self, steps: i16, len: usize) -> bool {
        if len == 0 {
            return false;
        }

        let old_index = self.cursor_index;
        let new_index = if steps >= 0 {
            (self.cursor_index + steps as usize).min(len.saturating_sub(1))
        } else {
            self.cursor_index
                .saturating_sub(steps.unsigned_abs() as usize)
        };

        if new_index != old_index {
            self.cursor_index = new_index;
            self.scroll_state = self.scroll_state.position(self.cursor_index);
            true
        } else {
            false
        }
    }

    /// Set the playing index and optionally sync cursor to it
    pub fn set_playing_index(&mut self, index: usize, sync_cursor: bool) {
        self.playing_index = index;
        if sync_cursor {
            self.cursor_index = index;
            self.scroll_state = self.scroll_state.position(self.cursor_index);
        }
    }

    /// Update content length for scrollbar
    pub fn set_content_length(&mut self, len: usize) {
        self.scroll_state = self.scroll_state.content_length(len);
    }

    /// Get current cursor index
    pub fn cursor(&self) -> usize {
        self.cursor_index
    }

    /// Get current playing index
    pub fn playing(&self) -> usize {
        self.playing_index
    }
}

#[derive(Default)]
pub struct UiState {
    pub queue: QueueState,
}
