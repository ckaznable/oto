use ratatui::{
    layout::{Constraint, Layout, Margin},
    prelude::*,
    text::Span,
    widgets::{
        List, ListItem, Scrollbar, ScrollbarOrientation, ScrollbarState, StatefulWidget, Widget,
    },
};

use crate::media::TracksTree;
use crate::tui::{
    AppState,
    collapsible::{CollapsibleWidget, CollapsibleWidgetGroup},
};

pub struct TracksPicker;

struct ArtistList;
struct AlbumList;
struct TrackList;

const LAYERS_NUM: usize = 3;

impl CollapsibleWidget<AppState> for TracksPicker {
    fn title(&self) -> &'static str {
        " Tracks "
    }

    fn render_expand_content(&self, area: Rect, buf: &mut Buffer, state: &mut AppState) {
        let expand_index = state.ui_state.borrow().tracks.expand_index;
        let widgets: &[&dyn CollapsibleWidget<AppState>; LAYERS_NUM] =
            &[&ArtistList, &AlbumList, &TrackList];
        let group = CollapsibleWidgetGroup::<LAYERS_NUM>::new(widgets, expand_index, 5);
        StatefulWidget::render(group, area, buf, state);
    }

    fn render_collapse(&self, area: Rect, buf: &mut Buffer, state: &mut AppState) {
        let theme = &state.theme;
        let playing_track = state.playing_track.borrow();
        let artist_name = playing_track.track.artist.as_deref().unwrap_or("No Artist");

        Line::from(Span::styled(
            artist_name,
            Style::default().fg(theme.media_artist),
        ))
        .style(Style::default().bg(theme.collapse_queue_bg))
        .render(area, buf);
    }
}

impl CollapsibleWidget<AppState> for ArtistList {
    fn title(&self) -> &'static str {
        " Artist "
    }

    fn render_expand_content(&self, area: Rect, buf: &mut Buffer, state: &mut AppState) {
        let ui_state = state.ui_state.borrow();
        let items: Vec<String> = ui_state
            .tracks
            .tree
            .iter()
            .map(|(name, _)| name.clone())
            .collect();
        let cursor_index = ui_state.tracks.artist_index;
        let scroll_state = ui_state.tracks.artist_scroll;
        drop(ui_state);

        let new_scroll = render_list_with_scrollbar(
            area,
            buf,
            state,
            &items,
            cursor_index,
            state.theme.media_artist,
            scroll_state,
        );
        state.ui_state.borrow_mut().tracks.artist_scroll = new_scroll;
    }

    fn render_collapse(&self, area: Rect, buf: &mut Buffer, state: &mut AppState) {
        self.render_expand_content(area, buf, state);
    }
}

impl CollapsibleWidget<AppState> for AlbumList {
    fn title(&self) -> &'static str {
        " Album "
    }

    fn render_expand_content(&self, area: Rect, buf: &mut Buffer, state: &mut AppState) {
        let ui_state = state.ui_state.borrow();
        let items = get_unique_albums(&ui_state.tracks.tree, ui_state.tracks.artist_index);
        let cursor_index = ui_state.tracks.album_inedx;
        let scroll_state = ui_state.tracks.album_scroll;
        drop(ui_state);

        let new_scroll = render_list_with_scrollbar(
            area,
            buf,
            state,
            &items,
            cursor_index,
            state.theme.queue_album,
            scroll_state,
        );
        state.ui_state.borrow_mut().tracks.album_scroll = new_scroll;
    }

    fn render_collapse(&self, area: Rect, buf: &mut Buffer, state: &mut AppState) {
        self.render_expand_content(area, buf, state);
    }
}

