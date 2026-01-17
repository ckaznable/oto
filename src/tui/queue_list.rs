use ratatui::{
    prelude::*,
    widgets::{List, ListItem, Widget},
};

use crate::media::TrackMeta;

pub struct QueueList<'a> {
    tracks: &'a [TrackMeta],
}

impl<'a> QueueList<'a> {
    pub fn new(tracks: &'a [TrackMeta]) -> Self {
        Self { tracks }
    }
}

impl Widget for QueueList<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let items: Vec<ListItem> = self
            .tracks
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
                    Span::styled(title, Style::default().fg(Color::White)),
                    Span::raw(" - "),
                    Span::styled(album, Style::default().fg(Color::Yellow)),
                    Span::styled(year, Style::default().fg(Color::Gray)),
                ]);

                ListItem::new(line)
            })
            .collect();

        let list = List::new(items);
        Widget::render(list, area, buf);
    }
}
