use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use tracing::info;

use crate::app::*;
use crate::trino::client::TrinoClient;

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
    },
    FetchNextPage {
        catalog: String,
        schema: String,
        table: String,
        offset: usize,
        limit: usize,
    },
}

#[derive(Debug)]
pub enum AsyncResult {
    Connect {
        log_id: usize,
        url: String,
        user: String,
        password: String,
        client: TrinoClient,
        result: Result<Vec<String>, String>,
    },
    FetchSchemas {
        log_id: usize,
        catalog: String,
        result: Result<Vec<String>, String>,
    },
    FetchTables {
        log_id: usize,
        catalog: String,
        schema: String,
        result: Result<Vec<String>, String>,
    },
    FetchTableMetadata {
        partitions_log_id: usize,
        cols_log_id: usize,
        partition_lines: Vec<String>,
        columns: Vec<VerticalColumn>,
    },
    ExecuteQuery {
        log_id: usize,
        query_buffer: String,
        query_cursor: usize,
        catalog: String,
        schema: String,
        table: String,
        is_paginated: bool,
        result: Result<crate::trino::types::QueryResults, String>,
    },
    FetchNextPage {
        log_id: usize,
        offset: usize,
        limit: usize,
        result: Result<crate::trino::types::QueryResults, String>,
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
