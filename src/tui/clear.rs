use ratatui::prelude::*;

pub struct ClearArea;

impl Widget for ClearArea {
    fn render(self, area: Rect, buf: &mut Buffer)
    where
        Self: Sized,
    {
        for y in area.top()..area.bottom() {
            for x in area.left()..area.right() {
                buf[(x, y)].set_char(' ');
            }
        }
    }
}
