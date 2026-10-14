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
        Block, BorderType, Borders, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState,
    },
};

use crate::app::ColumnModalState;
use crate::tui::screens::modal;
use crate::tui::theme;

pub fn render(
    frame: &mut Frame,
    area: Rect,
    modal: &ColumnModalState,
    has_value_filter: impl Fn(&str) -> bool,
) {
    let popup_area = modal::modal_medium(area);
    let num_selected = modal.selected_columns.len();
    let num_total = modal.columns.len();
    let title = format!(
        " Visible Columns ({} of {} visible) ",
        num_selected, num_total
    );

    let inner = modal::render_modal_frame(frame, popup_area, &title, true);

    if inner.height < 4 {
        return;
    }

    let chunks = Layout::vertical([
        Constraint::Length(3), // Dedicated search box
        Constraint::Min(0),    // Columns list
        Constraint::Length(1), // Hint area
    ])
    .split(inner);

    let search_area = chunks[0];
    let list_area = chunks[1];
    let hint_area = chunks[2];

    let search_title = if modal.is_searching {
        " Search [/] [Typing — Enter/Esc to finish] "
    } else {
        " Search [/] "
    };
    let search_block = Block::default()
        .title(search_title)
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(theme::border_style(modal.is_searching));
    let s_inner = search_block.inner(search_area);
    frame.render_widget(search_block, search_area);

    let search_line = if modal.search_query.is_empty() {
        if modal.is_searching {
            Line::from(vec![
                Span::styled("/ ", theme::warning_bold_style()),
                Span::styled("█", theme::warning_bold_style()),
            ])
        } else {
            Line::from(vec![
                Span::styled("/ ", theme::muted_style()),
                Span::styled("Type / to search columns...", theme::muted_style()),
            ])
        }
    } else {
        let mut spans = vec![
            Span::styled("/ ", theme::warning_bold_style()),
            Span::styled(&modal.search_query, theme::text_style()),
        ];
        if modal.is_searching {
            spans.push(Span::styled("█", theme::warning_bold_style()));
        }
        Line::from(spans)
    };
    frame.render_widget(Paragraph::new(search_line), s_inner);

    let matched_indices = modal.filtered_indices();
    let visible_count = list_area.height as usize;
    let mut lines = Vec::new();

    if matched_indices.is_empty() {
        lines.push(Line::from(Span::styled(
            "No matching columns found",
            theme::muted_style(),
        )));
    } else {
        for (v_idx, &orig_idx) in matched_indices
            .iter()
            .enumerate()
            .skip(modal.scroll)
            .take(visible_count)
        {
            let col = &modal.columns[orig_idx];
            let is_checked = modal.selected_columns.contains(col);
            let check_mark = if is_checked { "[x] " } else { "[ ] " };
            let is_cursor = v_idx == modal.cursor;
            let filtered = has_value_filter(col);

            let prefix_span = Span::styled(
                check_mark,
                if is_cursor {
                    theme::selection_style()
                } else if is_checked {
                    theme::success_bold_style()
                } else {
                    theme::muted_style()
                },
            );

            let name_span = Span::styled(
                col.as_str(),
                if is_cursor {
                    theme::selection_style()
                } else if is_checked {
                    theme::text_style()
                } else {
                    theme::muted_style()
                },
            );

            let mut line_spans = vec![prefix_span, name_span];
            if filtered {
                line_spans.push(Span::styled(
                    " [F]",
                    if is_cursor {
                        theme::selection_style()
                    } else {
                        theme::success_bold_style()
                    },
                ));
            }

            lines.push(Line::from(line_spans));
        }
    }

    let list_widget = Paragraph::new(lines);
    frame.render_widget(list_widget, list_area);

    if matched_indices.len() > visible_count && visible_count > 0 {
        let mut scrollbar_state =
            ScrollbarState::new(matched_indices.len().saturating_sub(visible_count))
                .position(modal.scroll);
        let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .begin_symbol(Some("▲"))
            .end_symbol(Some("▼"));
        frame.render_stateful_widget(scrollbar, list_area, &mut scrollbar_state);
    }

    let hint = if modal.is_searching {
        Line::from(Span::styled(
            "Type:filter • Enter:done • Esc:cancel • Bksp:del",
            theme::muted_style(),
        ))
    } else {
        Line::from(Span::styled(
            "Space:toggle • a:all/none • /:search • Enter:apply • Esc:cancel",
            theme::muted_style(),
        ))
    };
    frame.render_widget(Paragraph::new(hint), hint_area);
}
