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

pub fn compute_column_width(
    state: &ResultsState,
    orig_col_idx: usize,
    col_name: &str,
    available_width: usize,
) -> usize {
    let is_stats = is_stats_table(&state.columns);
    let max_data = state
        .rows
        .iter()
        .map(|r| {
            if is_stats && orig_col_idx == 0 && is_stats_summary_row(&state.columns, r) {
                "Summary".len()
            } else {
                r.get(orig_col_idx).map(|v| v.len()).unwrap_or(0)
            }
        })
        .max()
        .unwrap_or(0);
    let mut header_len = col_name.len();
    if state.column_filters.contains_key(col_name) {
        header_len += 4;
    }
    if let Some((ref sort_col, _)) = state.sort
        && sort_col == col_name
    {
        header_len += 2;
    }
    let raw_width = (header_len.max(max_data) as u16).max(8);
    let max_allowed = (available_width / 2).max(15) as u16;
    raw_width.min(max_allowed) as usize
}

pub fn compute_max_scroll_h(state: &ResultsState, available_width: usize) -> usize {
    let all_vis = state.visible_columns();
    let col_count = all_vis.len();
    if col_count == 0 {
        return 0;
    }
    let mut max_scroll_h = col_count.saturating_sub(1);
    let mut width_from_right = 0;
    for v_idx in (0..col_count).rev() {
        let (orig_col_idx, c) = &all_vis[v_idx];
        let needed = compute_column_width(state, *orig_col_idx, c, available_width) + 2;
        if width_from_right + needed <= available_width || v_idx == col_count - 1 {
            width_from_right += needed;
            max_scroll_h = v_idx;
        } else {
            break;
        }
    }
    max_scroll_h
}

pub fn visible_col_range_indices(
    state: &ResultsState,
    available_width: usize,
    scroll_h: usize,
) -> (usize, usize) {
    let all_vis = state.visible_columns();
    if all_vis.is_empty() {
        return (0, 0);
    }
    let first_vis = scroll_h.min(all_vis.len().saturating_sub(1));
    let mut cumulative_width = 0;
    let mut last_vis = first_vis;
    for (v_idx, (orig_col_idx, c)) in all_vis.iter().enumerate().skip(first_vis) {
        let needed = compute_column_width(state, *orig_col_idx, c, available_width) + 2;
        if v_idx == first_vis || cumulative_width + needed <= available_width {
            cumulative_width += needed;
            last_vis = v_idx;
        } else {
            break;
        }
    }
    (first_vis, last_vis)
}

pub fn scroll_table_horizontal_right(res: &mut ResultsState, avail_width: usize) {
    let all_vis = res.visible_columns();
    if all_vis.is_empty() {
        return;
    }
    let max_scroll_h = compute_max_scroll_h(res, avail_width);
    let new_scroll_h = (res.scroll_h.get() + 1).min(max_scroll_h);
    res.scroll_h.set(new_scroll_h);

    let (first_vis_idx, _last_vis_idx) =
        visible_col_range_indices(res, avail_width, new_scroll_h);
    let sel_vis_idx = all_vis
        .iter()
        .position(|(orig_idx, _)| *orig_idx == res.selected_col)
        .unwrap_or(0);
    if sel_vis_idx < first_vis_idx {
        res.selected_col = all_vis[first_vis_idx].0;
        res.last_selected_col.set(res.selected_col);
    }
}

pub fn scroll_table_horizontal_left(res: &mut ResultsState, avail_width: usize) {
    let all_vis = res.visible_columns();
    if all_vis.is_empty() {
        return;
    }
    let new_scroll_h = res.scroll_h.get().saturating_sub(1);
    res.scroll_h.set(new_scroll_h);

    let (_first_vis_idx, last_vis_idx) =
        visible_col_range_indices(res, avail_width, new_scroll_h);
    let sel_vis_idx = all_vis
        .iter()
        .position(|(orig_idx, _)| *orig_idx == res.selected_col)
        .unwrap_or(0);
    if sel_vis_idx > last_vis_idx {
        res.selected_col = all_vis[last_vis_idx].0;
        res.last_selected_col.set(res.selected_col);
    }
}

pub fn scroll_table_horizontal_to(res: &mut ResultsState, avail_width: usize, target_scroll_h: usize) {
    let all_vis = res.visible_columns();
    if all_vis.is_empty() {
        return;
    }
    let max_scroll_h = compute_max_scroll_h(res, avail_width);
    let new_scroll_h = target_scroll_h.min(max_scroll_h);
    res.scroll_h.set(new_scroll_h);

    let (first_vis_idx, last_vis_idx) =
        visible_col_range_indices(res, avail_width, new_scroll_h);
    let sel_vis_idx = all_vis
        .iter()
        .position(|(orig_idx, _)| *orig_idx == res.selected_col)
        .unwrap_or(0);
    if sel_vis_idx < first_vis_idx {
        res.selected_col = all_vis[first_vis_idx].0;
        res.last_selected_col.set(res.selected_col);
    } else if sel_vis_idx > last_vis_idx {
        res.selected_col = all_vis[last_vis_idx].0;
        res.last_selected_col.set(res.selected_col);
    }
}

