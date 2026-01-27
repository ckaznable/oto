use ratatui::prelude::*;
use strum::EnumCount;

use crate::tui::{AppState, collapsible::queue_list::QueueList};

pub mod queue_list;

#[derive(EnumCount, Clone, Copy)]
pub enum CollapseWidgets {
    QueueList,
}

impl CollapseWidgets {
    pub fn get(index: usize) -> Self {
        match index {
            0 => Self::QueueList,
            _ => Self::QueueList,
        }
    }

    pub fn widgets<'a>() -> &'a [&'a dyn CollapsibleWidget<AppState>; Self::COUNT] {
        &[&QueueList]
    }
}

pub trait CollapsibleWidget<T> {
    fn render_expand(&self, area: Rect, buf: &mut Buffer, state: &mut T);
    fn render_collapse(&self, area: Rect, buf: &mut Buffer, state: &mut T);

    fn render(&self, area: Rect, buf: &mut Buffer, state: &mut T) {
        match area.height {
            0 => (),
            1 => self.render_collapse(area, buf, state),
            _ => self.render_expand(area, buf, state),
        }
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
