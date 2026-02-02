use ratatui::{
    layout::{Margin, Rect},
    prelude::*,
    widgets::{Scrollbar, ScrollbarOrientation, ScrollbarState, StatefulWidget},
};

use crate::tui::theme::Theme;

pub fn render_scrollbar(
    area: Rect,
    buf: &mut Buffer,
    theme: &Theme,
    scroll_state: &mut ScrollbarState,
) {
    let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
        .begin_symbol(None)
        .end_symbol(None)
        .track_symbol(Some("│"))
        .thumb_symbol("█")
        .style(Style::default().fg(theme.overlay0));

    StatefulWidget::render(scrollbar, area.inner(Margin::new(0, 0)), buf, scroll_state);
}
