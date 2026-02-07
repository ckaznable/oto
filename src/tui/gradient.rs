use palette::{LinSrgb, Mix, Srgb};
use ratatui::prelude::*;

use crate::tui::AppState;

pub struct FineGradientIterator {
    start_linear: LinSrgb<f32>,
    end_linear: LinSrgb<f32>,
    current_step: u16,
    total_steps: u16,
}

impl FineGradientIterator {
    pub fn new(top_color: Color, height: u16) -> Self {
        let (r, g, b) = match top_color {
            Color::Rgb(r, g, b) => (r, g, b),
            _ => (0, 0, 0),
        };

        Self {
            start_linear: LinSrgb::new(r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0),
            end_linear: LinSrgb::new(0.0, 0.0, 0.0),
            current_step: 0,
            total_steps: height * 2,
        }
    }
}

impl Iterator for FineGradientIterator {
    type Item = Color;

    fn next(&mut self) -> Option<Self::Item> {
        if self.current_step >= self.total_steps {
            return None;
        }

        let t = (self.current_step as f32 / self.total_steps as f32).powf(1.5);
        let blended: LinSrgb<f32> = self.start_linear.mix(self.end_linear, t);
        let final_rgb: Srgb<u8> = Srgb::<f32>::from_linear(blended).into_format::<u8>();

        self.current_step += 1;
        Some(Color::Rgb(final_rgb.red, final_rgb.green, final_rgb.blue))
    }
}

#[derive(Default)]
pub struct GradientBackground;

impl StatefulWidget for GradientBackground {
    type State = AppState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        let ui = state.ui_state.borrow();
        let Some(color) = ui.theme_color else {
            return;
        };

        let mut color_iter = FineGradientIterator::new(color, area.height);

        for y in area.top()..area.bottom() {
            let upper_color = color_iter.next().unwrap_or(Color::Black);
            let lower_color = color_iter.next().unwrap_or(Color::Black);

            for x in area.left()..area.right() {
                buf[(x, y)]
                    .set_char('▄')
                    .set_bg(upper_color)
                    .set_fg(lower_color);
            }
        }
    }
}
