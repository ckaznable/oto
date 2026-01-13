use ratatui::{
    layout::{Constraint, Layout},
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
        let vol_len = vol_str.len() + 2;

        let app_mode_len = app_mode_str.len() + 3;

        let playing = state.playing.get();
        let current_time = format!(" {:02.0}:{:02.0} ", playing.current / 60., playing.current % 60.);
        let timer_len = current_time.len() + 1;

        let layout = Layout::horizontal([
            Constraint::Length((app_mode_len + timer_len) as u16),
            Constraint::Fill(1),
            Constraint::Length(vol_len as u16),
        ]);

        let [left, middle, right] = layout.areas(area);

        let mode_color = match state.app_mode.get() {
            crate::tui::AppMode::Normal => (Color::Reset, Color::Blue),
            crate::tui::AppMode::Playing => (Color::Black, Color::Green),
            crate::tui::AppMode::Paused => (Color::Black, Color::Red),
        };
        let timer_color = Color::Magenta;

        let line = Line::from(vec![
            Span::styled(format!(" {app_mode_str} "), Style::new().fg(mode_color.0).bg(mode_color.1)),
            Span::styled("", Style::new().fg(mode_color.1).bg(timer_color)),
        ]);
        line.render(left, buf);

        let line = Line::from(vec![
            Span::styled(current_time, Style::new().bg(timer_color).fg(Color::Black)),
            Span::styled("", Style::new().fg(timer_color).bg(Color::White)),
        ]);
        line.render(Rect {
            x: left.x + app_mode_len as u16,
            y: left.y,
            width: timer_len as u16,
            height: 1,
        }, buf);

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
        let ratio = (playing.current / playing.duration as f64).clamp(0.0, 1.0);
        let available_width = area.width as f64;
        let filled_width = available_width * ratio;

        let full_blocks = filled_width.floor() as u16;
        for i in 0..full_blocks {
            self.draw_cell(area.x + i, area.y, buf, 8);
        }
        let fraction = filled_width - filled_width.floor();
        self.draw_cell(area.x + full_blocks, area.y, buf, (fraction * 8.0) as u8);
    }
}
