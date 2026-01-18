use ratatui::{
    prelude::*,
    widgets::{List, ListItem, Widget},
};

use crate::tui::AppState;

pub struct QueueList;

impl StatefulWidget for QueueList {
    type State = AppState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        let theme = &state.theme;
        let playlist = state.playlist.borrow();
        let current_index = playlist.index;

        let items: Vec<ListItem> = playlist
            .list
            .iter()
            .enumerate()
            .map(|(index, track)| {
                let title = track.title.as_deref().unwrap_or("Unknown");
                let album = track.album.name.as_deref().unwrap_or("Unknown");
                let year = track
                    .album
                    .year
                    .map(|y| format!(" ({})", y))
                    .unwrap_or_default();

                let is_current = index == current_index;
                let bg_color = if is_current {
                    theme.queue_current_bg
                } else {
                    theme.base
                };

                let line = Line::from(vec![
                    Span::styled(title, Style::default().fg(theme.queue_title).bg(bg_color)),
                    Span::styled(" - ", Style::default().bg(bg_color)),
                    Span::styled(album, Style::default().fg(theme.queue_album).bg(bg_color)),
                    Span::styled(year, Style::default().fg(theme.queue_year).bg(bg_color)),
                ]);

                ListItem::new(line)
            })
            .collect();

        let list = List::new(items);
        Widget::render(list, area, buf);
    }
}
