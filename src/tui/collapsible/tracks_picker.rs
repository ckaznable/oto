use ratatui::{
    layout::{Constraint, Layout},
    prelude::*,
    text::Span,
    widgets::{Block, Borders, List, ListItem, ScrollbarState, StatefulWidget, Widget},
};

use crate::media::TracksTree;
use crate::tui::{
    collapsible::{CollapsibleWidget, CollapsibleWidgetGroup},
    scrollbar,
    state::TracksMode,
    AppState,
};

pub struct TracksPicker;

struct PrimaryList;
struct SecondaryList;
struct TrackList;
struct PlaylistList;

const LAYERS_NUM: usize = 4;

impl CollapsibleWidget<AppState> for TracksPicker {
    fn render_expand(&self, area: Rect, buf: &mut Buffer, state: &mut AppState) {
        let theme = &state.theme;
        let ui_state = state.ui_state.borrow();
        let mode = &ui_state.tracks.mode;
        let expand_index = ui_state.tracks.expand_index;

        let (artist_spans, album_spans): (Vec<Span>, Vec<Span>) = match mode {
            TracksMode::Artist => (
                vec![Span::styled(
                    "Artist",
                    Style::default().fg(theme.text).bold(),
                )],
                vec![
                    Span::styled("A", Style::default().fg(theme.mode_playing_fg)),
                    Span::styled("lbum", Style::default().fg(theme.overlay0)),
                ],
            ),
            TracksMode::Album => (
                vec![
                    Span::styled("A", Style::default().fg(theme.mode_playing_fg)),
                    Span::styled("rtist", Style::default().fg(theme.overlay0)),
                ],
                vec![Span::styled(
                    "Album",
                    Style::default().fg(theme.text).bold(),
                )],
            ),
        };

        let mut title_spans = vec![Span::raw(" ")];
        title_spans.extend(artist_spans);
        title_spans.push(Span::styled(" | ", Style::default().fg(theme.overlay0)));
        title_spans.extend(album_spans);
        title_spans.push(Span::raw(" "));

        let title = Line::from(title_spans);

        drop(ui_state);

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(theme.surface1))
            .title(title);

        let inner = block.inner(area);
        block.render(area, buf);

        let widgets: &[&dyn CollapsibleWidget<AppState>; LAYERS_NUM] =
            &[&PrimaryList, &SecondaryList, &TrackList, &PlaylistList];
        let group = CollapsibleWidgetGroup::<LAYERS_NUM>::new(widgets, expand_index, 4);
        StatefulWidget::render(group, inner, buf, state);
    }

    fn render_collapse(&self, area: Rect, buf: &mut Buffer, state: &mut AppState) {
        let theme = &state.theme;
        let playing_track = state.playing_track.borrow();
        let artist_name = playing_track.track.artist.as_deref().unwrap_or("Unknown");
        let album_name = playing_track
            .track
            .album
            .name
            .as_deref()
            .unwrap_or("Unknown");

        Line::from(Span::styled(
            format!(" 󰠃 {artist_name} 󰎆  {album_name}"),
            Style::default().fg(theme.media_artist),
        ))
        .style(Style::default().bg(theme.collapse_queue_bg))
        .render(area, buf);
    }
}

impl CollapsibleWidget<AppState> for PrimaryList {
    fn render_expand(&self, area: Rect, buf: &mut Buffer, state: &mut AppState) {
        let theme = &state.theme;
        let ui_state = state.ui_state.borrow();
        let tree = ui_state.tracks.current_tree();

        state.cache.borrow_mut().tracks_items.clear();
        for (i, (name, _)) in tree.iter().enumerate() {
            let is_selected = ui_state.tracks.is_primary_selected(i);
            state
                .cache
                .borrow_mut()
                .tracks_items
                .push((Line::raw(name.to_string()), is_selected));
        }

        let cursor_index = ui_state.tracks.primary_index;
        let scroll_state = ui_state.tracks.primary_scroll;

        let title = match ui_state.tracks.mode {
            TracksMode::Artist => " Artist ",
            TracksMode::Album => " Album ",
        };
        drop(ui_state);

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(theme.surface1))
            .title(title);

