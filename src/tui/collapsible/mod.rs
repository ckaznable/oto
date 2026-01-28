use ratatui::{
    prelude::*,
    widgets::{Block, Borders},
};
use strum::EnumCount;

use crate::tui::{
    AppState, collapsible::devices_list::DevicesList, collapsible::queue_list::QueueList,
};

pub mod devices_list;
pub mod queue_list;

#[derive(EnumCount, Clone, Copy)]
pub enum CollapseWidgets {
    QueueList,
    DevicesList,
}

impl CollapseWidgets {
    pub fn get(index: usize) -> Self {
        match index {
            0 => Self::QueueList,
            1 => Self::DevicesList,
            _ => Self::QueueList,
        }
    }

    pub fn widgets<'a>() -> &'a [&'a dyn CollapsibleWidget<AppState>; Self::COUNT] {
        &[&QueueList, &DevicesList]
    }
}

pub trait CollapsibleWidget<T: HasTheme> {
    fn title(&self) -> &'static str;
    fn render_expand_content(&self, area: Rect, buf: &mut Buffer, state: &mut T);
    fn render_collapse(&self, area: Rect, buf: &mut Buffer, state: &mut T);

    fn render(&self, area: Rect, buf: &mut Buffer, state: &mut T) {
        match area.height {
            0 => (),
            1 => self.render_collapse(area, buf, state),
            _ => {
                let theme = state.theme();
                let block = Block::default()
                    .title(self.title())
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(theme.border))
                    .title_style(Style::default().fg(theme.text).bold());

                let inner_area = block.inner(area);
                block.render(area, buf);

                self.render_expand_content(inner_area, buf, state);
            }
        }
    }
}

pub trait HasTheme {
    fn theme(&self) -> &crate::tui::theme::Theme;
}

impl HasTheme for AppState {
    fn theme(&self) -> &crate::tui::theme::Theme {
        &self.theme
    }
}

pub struct CollapsibleWidgetGroup;

impl StatefulWidget for CollapsibleWidgetGroup {
    type State = AppState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        const WIDGETS_NUM: usize = CollapseWidgets::COUNT;

        let index = state.ui_state.borrow().expand_index;

        let fill_len = area.height - WIDGETS_NUM.saturating_sub(WIDGETS_NUM - 1) as u16;

        let constraint: [u16; WIDGETS_NUM] =
            std::array::from_fn(|i| if i == index { fill_len } else { 1 });

        let layout = Layout::vertical(Constraint::from_lengths(constraint));
        let areas = layout.split(area);
        let widgets = CollapseWidgets::widgets();
        widgets.iter().enumerate().for_each(|(i, w)| {
            w.render(areas[i], buf, state);
        });
    }
}
