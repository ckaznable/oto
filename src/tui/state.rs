use std::{cell::RefCell, io::Cursor, rc::Rc};

use anyhow::Result;
use image::ImageReader;
use indexmap::IndexSet;
use ratatui::widgets::{ListItem, Row, ScrollbarState};
use rustc_hash::FxBuildHasher;

type FxIndexSet<T> = IndexSet<T, FxBuildHasher>;
pub type PickedPlaylistRef = Rc<RefCell<FxIndexSet<usize>>>;

use ratatui::text::Line;

#[derive(Default)]
pub struct CacheState {
    pub rows: Vec<Row<'static>>,
    pub list_items: Vec<ListItem<'static>>,
    pub tracks_items: Vec<(Line<'static>, bool)>,
}
use ratatui_image::{picker::Picker, protocol::StatefulProtocol};
use tui_input::Input;

use crate::{event::CursorMove, media::TracksTree, tui::image::UnCachedProtocol};

pub trait CursorMovable {
    fn cursor_index(&self) -> usize;
    fn set_cursor_index(&mut self, index: usize);
    fn set_scroll_position(&mut self, position: usize);

    fn move_to_start(&mut self) {
        self.set_cursor_index(0);
        self.set_scroll_position(0);
    }

    fn move_to_end(&mut self, len: usize) {
        self.set_cursor_index(len - 1);
        self.set_scroll_position(len - 1);
    }

    fn move_cursor(&mut self, steps: CursorMove, len: usize) -> bool {
        if len == 0 {
            return false;
        }

        use CursorMove::*;
        let old_index = self.cursor_index();
        let new_index = match steps {
            Steps(steps) => {
                if steps >= 0 {
                    if old_index == len - 1 {
                        0
                    } else {
                        (old_index + steps as usize).min(len.saturating_sub(1))
                    }
                } else if old_index == 0 {
                    len - 1
                } else {
                    old_index.saturating_sub(steps.unsigned_abs() as usize)
                }
            }
            Start => 0,
            End => len - 1,
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

pub struct TracksState {
    pub tree_by_artist: TracksTree,
    pub tree_by_album: TracksTree,
    pub playlist: PickedPlaylistRef,
    pub primary_index: usize,
    pub secondary_index: usize,
    pub track_index: usize,
    pub playlist_index: usize,
    pub expand_index: usize,
    pub primary_scroll: ScrollbarState,
    pub secondary_scroll: ScrollbarState,
    pub track_scroll: ScrollbarState,
    pub playlist_scroll: ScrollbarState,
    pub mode: TracksMode,
}

impl TracksState {
    pub fn new(playlist: PickedPlaylistRef) -> Self {
        Self {
            tree_by_artist: TracksTree::default(),
            tree_by_album: TracksTree::default(),
            playlist,
            primary_index: 0,
            secondary_index: 0,
            track_index: 0,
            playlist_index: 0,
            expand_index: 0,
            primary_scroll: ScrollbarState::default(),
            secondary_scroll: ScrollbarState::default(),
            track_scroll: ScrollbarState::default(),
            playlist_scroll: ScrollbarState::default(),
            mode: TracksMode::default(),
        }
    }
}

impl CursorMovable for TracksState {
    fn cursor_index(&self) -> usize {
        match self.expand_index {
            0 => self.primary_index,
            1 => self.secondary_index,
            2 => self.track_index,
            _ => self.playlist_index,
        }
    }

    fn set_cursor_index(&mut self, index: usize) {
        match self.expand_index {
            0 => self.primary_index = index,
            1 => self.secondary_index = index,
            2 => self.track_index = index,
            _ => self.playlist_index = index,
        }
    }

    fn set_scroll_position(&mut self, position: usize) {
        match self.expand_index {
            0 => self.primary_scroll = self.primary_scroll.position(position),
            1 => self.secondary_scroll = self.secondary_scroll.position(position),
            2 => self.track_scroll = self.track_scroll.position(position),
            _ => self.playlist_scroll = self.playlist_scroll.position(position),
        }
    }
}

impl TracksState {
    pub fn pick(&mut self) -> Option<()> {
        match self.expand_index {
            0 => self.pick_all_primary(self.is_primary_selected(self.primary_index)),
            1 => self.pick_all_secondary(self.is_secondary_selected(self.secondary_index)),
            2 => self.pick_track(),
            3 => {
                self.remove_from_playlist();
                None
            }
            _ => None,
        }
    }

    pub fn remove_from_playlist(&mut self) {
        self.playlist.borrow_mut().shift_remove_index(self.playlist_index);
    }

    pub fn pick_all_primary(&mut self, remove: bool) -> Option<()> {
        let tree = match self.mode {
            TracksMode::Artist => &self.tree_by_artist,
            TracksMode::Album => &self.tree_by_album,
        };

        let indices: Vec<usize> = tree
            .get(self.primary_index)?
            .1
            .iter()
            .flat_map(|(_, v)| v.iter().copied())
            .collect();

        let mut playlist = self.playlist.borrow_mut();
        for idx in indices {
            if remove {
                playlist.shift_remove(&idx);
            } else {
                playlist.insert(idx);
            }
        }

        Some(())
    }

    pub fn pick_all_secondary(&mut self, remove: bool) -> Option<()> {
        let tree = match self.mode {
            TracksMode::Artist => &self.tree_by_artist,
            TracksMode::Album => &self.tree_by_album,
        };

        let indices: Vec<usize> = tree
            .get(self.primary_index)?
            .1
            .get(self.secondary_index)?
            .1
            .clone();

        let mut playlist = self.playlist.borrow_mut();
        for idx in indices {
            if remove {
                playlist.shift_remove(&idx);
            } else {
                playlist.insert(idx);
            }
        }

        Some(())
    }

    pub fn pick_track(&mut self) -> Option<()> {
        let tree = match self.mode {
            TracksMode::Artist => &self.tree_by_artist,
            TracksMode::Album => &self.tree_by_album,
        };

        let idx = tree
            .get(self.primary_index)?
            .1
            .get(self.secondary_index)?
            .1
            .get(self.track_index)?;

        let mut playlist = self.playlist.borrow_mut();
        if playlist.contains(idx) {
            playlist.shift_remove(idx);
        } else {
            playlist.insert(*idx);
        }
        Some(())
    }

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
            2 => tree
                .get(self.primary_index)?
                .1
                .get(self.secondary_index)?
                .1
                .len(),
            _ => self.playlist.borrow().len(),
        };

        Some(len)
    }

    pub fn is_primary_selected(&self, index: usize) -> bool {
        let tree = self.current_tree();
        let playlist = self.playlist.borrow();
        if let Some((_, secondary_list)) = tree.get(index) {
            secondary_list.iter().all(|(_, track_indices)| {
                track_indices.iter().all(|idx| playlist.contains(idx))
            })
        } else {
            false
        }
    }

    pub fn is_secondary_selected(&self, index: usize) -> bool {
        let tree = self.current_tree();
        let playlist = self.playlist.borrow();
        if let Some((_, items)) = tree.get(self.primary_index)
            && let Some((_, track_indices)) = items.get(index)
        {
            return track_indices.iter().all(|idx| playlist.contains(idx));
        }
        false
    }

    pub fn clear_picked(&mut self) {
        self.playlist.borrow_mut().clear();
    }
}

pub struct SearchState {
    pub input: Input,
    pub cursor_index: usize,
    pub scroll_state: ScrollbarState,
    pub filtered_indices: Vec<usize>,
    pub playlist: PickedPlaylistRef,
}

impl SearchState {
    pub fn new(playlist: PickedPlaylistRef) -> Self {
        Self {
            input: Input::default(),
            cursor_index: 0,
            scroll_state: ScrollbarState::default(),
            filtered_indices: Vec::new(),
            playlist,
        }
    }
}

impl CursorMovable for SearchState {
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

impl SearchState {
    pub fn set_content_length(&mut self, len: usize) {
        self.scroll_state = self.scroll_state.content_length(len);
    }

