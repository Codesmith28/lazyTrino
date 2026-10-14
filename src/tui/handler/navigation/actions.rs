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

use crossterm::event::{KeyCode, KeyEvent};

use crate::app::{
    ACTIONS, Action, ActionState, ActivePanel, App, DrillDownState, Mode, ResultsState, Screen,
};
use crate::tui::handler::{Command, export};

use super::clipboard::copy_active_pane_content;
use super::drilldown::{drilldown_drill_into_selected, drilldown_go_up};
use super::list::check_trigger_infinite_scroll;
use super::normalize_key_code;

pub fn populate_table_ddl_results(s: &mut ActionState) {
    let Some(meta) = s.metadata.as_ref() else {
        return;
    };
    let ddl_text = meta.ddl_text.clone();
    let ddl_query = Action::TableDDL.build_query(&s.catalog, &s.schema, &s.table);
    let ddl_query_len = ddl_query.len();
    s.query_buffer = ddl_query.clone();
    s.query_cursor = ddl_query_len;
    s.results = Some(ResultsState {
        query_buffer: ddl_query,
        query_cursor: ddl_query_len,
        columns: vec!["Create Table".to_string()],
        rows: vec![vec![ddl_text]],
        scroll_v: std::cell::Cell::new(0),
        scroll_h: std::cell::Cell::new(0),
        loading: false,
        error: None,
        is_paginated: false,
        catalog: s.catalog.clone(),
        schema: s.schema.clone(),
        table: s.table.clone(),
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
    });
}

pub fn trigger_action(app: &mut App, action_idx: usize) -> Option<Command> {
    if action_idx >= ACTIONS.len() {
        return None;
    }
    if let Screen::Actions(ref mut s) = app.screen {
        let action = &ACTIONS[action_idx].2;

        if s.metadata_loading && matches!(action, Action::Partitions | Action::Schema) {
            s.selected = action_idx;
            s.results = None;
            s.drilldown = None;
            let query = action.build_query(&s.catalog, &s.schema, &s.table);
            s.query_cursor = query.len();
            s.query_buffer = query;
            if app.active_panel != ActivePanel::MainViewer {
                app.mouse_selection_anchor = None;
                app.mouse_selection_current = None;
            }
            app.active_panel = ActivePanel::MainViewer;
            return None;
        }
        if s.ddl_loading && matches!(action, Action::TableView | Action::TableDDL) {
            s.selected = action_idx;
            s.results = None;
            s.drilldown = None;
            let query = action.build_query(&s.catalog, &s.schema, &s.table);
            s.query_cursor = query.len();
            s.query_buffer = query;
            if app.active_panel != ActivePanel::MainViewer {
                app.mouse_selection_anchor = None;
                app.mouse_selection_current = None;
            }
            app.active_panel = ActivePanel::MainViewer;
            return None;
        }

        let prev_selected = s.selected;
        s.selected = action_idx;
        if app.active_panel != ActivePanel::MainViewer {
            app.mouse_selection_anchor = None;
            app.mouse_selection_current = None;
        }
        app.active_panel = ActivePanel::MainViewer;
        match action {
            Action::Partitions | Action::Schema => {
                s.results = None;
                s.drilldown = None;
                let query = action.build_query(&s.catalog, &s.schema, &s.table);
                s.query_cursor = query.len();
                s.query_buffer = query;
                None
            }
            Action::TableView => {
                let is_same_table_view = prev_selected == action_idx
                    && (s.drilldown.is_some()
                        || s.results.as_ref().is_some_and(|r| r.is_paginated));
                if is_same_table_view {
                    return None;
                }
                s.results = None;
                s.drilldown = None;
                let partition_cols = s
                    .metadata
                    .as_ref()
                    .map(|m| m.partitioned_by.clone())
                    .unwrap_or_default();
                if partition_cols.is_empty() {
                    let query = action.build_query(&s.catalog, &s.schema, &s.table);
                    let query_len = query.len();
                    s.query_buffer = query.clone();
                    s.query_cursor = query_len;
                    Some(Command::ExecuteQuery {
                        query,
                        is_paginated: true,
                        catalog: s.catalog.clone(),
                        schema: s.schema.clone(),
                        table: s.table.clone(),
                        filters: Vec::new(),
                    })
                } else {
                    let first_col = partition_cols[0].clone();
                    let query = crate::trino::queries::distinct_partition_values(
                        &s.catalog,
                        &s.schema,
                        &s.table,
                        &[],
                        &first_col,
                        crate::tui::handler::commands::DRILLDOWN_LEVEL_LIMIT,
                    );
                    let query_len = query.len();
                    s.query_buffer = query;
                    s.query_cursor = query_len;
                    s.drilldown = Some(DrillDownState {
                        partition_cols,
                        path: Vec::new(),
                        levels_cache: Vec::new(),
                        selected: 0,
                        loading: true,
                        error: None,
                        truncated: false,
                        table_count: None,
                        table_count_loading: true,
                        counts_cache: std::collections::HashMap::new(),
                        current_count_loading: true,
                    });
                    Some(Command::FetchPartitionLevel {
                        catalog: s.catalog.clone(),
                        schema: s.schema.clone(),
                        table: s.table.clone(),
                        filters: Vec::new(),
                        column: first_col,
                    })
                }
            }
            Action::TableDDL if s.metadata.is_some() => {
                s.drilldown = None;
                populate_table_ddl_results(s);
                None
            }
            _ => {
                let query = action.build_query(&s.catalog, &s.schema, &s.table);
                s.results = None;
                s.drilldown = None;
                let query_len = query.len();
                s.query_buffer = query.clone();
                s.query_cursor = query_len;
                Some(Command::ExecuteQuery {
                    query,
                    is_paginated: false,
                    catalog: s.catalog.clone(),
                    schema: s.schema.clone(),
                    table: s.table.clone(),
                    filters: Vec::new(),
                })
            }
        }
    } else {
        None
    }
}

