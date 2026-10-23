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

use crate::app::{ACTIONS, ActivePanel, App, SchemaState, Screen, TableState};
use crate::tui::handler::Command;

use super::clipboard::copy_active_pane_content;
use super::mod_list_selected;
use super::normalize_key_code;

pub fn check_trigger_infinite_scroll(app: &mut App) -> Option<Command> {
    if let Screen::Actions(ref mut action_state) = app.screen
        && let Some(state) = action_state.results.as_mut()
        && state.is_paginated
        && !state.is_fetching_next_page
        && state.has_more_rows
        && state.selected_row.max(state.scroll_v.get()) + 15 >= state.rows.len()
    {
        state.is_fetching_next_page = true;
        let offset = state.rows.len();
        return Some(Command::FetchNextPage {
            catalog: state.catalog.clone(),
            schema: state.schema.clone(),
            table: state.table.clone(),
            offset,
            limit: state.page_size,
            filters: state.filters.clone(),
        });
    }
    None
}

pub fn filter_items<'a>(items: &'a [String], search: &str) -> Vec<&'a String> {
    items
        .iter()
        .filter(|name| search.is_empty() || name.to_lowercase().contains(&search.to_lowercase()))
        .collect()
}

pub fn extract_list_labels(app: &App) -> Option<Vec<String>> {
    match &app.screen {
        Screen::Catalog(s) => Some(
            filter_items(&s.items, &app.search_query)
                .into_iter()
                .map(|x| x.trim().to_string())
                .collect(),
        ),
        Screen::Schema(s) => Some(
            filter_items(&s.items, &app.search_query)
                .into_iter()
                .map(|x| x.trim().to_string())
                .collect(),
        ),
        Screen::Table(s) => Some(
            filter_items(&s.items, &app.search_query)
                .into_iter()
                .map(|x| x.trim().to_string())
                .collect(),
        ),
        Screen::Actions(_) => Some(ACTIONS.iter().map(|(_, l, _)| l.to_string()).collect()),
        _ => None,
    }
}

pub fn get_selected_item_label(app: &App) -> Option<String> {
    match &app.screen {
        Screen::Catalog(s) => {
            let filtered = filter_items(&s.items, &app.search_query);
            if filtered.is_empty() {
                None
            } else {
                let idx = s.selected.min(filtered.len() - 1);
                Some(filtered[idx].trim().to_string())
            }
        }
        Screen::Schema(s) => {
            let filtered = filter_items(&s.items, &app.search_query);
            if filtered.is_empty() {
                None
            } else {
                let idx = s.selected.min(filtered.len() - 1);
                Some(filtered[idx].trim().to_string())
            }
        }
        Screen::Table(s) => {
            let filtered = filter_items(&s.items, &app.search_query);
            if filtered.is_empty() {
                None
            } else {
                let idx = s.selected.min(filtered.len() - 1);
                Some(filtered[idx].trim().to_string())
            }
        }
        _ => None,
    }
}

pub fn reset_list_selected_for_search(app: &mut App) {
    if let Some(items) = extract_list_labels(app) {
        if !items.is_empty() {
            if let Some(s) = super::get_selected(&app.screen)
                && s >= items.len()
            {
                mod_list_selected(&mut app.screen, items.len() - 1);
            }
        } else {
            mod_list_selected(&mut app.screen, 0);
        }
    }
}

pub fn update_number_buffer(app: &mut App, ch: char) {
    if ch.is_ascii_digit() {
        let mut buf = app.number_buffer.clone();
        buf.push(ch);
        let num: usize = buf.parse().unwrap_or(0);
        if let Some(items) = extract_list_labels(app) {
            if num <= items.len() && num > 0 {
                app.number_buffer = buf;
            } else {
                app.number_buffer = ch.to_string();
            }
        }
    }
}

pub fn jump_to_number(app: &mut App) {
    if app.number_buffer.is_empty() {
        return;
    }
    let num: usize = app.number_buffer.parse().unwrap_or(1);
    if let Some(items) = extract_list_labels(app)
        && num > 0
        && num <= items.len()
    {
        mod_list_selected(&mut app.screen, num - 1);
    }
    app.number_buffer.clear();
}

pub fn sync_list_selected_to_app(app: &mut App) {
    match &app.screen {
        Screen::Catalog(st) => app.last_selected_catalog = st.selected,
        Screen::Schema(st) => {
            app.last_selected_schema
                .insert(st.catalog.clone(), st.selected);
        }
        Screen::Table(st) => {
            app.last_selected_table
                .insert((st.catalog.clone(), st.schema.clone()), st.selected);
        }
        Screen::Actions(st) => {
            app.last_selected_action.insert(
                (st.catalog.clone(), st.schema.clone(), st.table.clone()),
                st.selected,
            );
        }
        _ => {}
    }
}

