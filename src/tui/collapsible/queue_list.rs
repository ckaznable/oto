use itertools::Either;
use ratatui::{
    layout::{Constraint, Layout, Margin},
    prelude::*,
    text::Span,
    widgets::{Cell, Row, Table, Widget},
};

use crate::tui::{AppState, LAYOUT_WIDTH_S, collapsible::CollapsibleWidget, scrollbar};

pub struct QueueList;

impl CollapsibleWidget<AppState> for QueueList {
    fn title(&self) -> Option<&'static str> {
        Some(" Queue ")
    }

    fn render_expand(&self, area: Rect, buf: &mut Buffer, state: &mut AppState) {
        let theme = &state.theme;
        let playlist = state.playlist.borrow();
        let list_len = match playlist.picked.as_deref() {
            Some(p) => p.len(),
            None => playlist.list.len(),
        };
        let playlist_index = playlist.index;

        let mut ui_state = state.ui_state.borrow_mut();
        ui_state.queue.set_content_length(list_len);
        ui_state.queue.playing_index = playlist_index;
        drop(ui_state);

        let cursor_index = state.ui_state.borrow().queue.cursor_index;
        let playing_index = state.ui_state.borrow().queue.playing_index;

        let is_compact = area.width <= LAYOUT_WIDTH_S;

        let layout = Layout::horizontal([Constraint::Fill(1), Constraint::Length(1)]);
        let [table_area, scrollbar_area] = layout.areas(area);
        let table_area = table_area.inner(Margin::new(1, 0));

        let visible_height = table_area.height.saturating_sub(1) as usize; // Subtract 1 for header
        let offset = calculate_scroll_offset(cursor_index, visible_height, list_len);

        state.cache.borrow_mut().rows.clear();

        let list_iter = match playlist.picked.as_deref() {
            Some(picked) => Either::Left(
                picked
                    .iter()
                    .enumerate()
                    .filter_map(|(i, idx)| Some((i, playlist.list.get(*idx)?)))
                    .skip(offset)
                    .take(visible_height),
            ),
            None => Either::Right(
                playlist
                    .list
                    .iter()
                    .enumerate()
                    .skip(offset)
                    .take(visible_height),
            ),
        };

        for (index, track) in list_iter {
            let title = track.title.clone().unwrap_or_else(|| "Unknown".to_string());
            let album = track
                .album
                .name
                .clone()
                .unwrap_or_else(|| "Unknown".to_string());
            let year = track.album.year.map(|y| format!("{y}")).unwrap_or_default();

            let duration = format!(
                "{:02}:{:02}",
                track.duration_secs / 60,
                track.duration_secs % 60
            );

            let is_playing = index == playing_index;
            let is_cursor = index == cursor_index;

            let bg_color = if is_cursor {
                theme.surface1
            } else if is_playing {
                theme.queue_current_bg
            } else {
                theme.base
            };

            let title_style = Style::default().fg(theme.queue_title);
            let album_style = Style::default().fg(theme.queue_album);
            let year_style = Style::default().fg(theme.queue_year);
            let duration_style = Style::default().fg(theme.queue_duration);

            let playing_indicator = String::from(if is_playing { "" } else { " " });

            let row = if is_compact {
                Row::new([
                    Cell::from(title).style(title_style),
                    Cell::from(duration).style(duration_style),
                ])
            } else {
                Row::new([
                    Cell::from(playing_indicator).style(Style::default().fg(theme.mode_playing_fg)),
                    Cell::from(title).style(title_style),
                    Cell::from(album).style(album_style),
                    Cell::from(year).style(year_style),
                    Cell::from(duration).style(duration_style),
                ])
            }
            .style(Style::default().bg(bg_color))
            .height(1);

            state.cache.borrow_mut().rows.push(row);
        }

        drop(playlist);

        let header = if is_compact {
            Row::new([Cell::from("Title"), Cell::from("Time")])
        } else {
            Row::new([
                Cell::from(""),
                Cell::from("Title"),
                Cell::from("Album"),
                Cell::from("Year"),
                Cell::from("Time"),
            ])
        }
        .style(
            Style::default()
                .fg(theme.queue_header)
                .bg(theme.base)
                .bold(),
        )
        .height(1);

        let mut cache = state.cache.borrow_mut();
        let table = Table::new(
            cache.rows.drain(..),
            if is_compact {
                [
                    Constraint::Fill(1),
                    Constraint::Length(6),
                    Constraint::Length(0),
                    Constraint::Length(0),
                    Constraint::Length(0),
                ]
            } else {
                [
                    Constraint::Length(2),
                    Constraint::Fill(1),
                    Constraint::Fill(1),
                    Constraint::Length(6),
                    Constraint::Length(6),
                ]
            },
        )
        .header(header)
        .column_spacing(1)
        .row_highlight_style(Style::default().bg(theme.surface1));

        Widget::render(table, table_area, buf);
        drop(cache);

        let mut scroll_state = state.ui_state.borrow().queue.scroll_state;
        scroll_state = scroll_state.position(cursor_index);

        scrollbar::render_scrollbar(scrollbar_area, buf, theme, &mut scroll_state);

        state.ui_state.borrow_mut().queue.scroll_state = scroll_state;
    }

    fn render_collapse(&self, area: Rect, buf: &mut Buffer, state: &mut AppState) {
        let theme = &state.theme;
        let track = &state.playing_track.borrow().track;

        Line::from(vec![
            Span::raw("  "),
            Span::styled(
                track.title.as_deref().unwrap_or_default(),
                Style::default().fg(theme.media_title),
            ),
            Span::raw(" - "),
            Span::styled(
                track.artist.as_deref().unwrap_or_default(),
                Style::default().fg(theme.media_artist),
            ),
        ])
        .style(Style::default().bg(theme.collapse_queue_bg))
        .render(area, buf);
    }
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
