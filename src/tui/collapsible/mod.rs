use ratatui::prelude::*;

use crate::tui::{AppState, collapsible::queue_list::QueueList};

pub mod queue_list;

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
        const WIDGETS_NUM: usize = 1;

        let index = state.ui_state.borrow().expand_index;
        let widgets: &[&dyn CollapsibleWidget<AppState>] = &[&QueueList];

        let fill_len = area.height - widgets.len().saturating_sub(1) as u16;

        let constraint: [u16; WIDGETS_NUM] =
            std::array::from_fn(|i| if i == index { fill_len } else { 1 });

        let layout = Layout::vertical(Constraint::from_lengths(constraint));
        let areas = layout.split(area);
        widgets.iter().enumerate().for_each(|(i, w)| {
            w.render(areas[i], buf, state);
        });
    }
}
