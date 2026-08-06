// Copyright 2026 Sarthak Siddhpura
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use ratatui::{
    Frame,
    layout::Rect,
    text::Line,
    widgets::{
        Block, BorderType, Borders, List, ListItem, ListState, Scrollbar, ScrollbarOrientation,
        ScrollbarState,
    },
};

use crate::{
    app::{App, CatalogState},
    tui::theme,
};

pub fn render(
    frame: &mut Frame,
    area: Rect,
    state: &CatalogState,
    search: &str,
    is_active: bool,
    app: &App,
) {
    render_selectable_list(
        frame,
        area,
        " Catalogs ",
        &state.items,
        state.selected,
        search,
        is_active,
        app,
    );
}

pub(crate) fn render_selectable_list(
    frame: &mut Frame,
    area: Rect,
    title: &str,
    items: &[String],
    selected: usize,
    search: &str,
    is_active: bool,
    app: &App,
) {
    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(theme::border_style(is_active));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let filtered: Vec<(usize, &String)> = items
        .iter()
        .enumerate()
        .filter(|(_, name)| {
            search.is_empty() || name.to_lowercase().contains(&search.to_lowercase())
        })
        .collect();

    if filtered.is_empty() {
        return;
    }

    let clamped_selected = selected.min(filtered.len().saturating_sub(1));

    let items: Vec<ListItem> = filtered
        .iter()
        .enumerate()
        .map(|(display_idx, (orig_idx, name))| {
            let item_y = inner.y + display_idx as u16;
            let is_mouse_sel = app.is_area_mouse_selected(inner.x, inner.width, item_y);

            let prefix = format!("{:>3} ", orig_idx + 1);
            let is_keyboard_sel = display_idx == clamped_selected;
            let line = if is_mouse_sel || is_keyboard_sel {
                Line::styled(format!("{prefix}{name}"), theme::selection_style())
            } else {
                Line::styled(format!("{prefix}{name}"), theme::text_style())
            };
            ListItem::new(line)
        })
        .collect();

    let mut list_state = ListState::default().with_selected(Some(clamped_selected));
    let list = List::new(items).highlight_style(theme::selection_style());

    frame.render_stateful_widget(list, inner, &mut list_state);

    if filtered.len() > 1 {
        let mut scrollbar_state =
            ScrollbarState::new(filtered.len().saturating_sub(1)).position(selected);
        frame.render_stateful_widget(
            Scrollbar::default()
                .orientation(ScrollbarOrientation::VerticalRight)
                .begin_symbol(Some("▲"))
                .end_symbol(Some("▼")),
            inner,
            &mut scrollbar_state,
        );
    }
}
