use ratatui::{
    layout::{Constraint, Layout, Margin},
    prelude::*,
    text::Span,
    widgets::{List, ListItem, Scrollbar, ScrollbarOrientation, StatefulWidget, Widget},
};

use crate::tui::{collapsible::CollapsibleWidget, AppState};

pub struct DevicesList;

impl CollapsibleWidget<AppState> for DevicesList {
    fn title(&self) -> &'static str {
        " Devices "
    }

    fn render_expand_content(&self, area: Rect, buf: &mut Buffer, state: &mut AppState) {
        let theme = &state.theme;
        let devices = state.devices.borrow();
        let current_device = devices.current;

        let flat_list: Vec<(i32, i32, String)> = devices
            .list
            .iter()
            .flat_map(|pcm| {
                let pcm_name = pcm.name.as_deref().unwrap_or("Unknown");
                if pcm.devices.is_empty() {
                    vec![(pcm.index, 0, pcm_name.to_string())]
                } else {
                    pcm.devices
                        .iter()
                        .map(|dev| {
                            let dev_name = dev.name.as_deref().unwrap_or("Default");
                            (pcm.index, dev.index, format!("{pcm_name} - {dev_name}"))
                        })
                        .collect()
                }
            })
            .collect();

        let list_len = flat_list.len();

        let mut ui_state = state.ui_state.borrow_mut();
        ui_state.devices.set_content_length(list_len);
        drop(ui_state);

        let cursor_index = state.ui_state.borrow().devices.cursor_index;

        let layout = Layout::horizontal([Constraint::Fill(1), Constraint::Length(1)]);
        let [list_area, scrollbar_area] = layout.areas(area);

        let visible_height = list_area.height as usize;
        let offset = calculate_scroll_offset(cursor_index, visible_height, list_len);

        state.ui_state.borrow_mut().cache.list_items.clear();

        let list_iter = flat_list
            .iter()
            .enumerate()
            .skip(offset)
            .take(visible_height);

        for (index, (pcm_idx, dev_idx, name)) in list_iter {
            let is_current = *pcm_idx == current_device.0 && *dev_idx == current_device.1;
            let is_cursor = index == cursor_index;

            let bg_color = if is_cursor {
                theme.surface1
            } else if is_current {
                theme.queue_current_bg
            } else {
                theme.base
            };

            let indicator = if is_current { ">" } else { " " };

            let line = Line::from(vec![
                Span::styled(
                    format!("{indicator} "),
                    Style::default().fg(theme.mode_playing_fg),
                ),
                Span::styled(name.clone(), Style::default().fg(theme.queue_title)),
            ]);

            let item = ListItem::new(line).style(Style::default().bg(bg_color));
            state.ui_state.borrow_mut().cache.list_items.push(item);
        }

        drop(devices);

        let mut ui_state = state.ui_state.borrow_mut();
        let list = List::new(ui_state.cache.list_items.drain(..));
        Widget::render(list, list_area, buf);
        drop(ui_state);

        let mut scroll_state = state.ui_state.borrow().devices.scroll_state;
        scroll_state = scroll_state.position(cursor_index);

        let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .begin_symbol(None)
            .end_symbol(None)
            .track_symbol(Some("│"))
            .thumb_symbol("█")
            .style(Style::default().fg(theme.overlay0));

        StatefulWidget::render(
            scrollbar,
            scrollbar_area.inner(Margin::new(0, 0)),
            buf,
            &mut scroll_state,
        );

        state.ui_state.borrow_mut().devices.scroll_state = scroll_state;
    }

    fn render_collapse(&self, area: Rect, buf: &mut Buffer, state: &mut AppState) {
        let theme = &state.theme;
        let devices = state.devices.borrow();
        let current_device = devices.current;

        let current_name = devices
            .list
            .iter()
            .find(|pcm| pcm.index == current_device.0)
            .map(|pcm| {
                let pcm_name = pcm.name.as_deref().unwrap_or("Unknown");
                let device_name = pcm
                    .devices
                    .iter()
                    .find(|d| d.index == current_device.1)
                    .and_then(|d| d.name.as_deref());

                match device_name {
                    Some(dev) => format!("{pcm_name} - {dev}"),
                    None => pcm_name.to_string(),
                }
            })
            .unwrap_or_else(|| "No Device".to_string());

        Line::from(vec![
            Span::raw("  "),
            Span::styled(current_name, Style::default().fg(theme.text)),
        ])
        .style(Style::default().bg(theme.collapse_devices_bg))
        .render(area, buf);
    }
}

fn calculate_scroll_offset(cursor: usize, visible_height: usize, total_items: usize) -> usize {
    if total_items <= visible_height {
        return 0;
    }

    let half_visible = visible_height / 2;

    if cursor < half_visible {
        0
    } else if cursor >= total_items.saturating_sub(half_visible) {
        total_items.saturating_sub(visible_height)
    } else {
        cursor.saturating_sub(half_visible)
    }
}