        let inner = block.inner(area);
        block.render(area, buf);

        let new_scroll = render_list_with_scrollbar(
            inner,
            buf,
            state,
            cursor_index,
            state.theme.media_artist,
            scroll_state,
        );
        state.ui_state.borrow_mut().tracks.primary_scroll = new_scroll;
    }

    fn render_collapse(&self, area: Rect, buf: &mut Buffer, state: &mut AppState) {
        self.render_expand(area, buf, state);
    }
}

impl CollapsibleWidget<AppState> for SecondaryList {
    fn render_expand(&self, area: Rect, buf: &mut Buffer, state: &mut AppState) {
        let theme = &state.theme;
        let ui_state = state.ui_state.borrow();
        let tree = ui_state.tracks.current_tree();

        state.cache.borrow_mut().tracks_items.clear();

        if let Some((_, items)) = tree.get(ui_state.tracks.primary_index) {
            for (i, (name, _)) in items.iter().enumerate() {
                let is_selected = ui_state.tracks.is_secondary_selected(i);
                state
                    .cache
                    .borrow_mut()
                    .tracks_items
                    .push((Line::raw(name.to_string()), is_selected));
            }
        }

        let cursor_index = ui_state.tracks.secondary_index;
        let scroll_state = ui_state.tracks.secondary_scroll;

        let title = match ui_state.tracks.mode {
            TracksMode::Artist => " Album ",
            TracksMode::Album => " Artist ",
        };
        drop(ui_state);

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(theme.surface1))
            .title(title);

        let inner = block.inner(area);
        block.render(area, buf);

        let new_scroll = render_list_with_scrollbar(
            inner,
            buf,
            state,
            cursor_index,
            state.theme.queue_album,
            scroll_state,
        );
        state.ui_state.borrow_mut().tracks.secondary_scroll = new_scroll;
    }

    fn render_collapse(&self, area: Rect, buf: &mut Buffer, state: &mut AppState) {
        self.render_expand(area, buf, state);
    }
}

impl CollapsibleWidget<AppState> for TrackList {
    fn title(&self) -> Option<&'static str> {
        Some(" Track ")
    }

    fn render_expand(&self, area: Rect, buf: &mut Buffer, state: &mut AppState) {
        let ui_state = state.ui_state.borrow();
        let tree = ui_state.tracks.current_tree();
        let track_indices = get_track_indices(
            tree,
            ui_state.tracks.primary_index,
            ui_state.tracks.secondary_index,
        );
        let cursor_index = ui_state.tracks.track_index;
        let scroll_state = ui_state.tracks.track_scroll;
        drop(ui_state);

        state.cache.borrow_mut().tracks_items.clear();

        let playlist = state.playlist.borrow();
        for &idx in track_indices.iter() {
            let name = playlist
                .list
                .get(idx)
                .and_then(|t| t.title.clone())
                .unwrap_or_else(|| "Unknown".to_string());
            let is_selected = state.ui_state.borrow().tracks.playlist.contains(&idx);
            state
                .cache
                .borrow_mut()
                .tracks_items
                .push((Line::raw(name), is_selected));
        }
        drop(playlist);

        let new_scroll = render_list_with_scrollbar(
            area,
            buf,
            state,
            cursor_index,
            state.theme.queue_title,
            scroll_state,
        );
        state.ui_state.borrow_mut().tracks.track_scroll = new_scroll;
    }

    fn render_collapse(&self, area: Rect, buf: &mut Buffer, state: &mut AppState) {
        self.render_expand(area, buf, state);
    }
}

