use std::io::Cursor;

use anyhow::Result;
use image::ImageReader;
use ratatui::widgets::ScrollbarState;
use ratatui_image::{picker::Picker, protocol::StatefulProtocol};

use crate::{media::TracksTree, tui::image::UnCachedProtocol};

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
pub struct DevicesListState {
    pub cursor_index: usize,
    pub scroll_state: ScrollbarState,
}

impl DevicesListState {
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

    /// Update content length for scrollbar
    pub fn set_content_length(&mut self, len: usize) {
        self.scroll_state = self.scroll_state.content_length(len);
    }

    /// Get current cursor index
    pub fn cursor(&self) -> usize {
        self.cursor_index
    }
}

pub struct MediaInfoState {
    pub picker: Option<Picker>,
    pub cover: Option<StatefulProtocol>,
}

impl Default for MediaInfoState {
    fn default() -> Self {
        Self {
            picker: Picker::from_query_stdio().ok(),
            cover: None,
        }
    }
}

impl MediaInfoState {
    pub fn set_cover(&mut self, data: &[u8]) -> Result<()> {
        let img = ImageReader::new(Cursor::new(data))
            .with_guessed_format()?
            .decode()?
            .into_rgb8();
        let dyimg = image::DynamicImage::ImageRgb8(img);

        let image = self.picker.as_ref().map(|p| p.new_resize_protocol(dyimg));
        if let Some(mut cover) = self.cover.take() {
            cover.last_encoding_result();
        }

        self.cover = image;
        Ok(())
    }
}

#[derive(Default)]
pub struct TracksState {
    pub tree: TracksTree,
    pub playlist: Vec<usize>,
    pub artist_index: usize,
    pub album_inedx: usize,
    pub track_index: usize,
    pub expand_index: usize,
    pub artist_scroll: ScrollbarState,
    pub album_scroll: ScrollbarState,
    pub track_scroll: ScrollbarState,
}

#[derive(Default)]
pub struct UiState {
    pub queue: QueueState,
    pub devices: DevicesListState,
    pub media_info: MediaInfoState,
    pub cover: Option<UnCachedProtocol>,
    pub tracks: TracksState,
    pub expand_index: usize,
}
