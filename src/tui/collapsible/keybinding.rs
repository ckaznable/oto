use ratatui::{prelude::*, text::Span, widgets::Widget};

use crate::tui::{collapsible::CollapsibleWidget, AppState};

pub struct KeyBinding;

impl CollapsibleWidget<AppState> for KeyBinding {
    fn title(&self) -> Option<&'static str> {
        Some(" Keybinding ")
    }

    fn render_expand(&self, area: Rect, buf: &mut Buffer, state: &mut AppState) {
        let theme = &state.theme;

        let bindings = [
            ("<Space>", "Play / Pause"),
            ("H", "Previous track"),
            ("L", "Next track"),
            ("j", "Cursor down"),
            ("k", "Cursor up"),
            ("h", "Sub-panel left"),
            ("l", "Sub-panel right"),
            ("J", "Volume down"),
            ("K", "Volume up"),
            ("<c-j>", "Next panel"),
            ("<c-k>", "Previous panel"),
            ("a", "Toggle Artist / Album"),
            ("<CR>", "Select item"),
            ("<Tab>", "Pick item"),
            ("<c-a>", "Append picked"),
            ("i", "Insert picked"),
            ("g", "Go to top"),
            ("G", "Go to bottom"),
            ("<c-f>", "Page down"),
            ("<c-b>", "Page up"),
            ("<c-d>", "Half page down"),
            ("<c-u>", "Half page up"),
            ("q", "Quit"),
        ];

        let key_style = Style::default().fg(theme.media_title).bold();
        let desc_style = Style::default().fg(theme.subtext0);
        let separator_style = Style::default().fg(theme.overlay0);

        for (i, (key, desc)) in bindings.iter().enumerate() {
            if i as u16 >= area.height {
                break;
            }

            let y = area.y + i as u16;
            let line = Line::from(vec![
                Span::raw("  "),
                Span::styled(format!("{:>8}", key), key_style),
                Span::styled("  │  ", separator_style),
                Span::styled(*desc, desc_style),
            ]);

            line.render(Rect::new(area.x, y, area.width, 1), buf);
        }
    }

    fn render_collapse(&self, area: Rect, buf: &mut Buffer, state: &mut AppState) {
        let theme = &state.theme;

        Line::from(vec![
            Span::styled(" 󰐎 <", Style::default().fg(theme.overlay0)),
            Span::styled("Space", Style::default().fg(theme.media_title)),
            Span::styled(">", Style::default().fg(theme.overlay0)),
            Span::styled(" 󰒮 󰒭 <", Style::default().fg(theme.overlay0)),
            Span::styled("H/L", Style::default().fg(theme.media_title)),
            Span::styled(">", Style::default().fg(theme.overlay0)),
            Span::styled("  <", Style::default().fg(theme.overlay0)),
            Span::styled("c-j/k", Style::default().fg(theme.media_title)),
            Span::styled(">", Style::default().fg(theme.overlay0)),
            Span::styled(" <", Style::default().fg(theme.overlay0)),
            Span::styled("q", Style::default().fg(theme.media_title)),
            Span::styled(">", Style::default().fg(theme.overlay0)),
            Span::styled(" Quit", Style::default().fg(theme.subtext0)),
        ])
        .style(Style::default().bg(theme.surface0))
        .render(area, buf);
    }
}
