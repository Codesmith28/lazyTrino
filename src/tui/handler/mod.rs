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

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use tracing::info;

use crate::app::*;
use crate::trino::client::TrinoClient;
use crate::trino::error::TrinoClientError;

mod commands;
mod export;
mod mouse;
mod navigation;
mod query;

pub fn extract_from_tables(sql: &str) -> Vec<String> {
    query::extract_from_tables(sql)
}

pub fn validate_and_build_query(
    user_input: &str,
    catalog: &str,
    schema: &str,
    table: &str,
) -> Result<String, String> {
    query::validate_and_build_query(user_input, catalog, schema, table)
}

pub fn handle_pane_focus_keys(app: &mut App, key: KeyEvent) -> bool {
    navigation::handle_pane_focus_keys(app, key)
}

pub fn trigger_action(app: &mut App, action_idx: usize) -> Option<Command> {
    navigation::trigger_action(app, action_idx)
}

pub fn handle_mouse_sync(app: &mut App, mouse: crossterm::event::MouseEvent) -> Option<Command> {
    mouse::handle_mouse_sync(app, mouse)
}

pub fn extract_selected_text(app: &App, anchor: (u16, u16), current: (u16, u16)) -> String {
    mouse::extract_selected_text(app, anchor, current)
}

pub fn dispatch_command(
    app: &mut App,
    cmd: Command,
    tx: &tokio::sync::mpsc::UnboundedSender<AsyncResult>,
) {
    commands::dispatch_command(app, cmd, tx)
}

pub fn handle_async_result(app: &mut App, result: AsyncResult) {
    commands::handle_async_result(app, result)
}

#[inline(always)]
fn preserve_public_api_symbols() {
    let _ = extract_from_tables as fn(&str) -> Vec<String>;
    let _ = validate_and_build_query as fn(&str, &str, &str, &str) -> Result<String, String>;
    let _ = handle_pane_focus_keys as fn(&mut App, KeyEvent) -> bool;
    let _ = trigger_action as fn(&mut App, usize) -> Option<Command>;
    let _ = extract_selected_text as fn(&App, (u16, u16), (u16, u16)) -> String;
}

pub enum Command {
    Connect {
        url: String,
        user: String,
        password: String,
    },
    FetchSchemas {
        catalog: String,
    },
    FetchTables {
        catalog: String,
        schema: String,
    },
    FetchTableMetadata {
        catalog: String,
        schema: String,
        table: String,
    },
    ExecuteQuery {
        query: String,
        is_paginated: bool,
        catalog: String,
        schema: String,
        table: String,
        /// Partition predicates baked into `query`, carried along so the
        /// resulting `ResultsState` (and any subsequent `FetchNextPage`)
        /// keeps using them. Empty for ordinary, non-drill-down queries.
        filters: Vec<(String, String)>,
    },
    FetchNextPage {
        catalog: String,
        schema: String,
        table: String,
        offset: usize,
        limit: usize,
        filters: Vec<(String, String)>,
    },
    /// Fetches the distinct values of the next unfixed partition column in
    /// a cd/ls-style drill-down, scoped by the partition predicates already
    /// fixed by the levels above it (`filters`). Used only for tables whose
    /// `partitioned_by` (from live `SHOW CREATE TABLE` recon) is non-empty.
    FetchPartitionLevel {
        catalog: String,
        schema: String,
        table: String,
        filters: Vec<(String, String)>,
        column: String,
    },
}

#[derive(Debug)]
pub enum AsyncResult {
    Connect {
        log_id: usize,
        url: String,
        user: String,
        password: String,
        result: Result<(TrinoClient, Vec<String>), TrinoClientError>,
    },
    FetchSchemas {
        log_id: usize,
        catalog: String,
        result: Result<Vec<String>, TrinoClientError>,
    },
    FetchTables {
        log_id: usize,
        catalog: String,
        schema: String,
        result: Result<Vec<String>, TrinoClientError>,
    },
    /// Sent as soon as `SHOW CREATE TABLE` returns — before the slower
    /// `$partitions` / `information_schema.columns` recon queries finish —
    /// so `Table DDL` and the partitioned-ness needed by `Table View` are
    /// available immediately instead of waiting on the full recon batch.
    FetchTableDdl {
        show_create_log_id: usize,
        partitioned_by: Vec<String>,
        location: String,
        ddl_text: String,
        show_create_error: Option<TrinoClientError>,
    },
    FetchTableMetadata {
        partitions_log_id: usize,
        cols_log_id: usize,
        partition_lines: Vec<String>,
        columns: Vec<VerticalColumn>,
        partitions_error: Option<TrinoClientError>,
        columns_error: Option<TrinoClientError>,
    },
    ExecuteQuery {
        log_id: usize,
        query_buffer: String,
        query_cursor: usize,
        catalog: String,
        schema: String,
        table: String,
        is_paginated: bool,
        filters: Vec<(String, String)>,
        result: Result<crate::trino::types::QueryResults, TrinoClientError>,
    },
    FetchNextPage {
        log_id: usize,
        offset: usize,
        limit: usize,
        result: Result<crate::trino::types::QueryResults, TrinoClientError>,
    },
    FetchPartitionLevel {
        log_id: usize,
        filters: Vec<(String, String)>,
        column: String,
        result: Result<crate::trino::types::QueryResults, TrinoClientError>,
    },
}

pub fn handle_key_sync(app: &mut App, key: KeyEvent) -> Option<Command> {
    preserve_public_api_symbols();
    match app.mode {
        Mode::QueryInput => return query::handle_query_input_mode(app, key),
        Mode::Search => return query::handle_search_mode(app, key),
        Mode::Normal => {}
    }

    let code = navigation::normalize_key_code(key.code);

    if code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
        info!("User pressed Ctrl+C, quitting...");
        app.should_quit = true;
        return None;
    }

    if navigation::handle_pane_focus_keys(app, key) {
        return None;
    }

    if code == KeyCode::Char('?') {
        app.prev_screen = Some(Box::new(app.screen.clone()));
        app.screen = Screen::Help;
        return None;
    }

    if code == KeyCode::Char('/') {
        app.mode = Mode::Search;
        return None;
    }

    if code == KeyCode::Esc {
        app.number_buffer.clear();
        let is_in_table = matches!(app.screen, Screen::Actions(_));
        if is_in_table && app.active_panel == ActivePanel::MainViewer {
            app.active_panel = ActivePanel::MenuPane;
            return None;
        } else {
            navigation::go_back(app);
            return None;
        }
    }

    if code == KeyCode::Enter && !app.number_buffer.is_empty() {
        navigation::jump_to_number(app);
        return None;
    }

    if let KeyCode::Char(c) = code
        && c.is_ascii_digit()
        && matches!(app.active_panel, ActivePanel::MainViewer)
    {
        navigation::update_number_buffer(app, c);
        return None;
    }

    match &app.screen {
        Screen::Connect(_) => navigation::connect_keys(app, key),
        Screen::Catalog(_) => navigation::catalog_keys(app, key),
        Screen::Schema(_) => navigation::schema_keys(app, key),
        Screen::Table(_) => navigation::table_keys(app, key),
        Screen::Actions(_) => navigation::actions_keys(app, key),
        Screen::Help => None,
    }
}
