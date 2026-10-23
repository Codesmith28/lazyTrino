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
    text::{Line, Span},
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
        theme::Entity::Catalog,
        &state.items,
        state.selected,
        search,
        is_active,
        app,
    );
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn render_selectable_list(
    frame: &mut Frame,
    area: Rect,
    title: &str,
    entity: theme::Entity,
    items: &[String],
    selected: usize,
    search: &str,
    is_active: bool,
    app: &App,
) {
    // See actions.rs for why this must be gated by pane focus.
    let line_sel = if is_active {
        crate::tui::handler::live_line_selection(app)
    } else {
        None
    };
    let title = match line_sel {
        Some(sel) => format!("{title}-- {} -- ", sel.kind.label()),
        None => title.to_string(),
    };
    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(theme::entity_border_style(entity, is_active))
        .title_style(if is_active {
            theme::bold_style(entity.color())
        } else {
            theme::muted_style()
        });
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

    // `highlight_style` is patched over each row's own style; on a cursor row
    // that is part of a selection it would turn the terminal-native highlight
    // into cyan-on-white, so it is dropped in that case.
    let mut cursor_in_selection = false;
    let items: Vec<ListItem> = filtered
        .iter()
        .enumerate()
        .map(|(display_idx, (_orig_idx, name))| {
            let item_y = inner.y + display_idx as u16;
            // See actions.rs for why this must be gated by pane focus.
            let is_mouse_sel =
                is_active && app.is_area_mouse_selected(inner.x, inner.width, item_y);

            let is_line_sel =
                line_sel.is_some_and(|sel| sel.contains(clamped_selected, display_idx));

            let prefix = format!("{:>3} ", display_idx + 1);
            let is_keyboard_sel = display_idx == clamped_selected;
            if display_idx == clamped_selected && (is_mouse_sel || is_line_sel) {
                cursor_in_selection = true;
            }
            let line = if is_mouse_sel || is_line_sel {
                Line::styled(format!("{prefix}{name}"), theme::text_selection_style())
            } else if is_keyboard_sel {
                Line::styled(
                    format!("{prefix}{name}"),
                    theme::selection_style_for(is_active),
                )
            } else {
                Line::from(vec![
                    Span::styled(prefix, theme::muted_style()),
                    Span::styled(name.as_str(), entity.style()),
                ])
            };
            ListItem::new(line)
        })
        .collect();

    let mut list_state = ListState::default().with_selected(Some(clamped_selected));
    let list = List::new(items).highlight_style(if cursor_in_selection {
        ratatui::style::Style::default()
    } else {
        theme::selection_style_for(is_active)
    });

    frame.render_stateful_widget(list, inner, &mut list_state);

    if filtered.len() > 1 {
        let mut scrollbar_state =
            ScrollbarState::new(filtered.len().saturating_sub(1)).position(clamped_selected);
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
