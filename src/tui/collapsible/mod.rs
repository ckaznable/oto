use ratatui::{
    prelude::*,
    widgets::{Block, Borders},
};
use strum::EnumCount;

use crate::tui::{
    AppState, collapsible::devices_list::DevicesList, collapsible::keybinding::KeyBinding,
    collapsible::queue_list::QueueList,
};

pub mod devices_list;
pub mod keybinding;
pub mod queue_list;
pub mod tracks_picker;

#[derive(EnumCount, Clone, Copy)]
pub enum CollapseWidgets {
    QueueList,
    DevicesList,
    KeyBinding,
}

impl CollapseWidgets {
    pub fn get(index: usize) -> Self {
        match index {
            0 => Self::QueueList,
            1 => Self::DevicesList,
            2 => Self::KeyBinding,
            _ => Self::QueueList,
        }
    }

    pub fn widgets<'a>() -> &'a [&'a dyn CollapsibleWidget<AppState>; Self::COUNT] {
        &[&QueueList, &DevicesList, &KeyBinding]
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

pub struct CollapsibleWidgetGroup<'a, const N: usize> {
    widgets: &'a [&'a dyn CollapsibleWidget<AppState>],
    index: usize,
    collapse_size: u16,
}

impl<'a, const N: usize> CollapsibleWidgetGroup<'a, N> {
    pub fn new(
        widgets: &'a [&'a dyn CollapsibleWidget<AppState>],
        index: usize,
        collapse_size: u16,
    ) -> Self {
        assert_eq!(widgets.len(), N, "Widgets count must match N");
        Self {
            widgets,
            index,
            collapse_size,
        }
    }
}

impl<'a, const N: usize> StatefulWidget for CollapsibleWidgetGroup<'a, N> {
    type State = AppState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        let fill_len = area.height.saturating_sub((N as u16).saturating_sub(self.collapse_size * N as u16));

        let lengths: [u16; N] = std::array::from_fn(|i| {
            if i == self.index {
                fill_len
            } else {
                self.collapse_size
            }
        });

        let constraints = lengths.map(Constraint::Length);

        let layout = Layout::vertical(constraints);
        let areas = layout.split(area);

        self.widgets.iter().enumerate().for_each(|(i, w)| {
            w.render(areas[i], buf, state);
        });
    }
}
