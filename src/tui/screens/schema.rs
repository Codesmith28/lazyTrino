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

use crate::{app::SchemaState, tui::theme};

pub fn render(frame: &mut Frame, area: Rect, state: &SchemaState, search: &str, is_active: bool) {
    let block = Block::default()
        .title(format!(" Schemas — {} ", state.catalog))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(theme::border_style(is_active));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let filtered: Vec<(usize, &String)> = state
        .items
        .iter()
        .enumerate()
        .filter(|(_, name)| {
            search.is_empty() || name.to_lowercase().contains(&search.to_lowercase())
        })
        .collect();

    if filtered.is_empty() {
        return;
    }

    let items: Vec<ListItem> = filtered
        .iter()
        .map(|(orig_idx, name)| {
            let prefix = format!("{:>3} ", orig_idx + 1);
            let line = if *orig_idx == state.selected {
                Line::styled(format!("{prefix}{name}"), theme::selection_style())
            } else {
                Line::styled(format!("{prefix}{name}"), theme::text_style())
            };
            ListItem::new(line)
        })
        .collect();

    let mut list_state = ListState::default().with_selected(Some(state.selected));
    let list = List::new(items).highlight_style(theme::selection_style());

    frame.render_stateful_widget(list, inner, &mut list_state);

    use ratatui::widgets::{Scrollbar, ScrollbarOrientation, ScrollbarState};
    if filtered.len() > 1 {
        let mut scrollbar_state =
            ScrollbarState::new(filtered.len().saturating_sub(1)).position(state.selected);
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