pub fn actions_keys(app: &mut App, key: KeyEvent) -> Option<Command> {
    let code = normalize_key_code(key.code);
    let is_ctrl = key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL);
    let is_shift = key.modifiers.contains(crossterm::event::KeyModifiers::SHIFT);
    let is_alt = key.modifiers.contains(crossterm::event::KeyModifiers::ALT);

    if !is_ctrl
        && let KeyCode::Char(c) = code
        && let Some(pos) = ACTIONS.iter().position(|(k, _, _)| *k == c)
    {
        return trigger_action(app, pos);
    }

    let safe_columns = crate::app::safe_select_columns(&app.vertical_schema_cols);
    if let Screen::Actions(ref mut s) = app.screen {
        match app.active_panel {
            ActivePanel::MenuPane => match code {
                KeyCode::Char('j') | KeyCode::Down => {
                    s.selected = (s.selected + 1) % ACTIONS.len();
                    s.results = None;
                    s.drilldown = None;
                    let action = &ACTIONS[s.selected].2;
                    let query = action.build_query(&s.catalog, &s.schema, &s.table);
                    s.query_cursor = query.len();
                    s.query_buffer = query;
                    None
                }
                KeyCode::Char('k') | KeyCode::Up => {
                    s.selected = if s.selected == 0 {
                        ACTIONS.len() - 1
                    } else {
                        s.selected - 1
                    };
                    s.results = None;
                    s.drilldown = None;
                    let action = &ACTIONS[s.selected].2;
                    let query = action.build_query(&s.catalog, &s.schema, &s.table);
                    s.query_cursor = query.len();
                    s.query_buffer = query;
                    None
                }
                KeyCode::Enter | KeyCode::Char('l') | KeyCode::Right => {
                    let idx = s.selected;
                    trigger_action(app, idx)
                }
                KeyCode::Char('h') | KeyCode::Left | KeyCode::Esc => {
                    super::go_back(app);
                    None
                }
                KeyCode::Char('y') => {
                    copy_active_pane_content(app);
                    None
                }
                KeyCode::Char('C') => {
                    if s.results.is_some() {
                        crate::tui::handler::column_modal::open_column_modal(app);
                    }
                    None
                }
                KeyCode::Char('S') => {
                    if let Some(ref mut res) = s.results {
                        res.toggle_sort(&app.vertical_schema_cols);
                    }
                    None
                }
                _ => None,
            },
            ActivePanel::MainViewer => {
                let in_drilldown = s.drilldown.is_some()
                    && s.selected < ACTIONS.len()
                    && matches!(ACTIONS[s.selected].2, Action::TableView);
                let at_leaf =
                    in_drilldown && s.drilldown.as_ref().map(|d| d.is_leaf()).unwrap_or(false);

                if in_drilldown && !at_leaf {
                    match code {
                        KeyCode::Char('j') | KeyCode::Down => {
                            if let Some(dd) = s.drilldown.as_mut() {
                                let depth = dd.depth();
                                if let Some(items) = dd.levels_cache.get(depth)
                                    && !items.is_empty()
                                {
                                    dd.selected = (dd.selected + 1).min(items.len() - 1);
                                }
                            }
                            maybe_fetch_selected_partition_count(s)
                        }
                        KeyCode::Char('k') | KeyCode::Up => {
                            if let Some(dd) = s.drilldown.as_mut() {
                                dd.selected = dd.selected.saturating_sub(1);
                            }
                            maybe_fetch_selected_partition_count(s)
                        }
                        KeyCode::PageDown | KeyCode::Char('d') | KeyCode::Char('f')
                            if code == KeyCode::PageDown || is_ctrl =>
                        {
                            if let Some(dd) = s.drilldown.as_mut() {
                                let depth = dd.depth();
                                if let Some(items) = dd.levels_cache.get(depth)
                                    && !items.is_empty()
                                {
                                    dd.selected =
                                        (dd.selected + 15).min(items.len().saturating_sub(1));
                                }
                            }
                            maybe_fetch_selected_partition_count(s)
                        }
                        KeyCode::PageUp | KeyCode::Char('u') | KeyCode::Char('b')
                            if code == KeyCode::PageUp || is_ctrl =>
                        {
                            if let Some(dd) = s.drilldown.as_mut() {
                                dd.selected = dd.selected.saturating_sub(15);
                            }
                            maybe_fetch_selected_partition_count(s)
                        }
                        KeyCode::Char('g') | KeyCode::Home | KeyCode::Char('0') => {
                            if let Some(dd) = s.drilldown.as_mut() {
                                dd.selected = 0;
                            }
                            maybe_fetch_selected_partition_count(s)
                        }
                        KeyCode::Char('G') | KeyCode::End | KeyCode::Char('$') => {
                            if let Some(dd) = s.drilldown.as_mut() {
                                let depth = dd.depth();
                                if let Some(items) = dd.levels_cache.get(depth) {
                                    dd.selected = items.len().saturating_sub(1);
                                }
                            }
                            maybe_fetch_selected_partition_count(s)
                        }
                        KeyCode::Char('>') | KeyCode::Enter | KeyCode::Right => {
                            drilldown_drill_into_selected(s, &safe_columns)
                        }
                        KeyCode::Char('<') | KeyCode::Left => {
                            drilldown_go_up(s);
                            None
                        }
                        KeyCode::Char('y') => {
                            copy_active_pane_content(app);
                            None
                        }
                        _ => None,
                    }
                } else {
                    let is_partitions = matches!(
                        ACTIONS.get(s.selected).map(|(_, _, a)| a),
                        Some(Action::Partitions)
                    );
                    let is_schema = matches!(
                        ACTIONS.get(s.selected).map(|(_, _, a)| a),
                        Some(Action::Schema)
                    );
                    match code {
                        KeyCode::Char('j') | KeyCode::Down => {
                            if is_partitions {
                                let max_lines = app.partition_tree_lines.len().saturating_sub(1);
                                app.partition_scroll = (app.partition_scroll + 1).min(max_lines);
                            } else if is_schema {
                                let max_cols = app.vertical_schema_cols.len().saturating_sub(1);
                                app.schema_scroll = (app.schema_scroll + 1).min(max_cols);
                            } else if let Some(ref mut res) = s.results {
                                if !res.rows.is_empty() {
                                    res.selected_row =
                                        (res.selected_row + 1).min(res.rows.len().saturating_sub(1));
                                }
                                return check_trigger_infinite_scroll(app);
                            }
                            None
                        }
                        KeyCode::Char('k') | KeyCode::Up => {
                            if is_partitions {
                                app.partition_scroll = app.partition_scroll.saturating_sub(1);
                            } else if is_schema {
                                app.schema_scroll = app.schema_scroll.saturating_sub(1);
                            } else if let Some(ref mut res) = s.results {
                                res.selected_row = res.selected_row.saturating_sub(1);
                            }
                            None
                        }
                        KeyCode::PageDown | KeyCode::Char('d') | KeyCode::Char('f')
                            if code == KeyCode::PageDown || is_ctrl =>
                        {
                            if is_partitions {
                                let max_lines = app.partition_tree_lines.len().saturating_sub(1);
                                app.partition_scroll = (app.partition_scroll + 15).min(max_lines);
                            } else if is_schema {
                                let max_cols = app.vertical_schema_cols.len().saturating_sub(1);
                                app.schema_scroll = (app.schema_scroll + 15).min(max_cols);
                            } else if let Some(ref mut res) = s.results {
                                if !res.rows.is_empty() {
                                    res.selected_row =
                                        (res.selected_row + 15).min(res.rows.len().saturating_sub(1));
                                }
                                return check_trigger_infinite_scroll(app);
                            }
                            None
                        }
                        KeyCode::PageUp | KeyCode::Char('u') | KeyCode::Char('b')
                            if code == KeyCode::PageUp || is_ctrl =>
                        {
                            if is_partitions {
                                app.partition_scroll = app.partition_scroll.saturating_sub(15);
                            } else if is_schema {
                                app.schema_scroll = app.schema_scroll.saturating_sub(15);
                            } else if let Some(ref mut res) = s.results {
                                res.selected_row = res.selected_row.saturating_sub(15);
                            }
                            None
                        }
                        KeyCode::Char('h') | KeyCode::Char('H') | KeyCode::Left => {
                            if let Some(ref mut res) = s.results {
                                let step = if is_ctrl || is_shift || is_alt || code == KeyCode::Char('H') {
                                    5
                                } else {
                                    1
                                };
                                for _ in 0..step {
                                    res.selected_col = res.prev_visible_col();
                                }
                            }
                            None
                        }
                        KeyCode::Char('l') | KeyCode::Char('L') | KeyCode::Right => {
                            if let Some(ref mut res) = s.results
                                && !res.columns.is_empty()
                            {
                                let step = if is_ctrl || is_shift || is_alt || code == KeyCode::Char('L') {
                                    5
                                } else {
                                    1
                                };
                                for _ in 0..step {
                                    res.selected_col = res.next_visible_col();
                                }
                            }
                            None
                        }
                        KeyCode::Char('[') | KeyCode::Char(',') => {
                            if let Some(ref mut res) = s.results {
                                res.selected_col = res.prev_visible_col();
                            }
                            None
                        }
                        KeyCode::Char(']') | KeyCode::Char('.') => {
                            if let Some(ref mut res) = s.results
                                && !res.columns.is_empty()
                            {
                                res.selected_col = res.next_visible_col();
                            }
                            None
                        }
                        KeyCode::Home | KeyCode::Char('0') => {
                            if is_partitions {
                                app.partition_scroll = 0;
                            } else if is_schema {
                                app.schema_scroll = 0;
                            } else if let Some(ref mut res) = s.results {
                                res.selected_col = res.first_visible_col();
                                res.scroll_h.set(0);
                            }
                            None
                        }
                        KeyCode::End | KeyCode::Char('$') => {
                            if is_partitions {
                                app.partition_scroll =
                                    app.partition_tree_lines.len().saturating_sub(1);
                            } else if is_schema {
                                app.schema_scroll =
                                    app.vertical_schema_cols.len().saturating_sub(1);
                            } else if let Some(ref mut res) = s.results {
                                res.selected_col = res.last_visible_col();
                            }
                            None
                        }
                        KeyCode::Char('<') => {
                            if at_leaf {
                                drilldown_go_up(s);
                            } else if let Some(ref mut res) = s.results {
                                for _ in 0..5 {
                                    res.selected_col = res.prev_visible_col();
                                }
                            }
                            None
                        }
                        KeyCode::Char('>') => {
                            if let Some(ref mut res) = s.results
                                && !res.columns.is_empty()
                            {
                                for _ in 0..5 {
                                    res.selected_col = res.next_visible_col();
                                }
                            }
                            None
                        }
                        KeyCode::Esc => {
                            app.set_active_panel(ActivePanel::MenuPane);
                            None
                        }
                        KeyCode::Char('g') => {
                            if is_partitions {
                                app.partition_scroll = 0;
                            } else if is_schema {
                                app.schema_scroll = 0;
                            } else if let Some(ref mut res) = s.results {
                                res.selected_row = 0;
                                res.scroll_v.set(0);
                                res.selected_col = res.first_visible_col();
                                res.scroll_h.set(0);
                            }
                            None
                        }
                        KeyCode::Char('G') => {
                            if is_partitions {
                                app.partition_scroll =
                                    app.partition_tree_lines.len().saturating_sub(1);
                            } else if is_schema {
                                app.schema_scroll =
                                    app.vertical_schema_cols.len().saturating_sub(1);
                            } else if let Some(ref mut res) = s.results {
                                res.selected_row = res.rows.len().saturating_sub(1);
                                return check_trigger_infinite_scroll(app);
                            }
                            None
                        }
                        KeyCode::Char('F') | KeyCode::Char('f') => {
                            if s.results.is_some() {
                                crate::tui::handler::filter_modal::open_filter_modal(app);
                            }
                            None
                        }
                        KeyCode::Char('C') => {
                            if s.results.is_some() {
                                crate::tui::handler::column_modal::open_column_modal(app);
                            }
                            None
                        }
                        KeyCode::Char('S') | KeyCode::Char('s') => {
                            if let Some(ref mut res) = s.results {
                                res.toggle_sort(&app.vertical_schema_cols);
                            }
                            None
                        }
                        KeyCode::Char('q') | KeyCode::Char(':') => {
                            if s.selected < ACTIONS.len()
                                && matches!(ACTIONS[s.selected].2, Action::TableView)
                            {
                                app.mode = Mode::QueryInput;
                            }
                            None
                        }
                        KeyCode::Char('y') => {
                            copy_active_pane_content(app);
                            None
                        }
                        KeyCode::Char('Y')
                            if s.selected < ACTIONS.len()
                                && !matches!(
                                    ACTIONS[s.selected].2,
                                    Action::Partitions | Action::Schema
                                ) =>
                        {
                            export::export_results_to_csv_file(app);
                            None
                        }
                        _ => None,
                    }
                }
            }
        }
    } else {
        None
    }
}

pub fn maybe_fetch_selected_partition_count(s: &ActionState) -> Option<Command> {
    let dd = s.drilldown.as_ref()?;
    let col = dd.next_column()?;
    let items = dd.levels_cache.get(dd.depth())?;
    let val = items.get(dd.selected)?;
    let mut f = dd.path.clone();
    f.push((col.to_string(), val.clone()));
    if !dd.counts_cache.contains_key(&f) {
        Some(Command::FetchPartitionCount {
            catalog: s.catalog.clone(),
            schema: s.schema.clone(),
            table: s.table.clone(),
            filters: f,
        })
    } else {
        None
    }
}
