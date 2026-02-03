use ratatui::{prelude::*, text::Span, widgets::Widget};

use crate::tui::{
    AppState,
    collapsible::{CollapseWidgets, CollapsibleWidget},
};

pub struct KeyBinding;

const BINDINGS: [(&str, &str); 25] = [
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
    ("/", "Search mode"),
    ("<Esc>", "Exit search"),
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

impl CollapsibleWidget<AppState> for KeyBinding {
    fn title(&self) -> Option<&'static str> {
        Some(" Keybinding ")
    }

    fn render_expand(&self, area: Rect, buf: &mut Buffer, state: &mut AppState) {
        let theme = &state.theme;

        let key_style = Style::default().fg(theme.media_title).bold();
        let desc_style = Style::default().fg(theme.subtext0);
        let separator_style = Style::default().fg(theme.overlay0);

        let mut cache = state.cache.borrow_mut();

        for (i, (key, desc)) in BINDINGS.iter().enumerate() {
            if i as u16 >= area.height {
                break;
            }

            cache.spans.clear();
            cache.spans.push(Span::raw("  "));
            cache
                .spans
                .push(Span::styled(format!("{:>8}", key), key_style));
            cache.spans.push(Span::styled("  │  ", separator_style));
            cache.spans.push(Span::styled(*desc, desc_style));

            let line = Line::from(cache.spans.drain(..).collect::<Vec<_>>());
            let y = area.y + i as u16;
            line.render(Rect::new(area.x, y, area.width, 1), buf);
        }
    }

    fn render_collapse(&self, area: Rect, buf: &mut Buffer, state: &mut AppState) {
        let theme = &state.theme;

        let mut cache = state.cache.borrow_mut();
        cache.spans.clear();

        if matches!(
            CollapseWidgets::get(state.ui_state.borrow().expand_index),
            CollapseWidgets::TrackPicker | CollapseWidgets::Search
        ) {
            cache
                .spans
                .push(Span::styled(" 󱫉 <", Style::default().fg(theme.overlay0)));
            cache
                .spans
                .push(Span::styled("Tab", Style::default().fg(theme.media_title)));
            cache
                .spans
                .push(Span::styled(">", Style::default().fg(theme.overlay0)));
            cache
                .spans
                .push(Span::styled(" 󰬳 <", Style::default().fg(theme.overlay0)));
            cache.spans.push(Span::styled(
                "Enter",
                Style::default().fg(theme.media_title),
            ));
            cache
                .spans
                .push(Span::styled(">", Style::default().fg(theme.overlay0)));
            cache
                .spans
                .push(Span::styled("  <", Style::default().fg(theme.overlay0)));
            cache
                .spans
                .push(Span::styled("i", Style::default().fg(theme.media_title)));
            cache
                .spans
                .push(Span::styled(">", Style::default().fg(theme.overlay0)));
            cache
                .spans
                .push(Span::styled(" 󰨿 <", Style::default().fg(theme.overlay0)));
            cache.spans.push(Span::styled(
                "<c-a>",
                Style::default().fg(theme.media_title),
            ));
            cache
                .spans
                .push(Span::styled(">", Style::default().fg(theme.overlay0)));
        }

        cache
            .spans
            .push(Span::styled(" 󰐎 <", Style::default().fg(theme.overlay0)));
        cache.spans.push(Span::styled(
            "Space",
            Style::default().fg(theme.media_title),
        ));
        cache
            .spans
            .push(Span::styled(">", Style::default().fg(theme.overlay0)));
        cache
            .spans
            .push(Span::styled(" 󰒮 󰒭 <", Style::default().fg(theme.overlay0)));
        cache
            .spans
            .push(Span::styled("H/L", Style::default().fg(theme.media_title)));
        cache
            .spans
            .push(Span::styled(">", Style::default().fg(theme.overlay0)));
        cache
            .spans
            .push(Span::styled("   <", Style::default().fg(theme.overlay0)));
        cache.spans.push(Span::styled(
            "c-j/k",
            Style::default().fg(theme.media_title),
        ));
        cache
            .spans
            .push(Span::styled(">", Style::default().fg(theme.overlay0)));
        cache
            .spans
            .push(Span::styled("  <", Style::default().fg(theme.overlay0)));
        cache
            .spans
            .push(Span::styled("j/k", Style::default().fg(theme.media_title)));
        cache
            .spans
            .push(Span::styled(">", Style::default().fg(theme.overlay0)));
        cache
            .spans
            .push(Span::styled(" Quit ", Style::default().fg(theme.subtext0)));
        cache
            .spans
            .push(Span::styled("<", Style::default().fg(theme.overlay0)));
        cache
            .spans
            .push(Span::styled("q", Style::default().fg(theme.media_title)));
        cache
            .spans
            .push(Span::styled(">", Style::default().fg(theme.overlay0)));

        let line = Line::from(cache.spans.drain(..).collect::<Vec<_>>());
        line.style(Style::default().bg(theme.surface0))
            .render(area, buf);
    }
}
