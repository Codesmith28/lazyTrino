use ratatui::{
    Frame,
    layout::{Constraint, Rect},
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
        if state.is_fetching_next_page {
            format!(
                "{title_prefix}Table View — Infinite Scroll ({} rows loaded, col {}/{}) [Fetching page...] ",
                state.rows.len(),
                state.scroll_h + 1,
                state.columns.len().max(1)
            )
        } else if !state.has_more_rows {
            format!(
                "{title_prefix}Table View — Infinite Scroll (All {} rows loaded, col {}/{}) ",
                state.rows.len(),
                state.scroll_h + 1,
                state.columns.len().max(1)
            )
        } else {
            format!(
                "{title_prefix}Table View — Infinite Scroll ({} rows loaded, col {}/{}) ",
                state.rows.len(),
                state.scroll_h + 1,
                state.columns.len().max(1)
            )
        }
    } else {
        format!(
            "{title_prefix}Sample Mode ({} sample rows, col {}/{}) ",
            state.rows.len(),
            state.scroll_h + 1,
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

    let mut cumulative_width = 0;
    let mut visible_cols: Vec<(usize, &String)> = Vec::new();
    let mut col_widths: Vec<Constraint> = Vec::new();
    let mut col_width_sizes: Vec<usize> = Vec::new();

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

    for (col_idx, c) in state.columns.iter().enumerate().skip(state.scroll_h) {
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
        let raw_width = (c.len().max(max_data) as u16).max(8);
        let max_allowed = (available_width / 2).max(15) as u16;
        let needed_width = raw_width.min(max_allowed);

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
        .map(|(_, c)| Cell::from(c.as_str()).style(header_style))
        .collect();
    let header = Row::new(header_cells).bottom_margin(1);

    let mut current_row_y = inner.y + 2; // header line + bottom margin 1
    let mut visible_rows: Vec<Row> = Vec::new();

    for row in state
        .rows
        .iter()
        .skip(state.scroll_v)
        .take((inner.height as usize).saturating_sub(2))
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
                let style = if is_mouse_sel {
                    theme::selection_style()
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
            ScrollbarState::new(state.rows.len().saturating_sub(1)).position(state.scroll_v);
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
            ScrollbarState::new(state.columns.len().saturating_sub(1)).position(state.scroll_h);
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
}
