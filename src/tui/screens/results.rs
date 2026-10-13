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
    layout::{Constraint, Rect},
    style::Modifier,
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Cell, Paragraph, Row, Table},
};

use crate::{app::ResultsState, tui::theme};

pub fn wrap_text(text: &str, max_width: usize) -> Vec<String> {
    if max_width == 0 {
        return vec![text.to_string()];
    }
    let mut lines = Vec::new();
    for raw_line in text.lines() {
        if raw_line.is_empty() {
            lines.push(String::new());
            continue;
        }
        let words: Vec<&str> = raw_line.split_whitespace().collect();
        if words.is_empty() {
            lines.push(String::new());
            continue;
        }
        let mut current_line = String::new();
        for word in words {
            if word.len() > max_width {
                if !current_line.is_empty() {
                    lines.push(current_line);
                    current_line = String::new();
                }
                let mut start = 0;
                while start < word.len() {
                    let end = (start + max_width).min(word.len());
                    lines.push(word[start..end].to_string());
                    start = end;
                }
            } else if current_line.is_empty() {
                current_line.push_str(word);
            } else if current_line.len() + 1 + word.len() <= max_width {
                current_line.push(' ');
                current_line.push_str(word);
            } else {
                lines.push(current_line);
                current_line = word.to_string();
            }
        }
        if !current_line.is_empty() {
            lines.push(current_line);
        }
    }
    if lines.is_empty() {
        vec![text.to_string()]
    } else {
        lines
    }
}

pub fn is_stats_table(columns: &[String]) -> bool {
    columns.iter().any(|c| {
        c.eq_ignore_ascii_case("distinct_values_count")
            || c.eq_ignore_ascii_case("nulls_fraction")
            || (c.eq_ignore_ascii_case("column_name")
                && columns.iter().any(|k| k.eq_ignore_ascii_case("row_count")))
    })
}

pub fn is_stats_summary_row(columns: &[String], row: &[String]) -> bool {
    if !is_stats_table(columns) {
        return false;
    }
    row.first()
        .map(|s| {
            let trimmed = s.trim();
            trimmed.is_empty()
                || trimmed.eq_ignore_ascii_case("null")
                || trimmed.eq_ignore_ascii_case("none")
        })
        .unwrap_or(false)
}

pub fn compute_stats_aggregates(
    columns: &[String],
    rows: &[Vec<String>],
) -> (Option<String>, Option<f64>) {
    let mut total_row_count = None;
    let mut total_data_size = 0.0;
    let mut has_data_size = false;

    let data_size_idx = columns
        .iter()
        .position(|c| c.eq_ignore_ascii_case("data_size"));
    let row_count_idx = columns
        .iter()
        .position(|c| c.eq_ignore_ascii_case("row_count"));

    for row in rows {
        if is_stats_summary_row(columns, row) {
            if let Some(idx) = row_count_idx
                && let Some(val) = row.get(idx)
            {
                let trimmed = val.trim();
                if !trimmed.is_empty()
                    && !trimmed.eq_ignore_ascii_case("null")
                    && !trimmed.eq_ignore_ascii_case("none")
                {
                    total_row_count = Some(trimmed.to_string());
                }
            }
        } else if let Some(idx) = data_size_idx
            && let Some(val) = row.get(idx)
            && let Ok(bytes) = val.trim().parse::<f64>()
        {
            total_data_size += bytes;
            has_data_size = true;
        }
    }

    (
        total_row_count,
        if has_data_size {
            Some(total_data_size)
        } else {
            None
        },
    )
}

