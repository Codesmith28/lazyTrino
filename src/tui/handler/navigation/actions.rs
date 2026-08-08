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
    s.results = Some(ResultsState {
        query_buffer: String::new(),
        query_cursor: 0,
        columns: vec!["Create Table".to_string()],
        rows: vec![vec![ddl_text]],
        scroll_v: 0,
        scroll_h: 0,
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
            if app.active_panel != ActivePanel::MainViewer {
                app.mouse_selection_anchor = None;
                app.mouse_selection_current = None;
            }
            app.active_panel = ActivePanel::MainViewer;
            return None;
        }
        if s.ddl_loading && matches!(action, Action::TableView | Action::TableDDL) {
            s.selected = action_idx;
            if app.active_panel != ActivePanel::MainViewer {
                app.mouse_selection_anchor = None;
                app.mouse_selection_current = None;
            }
            app.active_panel = ActivePanel::MainViewer;
            return None;
        }

        s.selected = action_idx;
        if app.active_panel != ActivePanel::MainViewer {
            app.mouse_selection_anchor = None;
            app.mouse_selection_current = None;
        }
        app.active_panel = ActivePanel::MainViewer;
        match action {
            Action::Partitions | Action::Schema => None,
            Action::TableView => {
                if s.drilldown.is_some() || s.results.is_some() {
                    return None;
                }
                let partition_cols = s
                    .metadata
                    .as_ref()
                    .map(|m| m.partitioned_by.clone())
                    .unwrap_or_default();
                if partition_cols.is_empty() {
                    let query = action.build_query(&s.catalog, &s.schema, &s.table);
                    s.results = None;
                    s.drilldown = None;
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
                    s.results = None;
                    s.drilldown = Some(DrillDownState {
                        partition_cols,
                        path: Vec::new(),
                        levels_cache: Vec::new(),
                        selected: 0,
                        loading: true,
                        error: None,
                        truncated: false,
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
                populate_table_ddl_results(s);
                None
            }
            _ => {
                let query = action.build_query(&s.catalog, &s.schema, &s.table);
                s.results = None;
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

    if let KeyCode::Char(c) = code
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
                    None
                }
                KeyCode::Char('k') | KeyCode::Up => {
                    s.selected = if s.selected == 0 {
                        ACTIONS.len() - 1
                    } else {
                        s.selected - 1
                    };
                    s.results = None;
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
                            None
                        }
                        KeyCode::Char('k') | KeyCode::Up => {
                            if let Some(dd) = s.drilldown.as_mut() {
                                dd.selected = dd.selected.saturating_sub(1);
                            }
                            None
                        }
                        KeyCode::Char('g') => {
                            if let Some(dd) = s.drilldown.as_mut() {
                                dd.selected = 0;
                            }
                            None
                        }
                        KeyCode::Char('G') => {
                            if let Some(dd) = s.drilldown.as_mut() {
                                let depth = dd.depth();
                                if let Some(items) = dd.levels_cache.get(depth) {
                                    dd.selected = items.len().saturating_sub(1);
                                }
                            }
                            None
                        }
                        KeyCode::Enter | KeyCode::Char('l') | KeyCode::Right => {
                            drilldown_drill_into_selected(s, &safe_columns)
                        }
                        KeyCode::Char('h') | KeyCode::Left => {
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
                    match code {
                        KeyCode::Char('j') | KeyCode::Down => {
                            if s.selected == 6 {
                                let max_lines = app.partition_tree_lines.len().saturating_sub(1);
                                app.partition_scroll = (app.partition_scroll + 1).min(max_lines);
                            } else if s.selected == 7 {
                                let max_cols = app.vertical_schema_cols.len().saturating_sub(1);
                                app.schema_scroll = (app.schema_scroll + 1).min(max_cols);
                            } else if let Some(ref mut res) = s.results {
                                if !res.rows.is_empty() {
                                    res.scroll_v =
                                        (res.scroll_v + 1).min(res.rows.len().saturating_sub(1));
                                }
                                return check_trigger_infinite_scroll(app);
                            }
                            None
                        }
                        KeyCode::Char('k') | KeyCode::Up => {
                            if s.selected == 6 {
                                app.partition_scroll = app.partition_scroll.saturating_sub(1);
                            } else if s.selected == 7 {
                                app.schema_scroll = app.schema_scroll.saturating_sub(1);
                            } else if let Some(ref mut res) = s.results {
                                res.scroll_v = res.scroll_v.saturating_sub(1);
                            }
                            None
                        }
                        KeyCode::Char('l') | KeyCode::Right => None,
                        KeyCode::Char('<') => {
                            if let Some(ref mut res) = s.results {
                                res.scroll_h = res.scroll_h.saturating_sub(1);
                            }
                            None
                        }
                        KeyCode::Char('>') => {
                            if let Some(ref mut res) = s.results
                                && !res.columns.is_empty()
                            {
                                res.scroll_h =
                                    (res.scroll_h + 1).min(res.columns.len().saturating_sub(1));
                            }
                            None
                        }
                        KeyCode::Char('h') | KeyCode::Left => {
                            if at_leaf {
                                drilldown_go_up(s);
                            }
                            None
                        }
                        KeyCode::Esc => {
                            app.set_active_panel(ActivePanel::MenuPane);
                            None
                        }
                        KeyCode::Char('g') => {
                            if s.selected == 6 {
                                app.partition_scroll = 0;
                            } else if s.selected == 7 {
                                app.schema_scroll = 0;
                            } else if let Some(ref mut res) = s.results {
                                res.scroll_v = 0;
                                res.scroll_h = 0;
                            }
                            None
                        }
                        KeyCode::Char('G') => {
                            if s.selected == 6 {
                                app.partition_scroll =
                                    app.partition_tree_lines.len().saturating_sub(1);
                            } else if s.selected == 7 {
                                app.schema_scroll =
                                    app.vertical_schema_cols.len().saturating_sub(1);
                            } else if let Some(ref mut res) = s.results {
                                res.scroll_v = res.rows.len().saturating_sub(1);
                                return check_trigger_infinite_scroll(app);
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
