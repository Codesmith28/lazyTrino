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
    widgets::{
        List, ListItem, ListState, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState,
    },
};

use crate::app::{SaveViewPromptState, SavedViewsModalState};
use crate::tui::screens::modal;
use crate::tui::theme;

pub fn render(frame: &mut Frame, area: Rect, modal: &SavedViewsModalState) {
    let popup_area = modal::modal_large(area);
    let count_info = if modal.selected_ids.is_empty() {
        format!("{} saved", modal.views.len())
    } else {
        format!(
            "{} saved, {} selected",
            modal.views.len(),
            modal.selected_ids.len()
        )
    };
    let title = format!(" Saved Views ({count_info}) ");
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
                let is_checked = modal.selected_ids.contains(&view.id);
                let prefix = if is_selected { "▸ " } else { "  " };
                let checkbox = if is_checked { "[x] " } else { "[ ] " };
                let check_style = if is_checked {
                    theme::success_style()
                } else {
                    theme::muted_style()
                };

                let title_line = Line::from(vec![
                    Span::styled(
                        prefix,
                        if is_selected {
                            theme::warning_bold_style()
                        } else {
                            theme::text_style()
                        },
                    ),
                    Span::styled(checkbox, check_style),
                    Span::styled(format!("{}. ", i + 1), theme::muted_style()),
                    Span::styled(
                        &view.name,
                        if is_selected {
                            theme::warning_bold_style()
                        } else {
                            theme::header_style()
                        },
                    ),
                    Span::styled(format!("  [{}]", view.saved_at), theme::muted_style()),
                ]);

                let q_preview = if view.query.len() > 60 {
                    format!("{}...", &view.query[..57])
                } else {
                    view.query.clone()
                };

                let detail_line = Line::from(vec![
                    Span::raw("      "),
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
        Span::styled("[Space]", theme::warning_bold_style()),
        Span::styled(" Toggle  ", theme::text_style()),
        Span::styled("[a]", theme::warning_bold_style()),
        Span::styled(" All/None  ", theme::text_style()),
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
    if !modal.pending_delete_ids.is_empty() {
        render_delete_confirmation(frame, area, modal, &modal.pending_delete_ids);
    }
}

fn render_delete_confirmation(
    frame: &mut Frame,
    area: Rect,
    modal: &SavedViewsModalState,
    delete_ids: &[String],
) {
    let confirm_area = modal::modal_small(area);
    let inner = modal::render_modal_frame(frame, confirm_area, " Delete Saved View ", true);

    let prompt = if delete_ids.len() == 1 {
        let view_name = modal
            .views
            .iter()
            .find(|v| v.id == delete_ids[0])
            .map(|v| v.name.as_str())
            .unwrap_or("view");
        vec![
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
        ]
    } else {
        let count = delete_ids.len();
        vec![
            Line::from(""),
            Line::from(vec![Span::styled(
                format!("Delete {count} selected saved views? (y/n)"),
                theme::warning_bold_style(),
            )]),
            Line::from(""),
            Line::from(vec![
                Span::styled("  [ y / Enter ] Confirm    ", theme::success_style()),
                Span::styled("  [ n / Esc ] Cancel", theme::muted_style()),
            ]),
        ]
    };

    frame.render_widget(Paragraph::new(prompt), inner);
}

pub fn render_save_prompt(frame: &mut Frame, area: Rect, prompt: &SaveViewPromptState) {
    let popup_area = modal::modal_small(area);
    let inner = modal::render_modal_frame(frame, popup_area, " Save Table View ", true);

    if inner.height < 4 {
        return;
    }

    let chunks = Layout::vertical([
        Constraint::Length(1), // Prompt label
        Constraint::Length(1), // Text input field
        Constraint::Min(0),    // Spacing
        Constraint::Length(1), // Footer buttons
    ])
    .split(inner);

    let label_line = Line::from(Span::styled(" Enter view name:", theme::header_style()));
    frame.render_widget(Paragraph::new(label_line), chunks[0]);

    let input_line = Line::from(vec![
        Span::styled(" ▸ ", theme::warning_bold_style()),
        Span::styled(&prompt.name_input, theme::text_style()),
        Span::styled("█", theme::warning_bold_style()),
    ]);
    frame.render_widget(Paragraph::new(input_line), chunks[1]);

    let footer = Line::from(vec![
        Span::styled("  [ Enter ]", theme::success_style()),
        Span::styled(" Save    ", theme::text_style()),
        Span::styled("[ Esc ]", theme::muted_style()),
        Span::styled(" Cancel", theme::text_style()),
    ]);
    frame.render_widget(Paragraph::new(footer), chunks[3]);
}