pub fn render(
    frame: &mut Frame,
    area: Rect,
    state: &ResultsState,
    _spinner: String,
    is_active: bool,
    app: &crate::app::App,
) {
    let title_prefix = " Query Results : ";
    let main_title = if state.invalid_query_error.is_some() {
        format!("{title_prefix}Invalid Query ")
    } else if state.is_paginated {
        let total_str = if let Some(tot) = state.total_count {
            format!(" of {} total", crate::app::format_count(tot))
        } else if state.total_count_loading {
            " (counting total...)".to_string()
        } else {
            String::new()
        };
        let partition_str = if !state.filters.is_empty() {
            let breadcrumb = state
                .filters
                .iter()
                .map(|(c, v)| format!("{c}={v}"))
                .collect::<Vec<_>>()
                .join(" › ");
            format!(" [{breadcrumb}]")
        } else {
            String::new()
        };
        if state.is_fetching_next_page {
            format!(
                "{title_prefix}Table View — Infinite Scroll{partition_str} ({} rows loaded{total_str}, col {}/{}) [Fetching page...] ",
                state.rows.len(),
                state.selected_col + 1,
                state.columns.len().max(1)
            )
        } else if !state.has_more_rows {
            format!(
                "{title_prefix}Table View — Infinite Scroll{partition_str} (All {} rows loaded{total_str}, col {}/{}) ",
                state.rows.len(),
                state.selected_col + 1,
                state.columns.len().max(1)
            )
        } else {
            format!(
                "{title_prefix}Table View — Infinite Scroll{partition_str} ({} rows loaded{total_str}, col {}/{}) ",
                state.rows.len(),
                state.selected_col + 1,
                state.columns.len().max(1)
            )
        }
    } else {
        format!(
            "{title_prefix}Sample Mode ({} sample rows, col {}/{}) ",
            state.rows.len(),
            state.selected_col + 1,
            state.columns.len().max(1)
        )
    };

    let block = Block::default()
        .title(main_title)
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(theme::border_style(is_active));

    let inner = block.inner(area);
    frame.render_widget(&block, area);

    if state.loading {
        return;
    }

    if let Some(invalid_msg) = &state.invalid_query_error {
        let err_lines = vec![
            Line::from(Span::styled(
                "✖ Invalid query for this view",
                theme::error_bold_style(),
            )),
            Line::from(""),
            Line::from(Span::styled(invalid_msg.as_str(), theme::error_style())),
            Line::from(""),
            Line::from(Span::styled(
                format!(
                    "Current View Scope: {}.{}.{}",
                    state.catalog, state.schema, state.table
                ),
                theme::info_style(),
            )),
            Line::from(""),
            Line::from(Span::styled(
                "Note: Queries in this view can only target data inside the current table.",
                theme::muted_style(),
            )),
            Line::from(Span::styled(
                "Enter full SQL queries operating on this table (e.g. 'SELECT * FROM table WHERE col = val').",
                theme::muted_style(),
            )),
        ];
        let p = Paragraph::new(err_lines).wrap(ratatui::widgets::Wrap { trim: false });
        frame.render_widget(p, inner);
        return;
    }

    if let Some(err) = &state.error {
        let err = Paragraph::new(err.as_str())
            .style(theme::error_style())
            .wrap(ratatui::widgets::Wrap { trim: false });
        frame.render_widget(err, inner);
        return;
    }

    if state.columns.is_empty() && state.rows.is_empty() {
        let empty = Paragraph::new("No results returned")
            .style(theme::secondary_style())
            .alignment(ratatui::layout::Alignment::Center)
            .wrap(ratatui::widgets::Wrap { trim: false });
        frame.render_widget(empty, inner);
        return;
    }

    if state.columns.is_empty() && !state.rows.is_empty() && state.rows[0].len() == 1 {
        let err = Paragraph::new(state.rows[0][0].as_str())
            .style(theme::error_style())
            .wrap(ratatui::widgets::Wrap { trim: false });
        frame.render_widget(err, inner);
        return;
    }

    let cell_style = theme::text_style();
    let header_style = theme::warning_bold_style();

    let available_width = inner.width.saturating_sub(4) as usize;

    let is_stats = is_stats_table(&state.columns);
    let (stats_total_row_count, stats_total_data_size) = if is_stats {
        compute_stats_aggregates(&state.columns, &state.rows)
    } else {
        (None, None)
    };

    let data_size_col_idx = state
        .columns
        .iter()
        .position(|c| c.eq_ignore_ascii_case("data_size"));
    let row_count_col_idx = state
        .columns
        .iter()
        .position(|c| c.eq_ignore_ascii_case("row_count"));

    let col_count = state.columns.len();
    let sel_col = state.selected_col.min(col_count.saturating_sub(1));

    let mut effective_scroll_h = state.scroll_h.min(col_count.saturating_sub(1));
    if sel_col < effective_scroll_h {
        effective_scroll_h = sel_col;
    }

    let compute_col_width = |col_idx: usize, col_name: &String| -> u16 {
        let max_data = state
            .rows
            .iter()
            .map(|r| {
                if is_stats && col_idx == 0 && is_stats_summary_row(&state.columns, r) {
                    "Summary".len()
                } else {
                    r.get(col_idx).map(|v| v.len()).unwrap_or(0)
                }
            })
            .max()
            .unwrap_or(0);
        let header_len = if state.column_filters.contains_key(col_name) {
            col_name.len() + 4
        } else {
            col_name.len()
        };
        let raw_width = (header_len.max(max_data) as u16).max(8);
        let max_allowed = (available_width / 2).max(15) as u16;
        raw_width.min(max_allowed)
    };

    while effective_scroll_h < sel_col {
        let mut width = 0;
        let mut fits = false;
        for (col_idx, c) in state.columns.iter().enumerate().skip(effective_scroll_h) {
            let needed = compute_col_width(col_idx, c) as usize + 2;
            if width + needed <= available_width || width == 0 {
                width += needed;
                if col_idx == sel_col {
                    fits = true;
                    break;
                }
            } else {
                break;
            }
        }
        if fits {
            break;
        }
        effective_scroll_h += 1;
    }

    let mut cumulative_width = 0;
    let mut visible_cols: Vec<(usize, &String)> = Vec::new();
    let mut col_widths: Vec<Constraint> = Vec::new();
    let mut col_width_sizes: Vec<usize> = Vec::new();

    for (col_idx, c) in state.columns.iter().enumerate().skip(effective_scroll_h) {
        let needed_width = compute_col_width(col_idx, c);

        if visible_cols.is_empty()
            || cumulative_width + (needed_width as usize) + 2 <= available_width
        {
            cumulative_width += (needed_width as usize) + 2;
            visible_cols.push((col_idx, c));
            col_widths.push(Constraint::Length(needed_width));
            col_width_sizes.push(needed_width as usize);
        } else {
            break;
        }
    }

    if col_widths.is_empty() {
        col_widths.push(Constraint::Length(15));
        col_width_sizes.push(15);
    }

    let header_cells: Vec<Cell> = visible_cols
        .iter()
        .map(|(_, c)| {
            let is_selected_col = state
                .columns
                .iter()
                .position(|name| name == *c)
                .is_some_and(|idx| idx == state.selected_col);
            let has_filter = state.column_filters.contains_key(*c);
            let display_name = if has_filter {
                format!("{c} [F]")
            } else {
                c.to_string()
            };
            let style = if is_selected_col && is_active {
                theme::warning_bold_style().add_modifier(Modifier::UNDERLINED)
            } else if is_selected_col {
                theme::header_style().add_modifier(Modifier::UNDERLINED)
            } else if has_filter {
                theme::success_bold_style()
            } else {
                header_style
            };
            Cell::from(display_name).style(style)
        })
        .collect();
    let header = Row::new(header_cells).bottom_margin(1);

    let take_rows = (inner.height as usize).saturating_sub(2).max(1);
    let mut effective_scroll_v = state.scroll_v;
    if !state.rows.is_empty() {
        let sel_row = state.selected_row.min(state.rows.len() - 1);
        if sel_row < effective_scroll_v {
            effective_scroll_v = sel_row;
        } else if sel_row >= effective_scroll_v + take_rows {
            effective_scroll_v = sel_row.saturating_sub(take_rows - 1);
        }
    }

    let mut current_row_y = inner.y + 2; // header line + bottom margin 1
    let mut visible_rows: Vec<Row> = Vec::new();

    for (row_idx, row) in state
        .rows
        .iter()
        .enumerate()
        .skip(effective_scroll_v)
        .take(take_rows)
    {
        let is_summary = is_stats && is_stats_summary_row(&state.columns, row);

        if is_summary {
            // Render border separator row with "=======" across all visible columns
            let border_cells: Vec<Cell> = visible_cols
                .iter()
                .enumerate()
                .map(|(v_idx, _)| {
                    let col_w = col_width_sizes.get(v_idx).copied().unwrap_or(15);
                    Cell::from("=".repeat(col_w)).style(theme::border_style(false))
                })
                .collect();
            current_row_y += 1;
            visible_rows.push(Row::new(border_cells).height(1));
        }

        let mut max_row_height: u16 = 1;
        let row_y = current_row_y;
        let cells: Vec<Cell> = visible_cols
            .iter()
            .enumerate()
            .map(|(v_idx, (col_idx, _))| {
                let raw_val = row.get(*col_idx).map(|s| s.as_str()).unwrap_or("");
                let is_raw_empty = raw_val.is_empty()
                    || raw_val.eq_ignore_ascii_case("null")
                    || raw_val.eq_ignore_ascii_case("none");

                let formatted_val;
                let display_val: &str = if is_summary && *col_idx == 0 {
                    "Summary"
                } else if is_summary && Some(*col_idx) == data_size_col_idx && is_raw_empty {
                    if let Some(total_bytes) = stats_total_data_size {
                        formatted_val = format!("{total_bytes:.1}");
                        formatted_val.as_str()
                    } else {
                        "—"
                    }
                } else if !is_summary && Some(*col_idx) == row_count_col_idx && is_raw_empty {
                    if let Some(ref total_rows) = stats_total_row_count {
                        total_rows.as_str()
                    } else {
                        "—"
                    }
                } else if is_summary && is_raw_empty {
                    "—"
                } else {
                    raw_val
                };

                let col_w = col_width_sizes.get(v_idx).copied().unwrap_or(15);
                let wrapped_lines = wrap_text(display_val, col_w);
                max_row_height = max_row_height.max(wrapped_lines.len() as u16);
                // See actions.rs for why this must be gated by pane focus.
                let is_mouse_sel =
                    is_active && app.is_area_mouse_selected(inner.x, inner.width, row_y);
                let is_cell_selected =
                    row_idx == state.selected_row && *col_idx == state.selected_col;
                let style = if is_mouse_sel {
                    theme::selection_style()
                } else if is_cell_selected {
                    theme::selection_style_for(is_active)
                } else if is_summary {
                    if *col_idx == 0 {
                        theme::warning_bold_style()
                    } else if display_val != "—" {
                        theme::success_bold_style()
                    } else {
                        theme::muted_style()
                    }
                } else {
                    cell_style
                };
                Cell::from(wrapped_lines.join("\n")).style(style)
            })
            .collect();
        current_row_y += max_row_height;
        visible_rows.push(Row::new(cells).height(max_row_height));
    }

    let table = Table::new(visible_rows, col_widths)
        .header(header)
        .column_spacing(2);

    use ratatui::widgets::{Scrollbar, ScrollbarOrientation, ScrollbarState};

    frame.render_widget(table, inner);

    if !state.rows.is_empty() {
        let mut v_scroll_state =
            ScrollbarState::new(state.rows.len().saturating_sub(1)).position(effective_scroll_v);
        frame.render_stateful_widget(
            Scrollbar::default()
                .orientation(ScrollbarOrientation::VerticalRight)
                .begin_symbol(Some("▲"))
                .end_symbol(Some("▼")),
            inner,
            &mut v_scroll_state,
        );
    }

    if state.columns.len() > 1 {
        let mut h_scroll_state =
            ScrollbarState::new(state.columns.len().saturating_sub(1)).position(effective_scroll_h);
        frame.render_stateful_widget(
            Scrollbar::default()
                .orientation(ScrollbarOrientation::HorizontalBottom)
                .begin_symbol(Some("◄"))
                .end_symbol(Some("►")),
            inner,
            &mut h_scroll_state,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_stats_table_identifies_trino_show_stats_headers() {
        let stats_cols = vec![
            "column_name".to_string(),
            "data_size".to_string(),
            "distinct_values_count".to_string(),
            "nulls_fraction".to_string(),
            "row_count".to_string(),
            "low_value".to_string(),
            "high_value".to_string(),
        ];
        assert!(is_stats_table(&stats_cols));

        let generic_cols = vec![
            "orderkey".to_string(),
            "totalprice".to_string(),
            "orderdate".to_string(),
        ];
        assert!(!is_stats_table(&generic_cols));
    }

    #[test]
    fn test_is_stats_summary_row_identifies_null_summary_row() {
        let stats_cols = vec![
            "column_name".to_string(),
            "data_size".to_string(),
            "distinct_values_count".to_string(),
            "nulls_fraction".to_string(),
            "row_count".to_string(),
            "low_value".to_string(),
            "high_value".to_string(),
        ];

        let regular_row = vec![
            "orderkey".to_string(),
            "NULL".to_string(),
            "1498948.0".to_string(),
            "0.0".to_string(),
            "NULL".to_string(),
            "1".to_string(),
            "6000000".to_string(),
        ];
        assert!(!is_stats_summary_row(&stats_cols, &regular_row));

        let summary_row_null = vec![
            "NULL".to_string(),
            "NULL".to_string(),
            "NULL".to_string(),
            "NULL".to_string(),
            "6001215.0".to_string(),
            "NULL".to_string(),
            "NULL".to_string(),
        ];
        assert!(is_stats_summary_row(&stats_cols, &summary_row_null));

        let summary_row_empty = vec![
            "".to_string(),
            "".to_string(),
            "".to_string(),
            "".to_string(),
            "6001215.0".to_string(),
            "".to_string(),
            "".to_string(),
        ];
        assert!(is_stats_summary_row(&stats_cols, &summary_row_empty));
    }

    #[test]
    fn test_compute_stats_aggregates_sums_bytes_and_finds_row_count() {
        let stats_cols = vec![
            "column_name".to_string(),
            "data_size".to_string(),
            "distinct_values_count".to_string(),
            "nulls_fraction".to_string(),
            "row_count".to_string(),
            "low_value".to_string(),
            "high_value".to_string(),
        ];

        let rows = vec![
            vec![
                "orderkey".to_string(),
                "NULL".to_string(),
                "1498948.0".to_string(),
                "0.0".to_string(),
                "NULL".to_string(),
                "1".to_string(),
                "6000000".to_string(),
            ],
            vec![
                "returnflag".to_string(),
                "1361361.0".to_string(),
                "3.0".to_string(),
                "0.0".to_string(),
                "NULL".to_string(),
                "NULL".to_string(),
                "NULL".to_string(),
            ],
            vec![
                "comment".to_string(),
                "125281177.0".to_string(),
                "4533595.0".to_string(),
                "0.0".to_string(),
                "NULL".to_string(),
                "NULL".to_string(),
                "NULL".to_string(),
            ],
            vec![
                "NULL".to_string(),
                "NULL".to_string(),
                "NULL".to_string(),
                "NULL".to_string(),
                "6001215.0".to_string(),
                "NULL".to_string(),
                "NULL".to_string(),
            ],
        ];

        let (total_rows, total_data_size) = compute_stats_aggregates(&stats_cols, &rows);
        assert_eq!(total_rows.as_deref(), Some("6001215.0"));
        assert_eq!(total_data_size, Some(1361361.0 + 125281177.0));
    }

    #[test]
    fn test_render_paginated_with_total_count_and_filters() {
        use ratatui::{Terminal, backend::TestBackend, layout::Rect};

        let state = ResultsState {
            query_buffer: "SELECT * FROM orders".to_string(),
            query_cursor: 20,
            columns: vec!["id".to_string(), "status".to_string()],
            rows: vec![
                vec!["1".to_string(), "OK".to_string()],
                vec!["2".to_string(), "PENDING".to_string()],
            ],
            scroll_v: 0,
            scroll_h: 0,
            loading: false,
            error: None,
            is_paginated: true,
            catalog: "iceberg".to_string(),
            schema: "sales".to_string(),
            table: "orders".to_string(),
            offset: 0,
            page_size: 100,
            is_fetching_next_page: false,
            has_more_rows: false,
            invalid_query_error: None,
            selection_anchor: None,
            filters: vec![("date".to_string(), "2026-08-06".to_string())],
            total_count: Some(45120),
            total_count_loading: false,
            table_count: Some(1234567),
            ..Default::default()
        };

        let app = crate::app::App::new(crate::config::ConnectionConfig::default(), false);
        let backend = TestBackend::new(140, 20);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|f| {
                render(f, Rect::new(0, 0, 140, 20), &state, "".to_string(), true, &app);
            })
            .unwrap();

        let buffer = terminal.backend().buffer().clone();
        let mut content = String::new();
        for y in 0..buffer.area.height {
            for x in 0..buffer.area.width {
                content.push_str(buffer.cell((x, y)).unwrap().symbol());
            }
            content.push('\n');
        }

        assert!(
            content.contains("[date=2026-08-06]"),
            "Title should contain partition filter: {content}"
        );
        assert!(
            content.contains("of 45,120 total"),
            "Title should contain total rows: {content}"
        );
    }
}
