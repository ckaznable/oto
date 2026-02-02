use ratatui::{
    prelude::*,
    widgets::{Block, Borders},
};
use strum::{EnumCount, FromRepr};

use crate::tui::{
    AppState,
    collapsible::{
        devices_list::DevicesList, keybinding::KeyBinding, queue_list::QueueList, search::Search,
        tracks_picker::TracksPicker,
    },
};

pub mod devices_list;
pub mod keybinding;
pub mod queue_list;
pub mod search;
pub mod tracks_picker;

#[derive(EnumCount, Clone, Copy, FromRepr)]
#[repr(u8)]
pub enum CollapseWidgets {
    QueueList,
    TrackPicker,
    Search,
    DevicesList,
    KeyBinding,
}

impl CollapseWidgets {
    pub fn get(index: usize) -> Self {
        Self::from_repr(index as u8).unwrap_or(Self::QueueList)
    }

    pub fn widgets<'a>() -> &'a [&'a dyn CollapsibleWidget<AppState>; Self::COUNT] {
        &[
            &QueueList,
            &TracksPicker,
            &Search,
            &DevicesList,
            &KeyBinding,
        ]
    }
}

pub trait CollapsibleWidget<T: HasTheme> {
    fn render_expand(&self, area: Rect, buf: &mut Buffer, state: &mut T);
    fn render_collapse(&self, area: Rect, buf: &mut Buffer, state: &mut T);

    fn title(&self) -> Option<&'static str> {
        None
    }

    fn render(&self, area: Rect, buf: &mut Buffer, state: &mut T) {
        match area.height {
            0 => (),
            1 => self.render_collapse(area, buf, state),
            _ => {
                let area = if let Some(title) = self.title() {
                    let theme = state.theme();
                    let block = Block::default()
                        .title(title)
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(theme.border))
                        .title_style(Style::default().fg(theme.text).bold());

                    let inner_area = block.inner(area);
                    block.render(area, buf);
                    inner_area
                } else {
                    area
                };

                self.render_expand(area, buf, state);
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
        debug_assert_eq!(widgets.len(), N, "Widgets count must match N");
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
        let fill_len = area
            .height
            .saturating_sub((N as u16).saturating_sub(self.collapse_size * N as u16));

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