    pub fn cursor(&self) -> usize {
        self.cursor_index
    }

    pub fn request(&self) {
        todo!()
    }

    pub fn update(&mut self, indices: Vec<usize>) {
        self.filtered_indices = indices;
        self.cursor_index = 0;
        self.scroll_state = ScrollbarState::default().content_length(self.filtered_indices.len());
    }
}

pub struct UiState {
    pub queue: QueueState,
    pub devices: DevicesListState,
    pub media_info: MediaInfoState,
    pub cover: Option<UnCachedProtocol>,
    pub tracks: TracksState,
    pub search: SearchState,
    pub expand_index: usize,
    pub picked_playlist: PickedPlaylistRef,
}

impl Default for UiState {
    fn default() -> Self {
        let picked_playlist: PickedPlaylistRef = Rc::new(RefCell::new(FxIndexSet::default()));
        Self {
            queue: QueueState::default(),
            devices: DevicesListState::default(),
            media_info: MediaInfoState::default(),
            cover: None,
            tracks: TracksState::new(Rc::clone(&picked_playlist)),
            search: SearchState::new(Rc::clone(&picked_playlist)),
            expand_index: 0,
            picked_playlist,
        }
    }
}

impl CursorMovable for UiState {
    fn cursor_index(&self) -> usize {
        match self.expand_index {
            0 => self.queue.cursor_index(),
            1 => self.tracks.cursor_index(),
            2 => self.search.cursor_index(),
            3 => self.devices.cursor_index(),
            _ => 0,
        }
    }

    fn set_cursor_index(&mut self, index: usize) {
        match self.expand_index {
            0 => self.queue.set_cursor_index(index),
            1 => self.tracks.set_cursor_index(index),
            2 => self.search.set_cursor_index(index),
            3 => self.devices.set_cursor_index(index),
            _ => {}
        }
    }

    fn set_scroll_position(&mut self, position: usize) {
        match self.expand_index {
            0 => self.queue.set_scroll_position(position),
            1 => self.tracks.set_scroll_position(position),
            2 => self.search.set_scroll_position(position),
            3 => self.devices.set_scroll_position(position),
            _ => {}
        }
    }
}