/// Opens `table` fresh on the table screen with `action_idx` selected; the
/// action runs once the table metadata it needs has arrived.
pub fn open_table(
    app: &mut App,
    catalog: String,
    schema: String,
    table: String,
    action_idx: usize,
) -> Command {
    app.partition_tree_lines.clear();
    app.vertical_schema_cols.clear();
    let action_idx = action_idx.min(ACTIONS.len() - 1);
    let default_query = ACTIONS[action_idx].2.build_query(&catalog, &schema, &table);
    let query_len = default_query.len();
    app.screen = Screen::Actions(crate::app::ActionState {
        catalog: catalog.clone(),
        schema: schema.clone(),
        table: table.clone(),
        selected: action_idx,
        pending_action: Some(action_idx),
        query_buffer: default_query,
        query_cursor: query_len,
        results: None,
        ddl_loading: true,
        metadata_loading: true,
        ..Default::default()
    });
    Command::FetchTableMetadata {
        catalog,
        schema,
        table,
    }
}

pub fn select_current_item(app: &mut App) -> Option<Command> {
    match &app.screen {
        Screen::Catalog(s) => {
            app.last_selected_catalog = s.selected;
            let catalog = get_selected_item_label(app)?;
            if app.schemas.contains_key(&catalog) {
                let items: Vec<String> = app.schemas[&catalog]
                    .iter()
                    .map(|x| x.trim().to_string())
                    .collect();
                let saved_sel = app.last_selected_schema.get(&catalog).copied().unwrap_or(0);
                let selected = saved_sel.min(items.len().saturating_sub(1));
                app.clear_mouse_selection();
                app.screen = Screen::Schema(SchemaState {
                    catalog,
                    items,
                    selected,
                });
                None
            } else {
                Some(Command::FetchSchemas { catalog })
            }
        }
        Screen::Schema(s) => {
            app.last_selected_schema
                .insert(s.catalog.clone(), s.selected);
            let schema = get_selected_item_label(app)?;
            let catalog = s.catalog.trim().to_string();
            if app.tables.contains_key(&(catalog.clone(), schema.clone())) {
                let items: Vec<String> = app.tables[&(catalog.clone(), schema.clone())]
                    .iter()
                    .map(|x| x.trim().to_string())
                    .collect();
                let saved_sel = app
                    .last_selected_table
                    .get(&(catalog.clone(), schema.clone()))
                    .copied()
                    .unwrap_or(0);
                let selected = saved_sel.min(items.len().saturating_sub(1));
                app.clear_mouse_selection();
                app.screen = Screen::Table(TableState {
                    catalog,
                    schema,
                    items,
                    selected,
                });
                None
            } else {
                Some(Command::FetchTables { catalog, schema })
            }
        }
        Screen::Table(s) => {
            app.last_selected_table
                .insert((s.catalog.clone(), s.schema.clone()), s.selected);
            let table = get_selected_item_label(app)?;
            let catalog = s.catalog.clone();
            let schema = s.schema.clone();
            app.main_panel_pct = 15;
            app.set_active_panel(ActivePanel::MenuPane);
            app.clear_mouse_selection();

            let key = (catalog.clone(), schema.clone(), table.clone());
            if let Some(cached_state) = app.cached_action_states.get(&key).cloned() {
                app.screen = Screen::Actions(cached_state);
                None
            } else {
                let selected_action = app.last_selected_action.get(&key).copied().unwrap_or(0);
                Some(open_table(app, catalog, schema, table, selected_action))
            }
        }
        Screen::Actions(s) => {
            let idx = s.selected;
            super::actions::trigger_action(app, idx)
        }
        _ => None,
    }
}

pub fn handle_list_navigation_keys(app: &mut App, key: KeyEvent) -> Option<Command> {
    let code = normalize_key_code(key.code);
    match code {
        KeyCode::Char('j') | KeyCode::Down => {
            if let Some(items) = extract_list_labels(app)
                && !items.is_empty()
                && let Some(s) = super::get_selected(&app.screen)
            {
                mod_list_selected(&mut app.screen, (s + 1).min(items.len() - 1));
                sync_list_selected_to_app(app);
            }
            None
        }
        KeyCode::Char('k') | KeyCode::Up => {
            if let Some(s) = super::get_selected(&app.screen) {
                mod_list_selected(&mut app.screen, s.saturating_sub(1));
                sync_list_selected_to_app(app);
            }
            None
        }
        KeyCode::Char('l') | KeyCode::Right | KeyCode::Enter => select_current_item(app),
        KeyCode::Char('h') | KeyCode::Left | KeyCode::Esc => {
            app.number_buffer.clear();
            super::go_back(app);
            None
        }
        KeyCode::Char('g') => {
            mod_list_selected(&mut app.screen, 0);
            sync_list_selected_to_app(app);
            None
        }
        KeyCode::Char('G') => {
            if let Some(items) = extract_list_labels(app)
                && !items.is_empty()
            {
                mod_list_selected(&mut app.screen, items.len() - 1);
                sync_list_selected_to_app(app);
            }
            None
        }
        KeyCode::Char('y') | KeyCode::Char('c') => {
            copy_active_pane_content(app);
            None
        }
        _ => None,
    }
}

pub fn catalog_keys(app: &mut App, key: KeyEvent) -> Option<Command> {
    handle_list_navigation_keys(app, key)
}

pub fn schema_keys(app: &mut App, key: KeyEvent) -> Option<Command> {
    handle_list_navigation_keys(app, key)
}

pub fn table_keys(app: &mut App, key: KeyEvent) -> Option<Command> {
    handle_list_navigation_keys(app, key)
}
