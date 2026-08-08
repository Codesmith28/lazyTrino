use crossterm::event::{KeyModifiers, MouseButton, MouseEvent, MouseEventKind};

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
    let border_x = ((term_width as u32 * app.main_panel_pct as u32) / 100) as u16;
    let is_in_table = matches!(app.screen, Screen::Actions(_));

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
                    app.mode = Mode::QueryInput;
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
            app.is_selecting_text = true;

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
            if app.is_selecting_text {
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
            let shift = mouse.modifiers.contains(KeyModifiers::SHIFT);
            if is_in_table {
                let selected_idx = match &app.screen {
                    Screen::Actions(a) => a.selected,
                    _ => 0,
                };
                if selected_idx == 6 {
                    let max_lines = app.partition_tree_lines.len().saturating_sub(1);
                    app.partition_scroll = (app.partition_scroll + 1).min(max_lines);
                } else if selected_idx == 7 {
                    let max_cols = app.vertical_schema_cols.len().saturating_sub(1);
                    app.schema_scroll = (app.schema_scroll + 1).min(max_cols);
                } else if let Screen::Actions(ref mut a) = app.screen
                    && let Some(ref mut res) = a.results
                {
                    if shift {
                        if !res.columns.is_empty() {
                            res.scroll_h =
                                (res.scroll_h + 1).min(res.columns.len().saturating_sub(1));
                        }
                    } else if !res.rows.is_empty() {
                        res.scroll_v = (res.scroll_v + 1).min(res.rows.len().saturating_sub(1));
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
            let shift = mouse.modifiers.contains(KeyModifiers::SHIFT);
            if is_in_table {
                let selected_idx = match &app.screen {
                    Screen::Actions(a) => a.selected,
                    _ => 0,
                };
                if selected_idx == 6 {
                    app.partition_scroll = app.partition_scroll.saturating_sub(1);
                } else if selected_idx == 7 {
                    app.schema_scroll = app.schema_scroll.saturating_sub(1);
                } else if let Screen::Actions(ref mut a) = app.screen
                    && let Some(ref mut res) = a.results
                {
                    if shift {
                        res.scroll_h = res.scroll_h.saturating_sub(1);
                    } else {
                        res.scroll_v = res.scroll_v.saturating_sub(1);
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

        let search_active = matches!(app.mode, Mode::Search);
        let search_height = if search_active {
            let total_chars = 3 + app.search_query.len();
            let lines = total_chars.div_ceil(inner_w);
            (lines as u16 + 2).clamp(3, 8)
        } else {
            3
        };

        let query_active = matches!(app.mode, Mode::QueryInput);
        let query_height = if query_active {
            let total_chars = 7 + state
                .results
                .as_ref()
                .map(|r| r.query_buffer.len())
                .unwrap_or_else(|| state.query_buffer.len());
            let lines = total_chars.div_ceil(inner_w);
            (lines as u16 + 2).clamp(3, 4)
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
        let preview_inner_y = search_height + query_height + 1;

        if state.selected == 6 {
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
        } else if state.selected == 7 {
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
            for r in start_row..=end_row {
                if r == preview_inner_y {
                    // Header line
                    lines.push(res.columns.join("\t"));
                } else if r >= preview_inner_y + 2 {
                    let idx = (r - (preview_inner_y + 2)) as usize + res.scroll_v;
                    if idx < res.rows.len() {
                        lines.push(res.rows[idx].join("\t"));
                    }
                }
            }
            return lines.join("\n");
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
            filters: Vec::new(),
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

        // Header line (preview_inner_y = 7) and first row (preview_inner_y + 2 = 9)
        let text = extract_selected_text(&app, (menu_x, 7), (menu_x, 9));
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
            selected: 6, // Partitions action
            query_buffer: "".to_string(),
            query_cursor: 0,
            results: None,
            ..Default::default()
        });

        let term_width = crossterm::terminal::size().unwrap_or((80, 24)).0;
        let right_x = term_width / 2;

        let text = extract_selected_text(&app, (right_x, 7), (right_x, 9));
        assert_eq!(
            text,
            " s3a://local-minio-bucket/lakehouse/\n ├── date=2024-01-01/\n └── date=2024-01-02/"
        );
    }
}
