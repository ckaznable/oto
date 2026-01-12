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

        let mode_color = match state.app_mode.get() {
            crate::tui::AppMode::Normal => (Color::Reset, Color::Blue),
            crate::tui::AppMode::Playing => (Color::Black, Color::Green),
            crate::tui::AppMode::Paused => (Color::Black, Color::Red),
        };

        let line = Line::from(vec![
            Span::styled(format!(" {app_mode_str} "), Style::new().fg(mode_color.0).bg(mode_color.1)),
            Span::styled("", Style::new().fg(mode_color.1)),
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

        ProgressBar.render(middle, buf, state);
        let playing = state.playing.get();
        let duration = format!("{:02}:{:02}", playing.duration / 60, playing.duration % 60);
        let current = format!("{:02.0}:{:02.0}", playing.current / 60., playing.current % 60.);
        let layout = Layout::horizontal([Constraint::Length(11)]).flex(Flex::Center);
        let [timer] = layout.areas(middle);
        let line = Line::from(vec![Span::styled(
            format!("{}/{}", current, duration),
            Style::new().fg(Color::White),
        )]);

        line.render(timer, buf);
    }
}

pub struct ProgressBar;

impl ProgressBar {
    fn draw_cell(&self, x: u16, y: u16, buf: &mut Buffer, width: u8) {
        if let Some(cell) = buf.cell_mut(Position::new(x, y)) {
            cell.set_char(match width {
                1 => '▏',
                2 => '▎',
                3 => '▍',
                4 => '▌',
                5 => '▋',
                6 => '▊',
                7 => '▉',
                8 => '█',
                _ => ' ',
            });

            cell.set_fg(Color::White);
        }
    }
}

impl StatefulWidget for ProgressBar {
    type State = AppState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        let playing = state.playing.get();
        let progress = (playing.current / playing.duration as f64) * 100.;
        let percent_per_cell = 100. / area.width as f32;
        for i in 0..area.width {
            let mut cell_width = 0u8;
            let current_cell_width = percent_per_cell * i;
            if current_cell_width < progress {
                cell_width = 8;
            }

            if current_cell_width > progress && current_cell_width - progress < percent_per_cell {
                cell_width = (current_cell_width - progress) % 8;
            }

            if cell_width == 0 {
                break;
            }

            self.draw_cell(area.x + i, area.y, buf, cell_width);
        }
    }
}
