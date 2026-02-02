use ratatui::{
    layout::{Constraint, Layout},
    prelude::*,
    text::Span,
    widgets::{Block, Borders, List, ListItem, Paragraph, Widget},
};

use crate::tui::{AppState, KeyHandleMode, collapsible::CollapsibleWidget, scrollbar};

pub struct Search;

impl CollapsibleWidget<AppState> for Search {
    fn title(&self) -> Option<&'static str> {
        Some(" Search ")
    }

    fn render_expand(&self, area: Rect, buf: &mut Buffer, state: &mut AppState) {
        let theme = &state.theme;
        let playlist = state.playlist.borrow();

        let query = {
            let ui_state = state.ui_state.borrow();
            ui_state.search.input.value().to_string()
        };

        let list_len = { state.ui_state.borrow().search.filtered_indices.len() };
        let cursor_index = state.ui_state.borrow().search.cursor_index;
        let scroll_state = state.ui_state.borrow().search.scroll_state;

        let layout = Layout::vertical([Constraint::Length(3), Constraint::Fill(1)]);
        let [input_area, list_area] = layout.areas(area);

        let border_color = match state.key_handle_mode() {
            KeyHandleMode::Edit => theme.mode_playing_fg,
            KeyHandleMode::App => theme.surface1,
        };

        let input_block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(border_color));

        let input_inner = input_block.inner(input_area);
        input_block.render(input_area, buf);

        let paragraph = Paragraph::new(query.clone()).style(Style::default().fg(theme.text));
        paragraph.render(input_inner, buf);

        let cursor_pos = {
            let ui_state = state.ui_state.borrow();
            ui_state.search.input.cursor()
        };
        let cursor_x = input_inner.x + cursor_pos as u16;
        if cursor_x < input_inner.x + input_inner.width {
            let cursor_char = query.chars().nth(cursor_pos).unwrap_or(' ');
            let cell = buf.cell_mut((cursor_x, input_inner.y));
            if let Some(cell) = cell {
                cell.set_style(Style::default().bg(theme.surface1).fg(theme.text))
                    .set_char(cursor_char);
            }
        }

        let layout = Layout::horizontal([Constraint::Fill(1), Constraint::Length(1)]);
        let [list_content_area, scrollbar_area] = layout.areas(list_area);

        let visible_height = list_content_area.height as usize;
        let offset = calculate_scroll_offset(cursor_index, visible_height, list_len);

        state.cache.borrow_mut().list_items.clear();

        let ui_state = state.ui_state.borrow();
        for (i, track_idx) in ui_state
            .search
            .filtered_indices
            .iter()
            .skip(offset)
            .take(visible_height)
            .copied()
            .enumerate()
        {
            let actual_index = offset + i;
            let track = playlist.list.get(track_idx);

            if let Some(track) = track {
                let title = track.title.clone().unwrap_or_else(|| "Unknown".to_string());
                let artist = track
                    .artist
                    .clone()
                    .unwrap_or_else(|| "Unknown".to_string());

                let is_cursor = actual_index == cursor_index;
                let bg_color = if is_cursor {
                    theme.surface1
                } else {
                    theme.base
                };
                let indicator = if is_cursor { ">" } else { " " };

                let line = Line::from(vec![
                    Span::styled(format!("{indicator} "), Style::default().fg(theme.text)),
                    Span::styled(title, Style::default().fg(theme.queue_title)),
                    Span::styled(" - ", Style::default().fg(theme.overlay0)),
                    Span::styled(artist, Style::default().fg(theme.queue_album)),
                ]);

                let item = ListItem::new(line).style(Style::default().bg(bg_color));
                state.cache.borrow_mut().list_items.push(item);
            }
        }

        drop(ui_state);
        drop(playlist);

        let mut cache = state.cache.borrow_mut();
        let list = List::new(cache.list_items.drain(..));
        Widget::render(list, list_content_area, buf);
        drop(cache);

        let mut scroll_state = scroll_state.content_length(list_len).position(cursor_index);

        scrollbar::render_scrollbar(scrollbar_area, buf, theme, &mut scroll_state);

        state.ui_state.borrow_mut().search.scroll_state = scroll_state;
    }

    fn render_collapse(&self, area: Rect, buf: &mut Buffer, state: &mut AppState) {
        let theme = &state.theme;

        let query = {
            let ui_state = state.ui_state.borrow();
            ui_state.search.input.value().to_string()
        };

        let display_text = if query.is_empty() {
            " 󰍉 Search".to_string()
        } else {
            format!(" 󰍉 Search: {}", query)
        };

        Line::from(Span::styled(display_text, Style::default().fg(theme.text)))
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