pub fn visible_columns_layout(
    state: &ResultsState,
    available_width: usize,
) -> (Vec<(usize, String)>, Vec<usize>, usize) {
    let all_visible_cols = state.visible_columns();
    let col_count = all_visible_cols.len();
    if col_count == 0 {
        return (Vec::new(), Vec::new(), 0);
    }
    let sel_vis_idx = all_visible_cols
        .iter()
        .position(|(orig_idx, _)| *orig_idx == state.selected_col)
        .unwrap_or(0);
    let last_sel_vis_idx = all_visible_cols
        .iter()
        .position(|(orig_idx, _)| *orig_idx == state.last_selected_col.get())
        .unwrap_or(sel_vis_idx);

    let h_margin = 2.min(col_count.saturating_sub(1) / 2);
    let mut effective_scroll_h = state.scroll_h.get().min(col_count.saturating_sub(1));

    if sel_vis_idx > last_sel_vis_idx {
        let target_vis_idx = (sel_vis_idx + h_margin).min(col_count.saturating_sub(1));
        while effective_scroll_h < sel_vis_idx {
            let mut width = 0;
            let mut fits = false;
            for (v_idx, (orig_col_idx, c)) in all_visible_cols.iter().enumerate().skip(effective_scroll_h) {
                let needed = compute_column_width(state, *orig_col_idx, c, available_width) + 2;
                if width + needed <= available_width || width == 0 {
                    width += needed;
                    if v_idx == target_vis_idx {
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
    } else if sel_vis_idx < last_sel_vis_idx
        && sel_vis_idx < effective_scroll_h + h_margin
    {
        effective_scroll_h = sel_vis_idx.saturating_sub(h_margin);
    }

    let max_scroll_h = compute_max_scroll_h(state, available_width);
    if effective_scroll_h > max_scroll_h {
        effective_scroll_h = max_scroll_h;
    }

    let mut cumulative_width = 0;
    let mut visible_cols: Vec<(usize, String)> = Vec::new();
    let mut col_width_sizes: Vec<usize> = Vec::new();

    for (_v_idx, (orig_col_idx, c)) in all_visible_cols.iter().enumerate().skip(effective_scroll_h) {
        let needed_width = compute_column_width(state, *orig_col_idx, c, available_width);

        if visible_cols.is_empty()
            || cumulative_width + needed_width + 2 <= available_width
        {
            cumulative_width += needed_width + 2;
            visible_cols.push((*orig_col_idx, (*c).clone()));
            col_width_sizes.push(needed_width);
        } else {
            break;
        }
    }

    if col_width_sizes.is_empty() {
        col_width_sizes.push(15);
    }

    (visible_cols, col_width_sizes, effective_scroll_h)
}

#[allow(clippy::too_many_arguments)]
pub fn compute_row_height(
    row: &[String],
    visible_cols: &[(usize, String)],
    col_width_sizes: &[usize],
    is_summary: bool,
    data_size_col_idx: Option<usize>,
    stats_total_data_size: Option<f64>,
    row_count_col_idx: Option<usize>,
    stats_total_row_count: Option<&String>,
) -> u16 {
    let mut max_row_height: u16 = 1;
    for (v_idx, (col_idx, _)) in visible_cols.iter().enumerate() {
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
            if let Some(total_rows) = stats_total_row_count {
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
    }
    let sep = if is_summary { 1 } else { 0 };
    max_row_height + sep
}

pub fn calculate_effective_scroll_v(
    current_scroll_v: usize,
    sel_row: usize,
    last_sel_row: usize,
    viewport_lines: usize,
    row_visual_heights: &[u16],
    total_rows: usize,
) -> usize {
    if total_rows == 0 {
        return 0;
    }
    let sel_row = sel_row.min(total_rows - 1);
    let current_scroll_v = current_scroll_v.min(total_rows - 1);

    // Count how many rows fit starting at current_scroll_v
    let mut used_lines: usize = 0;
    let mut visible_count: usize = 0;
    for &h in &row_visual_heights[current_scroll_v..] {
        if used_lines + h as usize <= viewport_lines || visible_count == 0 {
            used_lines += h as usize;
            visible_count += 1;
        } else {
            break;
        }
    }

    let v_margin = 4.min(visible_count.saturating_sub(1) / 2);
    let mut scroll_v = current_scroll_v;

    if sel_row > last_sel_row {
        // Cursor moved down: advance scroll_v if needed so sel_row has bottom margin
        let target_bottom = (sel_row + v_margin).min(total_rows - 1);
        while scroll_v < sel_row {
            let lines_needed: usize = row_visual_heights[scroll_v..=target_bottom]
                .iter()
                .map(|&h| h as usize)
                .sum();
            if lines_needed <= viewport_lines {
                break;
            }
            scroll_v += 1;
        }
    } else if sel_row < last_sel_row {
        // Cursor moved up: retreat scroll_v if needed so sel_row has top margin
        if sel_row < scroll_v + v_margin {
            scroll_v = sel_row.saturating_sub(v_margin);
        }
    } else if sel_row < scroll_v {
        // Cursor did not move (view scrolled by mouse or initial render):
        // only pull scroll_v up if sel_row is offscreen above the viewport
        scroll_v = sel_row;
    }

    // Ensure sel_row is always within the viewport:
    // If lines from scroll_v to sel_row exceed viewport_lines, advance scroll_v.
    while scroll_v < sel_row {
        let lines_to_sel: usize = row_visual_heights[scroll_v..=sel_row]
            .iter()
            .map(|&h| h as usize)
            .sum();
        if lines_to_sel <= viewport_lines {
            break;
        }
        scroll_v += 1;
    }

    // Compute max_scroll_v from bottom up so viewport is filled if scrolled to the end
    let mut max_scroll_v = total_rows.saturating_sub(1);
    let mut lines_from_bottom = 0;
    for r in (0..total_rows).rev() {
        let h = row_visual_heights[r] as usize;
        if lines_from_bottom + h <= viewport_lines || r == total_rows - 1 {
            lines_from_bottom += h;
            max_scroll_v = r;
        } else {
            break;
        }
    }

    if scroll_v > max_scroll_v && sel_row <= max_scroll_v {
        scroll_v = max_scroll_v;
    }

    scroll_v.min(total_rows.saturating_sub(1))
}

pub fn hit_test_cell(
    state: &ResultsState,
    inner: Rect,
    mouse_col: u16,
    mouse_row: u16,
) -> Option<(usize, usize)> {
    if state.rows.is_empty() || state.columns.is_empty() {
        return None;
    }
    if mouse_col < inner.x
        || mouse_col >= inner.x.saturating_add(inner.width)
        || mouse_row < inner.y
        || mouse_row >= inner.y.saturating_add(inner.height)
    {
        return None;
    }

    let available_width = inner.width.saturating_sub(4) as usize;
    let (visible_cols, col_width_sizes, _) =
        visible_columns_layout(state, available_width);
    if visible_cols.is_empty() {
        return None;
    }

    // 1. Column hit test
    let mut cur_x = inner.x;
    let mut clicked_col = visible_cols[0].0;
    for (v_idx, (orig_col_idx, _)) in visible_cols.iter().enumerate() {
        let w = col_width_sizes.get(v_idx).copied().unwrap_or(15) as u16;
        let next_x = cur_x + w + 2;
        if mouse_col >= cur_x && mouse_col < next_x {
            clicked_col = *orig_col_idx;
            break;
        }
        if mouse_col >= cur_x {
            clicked_col = *orig_col_idx;
        }
        cur_x = next_x;
    }

    // 2. Row hit test
    if mouse_row <= inner.y + 1 {
        // Click on header
        return Some((state.selected_row.min(state.rows.len() - 1), clicked_col));
    }

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

    let row_visual_heights: Vec<u16> = state
        .rows
        .iter()
        .map(|row| {
            let is_summary = is_stats && is_stats_summary_row(&state.columns, row);
            compute_row_height(
                row,
                &visible_cols,
                &col_width_sizes,
                is_summary,
                data_size_col_idx,
                stats_total_data_size,
                row_count_col_idx,
                stats_total_row_count.as_ref(),
            )
        })
        .collect();

    let has_h_scrollbar = state.visible_columns().len() > 1;
    if has_h_scrollbar && mouse_row >= inner.y + inner.height.saturating_sub(1) {
        return None;
    }
    let table_reserved_rows = if has_h_scrollbar { 3 } else { 2 };
    let viewport_lines = (inner.height as usize).saturating_sub(table_reserved_rows).max(1);

    let effective_scroll_v = calculate_effective_scroll_v(
        state.scroll_v.get(),
        state.selected_row,
        state.last_selected_row.get(),
        viewport_lines,
        &row_visual_heights,
        state.rows.len(),
    );

    let mut cur_y = inner.y + 2;
    let mut clicked_row = None;
    for (row_idx, _) in state.rows.iter().enumerate().skip(effective_scroll_v) {
        let h = row_visual_heights.get(row_idx).copied().unwrap_or(1);
        if mouse_row >= cur_y && mouse_row < cur_y + h {
            clicked_row = Some(row_idx);
            break;
        }
        cur_y += h;
        if cur_y >= inner.y + inner.height {
            break;
        }
    }

    clicked_row.map(|r| (r, clicked_col))
}

pub fn render(
    frame: &mut Frame,
    area: Rect,
    state: &ResultsState,
    _spinner: String,
    is_active: bool,
    app: &crate::app::App,
) {
    let title_prefix = if !state.table.is_empty() {
        format!(
            " Query Results — {} › {} › {} : ",
            state.catalog, state.schema, state.table
        )
    } else {
        " Query Results : ".to_string()
    };
    let sort_str = match &state.sort {
        Some((col, crate::app::SortDirection::Asc)) => format!(", sorted by {col} asc"),
        Some((col, crate::app::SortDirection::Desc)) => format!(", sorted by {col} desc"),
        None => String::new(),
    };
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
        let all_visible_cols = state.visible_columns();
        let vis_pos = all_visible_cols
            .iter()
            .position(|(idx, _)| *idx == state.selected_col)
            .map(|p| p + 1)
            .unwrap_or(1);
        let vis_tot = all_visible_cols.len().max(1);
        let hidden_count = state.columns.len().saturating_sub(all_visible_cols.len());
        let col_str = if hidden_count > 0 {
            format!("{vis_pos}/{vis_tot} ({hidden_count} hidden)")
        } else {
            format!("{vis_pos}/{vis_tot}")
        };

        if state.is_fetching_next_page {
            format!(
                "{title_prefix}Table View — Infinite Scroll{partition_str} ({} rows loaded{total_str}, col {col_str}{sort_str}) [Fetching page...] ",
                state.rows.len()
            )
        } else if !state.has_more_rows {
            format!(
                "{title_prefix}Table View — Infinite Scroll{partition_str} (All {} rows loaded{total_str}, col {col_str}{sort_str}) ",
                state.rows.len()
            )
        } else {
            format!(
                "{title_prefix}Table View — Infinite Scroll{partition_str} ({} rows loaded{total_str}, col {col_str}{sort_str}) ",
                state.rows.len()
            )
        }
    } else {
        let all_visible_cols = state.visible_columns();
        let vis_pos = all_visible_cols
            .iter()
            .position(|(idx, _)| *idx == state.selected_col)
            .map(|p| p + 1)
            .unwrap_or(1);
        let vis_tot = all_visible_cols.len().max(1);
        let hidden_count = state.columns.len().saturating_sub(all_visible_cols.len());
        let col_str = if hidden_count > 0 {
            format!("{vis_pos}/{vis_tot} ({hidden_count} hidden)")
        } else {
            format!("{vis_pos}/{vis_tot}")
        };

        format!(
            "{title_prefix}Sample Mode ({} sample rows, col {col_str}{sort_str}) ",
            state.rows.len()
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

    let (visible_cols, col_width_sizes, effective_scroll_h) =
        visible_columns_layout(state, available_width);
    state.scroll_h.set(effective_scroll_h);
    state.last_selected_col.set(state.selected_col);

    let col_widths: Vec<Constraint> = col_width_sizes
        .iter()
        .map(|&w| Constraint::Length(w as u16))
        .collect();

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

    let row_visual_heights: Vec<u16> = state
        .rows
        .iter()
        .map(|row| {
            let is_summary = is_stats && is_stats_summary_row(&state.columns, row);
            compute_row_height(
                row,
                &visible_cols,
                &col_width_sizes,
                is_summary,
                data_size_col_idx,
                stats_total_data_size,
                row_count_col_idx,
                stats_total_row_count.as_ref(),
            )
        })
        .collect();

    let has_h_scrollbar = state.columns.len() > 1;
    let table_reserved_rows = if has_h_scrollbar { 3 } else { 2 };
    let viewport_lines = (inner.height as usize).saturating_sub(table_reserved_rows).max(1);

    let effective_scroll_v = calculate_effective_scroll_v(
        state.scroll_v.get(),
        state.selected_row,
        state.last_selected_row.get(),
        viewport_lines,
        &row_visual_heights,
        state.rows.len(),
    );
    state.scroll_v.set(effective_scroll_v);
    state.last_selected_row.set(state.selected_row);

    let header_cells: Vec<Cell> = visible_cols
        .iter()
        .map(|(orig_col_idx, c)| {
            let is_selected_col = *orig_col_idx == state.selected_col;
            let has_filter = state.column_filters.contains_key(c);
            let sort_indicator = match &state.sort {
                Some((col_name, crate::app::SortDirection::Asc)) if col_name == c => " ▲",
                Some((col_name, crate::app::SortDirection::Desc)) if col_name == c => " ▼",
                _ => "",
            };
            let filter_indicator = if has_filter { " [F]" } else { "" };
            let display_name = format!("{c}{filter_indicator}{sort_indicator}");
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

    let mut col_x_offsets = Vec::with_capacity(visible_cols.len());
    let mut current_col_x = inner.x;
    for &w in &col_width_sizes {
        col_x_offsets.push(current_col_x);
        current_col_x += (w as u16) + 2;
    }

    let mut current_row_y = inner.y + 2; // header line + bottom margin 1
    let mut visible_rows: Vec<Row> = Vec::new();
    let mut total_rendered_lines: usize = 0;

    for (row_idx, row) in state
        .rows
        .iter()
        .enumerate()
        .skip(effective_scroll_v)
    {
        if total_rendered_lines >= viewport_lines && !visible_rows.is_empty() {
            break;
        }

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
            total_rendered_lines += 1;
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
                let col_x = col_x_offsets.get(v_idx).copied().unwrap_or(inner.x);
                let wrapped_lines = wrap_text(display_val, col_w);
                let cell_h = wrapped_lines.len() as u16;
                max_row_height = max_row_height.max(cell_h);

                let is_mouse_sel = is_active
                    && app.is_area_mouse_selected_rect(col_x, col_w as u16, row_y, cell_h.max(1));
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
        total_rendered_lines += max_row_height as usize;
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

    let all_vis_len = state.visible_columns().len();
    if all_vis_len > 1 {
        let mut h_scroll_state =
            ScrollbarState::new(all_vis_len.saturating_sub(1)).position(effective_scroll_h);
        frame.render_stateful_widget(
            Scrollbar::default()
                .orientation(ScrollbarOrientation::HorizontalBottom)
                .thumb_symbol("▄")
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
    use ratatui::{Terminal, backend::TestBackend};

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
            scroll_v: std::cell::Cell::new(0),
            scroll_h: std::cell::Cell::new(0),
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

    #[test]
    fn test_render_table_with_hidden_columns() {
        use ratatui::{Terminal, backend::TestBackend, layout::Rect};
        use std::collections::HashSet;

        let mut hidden = HashSet::new();
        hidden.insert("secret".to_string());

        let state = ResultsState {
            query_buffer: "SELECT id, secret, name FROM users".to_string(),
            query_cursor: 35,
            columns: vec!["id".to_string(), "secret".to_string(), "name".to_string()],
            rows: vec![
                vec!["1".to_string(), "pass123".to_string(), "Alice".to_string()],
            ],
            scroll_v: std::cell::Cell::new(0),
            scroll_h: std::cell::Cell::new(0),
            selected_row: 0,
            selected_col: 0,
            loading: false,
            error: None,
            is_paginated: false,
            catalog: "memory".to_string(),
            schema: "default".to_string(),
            table: "users".to_string(),
            hidden_columns: hidden,
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

        assert!(content.contains("id"), "visible column 'id' must be rendered");
        assert!(content.contains("name"), "visible column 'name' must be rendered");
        assert!(!content.contains("secret"), "hidden column 'secret' must not be rendered");
        assert!(!content.contains("pass123"), "hidden column value 'pass123' must not be rendered");
        assert!(content.contains("(1 hidden)"), "Title should indicate 1 hidden column: {content}");
    }

    #[test]
    fn test_cursor_navigation_and_incremental_viewport_scrolling() {
        use ratatui::{Terminal, backend::TestBackend, layout::Rect};

        let rows: Vec<Vec<String>> = (0..30)
            .map(|i| vec![format!("id_{i}"), format!("val_{i}"), format!("extra_{i}")])
            .collect();

        let mut state = ResultsState {
            query_buffer: "SELECT * FROM items".to_string(),
            query_cursor: 19,
            columns: vec!["id".to_string(), "val".to_string(), "extra".to_string()],
            rows,
            scroll_v: std::cell::Cell::new(0),
            scroll_h: std::cell::Cell::new(0),
            selected_row: 0,
            selected_col: 0,
            ..Default::default()
        };

        let app = crate::app::App::new(crate::config::ConnectionConfig::default(), false);
        let backend = TestBackend::new(80, 20);
        let mut terminal = Terminal::new(backend).unwrap();

        // Initial render: cursor at row 0, scroll_v should be 0
        terminal
            .draw(|f| {
                render(f, Rect::new(0, 0, 80, 20), &state, "".to_string(), true, &app);
            })
            .unwrap();
        assert_eq!(state.scroll_v.get(), 0, "Initial scroll_v should be 0");

        // Move cursor down to row 10 (take_rows is 15, margin is 4, 10 + 4 < 15): sheet should NOT move
        state.selected_row = 10;
        terminal
            .draw(|f| {
                render(f, Rect::new(0, 0, 80, 20), &state, "".to_string(), true, &app);
            })
            .unwrap();
        assert_eq!(
            state.scroll_v.get(),
            0,
            "Cursor moving within margin threshold should not scroll the sheet"
        );

        // Move cursor down to row 11 (11 + 4 = 15 >= 15): sheet should incrementally scroll to 1
        state.selected_row = 11;
        terminal
            .draw(|f| {
                render(f, Rect::new(0, 0, 80, 20), &state, "".to_string(), true, &app);
            })
            .unwrap();
        assert_eq!(
            state.scroll_v.get(),
            1,
            "Reaching threshold of 4 rows from bottom should incrementally scroll to 1"
        );

        // Move cursor down to row 12: sheet should incrementally scroll to 2
        state.selected_row = 12;
        terminal
            .draw(|f| {
                render(f, Rect::new(0, 0, 80, 20), &state, "".to_string(), true, &app);
            })
            .unwrap();
        assert_eq!(state.scroll_v.get(), 2, "Reaching threshold should incrementally scroll to 2");

        // Move cursor down to row 13: sheet should incrementally scroll to 3
        state.selected_row = 13;
        terminal
            .draw(|f| {
                render(f, Rect::new(0, 0, 80, 20), &state, "".to_string(), true, &app);
            })
            .unwrap();
        assert_eq!(state.scroll_v.get(), 3, "Reaching threshold should incrementally scroll to 3");

        // Now move cursor back UP to row 12 (within viewport with scroll_v = 3): sheet should NOT move
        state.selected_row = 12;
        terminal
            .draw(|f| {
                render(f, Rect::new(0, 0, 80, 20), &state, "".to_string(), true, &app);
            })
            .unwrap();
        assert_eq!(state.scroll_v.get(), 3, "Moving up within viewport should NOT move the sheet");

        // Move cursor up to row 7 (7 >= scroll_v 3 + margin 4): sheet should STILL not move
        state.selected_row = 7;
        terminal
            .draw(|f| {
                render(f, Rect::new(0, 0, 80, 20), &state, "".to_string(), true, &app);
            })
            .unwrap();
        assert_eq!(state.scroll_v.get(), 3, "Moving up within margin should keep scroll_v = 3");

        // Move cursor up past the margin threshold (row 6 < scroll_v 3 + margin 4): sheet incrementally scrolls up to 2
        state.selected_row = 6;
        terminal
            .draw(|f| {
                render(f, Rect::new(0, 0, 80, 20), &state, "".to_string(), true, &app);
            })
            .unwrap();
        assert_eq!(state.scroll_v.get(), 2, "Moving past top margin should incrementally scroll up to 2");

        // Move cursor up to row 0: sheet incrementally scrolls up to 0
        state.selected_row = 0;
        terminal
            .draw(|f| {
                render(f, Rect::new(0, 0, 80, 20), &state, "".to_string(), true, &app);
            })
            .unwrap();
        assert_eq!(state.scroll_v.get(), 0, "Moving to top should scroll back to 0");
    }

    #[test]
    fn test_wrapped_rows_incremental_viewport_scrolling() {
        // Rows with long text in column 1 that wrap into 3 lines each
        let rows: Vec<Vec<String>> = (0..20)
            .map(|i| {
                vec![
                    format!("id_{i}"),
                    "This is a very long description text that definitely wraps into several lines".to_string(),
                ]
            })
            .collect();

        let mut state = ResultsState {
            query_buffer: "SELECT id, desc FROM items".to_string(),
            query_cursor: 26,
            columns: vec!["id".to_string(), "description".to_string()],
            rows,
            scroll_v: std::cell::Cell::new(0),
            scroll_h: std::cell::Cell::new(0),
            selected_row: 0,
            selected_col: 0,
            ..Default::default()
        };

        let app = crate::app::App::new(crate::config::ConnectionConfig::default(), false);
        let backend = TestBackend::new(80, 20);
        let mut terminal = Terminal::new(backend).unwrap();

        // Initial render: row 0 selected, scroll_v = 0
        terminal
            .draw(|f| {
                render(f, Rect::new(0, 0, 80, 20), &state, "".to_string(), true, &app);
            })
            .unwrap();
        assert_eq!(state.scroll_v.get(), 0);

        // Move cursor down to row 5: because each row is ~3 lines tall, 5 rows exceed the viewport (17 lines).
        // Therefore, scroll_v must advance so row 5 is visible!
        state.selected_row = 5;
        terminal
            .draw(|f| {
                render(f, Rect::new(0, 0, 80, 20), &state, "".to_string(), true, &app);
            })
            .unwrap();
        assert!(
            state.scroll_v.get() > 0,
            "When rows wrap and occupy more visual lines, scroll_v must scroll earlier to keep cursor visible. Got: {}",
            state.scroll_v.get()
        );

        // Move cursor down to row 10: scroll_v must advance further
        state.selected_row = 10;
        terminal
            .draw(|f| {
                render(f, Rect::new(0, 0, 80, 20), &state, "".to_string(), true, &app);
            })
            .unwrap();
        assert!(
            state.scroll_v.get() >= 5,
            "scroll_v must advance as cursor moves down tall wrapped rows. Got: {}",
            state.scroll_v.get()
        );
        assert!(
            state.scroll_v.get() <= 10,
            "scroll_v must never scroll past the selected row"
        );
    }

    #[test]
    fn test_hit_test_cell_mapping() {
        let rows = vec![
            vec!["1".to_string(), "Alice".to_string()],
            vec!["2".to_string(), "Bob".to_string()],
            vec!["3".to_string(), "Charlie".to_string()],
        ];
        let state = ResultsState {
            query_buffer: "SELECT * FROM users".to_string(),
            query_cursor: 19,
            columns: vec!["id".to_string(), "name".to_string()],
            rows,
            scroll_v: std::cell::Cell::new(0),
            scroll_h: std::cell::Cell::new(0),
            selected_row: 0,
            selected_col: 0,
            ..Default::default()
        };

        let inner = Rect::new(10, 5, 60, 20);

        // Click outside bounds should return None
        assert_eq!(hit_test_cell(&state, inner, 5, 5), None);
        assert_eq!(hit_test_cell(&state, inner, 15, 30), None);

        // Header click at y = 5, column 0
        let res = hit_test_cell(&state, inner, 12, 5);
        assert!(res.is_some());
        assert_eq!(res.unwrap().1, 0, "Should hit column 0 ('id')");

        // Row 0 click at y = 7 (header is 5, separator is 6, first data row is 7)
        let res = hit_test_cell(&state, inner, 12, 7);
        assert_eq!(res, Some((0, 0)), "Should hit row 0, col 0");

        // Row 1 click at y = 8
        let res = hit_test_cell(&state, inner, 12, 8);
        assert_eq!(res, Some((1, 0)), "Should hit row 1, col 0");

        // Click on column 1 ("name") at row 1
        // col 0 width is 8 + 2 spacing = 10, so x = 22 is in col 1
        let res = hit_test_cell(&state, inner, 22, 8);
        assert_eq!(res, Some((1, 1)), "Should hit row 1, col 1");
    }

    #[test]
    fn test_wheel_scroll_viewport_not_clamped_to_cursor() {
        let rows: Vec<Vec<String>> = (0..50)
            .map(|i| vec![format!("id_{i}"), format!("val_{i}")])
            .collect();

        let mut state = ResultsState {
            query_buffer: "SELECT * FROM items".to_string(),
            query_cursor: 19,
            columns: vec!["id".to_string(), "val".to_string()],
            rows,
            scroll_v: std::cell::Cell::new(0),
            scroll_h: std::cell::Cell::new(0),
            selected_row: 0,
            selected_col: 0,
            ..Default::default()
        };

        let app = crate::app::App::new(crate::config::ConnectionConfig::default(), false);
        let backend = TestBackend::new(80, 20);
        let mut terminal = Terminal::new(backend).unwrap();

        // Initial render: cursor at 0, scroll_v 0
        terminal
            .draw(|f| {
                render(f, Rect::new(0, 0, 80, 20), &state, "".to_string(), true, &app);
            })
            .unwrap();
        assert_eq!(state.scroll_v.get(), 0);

        // Simulate user mouse-scrolling down to row 10 (without keyboard navigation)
        state.scroll_v.set(10);
        state.selected_row = 10;
        state.last_selected_row.set(10);
        terminal
            .draw(|f| {
                render(f, Rect::new(0, 0, 80, 20), &state, "".to_string(), true, &app);
            })
            .unwrap();
        assert_eq!(
            state.scroll_v.get(),
            10,
            "Mouse scrolling down must not clamp scroll_v back to top margin"
        );

        // Scroll further down to row 25
        state.scroll_v.set(25);
        state.selected_row = 25;
        state.last_selected_row.set(25);
        terminal
            .draw(|f| {
                render(f, Rect::new(0, 0, 80, 20), &state, "".to_string(), true, &app);
            })
            .unwrap();
        assert_eq!(
            state.scroll_v.get(),
            25,
            "Mouse scrolling down must allow continuous unlimited scrolling"
        );
    }

    #[test]
    fn test_horizontal_scroll_not_clamped_to_first_column() {
        let cols: Vec<String> = (0..20).map(|i| format!("col_{i}")).collect();
        let rows = vec![(0..20).map(|i| format!("val_{i}")).collect()];

        let mut state = ResultsState {
            query_buffer: "SELECT * FROM wide_table".to_string(),
            query_cursor: 24,
            columns: cols,
            rows,
            scroll_v: std::cell::Cell::new(0),
            scroll_h: std::cell::Cell::new(0),
            selected_col: 0,
            selected_row: 0,
            ..Default::default()
        };

        // Scroll horizontally to column 5 with column 5 selected
        state.scroll_h.set(5);
        state.selected_col = 5;
        state.last_selected_col.set(5);

        let (vis_cols, _, effective_scroll_h) = visible_columns_layout(&state, 80);
        assert_eq!(
            effective_scroll_h, 5,
            "Horizontal scroll to col 5 must remain at col 5"
        );
        assert_eq!(vis_cols.first().map(|(idx, _)| *idx), Some(5));
    }

    #[test]
    fn test_horizontal_scroll_helpers_right_and_left() {
        let cols: Vec<String> = (0..20).map(|i| format!("column_{i}")).collect();
        let rows = vec![(0..20).map(|i| format!("value_{i}")).collect()];

        let mut state = ResultsState {
            columns: cols,
            rows,
            scroll_v: std::cell::Cell::new(0),
            scroll_h: std::cell::Cell::new(0),
            selected_col: 0,
            selected_row: 0,
            ..Default::default()
        };

        let avail_width = 80;
        let max_scroll = compute_max_scroll_h(&state, avail_width);
        assert!(max_scroll > 0, "20 columns should require scrolling across 80 width");

        // Scroll right 3 times
        scroll_table_horizontal_right(&mut state, avail_width);
        assert_eq!(state.scroll_h.get(), 1);
        assert_eq!(state.selected_col, 1);

        scroll_table_horizontal_right(&mut state, avail_width);
        assert_eq!(state.scroll_h.get(), 2);
        assert_eq!(state.selected_col, 2);

        scroll_table_horizontal_right(&mut state, avail_width);
        assert_eq!(state.scroll_h.get(), 3);
        assert_eq!(state.selected_col, 3);

        // Layout rendered at scroll_h = 3 should keep effective_scroll_h at 3
        let (vis, _, eff_h) = visible_columns_layout(&state, avail_width);
        assert_eq!(eff_h, 3);
        assert_eq!(vis.first().map(|(idx, _)| *idx), Some(3));

        // Scroll left 2 times
        scroll_table_horizontal_left(&mut state, avail_width);
        assert_eq!(state.scroll_h.get(), 2);

        scroll_table_horizontal_left(&mut state, avail_width);
        assert_eq!(state.scroll_h.get(), 1);

        let (vis2, _, eff_h2) = visible_columns_layout(&state, avail_width);
        assert_eq!(eff_h2, 1);
        assert_eq!(vis2.first().map(|(idx, _)| *idx), Some(1));
    }

    #[test]
    fn test_scroll_table_horizontal_to() {
        let cols: Vec<String> = (0..20).map(|i| format!("column_{i}")).collect();
        let rows = vec![(0..20).map(|i| format!("value_{i}")).collect()];

        let mut state = ResultsState {
            columns: cols,
            rows,
            scroll_v: std::cell::Cell::new(0),
            scroll_h: std::cell::Cell::new(0),
            selected_col: 0,
            selected_row: 0,
            ..Default::default()
        };

        let avail_width = 80;
        let max_scroll = compute_max_scroll_h(&state, avail_width);

        // Jump to index 5
        scroll_table_horizontal_to(&mut state, avail_width, 5);
        assert_eq!(state.scroll_h.get(), 5);
        assert_eq!(state.selected_col, 5);

        // Jump beyond max_scroll_h
        scroll_table_horizontal_to(&mut state, avail_width, 999);
        assert_eq!(state.scroll_h.get(), max_scroll);
        assert!(state.selected_col >= max_scroll);

        // Jump back to 0
        scroll_table_horizontal_to(&mut state, avail_width, 0);
        assert_eq!(state.scroll_h.get(), 0);
        let (_first, last) = visible_col_range_indices(&state, avail_width, 0);
        assert!(state.selected_col <= last);
    }

    #[test]
    fn test_toggle_sort_cyclic_desc_asc_none() {
        let cols = vec!["id".to_string(), "name".to_string()];
        let rows = vec![
            vec!["3".to_string(), "Charlie".to_string()],
            vec!["1".to_string(), "Alice".to_string()],
            vec!["2".to_string(), "Bob".to_string()],
        ];
        let mut state = ResultsState {
            columns: cols,
            rows: rows.clone(),
            selected_col: 0,
            ..Default::default()
        };

        // 1st toggle: Descending
        state.toggle_sort(&[]);
        assert_eq!(
            state.sort,
            Some(("id".to_string(), crate::app::SortDirection::Desc))
        );
        assert_eq!(
            state.rows,
            vec![
                vec!["3".to_string(), "Charlie".to_string()],
                vec!["2".to_string(), "Bob".to_string()],
                vec!["1".to_string(), "Alice".to_string()],
            ]
        );

        // 2nd toggle: Ascending
        state.toggle_sort(&[]);
        assert_eq!(
            state.sort,
            Some(("id".to_string(), crate::app::SortDirection::Asc))
        );
        assert_eq!(
            state.rows,
            vec![
                vec!["1".to_string(), "Alice".to_string()],
                vec!["2".to_string(), "Bob".to_string()],
                vec!["3".to_string(), "Charlie".to_string()],
            ]
        );

        // 3rd toggle: No-sorting (original order restored)
        state.toggle_sort(&[]);
        assert_eq!(state.sort, None);
        assert_eq!(state.rows, rows);
    }

    #[test]
    fn test_sort_numeric_column_orders_properly() {
        let cols = vec!["val".to_string()];
        let rows = vec![
            vec!["10".to_string()],
            vec!["2".to_string()],
            vec!["NULL".to_string()],
            vec!["-5".to_string()],
            vec!["100.5".to_string()],
        ];
        let mut state = ResultsState {
            columns: cols,
            rows: rows.clone(),
            selected_col: 0,
            ..Default::default()
        };

        // Desc: 100.5, 10, 2, -5, NULL (null at bottom)
        state.toggle_sort(&[]);
        assert_eq!(
            state.rows,
            vec![
                vec!["100.5".to_string()],
                vec!["10".to_string()],
                vec!["2".to_string()],
                vec!["-5".to_string()],
                vec!["NULL".to_string()],
            ]
        );

        // Asc: -5, 2, 10, 100.5, NULL (null at bottom)
        state.toggle_sort(&[]);
        assert_eq!(
            state.rows,
            vec![
                vec!["-5".to_string()],
                vec!["2".to_string()],
                vec!["10".to_string()],
                vec!["100.5".to_string()],
                vec!["NULL".to_string()],
            ]
        );
    }

    #[test]
    fn test_sort_string_column_orders_properly() {
        let cols = vec!["name".to_string()];
        let rows = vec![
            vec!["banana".to_string()],
            vec!["Apple".to_string()],
            vec!["NULL".to_string()],
            vec!["cherry".to_string()],
        ];
        let mut state = ResultsState {
            columns: cols,
            rows,
            selected_col: 0,
            ..Default::default()
        };

        // Desc: cherry, banana, Apple, NULL
        state.toggle_sort(&[]);
        assert_eq!(
            state.rows,
            vec![
                vec!["cherry".to_string()],
                vec!["banana".to_string()],
                vec!["Apple".to_string()],
                vec!["NULL".to_string()],
            ]
        );

        // Asc: Apple, banana, cherry, NULL
        state.toggle_sort(&[]);
        assert_eq!(
            state.rows,
            vec![
                vec!["Apple".to_string()],
                vec!["banana".to_string()],
                vec!["cherry".to_string()],
                vec!["NULL".to_string()],
            ]
        );
    }

    #[test]
    fn test_render_table_with_sort_indicators() {
        use ratatui::{Terminal, backend::TestBackend, layout::Rect};

        let state = ResultsState {
            columns: vec!["id".to_string(), "name".to_string()],
            rows: vec![vec!["1".to_string(), "Alice".to_string()]],
            sort: Some(("id".to_string(), crate::app::SortDirection::Desc)),
            catalog: "iceberg".to_string(),
            schema: "sales".to_string(),
            table: "orders".to_string(),
            is_paginated: true,
            ..Default::default()
        };

        let backend = TestBackend::new(160, 20);
        let mut terminal = Terminal::new(backend).unwrap();
        let app = crate::app::App::new(crate::config::ConnectionConfig::default(), false);

        terminal
            .draw(|f| {
                render(f, Rect::new(0, 0, 160, 20), &state, "".to_string(), true, &app);
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

        assert!(content.contains("▼"), "Header should contain sort indicator: {content}");
        assert!(content.contains("sorted by id desc"), "Title should show sort info: {content}");
        assert!(content.contains("iceberg › sales › orders"), "Title should show table breadcrumb: {content}");
    }
}
