use ratatui::{
    layout::{Constraint, Layout},
    prelude::*,
    widgets::StatefulWidget,
};
use unicode_width::UnicodeWidthStr;

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

#[derive(Debug, Clone)]
pub struct SegmentData {
    pub content: String,
    pub fg: Color,
    pub bg: Color,
}

impl SegmentData {
    pub fn new<S: Into<String>>(content: S, fg: Color, bg: Color) -> Self {
        Self {
            content: content.into(),
            fg,
            bg,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct TriangleSegmentGroup {
    segments: Vec<SegmentData>,
}

impl TriangleSegmentGroup {
    pub fn add_segment(mut self, content: String, fg: Color, bg: Color) -> Self {
        self.segments.push(SegmentData::new(content, fg, bg));
        self
    }

    pub fn from_segments(segments: Vec<SegmentData>) -> Self {
        Self { segments }
    }

    pub fn constraints(&self) -> Vec<Constraint> {
        self.segments
            .iter()
            .map(|seg| Constraint::Length((seg.content.width() + 1) as u16))
            .collect()
    }

    pub fn render(&self, areas: &[Rect], buf: &mut Buffer, tail_bg: Color) {
        for (i, (seg, area)) in self.segments.iter().zip(areas.iter()).enumerate() {
            let next_bg = if i + 1 < self.segments.len() {
                self.segments[i + 1].bg
            } else {
                tail_bg
            };

            TriangleSegment::new(seg.content.clone())
                .fg(seg.fg)
                .bg(seg.bg)
                .with_triangle(TriangleDirection::Right, next_bg)
                .render(*area, buf);
        }
    }
}

pub struct StateBar;

impl StatefulWidget for StateBar {
    type State = AppState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        let theme = &state.theme;
        let app_mode_str = state.app_mode.get().to_string();
        let app_mode_formatted = format!(" {app_mode_str} ");

        let vol_icon = match state.volume.get() {
            0 => "",
            1..50 => "",
            _ => "",
        };

        let vol_str = format!(" {} {}% ", vol_icon, state.volume.get());

        let play_mode_icon = match state.play_mode.get() {
            crate::tui::PlayMode::Normal => "➡",
            crate::tui::PlayMode::Loop => "",
            crate::tui::PlayMode::LoopCurrent => "",
        };
        let play_mode_str = format!(" {} ", play_mode_icon);

        let track = state.playing_track.borrow();
        let sr = track.spec.sample_rate;
        let sample_rate_str = if sr >= 1_000_000 {
            format!(" {:.1}M ", (sr as f64 / 1_000_000.0 * 10.).trunc() / 10.)
        } else if sr >= 1_000 {
            format!(" {:.1}k ", (sr as f64 / 1_000.0 * 10.).trunc() / 10.)
        } else {
            format!(" {}Hz ", sr)
        };

        let sample_rate_color = if sr >= 2_822_400 {
            theme.sample_rate_dsd
        } else if sr >= 176_400 {
            theme.sample_rate_ultrahires
        } else if sr >= 88_200 {
            theme.sample_rate_hires
        } else {
            theme.sample_rate_cd
        };
        drop(track);

        let playing = state.playing.get();
        let current_time = format!(
            " {:02}:{:02.0} ",
            (playing.current / 60.).floor(),
            (playing.current % 60.).floor()
        );

        let mode_color = match state.app_mode.get() {
            crate::tui::AppMode::Normal => (theme.mode_normal_fg, theme.mode_normal_bg),
            crate::tui::AppMode::Playing => (theme.mode_playing_fg, theme.mode_playing_bg),
            crate::tui::AppMode::Paused => (theme.mode_paused_fg, theme.mode_paused_bg),
        };

        let arrow_bg = if state.playing.get().current > 0. {
            theme.progress_active_bg
        } else {
            theme.progress_inactive_bg
        };

        let group = TriangleSegmentGroup::default()
            .add_segment(app_mode_formatted, mode_color.0, mode_color.1)
            .add_segment(sample_rate_str, sample_rate_color, theme.sample_rate_bg)
            .add_segment(vol_str, theme.text, theme.volume_bg)
            .add_segment(play_mode_str, theme.text, theme.volume_icon_bg)
            .add_segment(current_time, theme.timer_fg, theme.timer_bg);

        let mut constraints = group.constraints();
        constraints.push(Constraint::Fill(1));

        let layout = Layout::horizontal(constraints).split(area);

        group.render(&layout[0..5], buf, arrow_bg);

        ProgressBar.render(layout[5], buf, state);
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
