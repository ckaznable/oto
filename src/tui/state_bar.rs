use ratatui::{
    layout::{Constraint, Flex, Layout},
    prelude::*,
    widgets::StatefulWidget,
};

use crate::tui::AppState;

pub struct StateBar;

impl StatefulWidget for StateBar {
    type State = AppState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        let app_mode_str = state.app_mode.get().to_string();

        let vol_icon = match state.volumn.get() {
            0 => "",
            1..50 => "",
            _ => "",
        };

        let vol_str = format!(" {} {}% ", vol_icon, state.volumn.get());

        let layout = Layout::horizontal([
            Constraint::Length((app_mode_str.len() + 3) as u16),
            Constraint::Fill(1),
            Constraint::Length((vol_str.len() + 2) as u16),
        ]);

        let [left, middle, right] = layout.areas(area);

        let line = Line::from(vec![
            Span::styled(format!(" {app_mode_str} "), Style::new().bg(Color::Green)),
            Span::styled("", Style::new().fg(Color::Green)),
        ]);
        line.render(left, buf);

        let layout = Layout::horizontal([
            Constraint::Length(3),
            Constraint::Length(vol_str.len() as u16),
        ]);
        let [left, right] = layout.areas(right);

        let line = Line::from(vec![
            Span::styled("", Style::new().fg(Color::Blue).bg(Color::Cyan)),
            Span::styled(vol_str, Style::new().bg(Color::Blue)),
        ]);
        line.render(right, buf);

        let line = Line::from(vec![Span::styled(
            format!(" {} ", ""),
            Style::new().bg(Color::Cyan),
        )]);
        line.render(left, buf);

        let playing = state.playing.get();
        let duration = format!("{:02}:{:02}", playing.duration / 60, playing.duration % 60);
        let current = format!("{:02.0}:{:02.0}", playing.current / 60., playing.current % 60.);
        let layout = Layout::horizontal([Constraint::Length(11)]).flex(Flex::Center);
        let [progress] = layout.areas(middle);
        let line = Line::from(vec![Span::styled(
            format!("{}/{}", current, duration),
            Style::default().fg(Color::White),
        )]);

        line.render(progress, buf);
    }
}
