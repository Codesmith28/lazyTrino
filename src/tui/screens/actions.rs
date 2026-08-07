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
    widgets::{Block, BorderType, Borders, List, ListItem, ListState},
};

use crate::{
    app::{ACTIONS, App},
    tui::theme,
};

#[allow(clippy::too_many_arguments)]
pub fn render(
    frame: &mut Frame,
    area: Rect,
    _catalog: &str,
    _schema: &str,
    table: &str,
    selected: usize,
    is_active: bool,
    app: &App,
) {
    let block = Block::default()
        .title(if area.width < 25 {
            " Menu ".to_string()
        } else {
            format!(" Menu — {table} ")
        })
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(theme::border_style(is_active));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let items: Vec<ListItem> = ACTIONS
        .iter()
        .enumerate()
        .map(|(i, (key, label, _))| {
            let item_y = inner.y + i as u16;
            // Gate stale mouse hover/click highlighting by pane focus: once
            // the user moves focus to another pane (via keyboard), a row
            // clicked earlier in this pane must stop appearing selected —
            // otherwise it stays highlighted forever since the underlying
            // anchor/current coordinates are only cleared by the *next*
            // mouse click, not by any keyboard-driven focus change.
            let is_mouse_sel =
                is_active && app.is_area_mouse_selected(inner.x, inner.width, item_y);

            let is_selected = i == selected;
            let prefix = if is_selected { "▸ " } else { "  " };
            let text = format!("{prefix}[{key}] {label}");
            let line = if is_mouse_sel {
                Line::styled(text, theme::selection_style())
            } else if is_selected {
                Line::styled(text, theme::selection_style_for(is_active))
            } else {
                Line::styled(text, theme::text_style())
            };
            ListItem::new(line)
        })
        .collect();

    let mut list_state = ListState::default().with_selected(Some(selected));
    let list = List::new(items).highlight_style(theme::selection_style_for(is_active));

    frame.render_stateful_widget(list, inner, &mut list_state);

    use ratatui::widgets::{Scrollbar, ScrollbarOrientation, ScrollbarState};
    if ACTIONS.len() > 1 {
        let mut scrollbar_state =
            ScrollbarState::new(ACTIONS.len().saturating_sub(1)).position(selected);
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
