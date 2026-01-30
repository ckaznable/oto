use std::rc::Rc;

use ratatui::{
    layout::{Constraint, Layout, Margin},
    prelude::*,
    text::Span,
    widgets::{
        Block, Borders, List, ListItem, Scrollbar, ScrollbarOrientation, ScrollbarState,
        StatefulWidget, Widget,
    },
};

use crate::media::TracksTree;
use crate::tui::{
    collapsible::{CollapsibleWidget, CollapsibleWidgetGroup},
    state::TracksMode,
    AppState,
};

pub struct TracksPicker;

struct PrimaryList;
struct SecondaryList;
struct TrackList;

const LAYERS_NUM: usize = 3;

impl CollapsibleWidget<AppState> for TracksPicker {
    fn render_expand_content(&self, area: Rect, buf: &mut Buffer, state: &mut AppState) {
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
            &[&PrimaryList, &SecondaryList, &TrackList];
        let group = CollapsibleWidgetGroup::<LAYERS_NUM>::new(widgets, expand_index, 5);
        StatefulWidget::render(group, inner, buf, state);
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

impl CollapsibleWidget<AppState> for PrimaryList {
    fn render_expand_content(&self, area: Rect, buf: &mut Buffer, state: &mut AppState) {
        let theme = &state.theme;
        let ui_state = state.ui_state.borrow();
        let tree = ui_state.tracks.current_tree();
        let items: Vec<Rc<String>> = tree.iter().map(|(name, _)| name.clone()).collect();
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
            &items,
            cursor_index,
            state.theme.media_artist,
            scroll_state,
        );
        state.ui_state.borrow_mut().tracks.primary_scroll = new_scroll;
    }

    fn render_collapse(&self, area: Rect, buf: &mut Buffer, state: &mut AppState) {
        self.render_expand_content(area, buf, state);
    }
}

impl CollapsibleWidget<AppState> for SecondaryList {
    fn render_expand_content(&self, area: Rect, buf: &mut Buffer, state: &mut AppState) {
        let theme = &state.theme;
        let ui_state = state.ui_state.borrow();
        let tree = ui_state.tracks.current_tree();
        let items = get_secondary_items(tree, ui_state.tracks.primary_index);
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
            &items,
            cursor_index,
            state.theme.queue_album,
            scroll_state,
        );
        state.ui_state.borrow_mut().tracks.secondary_scroll = new_scroll;
    }

    fn render_collapse(&self, area: Rect, buf: &mut Buffer, state: &mut AppState) {
        self.render_expand_content(area, buf, state);
    }
}

impl CollapsibleWidget<AppState> for TrackList {
    fn title(&self) -> Option<&'static str> {
        Some(" Track ")
    }

    fn render_expand_content(&self, area: Rect, buf: &mut Buffer, state: &mut AppState) {
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

fn render_list_with_scrollbar<T>(
    area: Rect,
    buf: &mut Buffer,
    state: &mut AppState,
    items: &[T],
    cursor_index: usize,
    text_color: Color,
    mut scroll_state: ScrollbarState,
) -> ScrollbarState
where
    T: std::ops::Deref,
    <T as std::ops::Deref>::Target: AsRef<str>,
{
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
            Span::styled((*text).as_ref().to_owned(), Style::default().fg(text_color)),
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

fn get_secondary_items(tree: &TracksTree, primary_index: usize) -> Vec<Rc<String>> {
    tree.get(primary_index)
        .map(|(_, items)| items.iter().map(|(name, _)| name.clone()).collect())
        .unwrap_or_default()
}

fn get_track_indices(
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
