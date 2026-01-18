use ratatui::{
    layout::Constraint,
    prelude::*,
    widgets::{Paragraph, Widget},
};

use crate::tui::AppState;

pub struct MediaInfo;

impl StatefulWidget for MediaInfo {
    type State = AppState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        let theme = &state.theme;
        let track = state.playing_track.borrow();
        let sr = track.spec.sample_rate;
        let track = &track.track;

        let title = track.title.as_deref().unwrap_or("Unknown");
        let album = track.album.name.as_deref().unwrap_or("Unknown");
        let artist = track.artist.as_deref().unwrap_or("Unknown");

        let sample_rate_str = if sr >= 1_000_000 {
            format!("{:.1} MHz", ((sr as f64 / 1_000_000.0) * 10.).trunc() / 10.)
        } else if sr >= 1_000 {
            format!("{:.1} kHz", ((sr as f64 / 1_000.0) * 10.).trunc() / 10.)
        } else {
            format!("{} Hz", sr)
        };

        let title_height = 1;
        let artist_height = 1;
        let album_height = 1;
        let sample_rate_height = 1;
        let spacing = 3;

        let total_height =
            title_height + artist_height + album_height + sample_rate_height + spacing;

        if area.height < total_height {
            let text = format!("{}\n{}\n{}\n{}", title, artist, album, sample_rate_str);
            Paragraph::new(text)
                .alignment(Alignment::Center)
                .render(area, buf);
            return;
        }

        let layout = Layout::vertical([
            Constraint::Length(title_height),
            Constraint::Length(1),
            Constraint::Length(artist_height),
            Constraint::Length(1),
            Constraint::Length(album_height),
            Constraint::Length(1),
            Constraint::Length(sample_rate_height),
            Constraint::Fill(1),
        ]);

        let areas = layout.split(area);

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

        Paragraph::new(sample_rate_str)
            .style(Style::default().fg(theme.media_sample_rate))
            .alignment(Alignment::Center)
            .render(areas[6], buf);
    }
}
