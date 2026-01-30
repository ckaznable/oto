use std::io::Cursor;

use anyhow::Result;
use image::ImageReader;
use ratatui::widgets::{ListItem, Row, ScrollbarState};

#[derive(Default)]
pub struct CacheState {
    pub rows: Vec<Row<'static>>,
    pub list_items: Vec<ListItem<'static>>,
}
use ratatui_image::{picker::Picker, protocol::StatefulProtocol};

use crate::{media::TracksTree, tui::image::UnCachedProtocol};

pub trait CursorMovable {
    fn cursor_index(&self) -> usize;
    fn set_cursor_index(&mut self, index: usize);
    fn set_scroll_position(&mut self, position: usize);

    fn move_cursor(&mut self, steps: i16, len: usize) -> bool {
        if len == 0 {
            return false;
        }

        let old_index = self.cursor_index();
        let new_index = if steps >= 0 {
            (old_index + steps as usize).min(len.saturating_sub(1))
        } else {
            old_index.saturating_sub(steps.unsigned_abs() as usize)
        };

        if new_index != old_index {
            self.set_cursor_index(new_index);
            self.set_scroll_position(new_index);
            true
        } else {
            false
        }
    }
}

#[derive(Default)]
pub struct QueueState {
    pub cursor_index: usize,
    pub playing_index: usize,
    pub scroll_state: ScrollbarState,
}

impl CursorMovable for QueueState {
    fn cursor_index(&self) -> usize {
        self.cursor_index
    }

    fn set_cursor_index(&mut self, index: usize) {
        self.cursor_index = index;
    }

    fn set_scroll_position(&mut self, position: usize) {
        self.scroll_state = self.scroll_state.position(position);
    }
}

impl QueueState {
    pub fn new() -> Self {
        Self::default()
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

impl CursorMovable for DevicesListState {
    fn cursor_index(&self) -> usize {
        self.cursor_index
    }

    fn set_cursor_index(&mut self, index: usize) {
        self.cursor_index = index;
    }

    fn set_scroll_position(&mut self, position: usize) {
        self.scroll_state = self.scroll_state.position(position);
    }
}

impl DevicesListState {
    pub fn new() -> Self {
        Self::default()
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
pub enum TracksMode {
    #[default]
    Artist,
    Album,
}

#[derive(Default)]
pub struct TracksState {
    pub tree_by_artist: TracksTree,
    pub tree_by_album: TracksTree,
    pub playlist: Vec<usize>,
    pub primary_index: usize,
    pub secondary_index: usize,
    pub track_index: usize,
    pub expand_index: usize,
    pub primary_scroll: ScrollbarState,
    pub secondary_scroll: ScrollbarState,
    pub track_scroll: ScrollbarState,
    pub mode: TracksMode,
}

impl CursorMovable for TracksState {
    fn cursor_index(&self) -> usize {
        match self.expand_index {
            0 => self.primary_index,
            1 => self.secondary_index,
            _ => self.track_index,
        }
    }

    fn set_cursor_index(&mut self, index: usize) {
        match self.expand_index {
            0 => self.primary_index = index,
            1 => self.secondary_index = index,
            _ => self.track_index = index,
        }
    }

    fn set_scroll_position(&mut self, position: usize) {
        match self.expand_index {
            0 => self.primary_scroll = self.primary_scroll.position(position),
            1 => self.secondary_scroll = self.secondary_scroll.position(position),
            _ => self.track_scroll = self.track_scroll.position(position),
        }
    }
}

impl TracksState {
    pub fn current_tree(&self) -> &TracksTree {
        match self.mode {
            TracksMode::Artist => &self.tree_by_artist,
            TracksMode::Album => &self.tree_by_album,
        }
    }

    #[allow(clippy::len_without_is_empty)]
    pub fn len(&self) -> Option<usize> {
        let tree = self.current_tree();
        let len = match self.expand_index {
            0 => tree.len(),
            1 => tree.get(self.primary_index)?.1.len(),
            _ => tree
                .get(self.primary_index)?
                .1
                .get(self.secondary_index)?
                .1
                .len(),
        };

        Some(len)
    }
}

#[derive(Default)]
pub struct UiState {
    pub queue: QueueState,
    pub devices: DevicesListState,
    pub media_info: MediaInfoState,
    pub cover: Option<UnCachedProtocol>,
    pub tracks: TracksState,
    pub expand_index: usize,
    pub cache: CacheState,
}
