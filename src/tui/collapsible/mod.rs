use ratatui::prelude::*;

use crate::tui::AppState;

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

pub struct CollapsibleWidgetGroup<T> {
    widgets: Vec<Box<dyn CollapsibleWidget<T>>>,
    index: usize,
}

impl StatefulWidget for CollapsibleWidgetGroup<AppState> {
    type State = AppState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        let fill_len = area.height - self.widgets.len().saturating_sub(1) as u16;
        let layout = Layout::vertical(Constraint::from_lengths(
            self.widgets
                .iter()
                .enumerate()
                .map(|(i, _)| if i == self.index { fill_len } else { 1 })
                .collect::<Box<[u16]>>(),
        ));

        let areas = layout.split(area);
        self.widgets.into_iter().enumerate().for_each(|(i, w)| {
            let area = areas[i];
            w.render(area, buf, state);
        });
    }
}
