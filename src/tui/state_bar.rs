use ratatui::{
    layout::{Constraint, Layout},
    prelude::*,
    widgets::StatefulWidget,
};

use crate::tui::AppState;

#[derive(Debug, Clone, Copy)]
pub enum TriangleDirection {
    Left,
    Right,
}

#[derive(Debug, Clone)]
pub struct TriangleSegment {
    content: String,
    fg: Color,
    bg: Color,
    triangle: Option<(TriangleDirection, Color)>,
}

impl TriangleSegment {
    pub fn new<S: Into<String>>(content: S) -> Self {
        Self {
            content: content.into(),
            fg: Color::Reset,
            bg: Color::Reset,
            triangle: None,
        }
    }

    pub fn fg(mut self, color: Color) -> Self {
        self.fg = color;
        self
    }

    pub fn bg(mut self, color: Color) -> Self {
        self.bg = color;
        self
    }

    pub fn with_triangle(mut self, direction: TriangleDirection, tail_bg: Color) -> Self {
        self.triangle = Some((direction, tail_bg));
        self
    }

    pub fn tail_bg(mut self, color: Color) -> Self {
        if let Some((dir, _)) = self.triangle {
            self.triangle = Some((dir, color));
        }
        self
    }

    pub fn build(self) -> Vec<Span<'static>> {
        let mut spans = Vec::new();

        if let Some((direction, tail_bg)) = self.triangle {
            match direction {
                TriangleDirection::Left => {
                    spans.push(Span::styled("", Style::new().fg(self.bg).bg(tail_bg)));
                    spans.push(Span::styled(
                        self.content,
                        Style::new().fg(self.fg).bg(self.bg),
                    ));
                }
                TriangleDirection::Right => {
                    spans.push(Span::styled(
                        self.content,
                        Style::new().fg(self.fg).bg(self.bg),
                    ));
                    spans.push(Span::styled("", Style::new().fg(self.bg).bg(tail_bg)));
                }
            }
        } else {
            spans.push(Span::styled(
                self.content,
                Style::new().fg(self.fg).bg(self.bg),
            ));
        }

        spans
    }

    pub fn render(self, area: Rect, buf: &mut Buffer) {
        let spans = self.build();
        Line::from(spans).render(area, buf);
    }
}

pub struct StateBar;

impl StatefulWidget for StateBar {
    type State = AppState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        let theme = &state.theme;
        let app_mode_str = state.app_mode.get().to_string();

        let vol_icon = match state.volume.get() {
            0 => "",
            1..50 => "",
            _ => "",
        };

        let vol_str = format!(" {} {}% ", vol_icon, state.volume.get());
        let vol_len = vol_str.len() + 2;

        let app_mode_len = app_mode_str.len() + 3;

        let playing = state.playing.get();
        let current_time = format!(
            " {:02}:{:02.0} ",
            (playing.current / 60.).floor(),
            (playing.current % 60.).floor()
        );
        let timer_len = current_time.len() + 1;

        let layout = Layout::horizontal([
            Constraint::Length((app_mode_len + timer_len) as u16),
            Constraint::Fill(1),
            Constraint::Length(vol_len as u16),
        ]);

        let [left, middle, right] = layout.areas(area);

        let mode_color = match state.app_mode.get() {
            crate::tui::AppMode::Normal => (theme.mode_normal_fg, theme.mode_normal_bg),
            crate::tui::AppMode::Playing => (theme.mode_playing_fg, theme.mode_playing_bg),
            crate::tui::AppMode::Paused => (theme.mode_paused_fg, theme.mode_paused_bg),
        };

        TriangleSegment::new(format!(" {app_mode_str} "))
            .fg(mode_color.0)
            .bg(mode_color.1)
            .with_triangle(TriangleDirection::Right, theme.timer_bg)
            .render(left, buf);

        let arrow_bg = if state.playing.get().current > 0. {
            theme.progress_active_bg
        } else {
            theme.progress_inactive_bg
        };

        TriangleSegment::new(current_time)
            .fg(theme.timer_fg)
            .bg(theme.timer_bg)
            .with_triangle(TriangleDirection::Right, arrow_bg)
            .render(
                Rect {
                    x: left.x + app_mode_len as u16,
                    y: left.y,
                    width: timer_len as u16,
                    height: 1,
                },
                buf,
            );

        let layout = Layout::horizontal([
            Constraint::Length(3),
            Constraint::Length(vol_str.len() as u16),
        ]);
        let [left, right] = layout.areas(right);

        TriangleSegment::new(vol_str)
            .bg(theme.volume_bg)
            .with_triangle(TriangleDirection::Left, theme.volume_icon_bg)
            .render(right, buf);

        let line = Line::from(vec![Span::styled(
            format!(
                " {} ",
                match state.play_mode.get() {
                    crate::tui::PlayMode::Normal => "➡",
                    crate::tui::PlayMode::Loop => "",
                    crate::tui::PlayMode::LoopCurrent => "",
                }
            ),
            Style::new().bg(theme.volume_icon_bg),
        )]);
        line.render(left, buf);

        ProgressBar.render(middle, buf, state);
    }
}

pub struct ProgressBar;

impl ProgressBar {
    fn draw_cell(&self, x: u16, y: u16, buf: &mut Buffer, width: u8, fg: Color) {
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

            cell.set_fg(fg);
        }
    }
}

impl StatefulWidget for ProgressBar {
    type State = AppState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        let theme = &state.theme;
        let playing = state.playing.get();
        let ratio = (playing.current / playing.duration as f64).clamp(0.0, 1.0);
        let available_width = area.width as f64;
        let filled_width = available_width * ratio;

        let full_blocks = filled_width.floor() as u16;
        for i in 0..full_blocks {
            self.draw_cell(area.x + i, area.y, buf, 8, theme.progress_bar_fg);
        }
        let fraction = filled_width - filled_width.floor();
        self.draw_cell(
            area.x + full_blocks,
            area.y,
            buf,
            (fraction * 8.0) as u8,
            theme.progress_bar_fg,
        );
    }
}
