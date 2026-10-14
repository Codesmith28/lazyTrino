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

use crate::app::{FilterModalState, FilterMode, NumericOp};
use crate::tui::screens::modal;
use crate::tui::theme;

pub fn render(frame: &mut Frame, area: Rect, modal: &FilterModalState) {
    let popup_area = modal::modal_medium(area);

    let title = match modal.mode {
        FilterMode::Numeric => format!(" Filter Column: {} (Numeric Filter) ", modal.column_name),
        FilterMode::Values => format!(
            " Filter Column: {} ({} of {} selected) ",
            modal.column_name,
            modal.selected_values.len(),
            modal.values.len()
        ),
    };

    let inner = modal::render_modal_frame(frame, popup_area, &title, true);

    if inner.height < 4 {
        return;
    }

    match modal.mode {
        FilterMode::Numeric => render_numeric_mode(frame, inner, modal),
        FilterMode::Values => render_values_mode(frame, inner, modal),
    }
}

fn render_numeric_mode(frame: &mut Frame, area: Rect, modal: &FilterModalState) {
    let chunks = Layout::vertical([
        Constraint::Length(1), // Tab row
        Constraint::Length(1), // Spacer
        Constraint::Length(2), // Operator selector bar
        Constraint::Length(3), // Value input row
        Constraint::Length(1), // SQL preview
        Constraint::Length(1), // Matching statistics
        Constraint::Min(0),    // Space
        Constraint::Length(1), // Hint area
    ])
    .split(area);

    let tab_area = chunks[0];
    let op_area = chunks[2];
    let val_area = chunks[3];
    let preview_area = chunks[4];
    let stats_area = chunks[5];
    let hint_area = chunks[7];

    // Tab row
    let tab_line = Line::from(vec![
        Span::styled(" [ 1: Distinct Values ] ", theme::muted_style()),
        Span::styled(
            " [● 2: Numeric Comparator (<, <=, >, >=, =)] ",
            theme::warning_bold_style(),
        ),
        Span::styled("  (Tab to switch)", theme::muted_style()),
    ]);
    frame.render_widget(Paragraph::new(tab_line), tab_area);

    // Operator row
    let mut op_spans = vec![Span::styled(" Operator:  ", theme::bold_text_style())];
    for (idx, op) in NumericOp::ALL.iter().enumerate() {
        if idx > 0 {
            op_spans.push(Span::raw("  "));
        }
        let is_selected = *op == modal.numeric_op;
        if is_selected {
            op_spans.push(Span::styled(
                format!("▶[ {} ]◀", op.as_str()),
                theme::selection_style(),
            ));
        } else {
            op_spans.push(Span::styled(
                format!("[ {} ]", op.as_str()),
                theme::muted_style(),
            ));
        }
    }
    let op_line1 = Line::from(op_spans);
    let op_line2 = Line::from(vec![
        Span::styled(" Active:    ", theme::muted_style()),
        Span::styled(modal.numeric_op.label(), theme::warning_bold_style()),
        Span::styled("  (←/→ or type operator to change)", theme::muted_style()),
    ]);
    frame.render_widget(Paragraph::new(vec![op_line1, op_line2]), op_area);

    // Value input row
    let val_block = Block::default()
        .title(" Filter Value / Condition ")
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(theme::border_style(true));
    let val_inner = val_block.inner(val_area);
    frame.render_widget(val_block, val_area);

    let val_line = if modal.numeric_input.is_empty() {
        Line::from(vec![
            Span::styled(
                format!(" [ {} ] ", modal.numeric_op.as_str()),
                theme::warning_bold_style(),
            ),
            Span::styled(
                "Type number (e.g. 100, 25.5, -5) or condition (e.g. >= 50)... ",
                theme::muted_style(),
            ),
            Span::styled("█", theme::warning_bold_style()),
        ])
    } else {
        let has_typed_op = NumericOp::from_str_prefix(&modal.numeric_input).is_some();
        let mut spans = Vec::new();
        if !has_typed_op {
            spans.push(Span::styled(
                format!(" [ {} ] ", modal.numeric_op.as_str()),
                theme::warning_bold_style(),
            ));
        }
        spans.push(Span::styled(&modal.numeric_input, theme::bold_text_style()));
        spans.push(Span::styled("█", theme::warning_bold_style()));
        Line::from(spans)
    };
    frame.render_widget(Paragraph::new(val_line), val_inner);

    // SQL Preview row
    let preview_line = if let Some(ref err) = modal.numeric_error {
        Line::from(vec![
            Span::styled(" ⚠ ", theme::error_bold_style()),
            Span::styled(err, theme::error_style()),
        ])
    } else {
        match modal.parse_numeric_filter() {
            Ok(Some((op, _val, raw))) => Line::from(vec![
                Span::styled(" SQL Filter: ", theme::muted_style()),
                Span::styled(
                    format!("\"{}\" {} {}", modal.column_name, op.as_str(), raw),
                    theme::warning_bold_style(),
                ),
            ]),
            Ok(None) => Line::from(vec![
                Span::styled(" SQL Filter: ", theme::muted_style()),
                Span::styled(
                    "(no filter — shows all rows; press Enter to clear)",
                    theme::muted_style(),
                ),
            ]),
            Err(err) => Line::from(vec![
                Span::styled(" ⚠ ", theme::error_bold_style()),
                Span::styled(err, theme::error_style()),
            ]),
        }
    };
    frame.render_widget(Paragraph::new(preview_line), preview_area);

    // Matching stats row
    if let Some((matching, total)) = modal.numeric_matching_count() {
        let pct = if total > 0 {
            (matching as f64 / total as f64) * 100.0
        } else {
            0.0
        };
        let stats_line = if matching > 0 {
            Line::from(vec![
                Span::styled(" ✔ Matches: ", theme::success_bold_style()),
                Span::styled(
                    format!("{matching} of {total} distinct values in loaded rows ({pct:.1}%)"),
                    theme::success_style(),
                ),
            ])
        } else {
            Line::from(vec![
                Span::styled(" ⚠ Matches: ", theme::warning_bold_style()),
                Span::styled(
                    format!("0 of {total} distinct values in loaded rows (no matching rows)"),
                    theme::warning_style(),
                ),
            ])
        };
        frame.render_widget(Paragraph::new(stats_line), stats_area);
    }

    // Hint area
    let hint_line = Line::from(vec![
        Span::styled(" [Enter] ", theme::bold_text_style()),
        Span::styled("Apply  ", theme::muted_style()),
        Span::styled(" [Tab] ", theme::bold_text_style()),
        Span::styled("Values Tab  ", theme::muted_style()),
        Span::styled(" [←/→] ", theme::bold_text_style()),
        Span::styled("Operator  ", theme::muted_style()),
        Span::styled(" [c] ", theme::bold_text_style()),
        Span::styled("Clear  ", theme::muted_style()),
        Span::styled(" [Esc] ", theme::bold_text_style()),
        Span::styled("Cancel", theme::muted_style()),
    ]);
    frame.render_widget(Paragraph::new(hint_line), hint_area);
}

