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
    layout::{Constraint, Layout, Rect},
    text::{Line, Span},
    widgets::{List, ListItem, ListState, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState},
};

use crate::app::SavedViewsModalState;
use crate::tui::screens::modal;
use crate::tui::theme;

pub fn render(frame: &mut Frame, area: Rect, modal: &SavedViewsModalState) {
    let popup_area = modal::modal_large(area);
    let title = format!(" Saved Views ({} saved) ", modal.views.len());
    let inner = modal::render_modal_frame(frame, popup_area, &title, true);

    if inner.height < 4 {
        return;
    }

    let chunks = Layout::vertical([
        Constraint::Min(0),    // Views list
        Constraint::Length(1), // Hint footer
    ])
    .split(inner);

    let list_area = chunks[0];
    let hint_area = chunks[1];

    if modal.views.is_empty() {
        let empty_msg = vec![
            Line::from(""),
            Line::from(Span::styled("  No saved views yet.", theme::header_style())),
            Line::from(""),
            Line::from(Span::styled(
                "  Press Ctrl+s inside any table to capture the active view,",
                theme::muted_style(),
            )),
            Line::from(Span::styled(
                "  including filters, column visibility, sort state, and query.",
                theme::muted_style(),
            )),
        ];
        frame.render_widget(Paragraph::new(empty_msg), list_area);
    } else {
        let items: Vec<ListItem> = modal
            .views
            .iter()
            .enumerate()
            .map(|(i, view)| {
                let is_selected = i == modal.selected;
                let prefix = if is_selected { "▸ " } else { "  " };
                let title_line = Line::from(vec![
                    Span::styled(prefix, if is_selected { theme::warning_bold_style() } else { theme::text_style() }),
                    Span::styled(format!("{}. ", i + 1), theme::muted_style()),
                    Span::styled(&view.name, if is_selected { theme::warning_bold_style() } else { theme::header_style() }),
                    Span::styled(format!("  [{}]", view.saved_at), theme::muted_style()),
                ]);

                let q_preview = if view.query.len() > 60 {
                    format!("{}...", &view.query[..57])
                } else {
                    view.query.clone()
                };

                let detail_line = Line::from(vec![
                    Span::raw("    "),
                    Span::styled(view.summary(), theme::text_style()),
                    Span::styled(format!(" | SQL: {q_preview}"), theme::muted_style()),
                ]);

                let item_style = if is_selected {
                    theme::selection_style_for(true)
                } else {
                    theme::text_style()
                };

                ListItem::new(vec![title_line, detail_line, Line::from("")]).style(item_style)
            })
            .collect();

        let mut list_state = ListState::default().with_selected(Some(modal.selected));
        let list = List::new(items).highlight_style(theme::selection_style_for(true));
        frame.render_stateful_widget(list, list_area, &mut list_state);

        if modal.views.len() > 1 {
            let mut scroll_state =
                ScrollbarState::new(modal.views.len().saturating_sub(1)).position(modal.selected);
            frame.render_stateful_widget(
                Scrollbar::default()
                    .orientation(ScrollbarOrientation::VerticalRight)
                    .begin_symbol(Some("▲"))
                    .end_symbol(Some("▼")),
                list_area,
                &mut scroll_state,
            );
        }
    }

    let hint = Line::from(vec![
        Span::styled("[Enter]", theme::warning_bold_style()),
        Span::styled(" Restore  ", theme::text_style()),
        Span::styled("[d/Del]", theme::warning_bold_style()),
        Span::styled(" Delete  ", theme::text_style()),
        Span::styled("[↑/↓/j/k]", theme::warning_bold_style()),
        Span::styled(" Navigate  ", theme::text_style()),
        Span::styled("[Esc/q]", theme::warning_bold_style()),
        Span::styled(" Close", theme::text_style()),
    ]);
    frame.render_widget(Paragraph::new(hint), hint_area);

    // If pending deletion, render Delete Confirmation modal on top (ModalPreset::Small)
    if let Some(ref del_id) = modal.pending_delete {
        render_delete_confirmation(frame, area, modal, del_id);
    }
}

fn render_delete_confirmation(
    frame: &mut Frame,
    area: Rect,
    modal: &SavedViewsModalState,
    delete_id: &str,
) {
    let confirm_area = modal::modal_small(area);
    let inner = modal::render_modal_frame(frame, confirm_area, " Delete Saved View ", true);

    let view_name = modal
        .views
        .iter()
        .find(|v| v.id == delete_id)
        .map(|v| v.name.as_str())
        .unwrap_or("view");

    let prompt = vec![
        Line::from(""),
        Line::from(vec![
            Span::styled("Delete saved view ", theme::text_style()),
            Span::styled(format!("\"{view_name}\""), theme::warning_bold_style()),
            Span::styled("? (y/n)", theme::text_style()),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("  [ y / Enter ] Confirm    ", theme::success_style()),
            Span::styled("  [ n / Esc ] Cancel", theme::muted_style()),
        ]),
    ];

    frame.render_widget(Paragraph::new(prompt), inner);
}