impl CollapsibleWidget<AppState> for PlaylistList {
    fn title(&self) -> Option<&'static str> {
        Some(" Playlist ")
    }

    fn render_expand(&self, area: Rect, buf: &mut Buffer, state: &mut AppState) {
        let ui_state = state.ui_state.borrow();
        let playlist_indices = &ui_state.tracks.playlist;
        let cursor_index = ui_state.tracks.playlist_index;
        let scroll_state = ui_state.tracks.playlist_scroll;

        let app_playlist = state.playlist.borrow();
        state.cache.borrow_mut().tracks_items.clear();

        for &idx in playlist_indices.iter() {
            let name = app_playlist.list.get(idx).map(|t| {
                let theme = &state.theme;
                let title = t.title.as_deref().unwrap_or("Unknown").to_string();
                let artist = t.artist.as_deref().unwrap_or("Unknown").to_string();
                let album = t.album.name.as_deref().unwrap_or("Unknown").to_string();

                Line::from(vec![
                    Span::styled(title, Style::default().fg(theme.text).bold()),
                    Span::raw(" - "),
                    Span::styled(artist, Style::default().fg(theme.mode_playing_fg)),
                    Span::raw(" - "),
                    Span::styled(album, Style::default().fg(theme.media_title)),
                ])
            });

            if let Some(line) = name {
                state.cache.borrow_mut().tracks_items.push((line, false));
            }
        }

        drop(ui_state);
        drop(app_playlist);

        let new_scroll = render_list_with_scrollbar(
            area,
            buf,
            state,
            cursor_index,
            state.theme.queue_title,
            scroll_state,
        );
        state.ui_state.borrow_mut().tracks.playlist_scroll = new_scroll;
    }

    fn render_collapse(&self, area: Rect, buf: &mut Buffer, state: &mut AppState) {
        self.render_expand(area, buf, state);
    }
}

fn render_list_with_scrollbar(
    area: Rect,
    buf: &mut Buffer,
    state: &mut AppState,
    cursor_index: usize,
    text_color: Color,
    mut scroll_state: ScrollbarState,
) -> ScrollbarState {
    let theme = &state.theme;
    let layout = Layout::horizontal([Constraint::Fill(1), Constraint::Length(1)]);
    let [list_area, scrollbar_area] = layout.areas(area);

    let visible_height = list_area.height as usize;
    let list_len = state.cache.borrow().tracks_items.len();
    let offset = calculate_scroll_offset(cursor_index, visible_height, list_len);

    state.cache.borrow_mut().list_items.clear();

    let cache_ref = state.cache.borrow();
    let items_iter = cache_ref
        .tracks_items
        .iter()
        .enumerate()
        .skip(offset)
        .take(visible_height);

    // Collect items first to release borrow
    let items_to_render: Vec<(usize, Line, bool)> = items_iter
        .map(|(idx, (text, is_selected))| (idx, text.clone(), *is_selected))
        .collect();
    drop(cache_ref);

    for (idx, text, is_selected) in items_to_render {
        let is_cursor = idx == cursor_index;
        let bg_color = if is_cursor {
            theme.surface1
        } else {
            theme.base
        };
        let indicator = if is_cursor { ">" } else { " " };

        let prefix = if is_selected { "*" } else { "" };
        let prefix_style = if is_selected {
            Style::default().fg(theme.mode_playing_fg)
        } else {
            Style::default().fg(text_color)
        };

        let mut spans = vec![Span::styled(format!("{indicator} {prefix}"), prefix_style)];
        spans.extend(text.spans);

        let line = Line::from(spans);

        let item = ListItem::new(line).style(Style::default().bg(bg_color));
        state.cache.borrow_mut().list_items.push(item);
    }

    let mut cache = state.cache.borrow_mut();
    Widget::render(List::new(cache.list_items.drain(..)), list_area, buf);
    drop(cache);

    scroll_state = scroll_state.content_length(list_len).position(cursor_index);

    scrollbar::render_scrollbar(scrollbar_area, buf, theme, &mut scroll_state);
    scroll_state
}

pub fn get_track_indices(
    tree: &TracksTree,
    primary_index: usize,
    secondary_index: usize,
) -> Vec<usize> {
    tree.get(primary_index)
        .and_then(|(_, items)| items.get(secondary_index))
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
