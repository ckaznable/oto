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

        let items: Vec<ListItem> = playlist
            .list
            .iter()
            .map(|track| {
                let title = track.title.as_deref().unwrap_or("Unknown");
                let album = track.album.name.as_deref().unwrap_or("Unknown");
                let year = track
                    .album
                    .year
                    .map(|y| format!(" ({})", y))
                    .unwrap_or_default();

                let line = Line::from(vec![
                    Span::styled(title, Style::default().fg(theme.queue_title)),
                    Span::raw(" - "),
                    Span::styled(album, Style::default().fg(theme.queue_album)),
                    Span::styled(year, Style::default().fg(theme.queue_year)),
                ]);

                ListItem::new(line)
            })
            .collect();

        let list = List::new(items);
        Widget::render(list, area, buf);
    }
}
