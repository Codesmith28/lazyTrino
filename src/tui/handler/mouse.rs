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

use crossterm::event::{KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;

use crate::app::*;

use super::{
    Command,
    navigation::{
        check_trigger_infinite_scroll, extract_list_labels, get_selected, mod_list_selected,
        trigger_action,
    },
    query::{
        active_query_buffer, active_query_state_mut, copy_to_clipboard, query_bar_layout,
        query_text_index_from_mouse,
    },
};

pub fn handle_mouse_sync(app: &mut App, mouse: MouseEvent) -> Option<Command> {
    let (term_width, term_height) = crossterm::terminal::size().unwrap_or((80, 24));
    if term_width == 0 || term_height == 0 {
        return None;
    }

    let bottom_y = term_height.saturating_sub(7);
    let is_in_table = matches!(app.screen, Screen::Actions(_));
    let border_pct = if is_in_table {
        if app.main_panel_pct > 30 {
            15
        } else {
            app.main_panel_pct.clamp(8, 30)
        }
    } else if app.main_panel_pct <= 30 {
        60
    } else {
        app.main_panel_pct
    };
    let border_x = ((term_width as u32 * border_pct as u32) / 100) as u16;

    match mouse.kind {
        MouseEventKind::Down(MouseButton::Left) => {
            if let Some((query_x, query_y, query_width, query_height, inner_w)) =
                query_bar_layout(app, term_width, term_height)
            {
                let inside_query_bar = mouse.column >= query_x
                    && mouse.column < query_x.saturating_add(query_width)
                    && mouse.row >= query_y
                    && mouse.row < query_y.saturating_add(query_height);

                if inside_query_bar {
                    let buf = active_query_buffer(app).unwrap_or_default();
                    let idx = query_text_index_from_mouse(
                        mouse.column,
                        mouse.row,
                        query_x,
                        query_y,
                        inner_w,
                        &buf,
                    );
                    let is_table_view = match &app.screen {
                        Screen::Actions(a) => a.selected == 0,
                        _ => false,
                    };
                    if is_table_view {
                        app.mode = Mode::QueryInput;
                    }
                    app.set_active_panel(ActivePanel::MainViewer);
                    app.is_dragging_query_select = true;
                    app.is_selecting_text = false;
                    app.mouse_selection_anchor = None;
                    app.mouse_selection_current = None;
                    if let Some(state) = active_query_state_mut(app) {
                        state.selection_anchor = Some(idx);
                        state.query_cursor = idx;
                        state.invalid_query_error = None;
                    }
                    return None;
                }
            }

            app.mouse_selection_anchor = Some((mouse.column, mouse.row));
            app.mouse_selection_current = Some((mouse.column, mouse.row));
            app.is_selecting_text = false;

            if mouse.row < bottom_y && (mouse.column as i32 - border_x as i32).abs() <= 1 {
                app.is_dragging_resizer = true;
            } else {
                app.is_dragging_resizer = false;

                if mouse.column < border_x && mouse.row < bottom_y {
                    if is_in_table {
                        let clicked_row = mouse.row.saturating_sub(1) as usize;
                        if clicked_row < ACTIONS.len() {
                            return trigger_action(app, clicked_row);
                        } else {
                            app.set_active_panel(ActivePanel::MenuPane);
                        }
                    } else {
                        app.mode = Mode::Normal;
                        app.set_active_panel(ActivePanel::MainViewer);
                    }
                } else if mouse.column >= border_x && mouse.row < bottom_y {
                    app.mode = Mode::Normal;
                    app.set_active_panel(ActivePanel::MainViewer);

                    if let Some(inner) = results_table_inner_rect(app, term_width, term_height)
                        && let Screen::Actions(ref mut a) = app.screen
                        && let Some(ref mut res) = a.results
                        && let Some((row_idx, col_idx)) =
                            crate::tui::screens::results::hit_test_cell(
                                res,
                                inner,
                                mouse.column,
                                mouse.row,
                            )
                    {
                        res.selected_row = row_idx;
                        res.selected_col = col_idx;
                        res.last_selected_row.set(row_idx);
                        res.last_selected_col.set(col_idx);
                        app.clear_mouse_selection();
                    }
                }
            }
        }
        MouseEventKind::Drag(MouseButton::Left) => {
            if app.is_dragging_query_select {
                if let Some((query_x, query_y, _, _, inner_w)) =
                    query_bar_layout(app, term_width, term_height)
                {
                    let buf = active_query_buffer(app).unwrap_or_default();
                    let idx = query_text_index_from_mouse(
                        mouse.column,
                        mouse.row,
                        query_x,
                        query_y,
                        inner_w,
                        &buf,
                    );
                    if let Some(state) = active_query_state_mut(app) {
                        state.query_cursor = idx;
                        if state.selection_anchor == Some(idx) {
                            state.clear_selection();
                        }
                    }
                }
                return None;
            }
            if let Some((a_x, a_y)) = app.mouse_selection_anchor
                && (mouse.column != a_x || mouse.row != a_y)
            {
                app.is_selecting_text = true;
                app.mouse_selection_current = Some((mouse.column, mouse.row));
            }
            if app.is_dragging_resizer {
                let pct = ((mouse.column as u32 * 100) / term_width as u32) as u16;
                app.main_panel_pct = pct.clamp(8, 80);
            }
        }
        MouseEventKind::Up(MouseButton::Left) => {
            app.is_dragging_resizer = false;
            if app.is_dragging_query_select {
                app.is_dragging_query_select = false;
                if let Some(state) = active_query_state_mut(app)
                    && state.selection_anchor == Some(state.query_cursor)
                {
                    state.clear_selection();
                }
                return None;
            }
            if app.is_selecting_text {
                app.is_selecting_text = false;
                if let (Some(anchor), Some(current)) =
                    (app.mouse_selection_anchor, app.mouse_selection_current)
                    && anchor != current
                {
                    let text = extract_selected_text(app, anchor, current);
                    if !text.is_empty() {
                        copy_to_clipboard(&text);
                        app.copied_toast =
                            Some((text.chars().take(30).collect(), std::time::Instant::now()));
                    }
                }
            }
        }
        MouseEventKind::ScrollDown => {
            if app.help_modal.is_some() || matches!(app.screen, Screen::Help) {
                if let Some(ref mut m) = app.help_modal {
                    m.scroll = m.scroll.saturating_add(1);
                    app.help_scroll = m.scroll;
                } else {
                    app.help_scroll = app.help_scroll.saturating_add(1);
                }
                return None;
            }
            let shift = mouse.modifiers.contains(KeyModifiers::SHIFT);
            if is_in_table {
                let selected_idx = match &app.screen {
                    Screen::Actions(a) => a.selected,
                    _ => 0,
                };
                let action = ACTIONS.get(selected_idx).map(|(_, _, a)| a);
                if matches!(action, Some(Action::Partitions)) {
                    let max_lines = app.partition_tree_lines.len().saturating_sub(1);
                    app.partition_scroll = (app.partition_scroll + 3).min(max_lines);
                } else if matches!(action, Some(Action::Schema)) {
                    let max_cols = app.vertical_schema_cols.len().saturating_sub(1);
                    app.schema_scroll = (app.schema_scroll + 3).min(max_cols);
                } else if let Screen::Actions(ref mut a) = app.screen
                    && let Some(ref mut dd) = a.drilldown
                    && a.results.is_none()
                {
                    let depth = dd.depth();
                    if let Some(items) = dd.levels_cache.get(depth)
                        && !items.is_empty()
                    {
                        dd.selected = (dd.selected + 3).min(items.len().saturating_sub(1));
                    }
                } else if let Screen::Actions(ref mut a) = app.screen
                    && let Some(ref mut res) = a.results
                {
                    if shift {
                        if !res.columns.is_empty() {
                            let new_h = (res.scroll_h.get() + 1).min(res.columns.len().saturating_sub(1));
                            res.scroll_h.set(new_h);
                            if res.selected_col < new_h {
                                res.selected_col = new_h;
                                res.last_selected_col.set(new_h);
                            }
                        }
                    } else if !res.rows.is_empty() {
                        let delta = 3;
                        let new_scroll_v = (res.scroll_v.get() + delta).min(res.rows.len().saturating_sub(1));
                        res.scroll_v.set(new_scroll_v);
                        if res.selected_row < new_scroll_v {
                            res.selected_row = new_scroll_v;
                            res.last_selected_row.set(new_scroll_v);
                        }
                        return check_trigger_infinite_scroll(app);
                    }
                }
            } else if let Some(items) = extract_list_labels(app)
                && !items.is_empty()
                && let Some(s) = get_selected(&app.screen)
            {
                mod_list_selected(&mut app.screen, (s + 1).min(items.len().saturating_sub(1)));
            }
        }
        MouseEventKind::ScrollUp => {
            if app.help_modal.is_some() || matches!(app.screen, Screen::Help) {
                if let Some(ref mut m) = app.help_modal {
                    m.scroll = m.scroll.saturating_sub(1);
                    app.help_scroll = m.scroll;
                } else {
                    app.help_scroll = app.help_scroll.saturating_sub(1);
                }
                return None;
            }
            let shift = mouse.modifiers.contains(KeyModifiers::SHIFT);
            let view_rows = results_table_inner_rect(app, term_width, term_height)
                .map(|r| (r.height as usize).saturating_sub(3).max(1))
                .unwrap_or(15);
            if is_in_table {
                let selected_idx = match &app.screen {
                    Screen::Actions(a) => a.selected,
                    _ => 0,
                };
                let action = ACTIONS.get(selected_idx).map(|(_, _, a)| a);
                if matches!(action, Some(Action::Partitions)) {
                    app.partition_scroll = app.partition_scroll.saturating_sub(3);
                } else if matches!(action, Some(Action::Schema)) {
                    app.schema_scroll = app.schema_scroll.saturating_sub(3);
                } else if let Screen::Actions(ref mut a) = app.screen
                    && let Some(ref mut dd) = a.drilldown
                    && a.results.is_none()
                {
                    dd.selected = dd.selected.saturating_sub(3);
                } else if let Screen::Actions(ref mut a) = app.screen
                    && let Some(ref mut res) = a.results
                {
                    if shift {
                        res.scroll_h.set(res.scroll_h.get().saturating_sub(1));
                    } else {
                        let delta = 3;
                        let new_scroll_v = res.scroll_v.get().saturating_sub(delta);
                        res.scroll_v.set(new_scroll_v);
                        let max_visible_row = new_scroll_v + view_rows.saturating_sub(1);
                        if res.selected_row > max_visible_row {
                            res.selected_row = max_visible_row.min(res.rows.len().saturating_sub(1));
                            res.last_selected_row.set(res.selected_row);
                        }
                    }
                }
            } else if let Some(s) = get_selected(&app.screen) {
                mod_list_selected(&mut app.screen, s.saturating_sub(1));
            }
        }
        _ => {}
    }
    None
}

pub fn results_table_inner_rect(
    app: &App,
    term_width: u16,
    term_height: u16,
) -> Option<Rect> {
    let bottom_y = term_height.saturating_sub(7);
    if bottom_y == 0 {
        return None;
    }
    let Screen::Actions(a) = &app.screen else {
        return None;
    };
    a.results.as_ref()?;

    let menu_pct = if app.main_panel_pct > 30 {
        15
    } else {
        app.main_panel_pct.clamp(8, 30)
    };
    let border_x = ((term_width as u32 * menu_pct as u32) / 100) as u16;
    let preview_width = term_width.saturating_sub(border_x);
    let inner_w = preview_width.saturating_sub(2).max(1) as usize;

    let query_height = if matches!(app.mode, Mode::QueryInput) {
        let total_chars = 7
            + a.results
                .as_ref()
                .map(|r| r.query_buffer.len())
                .unwrap_or_else(|| a.query_buffer.len());
        let lines = total_chars.div_ceil(inner_w);
        (lines as u16 + 2).clamp(3, 7)
    } else {
        3
    };

    if query_height >= bottom_y {
        return None;
    }

    let preview_height = bottom_y.saturating_sub(query_height);
    if preview_width < 2 || preview_height < 2 {
        return None;
    }

    Some(Rect {
        x: border_x + 1,
        y: query_height + 1,
        width: preview_width.saturating_sub(2),
        height: preview_height.saturating_sub(2),
    })
}

pub fn extract_selected_text(app: &App, anchor: (u16, u16), current: (u16, u16)) -> String {
    let (term_width, term_height) = crossterm::terminal::size().unwrap_or((80, 24));
    if term_width == 0 || term_height == 0 {
        return String::new();
    }

    let bottom_y = term_height.saturating_sub(7);

    let start_row = anchor.1.min(current.1);
    let end_row = anchor.1.max(current.1);
    let min_col = anchor.0.min(current.0);
    let max_col = anchor.0.max(current.0);

    // 1. Selection in Bottom Query Inspector Pane
    if anchor.1 >= bottom_y || current.1 >= bottom_y {
        let mut lines = Vec::new();
        let inner_y = bottom_y + 1;
        let rev_logs: Vec<&QueryLogEntry> = app.query_logs.iter().rev().collect();
        for r in start_row..=end_row {
            if r >= inner_y && r < term_height.saturating_sub(1) {
                let idx = (r - inner_y) as usize + app.query_inspector_scroll;
                if idx < rev_logs.len() {
                    lines.push(rev_logs[idx].sql.clone());
                }
            }
        }
        return lines.join("\n");
    }

    let is_in_table = matches!(app.screen, Screen::Actions(_));

    if !is_in_table {
        // Phase 1 screens: Catalog, Schema, Table
        let list_pct = if app.main_panel_pct <= 30 {
            60
        } else {
            app.main_panel_pct
        };
        let border_x = ((term_width as u32 * list_pct as u32) / 100) as u16;

        let inner_w = border_x.saturating_sub(2).max(1) as usize;
        let search_active = matches!(app.mode, Mode::Search);
        let search_height = if search_active {
            let total_chars = 3 + app.search_query.len();
            let lines = total_chars.div_ceil(inner_w);
            (lines as u16 + 2).clamp(3, 8)
        } else {
            3
        };

        if min_col < border_x {
            let inner_y = search_height + 1;
            match &app.screen {
                Screen::Catalog(s) => {
                    let mut lines = Vec::new();
                    for r in start_row..=end_row {
                        if r >= inner_y {
                            let idx = (r - inner_y) as usize;
                            if idx < s.items.len() {
                                lines.push(s.items[idx].trim().to_string());
                            }
                        }
                    }
                    return lines.join("\n");
                }
                Screen::Schema(s) => {
                    let mut lines = Vec::new();
                    for r in start_row..=end_row {
                        if r >= inner_y {
                            let idx = (r - inner_y) as usize;
                            if idx < s.items.len() {
                                lines.push(s.items[idx].trim().to_string());
                            }
                        }
                    }
                    return lines.join("\n");
                }
                Screen::Table(s) => {
                    let mut lines = Vec::new();
                    for r in start_row..=end_row {
                        if r >= inner_y {
                            let idx = (r - inner_y) as usize;
                            if idx < s.items.len() {
                                lines.push(s.items[idx].trim().to_string());
                            }
                        }
                    }
                    return lines.join("\n");
                }
                _ => {}
            }
        }
    } else if let Screen::Actions(state) = &app.screen {
        // Phase 2 screen: Screen::Actions
        let menu_pct = if app.main_panel_pct > 30 {
            15
        } else {
            app.main_panel_pct.clamp(8, 30)
        };
        let border_x = ((term_width as u32 * menu_pct as u32) / 100) as u16;

        let preview_w = term_width.saturating_sub(border_x);
        let inner_w = preview_w.saturating_sub(2).max(1) as usize;

        let query_active = matches!(app.mode, Mode::QueryInput);
        let query_height = if query_active {
            let total_chars = 7 + state
                .results
                .as_ref()
                .map(|r| r.query_buffer.len())
                .unwrap_or_else(|| state.query_buffer.len());
            let lines = total_chars.div_ceil(inner_w);
            (lines as u16 + 2).clamp(3, 7)
        } else {
            3
        };

        // Left Menu Pane (Actions list)
        if max_col < border_x || (app.active_panel == ActivePanel::MenuPane && min_col < border_x) {
            let inner_y = 1;
            let mut lines = Vec::new();
            for r in start_row..=end_row {
                if r >= inner_y {
                    let idx = (r - inner_y) as usize;
                    if idx < ACTIONS.len() {
                        lines.push(ACTIONS[idx].1.to_string());
                    }
                }
            }
            if !lines.is_empty() {
                return lines.join("\n");
            }
        }

        // Right Main Viewer Pane
        let preview_inner_y = query_height + 1;

        if matches!(
            ACTIONS.get(state.selected).map(|(_, _, a)| a),
            Some(crate::app::Action::Partitions)
        ) {
            // Action::Partitions
            let mut lines = Vec::new();
            for r in start_row..=end_row {
                if r >= preview_inner_y {
                    let idx = (r - preview_inner_y) as usize + app.partition_scroll;
                    if idx < app.partition_tree_lines.len() {
                        lines.push(app.partition_tree_lines[idx].clone());
                    }
                }
            }
            return lines.join("\n");
        } else if matches!(
            ACTIONS.get(state.selected).map(|(_, _, a)| a),
            Some(crate::app::Action::Schema)
        ) {
            // Action::Schema
            let mut lines = Vec::new();
            for r in start_row..=end_row {
                if r >= preview_inner_y + 2 {
                    let idx = (r - (preview_inner_y + 2)) as usize + app.schema_scroll;
                    if idx < app.vertical_schema_cols.len() {
                        let col = &app.vertical_schema_cols[idx];
                        lines.push(format!(
                            "{}\t{}\t{}\t{}",
                            col.name, col.data_type, col.key_meta, col.description
                        ));
                    }
                }
            }
            return lines.join("\n");
        } else if let Some(ref res) = state.results {
            // Results table view
            let mut lines = Vec::new();
            if let Some(inner) = results_table_inner_rect(app, term_width, term_height) {
                let available_width = inner.width.saturating_sub(4) as usize;
                let (visible_cols, col_width_sizes, _) =
                    crate::tui::screens::results::visible_columns_layout(res, available_width);

                let mut sel_cols = Vec::new();
                let mut cur_col_x = inner.x;
                for (v_idx, (orig_col_idx, _)) in visible_cols.iter().enumerate() {
                    let w = col_width_sizes.get(v_idx).copied().unwrap_or(15) as u16;
                    let next_x = cur_col_x + w + 2;
                    if cur_col_x <= max_col && next_x > min_col {
                        sel_cols.push(*orig_col_idx);
                    }
                    cur_col_x = next_x;
                }
                if sel_cols.is_empty() && !visible_cols.is_empty() {
                    sel_cols = visible_cols.iter().map(|(orig_idx, _)| *orig_idx).collect();
                }

                let is_stats = crate::tui::screens::results::is_stats_table(&res.columns);
                let (stats_total_row_count, stats_total_data_size) = if is_stats {
                    crate::tui::screens::results::compute_stats_aggregates(&res.columns, &res.rows)
                } else {
                    (None, None)
                };
                let data_size_col_idx = res
                    .columns
                    .iter()
                    .position(|c| c.eq_ignore_ascii_case("data_size"));
                let row_count_col_idx = res
                    .columns
                    .iter()
                    .position(|c| c.eq_ignore_ascii_case("row_count"));

                let row_visual_heights: Vec<u16> = res
                    .rows
                    .iter()
                    .map(|row| {
                        let is_summary = is_stats
                            && crate::tui::screens::results::is_stats_summary_row(&res.columns, row);
                        crate::tui::screens::results::compute_row_height(
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

                let has_h_scrollbar = res.columns.len() > 1;
                let table_reserved_rows = if has_h_scrollbar { 3 } else { 2 };
                let viewport_lines =
                    (inner.height as usize).saturating_sub(table_reserved_rows).max(1);

                let effective_scroll_v =
                    crate::tui::screens::results::calculate_effective_scroll_v(
                        res.scroll_v.get(),
                        res.selected_row,
                        res.last_selected_row.get(),
                        viewport_lines,
                        &row_visual_heights,
                        res.rows.len(),
                    );

                if start_row <= inner.y + 1 && end_row >= inner.y {
                    let header = sel_cols
                        .iter()
                        .map(|&i| res.columns.get(i).map(|s| s.as_str()).unwrap_or(""))
                        .collect::<Vec<_>>()
                        .join("\t");
                    lines.push(header);
                }

                let mut cur_y = inner.y + 2;
                for (row_idx, row) in res.rows.iter().enumerate().skip(effective_scroll_v) {
                    let h = row_visual_heights.get(row_idx).copied().unwrap_or(1);
                    if cur_y <= end_row && cur_y + h > start_row {
                        let row_line = sel_cols
                            .iter()
                            .map(|&i| row.get(i).map(|s| s.as_str()).unwrap_or(""))
                            .collect::<Vec<_>>()
                            .join("\t");
                        lines.push(row_line);
                    }
                    cur_y += h;
                    if cur_y >= inner.y + inner.height {
                        break;
                    }
                }
                return lines.join("\n");
            }
        }
    }

    String::new()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ConnectionConfig;

    fn sample_app() -> App {
        App::new(
            ConnectionConfig {
                url: "http://localhost:8080".to_string(),
                user: "admin".to_string(),
                password: "".to_string(),
            },
            false,
        )
    }

    #[test]
    fn test_extract_selected_text_catalog_screen() {
        let mut app = sample_app();
        app.clear_mouse_selection();
        app.screen = Screen::Catalog(CatalogState {
            items: vec![
                "iceberg".to_string(),
                "tpch".to_string(),
                "system".to_string(),
            ],
            selected: 0,
        });

        let text = extract_selected_text(&app, (5, 4), (5, 5));
        assert_eq!(text, "iceberg\ntpch");
    }

    #[test]
    fn test_extract_selected_text_query_inspector() {
        let mut app = sample_app();
        let term_height = crossterm::terminal::size().unwrap_or((80, 24)).1;
        let bottom_y = term_height.saturating_sub(7);

        app.add_query_log("SELECT 1".to_string());
        app.add_query_log("SELECT 2".to_string());

        let text = extract_selected_text(&app, (10, bottom_y + 1), (10, bottom_y + 1));
        assert_eq!(text, "SELECT 2");
    }

    #[test]
    fn test_extract_selected_text_actions_results() {
        let mut app = sample_app();
        let res_state = ResultsState {
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
            filters: Vec::new(),
            total_count: None,
            total_count_loading: false,
            table_count: None,
            ..Default::default()
        };

        app.clear_mouse_selection();
        app.screen = Screen::Actions(ActionState {
            catalog: "iceberg".to_string(),
            schema: "sales".to_string(),
            table: "orders".to_string(),
            selected: 0,
            query_buffer: "SELECT * FROM orders".to_string(),
            query_cursor: 20,
            results: Some(res_state),
            ..Default::default()
        });

        let term_width = crossterm::terminal::size().unwrap_or((80, 24)).0;
        let menu_x = term_width / 2; // Right side

        // Header line (preview_inner_y = 4) and first row (preview_inner_y + 2 = 6)
        let text = extract_selected_text(&app, (menu_x, 4), (menu_x, 6));
        assert_eq!(text, "id\tstatus\n1\tOK");
    }

    #[test]
    fn test_extract_selected_text_partitions() {
        let mut app = sample_app();
        app.partition_tree_lines = vec![
            " s3a://local-minio-bucket/lakehouse/".to_string(),
            " ├── date=2024-01-01/".to_string(),
            " └── date=2024-01-02/".to_string(),
        ];
        app.clear_mouse_selection();
        app.screen = Screen::Actions(ActionState {
            catalog: "iceberg".to_string(),
            schema: "sales".to_string(),
            table: "orders".to_string(),
            selected: ACTIONS
                .iter()
                .position(|(_, _, a)| *a == crate::app::Action::Partitions)
                .unwrap(),
            query_buffer: "".to_string(),
            query_cursor: 0,
            results: None,
            ..Default::default()
        });

        let term_width = crossterm::terminal::size().unwrap_or((80, 24)).0;
        let right_x = term_width / 2;

        let text = extract_selected_text(&app, (right_x, 4), (right_x, 6));
        assert_eq!(
            text,
            " s3a://local-minio-bucket/lakehouse/\n ├── date=2024-01-01/\n └── date=2024-01-02/"
        );
    }

    #[test]
    fn test_mouse_click_selects_cell_not_entire_line() {
        let mut app = sample_app();
        let res_state = ResultsState {
            query_buffer: "SELECT * FROM orders".to_string(),
            query_cursor: 20,
            columns: vec!["id".to_string(), "status".to_string()],
            rows: vec![
                vec!["1".to_string(), "OK".to_string()],
                vec!["2".to_string(), "PENDING".to_string()],
                vec!["3".to_string(), "FAILED".to_string()],
            ],
            scroll_v: std::cell::Cell::new(0),
            scroll_h: std::cell::Cell::new(0),
            selected_row: 0,
            selected_col: 0,
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
            ..Default::default()
        };

        app.clear_mouse_selection();
        app.screen = Screen::Actions(ActionState {
            catalog: "iceberg".to_string(),
            schema: "sales".to_string(),
            table: "orders".to_string(),
            selected: 0,
            query_buffer: "SELECT * FROM orders".to_string(),
            query_cursor: 20,
            results: Some(res_state),
            ..Default::default()
        });

        let (term_width, term_height) = crossterm::terminal::size().unwrap_or((80, 24));
        let inner = results_table_inner_rect(&app, term_width, term_height).expect("inner rect");

        // Row 1 click at (inner.x + 2, inner.y + 3)
        // Header is at inner.y, bottom margin is inner.y + 1, row 0 is inner.y + 2, row 1 is inner.y + 3
        let click_event = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: inner.x + 2,
            row: inner.y + 3,
            modifiers: KeyModifiers::NONE,
        };

        handle_mouse_sync(&mut app, click_event);

        if let Screen::Actions(a) = &app.screen {
            let res = a.results.as_ref().unwrap();
            assert_eq!(res.selected_row, 1, "Clicking row 1 must set selected_row to 1");
            assert_eq!(res.selected_col, 0, "Clicking column 0 must set selected_col to 0");
        } else {
            panic!("Expected Screen::Actions");
        }

        // Must NOT set is_selecting_text (no blue line highlight)
        assert!(!app.is_selecting_text, "A single click must not start text selection");

        // MouseUp event without drag
        let up_event = MouseEvent {
            kind: MouseEventKind::Up(MouseButton::Left),
            column: inner.x + 2,
            row: inner.y + 3,
            modifiers: KeyModifiers::NONE,
        };
        handle_mouse_sync(&mut app, up_event);
        assert!(app.copied_toast.is_none(), "A single click must not copy row to clipboard");
    }

    #[test]
    fn test_mouse_scroll_down_and_up_in_table_view() {
        let mut app = App::new(crate::config::ConnectionConfig::default(), false);
        let rows: Vec<Vec<String>> = (0..50)
            .map(|i| vec![format!("id_{i}"), format!("val_{i}")])
            .collect();
        let res_state = ResultsState {
            columns: vec!["id".to_string(), "val".to_string()],
            rows,
            scroll_v: std::cell::Cell::new(0),
            scroll_h: std::cell::Cell::new(0),
            selected_row: 0,
            selected_col: 0,
            is_paginated: true,
            ..Default::default()
        };
        app.screen = Screen::Actions(ActionState {
            catalog: "iceberg".to_string(),
            schema: "sales".to_string(),
            table: "orders".to_string(),
            selected: 0,
            results: Some(res_state),
            ..Default::default()
        });

        let scroll_down = MouseEvent {
            kind: MouseEventKind::ScrollDown,
            column: 10,
            row: 10,
            modifiers: KeyModifiers::NONE,
        };

        // Scroll down 4 times
        for _ in 0..4 {
            handle_mouse_sync(&mut app, scroll_down);
        }

        if let Screen::Actions(a) = &app.screen {
            let res = a.results.as_ref().unwrap();
            assert_eq!(res.scroll_v.get(), 12, "4 scroll down events must advance scroll_v by 12");
            assert!(res.selected_row >= 12, "selected_row must stay within visible viewport");
        } else {
            panic!("Expected Screen::Actions");
        }

        let scroll_up = MouseEvent {
            kind: MouseEventKind::ScrollUp,
            column: 10,
            row: 10,
            modifiers: KeyModifiers::NONE,
        };

        // Scroll up 2 times
        for _ in 0..2 {
            handle_mouse_sync(&mut app, scroll_up);
        }

        if let Screen::Actions(a) = &app.screen {
            let res = a.results.as_ref().unwrap();
            assert_eq!(res.scroll_v.get(), 6, "2 scroll up events must retreat scroll_v by 6");
        } else {
            panic!("Expected Screen::Actions");
        }
    }

    #[test]
    fn test_mouse_scroll_in_drilldown() {
        let mut app = App::new(crate::config::ConnectionConfig::default(), false);
        let items: Vec<String> = (0..20).map(|i| format!("partition_{i}")).collect();
        let dd = DrillDownState {
            partition_cols: vec!["dt".to_string()],
            path: Vec::new(),
            levels_cache: vec![items],
            selected: 0,
            ..Default::default()
        };
        app.screen = Screen::Actions(ActionState {
            catalog: "iceberg".to_string(),
            schema: "sales".to_string(),
            table: "orders".to_string(),
            selected: 0,
            drilldown: Some(dd),
            results: None,
            ..Default::default()
        });

        let scroll_down = MouseEvent {
            kind: MouseEventKind::ScrollDown,
            column: 10,
            row: 10,
            modifiers: KeyModifiers::NONE,
        };
        handle_mouse_sync(&mut app, scroll_down);

        if let Screen::Actions(a) = &app.screen {
            let dd = a.drilldown.as_ref().unwrap();
            assert_eq!(dd.selected, 3, "ScrollDown in drilldown must advance selected by 3");
        }

        let scroll_up = MouseEvent {
            kind: MouseEventKind::ScrollUp,
            column: 10,
            row: 10,
            modifiers: KeyModifiers::NONE,
        };
        handle_mouse_sync(&mut app, scroll_up);

        if let Screen::Actions(a) = &app.screen {
            let dd = a.drilldown.as_ref().unwrap();
            assert_eq!(dd.selected, 0, "ScrollUp in drilldown must retreat selected by 3");
        }
    }
}