fn render_values_mode(frame: &mut Frame, area: Rect, modal: &FilterModalState) {
    let (tab_area, search_area, list_area, hint_area) = if modal.is_numeric_col {
        let chunks = Layout::vertical([
            Constraint::Length(1), // Tab header
            Constraint::Length(3), // Dedicated search box
            Constraint::Min(0),    // Values list
            Constraint::Length(1), // Hint area
        ])
        .split(area);
        (Some(chunks[0]), chunks[1], chunks[2], chunks[3])
    } else {
        let chunks = Layout::vertical([
            Constraint::Length(3), // Dedicated search box
            Constraint::Min(0),    // Values list
            Constraint::Length(1), // Hint area
        ])
        .split(area);
        (None, chunks[0], chunks[1], chunks[2])
    };

    if let Some(t_area) = tab_area {
        let tab_line = Line::from(vec![
            Span::styled(" [● 1: Distinct Values] ", theme::warning_bold_style()),
            Span::styled(
                " [ 2: Numeric Comparator (<, <=, >, >=, =) ] ",
                theme::muted_style(),
            ),
            Span::styled("  (Tab to switch)", theme::muted_style()),
        ]);
        frame.render_widget(Paragraph::new(tab_line), t_area);
    }

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
                Span::styled(
                    if modal.is_numeric_col {
                        "Type / to search values or filter (<, <=, >, >=, =)..."
                    } else {
                        "Type / to search values..."
                    },
                    theme::muted_style(),
                ),
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
        if modal.is_numeric_col
            && let Some((op, num_str)) = NumericOp::from_str_prefix(&modal.search_query)
            && num_str.parse::<f64>().is_ok()
        {
            spans.push(Span::styled(
                format!("  [Filter: {} {} - Enter to apply]", op.as_str(), num_str),
                theme::warning_bold_style(),
            ));
        }
        Line::from(spans)
    };
    frame.render_widget(Paragraph::new(search_line), s_inner);

    let matched_indices = modal.filtered_indices();
    let visible_count = list_area.height as usize;
    let mut lines = Vec::new();

    if matched_indices.is_empty() {
        lines.push(Line::from(Span::styled(
            "No matching values found",
            theme::muted_style(),
        )));
    } else {
        for (v_idx, &orig_idx) in matched_indices
            .iter()
            .enumerate()
            .skip(modal.scroll)
            .take(visible_count)
        {
            let val = &modal.values[orig_idx];
            let is_checked = modal.selected_values.contains(val);
            let check_mark = if is_checked { "[x] " } else { "[ ] " };
            let is_cursor = v_idx == modal.cursor;

            let display_val = if val.is_empty() {
                "<empty>"
            } else {
                val.as_str()
            };

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

            let text_span = Span::styled(
                display_val,
                if is_cursor {
                    theme::selection_style()
                } else {
                    theme::text_style()
                },
            );

            let line = Line::from(vec![prefix_span, text_span]);
            lines.push(line);
        }
    }

    let paragraph = Paragraph::new(lines);
    frame.render_widget(paragraph, list_area);

    if matched_indices.len() > visible_count && visible_count > 0 {
        let mut scrollbar_state =
            ScrollbarState::new(matched_indices.len().saturating_sub(visible_count))
                .position(modal.scroll);
        frame.render_stateful_widget(
            Scrollbar::default()
                .orientation(ScrollbarOrientation::VerticalRight)
                .begin_symbol(Some("▲"))
                .end_symbol(Some("▼")),
            list_area,
            &mut scrollbar_state,
        );
    }

    let hint = if modal.is_searching {
        Line::from(vec![
            Span::styled(" [Typing] ", theme::warning_bold_style()),
            Span::styled(
                "Enter:done/apply • Esc:clear search • Bksp:del",
                theme::muted_style(),
            ),
        ])
    } else if modal.is_numeric_col {
        Line::from(vec![
            Span::styled(" [Space] ", theme::bold_text_style()),
            Span::styled("Toggle  ", theme::muted_style()),
            Span::styled(" [a] ", theme::bold_text_style()),
            Span::styled("All/None  ", theme::muted_style()),
            Span::styled(" [Tab] ", theme::bold_text_style()),
            Span::styled("Numeric Tab  ", theme::muted_style()),
            Span::styled(" [/] ", theme::bold_text_style()),
            Span::styled("Search  ", theme::muted_style()),
            Span::styled(" [Enter] ", theme::bold_text_style()),
            Span::styled("Apply  ", theme::muted_style()),
            Span::styled(" [Esc] ", theme::bold_text_style()),
            Span::styled("Cancel", theme::muted_style()),
        ])
    } else {
        Line::from(vec![
            Span::styled(" [Space] ", theme::bold_text_style()),
            Span::styled("Toggle  ", theme::muted_style()),
            Span::styled(" [a] ", theme::bold_text_style()),
            Span::styled("All/None  ", theme::muted_style()),
            Span::styled(" [/] ", theme::bold_text_style()),
            Span::styled("Search  ", theme::muted_style()),
            Span::styled(" [Enter] ", theme::bold_text_style()),
            Span::styled("Apply  ", theme::muted_style()),
            Span::styled(" [Esc] ", theme::bold_text_style()),
            Span::styled("Cancel", theme::muted_style()),
        ])
    };
    frame.render_widget(Paragraph::new(hint), hint_area);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::NumericFocus;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use std::collections::HashSet;

    #[test]
    fn test_render_numeric_mode() {
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        let modal = FilterModalState {
            column_index: 0,
            column_name: "amount".to_string(),
            is_numeric_col: true,
            mode: FilterMode::Numeric,
            values: vec!["10".to_string(), "20".to_string(), "50".to_string()],
            selected_values: HashSet::new(),
            cursor: 0,
            scroll: 0,
            search_query: String::new(),
            is_searching: false,
            numeric_op: NumericOp::Lt,
            numeric_input: "25".to_string(),
            numeric_focus: NumericFocus::Value,
            numeric_error: None,
            last_op_char: None,
        };
        terminal.draw(|f| render(f, f.area(), &modal)).unwrap();
    }

    #[test]
    fn test_render_values_mode_numeric_col() {
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        let modal = FilterModalState {
            column_index: 0,
            column_name: "amount".to_string(),
            is_numeric_col: true,
            mode: FilterMode::Values,
            values: vec!["10".to_string(), "20".to_string(), "50".to_string()],
            selected_values: ["10".to_string()].into_iter().collect(),
            cursor: 0,
            scroll: 0,
            search_query: "< 25".to_string(),
            is_searching: false,
            numeric_op: NumericOp::Lt,
            numeric_input: String::new(),
            numeric_focus: NumericFocus::Value,
            numeric_error: None,
            last_op_char: None,
        };
        terminal.draw(|f| render(f, f.area(), &modal)).unwrap();
    }
}
