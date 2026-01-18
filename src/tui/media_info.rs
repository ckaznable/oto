use ratatui::{
    layout::Constraint,
    prelude::*,
    widgets::{Block, BorderType, Borders, Paragraph, Widget},
};

use crate::tui::AppState;

pub struct MediaInfo {
    pub border: bool,
}

impl Default for MediaInfo {
    fn default() -> Self {
        Self { border: true }
    }
}

impl StatefulWidget for MediaInfo {
    type State = AppState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        let theme = &state.theme;

        let block = if self.border {
            Block::default()
                .borders(Borders::RIGHT)
                .border_type(BorderType::QuadrantInside)
                .border_style(Style::default().fg(theme.mantle))
                .style(Style::default().bg(theme.mantle))
        } else {
            Block::default().style(Style::default().bg(theme.mantle))
        };

        let inner = block.inner(area);
        block.render(area, buf);

        let track = state.playing_track.borrow();
        let track = &track.track;

        let title = track.title.as_deref().unwrap_or("Unknown");
        let album = track.album.name.as_deref().unwrap_or("Unknown");
        let artist = track.artist.as_deref().unwrap_or("Unknown");

        let title_height = 1;
        let artist_height = 1;
        let album_height = 1;
        let spacing = 2;

        let total_height = title_height + artist_height + album_height + spacing;

        if inner.height < total_height {
            let text = format!("{}\n{}\n{}", title, artist, album);
            Paragraph::new(text)
                .alignment(Alignment::Center)
                .render(inner, buf);
            return;
        }

        let layout = Layout::vertical([
            Constraint::Length(title_height),
            Constraint::Length(1),
            Constraint::Length(artist_height),
            Constraint::Length(1),
            Constraint::Length(album_height),
            Constraint::Fill(1),
        ]);

        let areas = layout.split(inner);

        Paragraph::new(title)
            .style(Style::default().fg(theme.media_title).bold())
            .alignment(Alignment::Center)
            .render(areas[0], buf);

        Paragraph::new(format!("by {}", artist))
            .style(Style::default().fg(theme.media_artist))
            .alignment(Alignment::Center)
            .render(areas[2], buf);

        Paragraph::new(album)
            .style(Style::default().fg(theme.media_album))
            .alignment(Alignment::Center)
            .render(areas[4], buf);
    }
}