impl CollapsibleWidget<AppState> for TrackList {
    fn title(&self) -> &'static str {
        " Track "
    }

    fn render_expand_content(&self, area: Rect, buf: &mut Buffer, state: &mut AppState) {
        let ui_state = state.ui_state.borrow();
        let track_indices = get_track_indices(
            &ui_state.tracks.tree,
            ui_state.tracks.artist_index,
            ui_state.tracks.album_inedx,
        );
        let cursor_index = ui_state.tracks.track_index;
        let scroll_state = ui_state.tracks.track_scroll;
        drop(ui_state);

        let playlist = state.playlist.borrow();
        let items: Vec<String> = track_indices
            .iter()
            .map(|&idx| {
                playlist
                    .list
                    .get(idx)
                    .and_then(|t| t.title.clone())
                    .unwrap_or_else(|| "Unknown".to_string())
            })
            .collect();
        drop(playlist);

        let new_scroll = render_list_with_scrollbar(
            area,
            buf,
            state,
            &items,
            cursor_index,
            state.theme.queue_title,
            scroll_state,
        );
        state.ui_state.borrow_mut().tracks.track_scroll = new_scroll;
    }

    fn render_collapse(&self, area: Rect, buf: &mut Buffer, state: &mut AppState) {
        self.render_expand_content(area, buf, state);
    }
}

fn render_list_with_scrollbar(
    area: Rect,
    buf: &mut Buffer,
    state: &mut AppState,
    items: &[String],
    cursor_index: usize,
    text_color: Color,
    mut scroll_state: ScrollbarState,
) -> ScrollbarState {
    let theme = &state.theme;
    let layout = Layout::horizontal([Constraint::Fill(1), Constraint::Length(1)]);
    let [list_area, scrollbar_area] = layout.areas(area);

    let visible_height = list_area.height as usize;
    let list_len = items.len();
    let offset = calculate_scroll_offset(cursor_index, visible_height, list_len);

    state.ui_state.borrow_mut().cache.list_items.clear();

    let list_iter = items.iter().enumerate().skip(offset).take(visible_height);

    for (idx, text) in list_iter {
        let is_cursor = idx == cursor_index;
        let bg_color = if is_cursor {
            theme.surface1
        } else {
            theme.base
        };
        let indicator = if is_cursor { ">" } else { " " };

        let line = Line::from(vec![
            Span::styled(
                format!("{indicator} "),
                Style::default().fg(theme.mode_playing_fg),
            ),
            Span::styled(text.clone(), Style::default().fg(text_color)),
        ]);

        let item = ListItem::new(line).style(Style::default().bg(bg_color));
        state.ui_state.borrow_mut().cache.list_items.push(item);
    }

    let mut ui_state = state.ui_state.borrow_mut();
    Widget::render(
        List::new(ui_state.cache.list_items.drain(..)),
        list_area,
        buf,
    );
    drop(ui_state);

    scroll_state = scroll_state.content_length(list_len).position(cursor_index);

    let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
        .begin_symbol(None)
        .end_symbol(None)
        .track_symbol(Some("│"))
        .thumb_symbol("█")
        .style(Style::default().fg(theme.overlay0));

    StatefulWidget::render(
        scrollbar,
        scrollbar_area.inner(Margin::new(0, 0)),
        buf,
        &mut scroll_state,
    );
    scroll_state
}

fn get_unique_albums(tree: &TracksTree, artist_index: usize) -> Vec<String> {
    tree.get(artist_index)
        .map(|(_, albums)| albums.iter().map(|(name, _)| name.clone()).collect())
        .unwrap_or_default()
}

fn get_track_indices(tree: &TracksTree, artist_index: usize, album_index: usize) -> Vec<usize> {
    tree.get(artist_index)
        .and_then(|(_, albums)| albums.get(album_index))
        .map(|(_, indices)| indices.clone())
        .unwrap_or_default()
}

fn calculate_scroll_offset(cursor: usize, visible_height: usize, total_items: usize) -> usize {
    if total_items <= visible_height {
        return 0;
    }

    let half_visible = visible_height / 2;

    if cursor < half_visible {
        0
    } else if cursor >= total_items.saturating_sub(half_visible) {
        total_items.saturating_sub(visible_height)
    } else {
        cursor.saturating_sub(half_visible)
    }
}
