use ratatui::{
    layout::Constraint,
    prelude::*,
    widgets::{Block, Padding, Paragraph, Widget},
};

use crate::{
    arena_alloc,
    tui::{AppState, clear::ClearArea, gradient::GradientBackground},
};

pub struct MediaInfo;

impl StatefulWidget for MediaInfo {
    type State = AppState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        let theme = state.theme.clone();

        let block = Block::default()
            .padding(Padding::uniform(1))
            .style(Style::default().bg(theme.mantle));

        let inner = block.inner(area);
        block.render(area, buf);
        GradientBackground.render(area, buf, state);

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

        let mut alloc = state.alloc.borrow_mut();
        if inner.height < total_height {
            let text_ptr = arena_alloc!(&mut alloc, "{}\n{}\n{}", title, artist, album);
            Paragraph::new(alloc.get(text_ptr))
                .alignment(Alignment::Center)
                .render(inner, buf);
            alloc.clear();
            return;
        }

        let layout = if inner.height < 25 {
            Layout::vertical([Constraint::Fill(1)])
        } else {
            Layout::vertical([
                Constraint::Length(inner.width / 2),
                Constraint::Length(1),
                Constraint::Length(title_height),
                Constraint::Length(1),
                Constraint::Length(artist_height),
                Constraint::Length(1),
                Constraint::Length(album_height),
                Constraint::Fill(1),
            ])
        };

        let areas = layout.split(inner);

        {
            use ratatui_image::StatefulImage;
            if let Some(cover) = state.ui_state.borrow_mut().cover.as_mut() {
                let area = if inner.width <= 30 {
                    areas[0].centered_horizontally(Constraint::Ratio(4, 5))
                } else {
                    areas[0]
                };
                StatefulImage::new().render(area, buf, cover);
            }
        }

        if areas.len() > 1 {
            ClearArea.render(areas[2], buf);
            ClearArea.render(areas[4], buf);
            ClearArea.render(areas[6], buf);

            Paragraph::new(title)
                .style(Style::default().fg(theme.media_title).bold())
                .alignment(Alignment::Center)
                .render(areas[2], buf);

            let artist_ptr = arena_alloc!(&mut alloc, "by {}", artist);
            Paragraph::new(alloc.get(artist_ptr))
                .style(Style::default().fg(theme.media_artist))
                .alignment(Alignment::Center)
                .render(areas[4], buf);

            Paragraph::new(album)
                .style(Style::default().fg(theme.media_album))
                .alignment(Alignment::Center)
                .render(areas[6], buf);
        }

        alloc.clear();
    }
}
