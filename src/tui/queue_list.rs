use ratatui::{
    layout::{Constraint, Layout, Margin},
    prelude::*,
    widgets::{Cell, Row, Scrollbar, ScrollbarOrientation, StatefulWidget, Table, Widget},
};

use crate::tui::{AppState, LAYOUT_WIDTH_S};

pub struct QueueList;

impl StatefulWidget for QueueList {
    type State = AppState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        let theme = &state.theme;
        let playlist = state.playlist.borrow();
        let list_len = playlist.list.len();
        let playlist_index = playlist.index;

        {
            let mut ui_state = state.ui_state.borrow_mut();
            ui_state.queue.set_content_length(list_len);
            ui_state.queue.playing_index = playlist_index;
        }

        let cursor_index = state.ui_state.borrow().queue.cursor_index;
        let playing_index = state.ui_state.borrow().queue.playing_index;

        let is_compact = area.width <= LAYOUT_WIDTH_S;

        let rows: Vec<Row> = playlist
            .list
            .iter()
            .enumerate()
            .map(|(index, track)| {
                let title = track.title.clone().unwrap_or_else(|| "Unknown".to_string());
                let album = track
                    .album
                    .name
                    .clone()
                    .unwrap_or_else(|| "Unknown".to_string());
                let year = track
                    .album
                    .year
                    .map(|y| format!("{y}"))
                    .unwrap_or_default();

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

                if is_compact {
                    Row::new([
                        Cell::from(title).style(title_style),
                        Cell::from(duration).style(duration_style),
                    ])
                } else {
                    Row::new([
                        Cell::from(playing_indicator)
                            .style(Style::default().fg(theme.mode_playing_fg)),
                        Cell::from(title).style(title_style),
                        Cell::from(album).style(album_style),
                        Cell::from(year).style(year_style),
                        Cell::from(duration).style(duration_style),
                    ])
                }
                .style(Style::default().bg(bg_color))
                .height(1)
            })
            .collect();

        drop(playlist);

        let layout = Layout::horizontal([Constraint::Fill(1), Constraint::Length(1)]);
        let [table_area, scrollbar_area] = layout.areas(area);
        let table_area = table_area.inner(Margin::new(1, 0));

        let widths = if is_compact {
            vec![Constraint::Fill(1), Constraint::Length(6)]
        } else {
            vec![
                Constraint::Length(2),
                Constraint::Fill(1),
                Constraint::Fill(1),
                Constraint::Length(6),
                Constraint::Length(6),
            ]
        };

        let visible_height = table_area.height.saturating_sub(1) as usize; // Subtract 1 for header
        let offset = calculate_scroll_offset(cursor_index, visible_height, list_len);

        let visible_rows: Vec<Row> = rows.into_iter().skip(offset).take(visible_height).collect();

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

        let table = Table::new(visible_rows, widths)
            .header(header)
            .column_spacing(1)
            .row_highlight_style(Style::default().bg(theme.surface1));

        Widget::render(table, table_area, buf);

        let mut scroll_state = state.ui_state.borrow().queue.scroll_state;
        scroll_state = scroll_state.position(cursor_index);

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

        state.ui_state.borrow_mut().queue.scroll_state = scroll_state;
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
