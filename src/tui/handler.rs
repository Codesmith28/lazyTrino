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

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseEvent, MouseEventKind};
use tracing::{error, info};

use crate::app::*;
use crate::trino::client::TrinoClient;
use crate::trino::queries;

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

pub fn extract_from_tables(sql: &str) -> Vec<String> {
    let mut tables = Vec::new();
    let tokens: Vec<&str> = sql.split_whitespace().collect();
    let mut i = 0;
    while i < tokens.len() {
        let upper = tokens[i].to_uppercase();
        if (upper == "FROM" || upper == "JOIN")
            && i + 1 < tokens.len() {
                let next_token = tokens[i + 1];
                if !next_token.starts_with('(') {
                    let clean_table = next_token.trim_matches(|c| c == ',' || c == ';' || c == '(' || c == ')');
                    let clean_upper = clean_table.to_uppercase();
                    let sql_keywords = [
                        "SELECT", "WHERE", "GROUP", "HAVING", "ORDER", "LIMIT", "JOIN", "LEFT",
                        "RIGHT", "INNER", "OUTER", "CROSS", "FULL", "ON", "USING", "AS",
                    ];
                    if !clean_table.is_empty() && !sql_keywords.contains(&clean_upper.as_str()) {
                        tables.push(clean_table.to_string());
                    }
                }
            }
        i += 1;
    }
    tables
}

pub fn validate_and_build_query(
    user_input: &str,
    catalog: &str,
    schema: &str,
    table: &str,
) -> Result<String, String> {
    let trimmed = user_input.trim();
    if trimmed.is_empty() {
        return Ok(crate::trino::queries::page_query(catalog, schema, table, 0, 100));
    }

    let full_target = format!("{catalog}.{schema}.{table}").to_lowercase();
    let schema_target = format!("{schema}.{table}").to_lowercase();
    let table_target = table.to_lowercase();

    let extracted_tables = extract_from_tables(trimmed);

    if extracted_tables.is_empty() {
        return Err(format!(
            "Invalid query: Please enter a full SQL query targeting table '{table}' (e.g. SELECT * FROM {table} WHERE ...)."
        ));
    }

    for t in &extracted_tables {
        let normalized = t.replace(['"', '`', '\''], "").to_lowercase();
        let matches_table = normalized == table_target
            || normalized == schema_target
            || normalized == full_target;
        if !matches_table {
            return Err(format!(
                "Query targets table '{}', but current view scope is '{}.{}.{}'. Queries in this view must operate on table '{}'.",
                t, catalog, schema, table, table
            ));
        }
    }

    Ok(trimmed.to_string())
}

fn check_trigger_infinite_scroll(app: &mut App) -> Option<Command> {
    if let Screen::Actions(ref mut action_state) = app.screen
        && let Some(state) = action_state.results.as_mut()
        && state.is_paginated
        && !state.is_fetching_next_page
        && state.has_more_rows
        && state.scroll_v + 15 >= state.rows.len()
    {
        state.is_fetching_next_page = true;
        let offset = state.rows.len();
        return Some(Command::FetchNextPage {
            catalog: state.catalog.clone(),
            schema: state.schema.clone(),
            table: state.table.clone(),
            offset,
            limit: state.page_size,
        });
    }
    None
}

fn extract_list_labels(screen: &Screen) -> Option<Vec<String>> {
    match screen {
        Screen::Catalog(s) => Some(s.items.iter().map(|x| x.trim().to_string()).collect()),
        Screen::Schema(s) => Some(s.items.iter().map(|x| x.trim().to_string()).collect()),
        Screen::Table(s) => Some(s.items.iter().map(|x| x.trim().to_string()).collect()),
        Screen::Actions(_) => Some(ACTIONS.iter().map(|(_, l, _)| l.to_string()).collect()),
        _ => None,
    }
}

fn mod_list_selected(screen: &mut Screen, new_selected: usize) {
    match screen {
        Screen::Catalog(s) => s.selected = new_selected,
        Screen::Schema(s) => s.selected = new_selected,
        Screen::Table(s) => s.selected = new_selected,
        Screen::Actions(s) => s.selected = new_selected,
        _ => {}
    }
}

fn get_selected(screen: &Screen) -> Option<usize> {
    match screen {
        Screen::Catalog(s) => Some(s.selected),
        Screen::Schema(s) => Some(s.selected),
        Screen::Table(s) => Some(s.selected),
        Screen::Actions(s) => Some(s.selected),
        _ => None,
    }
}

fn update_number_buffer(app: &mut App, ch: char) {
    if ch.is_ascii_digit() {
        let mut buf = app.number_buffer.clone();
        buf.push(ch);
        let num: usize = buf.parse().unwrap_or(0);
        if let Some(items) = extract_list_labels(&app.screen) {
            if num <= items.len() && num > 0 {
                app.number_buffer = buf;
            } else {
                app.number_buffer = ch.to_string();
            }
        }
    }
}

fn jump_to_number(app: &mut App) {
    if app.number_buffer.is_empty() {
        return;
    }
    let num: usize = app.number_buffer.parse().unwrap_or(1);
    if let Some(items) = extract_list_labels(&app.screen)
        && num > 0 && num <= items.len() {
            mod_list_selected(&mut app.screen, num - 1);
        }
    app.number_buffer.clear();
}

fn handle_search_mode(app: &mut App, key: KeyEvent) -> Option<Command> {
    match key.code {
        KeyCode::Char(c) if !c.is_ascii_control() => {
            app.search_query.push(c);
        }
        KeyCode::Backspace => {
            app.search_query.pop();
        }
        KeyCode::Enter | KeyCode::Esc => {
            if key.code == KeyCode::Esc {
                app.search_query.clear();
            }
            app.mode = Mode::Normal;
            app.active_panel = ActivePanel::MainViewer;
        }
        _ => {}
    }
    None
}

fn go_back(app: &mut App) {
    let logged_in = app.trino_client.is_some();
    app.main_panel_pct = 60;

    let next = match &app.screen {
        Screen::Help => app.prev_screen.take().map(|p| *p),
        Screen::Catalog(_) => {
            if logged_in {
                None
            } else {
                let c = app.config.clone();
                Some(Screen::Connect(ConnectState {
                    url: c.url,
                    user: c.user,
                    password: c.password,
                    focused: 0,
                    loading: false,
                    error: None,
                }))
            }
        }
        Screen::Schema(s) => {
            let cat = s.catalog.clone();
            let catalogs = app.catalogs.clone();
            let idx = catalogs.iter().position(|c| *c == cat).unwrap_or(0);
            Some(Screen::Catalog(CatalogState {
                items: catalogs,
                selected: idx,
            }))
        }
        Screen::Table(s) => {
            let cat = s.catalog.clone();
            let schema_name = s.schema.clone();
            let schemas = app.schemas.get(&cat).cloned().unwrap_or_default();
            let idx = schemas.iter().position(|c| c.trim() == schema_name.trim()).unwrap_or(0);
            Some(Screen::Schema(SchemaState {
                catalog: cat,
                items: schemas,
                selected: idx,
            }))
        }
        Screen::Actions(s) => {
            let cat = s.catalog.clone();
            let schema_name = s.schema.clone();
            let tables = app
                .tables
                .get(&(cat.clone(), schema_name.clone()))
                .cloned()
                .unwrap_or_default();
            let idx = tables.iter().position(|t| t.trim() == s.table.trim()).unwrap_or(0);
            Some(Screen::Table(TableState {
                catalog: cat,
                schema: schema_name,
                items: tables,
                selected: idx,
            }))
        }
        Screen::Connect(_) => None,
    };

    if let Some(s) = next {
        app.screen = s;
    }
}

use crossterm::event::MouseButton;

/// macOS terminals can emit Option+hjkl/g as special glyphs when Option is not configured as Meta.
/// Remap those characters back to plain vim-style motion keys so Alt/Option navigation still works.
fn normalize_key_code(code: KeyCode) -> KeyCode {
    match code {
        KeyCode::Char('∆') => KeyCode::Char('j'),
        KeyCode::Char('˚') => KeyCode::Char('k'),
        KeyCode::Char('˙') => KeyCode::Char('h'),
        KeyCode::Char('¬') => KeyCode::Char('l'),
        KeyCode::Char('©') => KeyCode::Char('g'),
        _ => code,
    }
}

fn is_enter_key(code: KeyCode) -> bool {
    matches!(code, KeyCode::Enter | KeyCode::Char('\r') | KeyCode::Char('\n'))
}

fn prev_word_pos(s: &str, cursor: usize) -> usize {
    if cursor == 0 {
        return 0;
    }
    let chars: Vec<char> = s.chars().collect();
    let mut i = cursor.min(chars.len());
    while i > 0 && chars[i - 1].is_whitespace() {
        i -= 1;
    }
    while i > 0 && !chars[i - 1].is_whitespace() {
        i -= 1;
    }
    i
}

fn next_word_pos(s: &str, cursor: usize) -> usize {
    let chars: Vec<char> = s.chars().collect();
    let len = chars.len();
    let mut i = cursor.min(len);
    while i < len && !chars[i].is_whitespace() {
        i += 1;
    }
    while i < len && chars[i].is_whitespace() {
        i += 1;
    }
    i
}

fn copy_to_clipboard(text: &str) {
    if let Ok(mut board) = arboard::Clipboard::new() {
        let _ = board.set_text(text);
    }
}

fn paste_from_clipboard() -> Option<String> {
    if let Ok(mut board) = arboard::Clipboard::new() {
        board.get_text().ok()
    } else {
        None
    }
}

fn query_text_index_from_mouse(
    mouse_col: u16,
    mouse_row: u16,
    query_x_start: u16,
    query_y_start: u16,
    inner_w: usize,
    buffer_len: usize,
) -> usize {
    if inner_w == 0 {
        return 0;
    }
    let rel_y = mouse_row.saturating_sub(query_y_start + 1) as usize;
    let rel_x = mouse_col.saturating_sub(query_x_start + 1) as usize;

    let char_idx = if rel_y == 0 {
        let line0_x = rel_x.saturating_sub(7);
        let max_line0 = inner_w.saturating_sub(7);
        line0_x.min(max_line0)
    } else {
        let max_line0 = inner_w.saturating_sub(7);
        let line_x = rel_x.min(inner_w);
        max_line0 + (rel_y - 1) * inner_w + line_x
    };

    char_idx.min(buffer_len)
}

fn active_query_state_mut(app: &mut App) -> Option<&mut ResultsState> {
    match &mut app.screen {
        Screen::Actions(action_state)
            if action_state.selected < ACTIONS.len()
                && matches!(ACTIONS[action_state.selected].2, Action::TableView) =>
        {
            action_state.results.as_mut()
        }
        _ => None,
    }
}

fn active_query_buffer_len(app: &App) -> Option<usize> {
    match &app.screen {
        Screen::Actions(action_state)
            if action_state.selected < ACTIONS.len()
                && matches!(ACTIONS[action_state.selected].2, Action::TableView) =>
        {
            action_state.results.as_ref().map(|state| state.query_buffer.len())
        }
        _ => None,
    }
}

fn query_bar_layout(app: &App, term_width: u16, term_height: u16) -> Option<(u16, u16, u16, u16, usize)> {
    let query_len = active_query_buffer_len(app)?;
    let bottom_y = term_height.saturating_sub(7);
    if bottom_y == 0 {
        return None;
    }

    let menu_pct = if app.main_panel_pct > 30 {
        15
    } else {
        app.main_panel_pct.clamp(8, 30)
    };
    let query_x = ((term_width as u32 * menu_pct as u32) / 100) as u16;
    let query_width = term_width.saturating_sub(query_x);
    let inner_w = query_width.saturating_sub(2).max(1) as usize;

    let search_height = if matches!(app.mode, Mode::Search) {
        let total_chars = 3 + app.search_query.len();
        let lines = total_chars.div_ceil(inner_w);
        (lines as u16 + 2).clamp(3, 8)
    } else {
        3
    };

    let query_height = if matches!(app.mode, Mode::QueryInput) {
        let total_chars = 7 + query_len;
        let lines = total_chars.div_ceil(inner_w);
        (lines as u16 + 2).clamp(3, 4)
    } else {
        3
    };

    if search_height + query_height > bottom_y {
        return None;
    }

    Some((query_x, search_height, query_width, query_height, inner_w))
}

fn set_query_cursor(state: &mut ResultsState, new_cursor: usize, selecting: bool) {
    let new_cursor = new_cursor.min(state.query_buffer.len());
    if selecting {
        if state.selection_anchor.is_none() {
            state.selection_anchor = Some(state.query_cursor);
        }
    } else {
        state.clear_selection();
    }
    state.query_cursor = new_cursor;
    if state.selection_anchor == Some(state.query_cursor) {
        state.clear_selection();
    }
}

fn insert_query_text(state: &mut ResultsState, text: &str) {
    state.delete_selection();
    state.query_buffer.insert_str(state.query_cursor, text);
    state.query_cursor += text.len();
    state.invalid_query_error = None;
}

fn query_selection_text(state: &ResultsState) -> Option<String> {
    let (start, end) = state.selection_range()?;
    Some(state.query_buffer[start..end].to_string())
}

fn handle_query_input_mode(app: &mut App, key: KeyEvent) -> Option<Command> {
    let mut copied_text = None;
    let mut exit_mode = false;
    let mut command = None;

    {
        let Some(state) = active_query_state_mut(app) else {
            app.mode = Mode::Normal;
            return None;
        };

        let code = key.code;
        let selecting = key.modifiers.contains(KeyModifiers::SHIFT);
        let word_jump = key.modifiers.contains(KeyModifiers::ALT);
        let command_modifier = key
            .modifiers
            .intersects(KeyModifiers::CONTROL | KeyModifiers::SUPER);

        if command_modifier {
            match code {
                KeyCode::Char('a') | KeyCode::Char('A') => {
                    state.select_all();
                    return None;
                }
                KeyCode::Char('c') | KeyCode::Char('C') => {
                    copied_text = query_selection_text(state);
                    exit_mode = false;
                }
                KeyCode::Char('v') | KeyCode::Char('V') => {
                    if let Some(text) = paste_from_clipboard() {
                        insert_query_text(state, &text);
                    }
                    return None;
                }
                _ => {}
            }
        }

        if copied_text.is_some() {
            // defer clipboard write until after the mutable borrow ends
        } else if is_enter_key(code) {
            let input = state.query_buffer.clone();
            let cat = state.catalog.clone();
            let sch = state.schema.clone();
            let tbl = state.table.clone();
            let is_paginated = state.is_paginated;

            match validate_and_build_query(&input, &cat, &sch, &tbl) {
                Ok(full_sql) => {
                    state.invalid_query_error = None;
                    state.clear_selection();
                    exit_mode = true;
                    command = Some(Command::ExecuteQuery {
                        query: full_sql,
                        is_paginated,
                        catalog: cat,
                        schema: sch,
                        table: tbl,
                    });
                }
                Err(err_msg) => {
                    state.invalid_query_error = Some(err_msg);
                }
            }
        } else {
            match code {
                KeyCode::Esc => {
                    state.clear_selection();
                    exit_mode = true;
                }
                KeyCode::Left => {
                    let new_cursor = if word_jump {
                        prev_word_pos(&state.query_buffer, state.query_cursor)
                    } else {
                        state.query_cursor.saturating_sub(1)
                    };
                    set_query_cursor(state, new_cursor, selecting);
                }
                KeyCode::Right => {
                    let new_cursor = if word_jump {
                        next_word_pos(&state.query_buffer, state.query_cursor)
                    } else {
                        (state.query_cursor + 1).min(state.query_buffer.len())
                    };
                    set_query_cursor(state, new_cursor, selecting);
                }
                KeyCode::Home => set_query_cursor(state, 0, selecting),
                KeyCode::End => set_query_cursor(state, state.query_buffer.len(), selecting),
                KeyCode::Backspace => {
                    if !state.delete_selection() && state.query_cursor > 0 {
                        state.query_cursor -= 1;
                        state.query_buffer.remove(state.query_cursor);
                    }
                    state.invalid_query_error = None;
                }
                KeyCode::Delete => {
                    if !state.delete_selection() && state.query_cursor < state.query_buffer.len() {
                        state.query_buffer.remove(state.query_cursor);
                    }
                    state.invalid_query_error = None;
                }
                KeyCode::Char(c) if !command_modifier && !key.modifiers.contains(KeyModifiers::ALT) => {
                    insert_query_text(state, &c.to_string());
                }
                _ => {}
            }
        }
    }

    if let Some(text) = copied_text {
        copy_to_clipboard(&text);
        app.copied_toast = Some((text.chars().take(30).collect(), std::time::Instant::now()));
        return None;
    }

    if exit_mode {
        app.mode = Mode::Normal;
    }

    command
}

pub fn handle_pane_focus_keys(app: &mut App, key: KeyEvent) -> bool {
    let has_pane_modifier = key.modifiers.contains(KeyModifiers::SHIFT);
    let code = normalize_key_code(key.code);
    let is_h = (code == KeyCode::Char('H')) || (code == KeyCode::Char('h') && has_pane_modifier);
    let is_l = (code == KeyCode::Char('L')) || (code == KeyCode::Char('l') && has_pane_modifier);

    let is_left = key.code == KeyCode::Left && has_pane_modifier;
    let is_right = key.code == KeyCode::Right && has_pane_modifier;

    if !(is_h || is_l || is_left || is_right || code == KeyCode::Tab) {
        return false;
    }

    let is_in_table = matches!(app.screen, Screen::Actions(_));

    if is_in_table {
        if is_h || is_left {
            app.active_panel = ActivePanel::MenuPane;
            return true;
        }
        if is_l || is_right || code == KeyCode::Tab {
            if app.active_panel == ActivePanel::MenuPane {
                app.active_panel = ActivePanel::MainViewer;
            } else {
                app.active_panel = ActivePanel::MenuPane;
            }
            return true;
        }
    }

    false
}

pub fn trigger_action(app: &mut App, action_idx: usize) -> Option<Command> {
    if action_idx >= ACTIONS.len() {
        return None;
    }
    if let Screen::Actions(ref mut s) = app.screen {
        s.selected = action_idx;
        app.active_panel = ActivePanel::MainViewer;
        let action = &ACTIONS[action_idx].2;
        match action {
            Action::Partitions => {
                if app.partition_tree_lines.is_empty() {
                    return Some(Command::FetchTableMetadata {
                        catalog: s.catalog.clone(),
                        schema: s.schema.clone(),
                        table: s.table.clone(),
                    });
                }
                None
            }
            Action::Schema => {
                if app.vertical_schema_cols.is_empty() {
                    return Some(Command::FetchTableMetadata {
                        catalog: s.catalog.clone(),
                        schema: s.schema.clone(),
                        table: s.table.clone(),
                    });
                }
                None
            }
            _ => {
                let is_paginated = matches!(action, Action::TableView);
                let query = action.build_query(&s.catalog, &s.schema, &s.table);
                s.results = None;
                Some(Command::ExecuteQuery {
                    query,
                    is_paginated,
                    catalog: s.catalog.clone(),
                    schema: s.schema.clone(),
                    table: s.table.clone(),
                })
            }
        }
    } else {
        None
    }
}

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
                    let idx = query_text_index_from_mouse(
                        mouse.column,
                        mouse.row,
                        query_x,
                        query_y,
                        inner_w,
                        active_query_buffer_len(app).unwrap_or(0),
                    );
                    app.mode = Mode::QueryInput;
                    app.active_panel = ActivePanel::MainViewer;
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
                            app.active_panel = ActivePanel::MenuPane;
                        }
                    } else {
                        app.mode = Mode::Normal;
                        app.active_panel = ActivePanel::MainViewer;
                    }
                } else if mouse.column >= border_x && mouse.row < bottom_y {
                    app.mode = Mode::Normal;
                    app.active_panel = ActivePanel::MainViewer;
                }
            }
        }
        MouseEventKind::Drag(MouseButton::Left) => {
            if app.is_dragging_query_select {
                if let Some((query_x, query_y, _, _, inner_w)) =
                    query_bar_layout(app, term_width, term_height)
                {
                    let idx = query_text_index_from_mouse(
                        mouse.column,
                        mouse.row,
                        query_x,
                        query_y,
                        inner_w,
                        active_query_buffer_len(app).unwrap_or(0),
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
                if let (Some(anchor), Some(current)) = (app.mouse_selection_anchor, app.mouse_selection_current) {
                    let text = extract_selected_text(app, anchor, current);
                    if !text.is_empty() {
                        copy_to_clipboard(&text);
                        app.copied_toast = Some((text.chars().take(30).collect(), std::time::Instant::now()));
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
                if selected_idx == 7 {
                    let max_lines = app.partition_tree_lines.len().saturating_sub(1);
                    app.partition_scroll = (app.partition_scroll + 1).min(max_lines);
                } else if selected_idx == 8 {
                    let max_cols = app.vertical_schema_cols.len().saturating_sub(1);
                    app.schema_scroll = (app.schema_scroll + 1).min(max_cols);
                } else if let Screen::Actions(ref mut a) = app.screen
                    && let Some(ref mut res) = a.results {
                        if shift {
                            if !res.columns.is_empty() {
                                res.scroll_h = (res.scroll_h + 1).min(res.columns.len().saturating_sub(1));
                            }
                        } else if !res.rows.is_empty() {
                            res.scroll_v = (res.scroll_v + 1).min(res.rows.len().saturating_sub(1));
                            return check_trigger_infinite_scroll(app);
                        }
                    }
            } else if let Some(items) = extract_list_labels(&app.screen)
                && !items.is_empty()
                    && let Some(s) = get_selected(&app.screen) {
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
                if selected_idx == 7 {
                    app.partition_scroll = app.partition_scroll.saturating_sub(1);
                } else if selected_idx == 8 {
                    app.schema_scroll = app.schema_scroll.saturating_sub(1);
                } else if let Screen::Actions(ref mut a) = app.screen
                    && let Some(ref mut res) = a.results {
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

pub fn handle_key_sync(app: &mut App, key: KeyEvent) -> Option<Command> {
    match app.mode {
        Mode::QueryInput => return handle_query_input_mode(app, key),
        Mode::Search => return handle_search_mode(app, key),
        Mode::Normal => {}
    }

    let code = normalize_key_code(key.code);

    if code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
        info!("User pressed Ctrl+C, quitting...");
        app.should_quit = true;
        return None;
    }

    if handle_pane_focus_keys(app, key) {
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
            go_back(app);
            return None;
        }
    }

    if code == KeyCode::Enter && !app.number_buffer.is_empty() {
        jump_to_number(app);
        return None;
    }

    if let KeyCode::Char(c) = code
        && c.is_ascii_digit() && matches!(app.active_panel, ActivePanel::MainViewer) {
            update_number_buffer(app, c);
            return None;
        }

    match &app.screen {
        Screen::Connect(_) => connect_keys(app, key),
        Screen::Catalog(_) => catalog_keys(app, key),
        Screen::Schema(_) => schema_keys(app, key),
        Screen::Table(_) => table_keys(app, key),
        Screen::Actions(_) => actions_keys(app, key),
        Screen::Help => None,
    }
}

fn select_current_item(app: &mut App) -> Option<Command> {
    match &app.screen {
        Screen::Catalog(s) => {
            if s.items.is_empty() {
                return None;
            }
            let catalog = s.items[s.selected].trim().to_string();
            if app.schemas.contains_key(&catalog) {
                let items = app.schemas[&catalog].iter().map(|x| x.trim().to_string()).collect();
                app.screen = Screen::Schema(SchemaState {
                    catalog,
                    items,
                    selected: 0,
                });
                None
            } else {
                Some(Command::FetchSchemas { catalog })
            }
        }
        Screen::Schema(s) => {
            if s.items.is_empty() {
                return None;
            }
            let schema = s.items[s.selected].trim().to_string();
            let catalog = s.catalog.trim().to_string();
            if app.tables.contains_key(&(catalog.clone(), schema.clone())) {
                let items = app.tables[&(catalog.clone(), schema.clone())]
                    .iter()
                    .map(|x| x.trim().to_string())
                    .collect();
                app.screen = Screen::Table(TableState {
                    catalog,
                    schema,
                    items,
                    selected: 0,
                });
                None
            } else {
                Some(Command::FetchTables { catalog, schema })
            }
        }
        Screen::Table(s) => {
            if s.items.is_empty() {
                return None;
            }
            let catalog = s.catalog.clone();
            let schema = s.schema.clone();
            let table = s.items[s.selected].trim().to_string();
            app.main_panel_pct = 15;
            app.active_panel = ActivePanel::MenuPane;
            app.partition_tree_lines.clear();
            app.vertical_schema_cols.clear();
            let default_query = ACTIONS[0].2.build_query(&catalog, &schema, &table);
            let query_len = default_query.len();
            app.screen = Screen::Actions(ActionState {
                catalog,
                schema,
                table,
                selected: 0,
                query_buffer: default_query,
                query_cursor: query_len,
                results: None,
            });
            None
        }
        Screen::Actions(s) => {
            let idx = s.selected;
            trigger_action(app, idx)
        }
        _ => None,
    }
}

fn connect_keys(app: &mut App, key: KeyEvent) -> Option<Command> {
    let state_loading = match &app.screen {
        Screen::Connect(s) => s.loading,
        _ => return None,
    };
    if state_loading {
        return None;
    }
    let state = match &mut app.screen {
        Screen::Connect(s) => s,
        _ => return None,
    };
    match key.code {
        KeyCode::Tab | KeyCode::Char('\t') => {
            state.focused = (state.focused + 1) % 3;
        }
        KeyCode::Backspace => match state.focused {
            0 => { state.url.pop(); }
            1 => { state.user.pop(); }
            2 => { state.password.pop(); }
            _ => {},
        },
        KeyCode::Char(c) => match state.focused {
            0 => state.url.push(c),
            1 => state.user.push(c),
            2 => state.password.push(c),
            _ => {},
        },
        KeyCode::Enter
            if !state.url.is_empty() && !state.user.is_empty() => {
                let url = state.url.clone();
                let user = state.user.clone();
                let password = state.password.clone();
                state.loading = true;
                return Some(Command::Connect { url, user, password });
            }
        _ => {}
    }
    None
}

fn handle_list_navigation_keys(app: &mut App, key: KeyEvent) -> Option<Command> {
    let code = normalize_key_code(key.code);
    match code {
        KeyCode::Char('j') | KeyCode::Down => {
            if let Some(items) = extract_list_labels(&app.screen)
                && !items.is_empty()
                    && let Some(s) = get_selected(&app.screen) {
                        mod_list_selected(&mut app.screen, (s + 1).min(items.len() - 1));
                    }
            None
        }
        KeyCode::Char('k') | KeyCode::Up => {
            if let Some(s) = get_selected(&app.screen) {
                mod_list_selected(&mut app.screen, s.saturating_sub(1));
            }
            None
        }
        KeyCode::Char('l') | KeyCode::Right | KeyCode::Enter => select_current_item(app),
        KeyCode::Char('h') | KeyCode::Left | KeyCode::Esc => {
            app.number_buffer.clear();
            go_back(app);
            None
        }
        KeyCode::Char('g') => {
            mod_list_selected(&mut app.screen, 0);
            None
        }
        KeyCode::Char('G') => {
            if let Some(items) = extract_list_labels(&app.screen)
                && !items.is_empty() {
                    mod_list_selected(&mut app.screen, items.len() - 1);
                }
            None
        }
        _ => None,
    }
}

fn catalog_keys(app: &mut App, key: KeyEvent) -> Option<Command> {
    handle_list_navigation_keys(app, key)
}

fn schema_keys(app: &mut App, key: KeyEvent) -> Option<Command> {
    handle_list_navigation_keys(app, key)
}

fn table_keys(app: &mut App, key: KeyEvent) -> Option<Command> {
    handle_list_navigation_keys(app, key)
}

fn actions_keys(app: &mut App, key: KeyEvent) -> Option<Command> {
    let code = normalize_key_code(key.code);

    if let KeyCode::Char(c) = code
        && let Some(pos) = ACTIONS.iter().position(|(k, _, _)| *k == c) {
            return trigger_action(app, pos);
        }

    if let Screen::Actions(ref mut s) = app.screen {
        match app.active_panel {
            ActivePanel::MenuPane => match code {
                KeyCode::Char('j') | KeyCode::Down => {
                    s.selected = (s.selected + 1) % ACTIONS.len();
                    None
                }
                KeyCode::Char('k') | KeyCode::Up => {
                    s.selected = if s.selected == 0 { ACTIONS.len() - 1 } else { s.selected - 1 };
                    None
                }
                KeyCode::Enter | KeyCode::Char('l') | KeyCode::Right => {
                    let idx = s.selected;
                    trigger_action(app, idx)
                }
                KeyCode::Char('h') | KeyCode::Left | KeyCode::Esc => {
                    go_back(app);
                    None
                }
                _ => None,
            },
            ActivePanel::MainViewer => match code {
                KeyCode::Char('j') | KeyCode::Down => {
                    if s.selected == 7 {
                        let max_lines = app.partition_tree_lines.len().saturating_sub(1);
                        app.partition_scroll = (app.partition_scroll + 1).min(max_lines);
                    } else if s.selected == 8 {
                        let max_cols = app.vertical_schema_cols.len().saturating_sub(1);
                        app.schema_scroll = (app.schema_scroll + 1).min(max_cols);
                    } else if let Some(ref mut res) = s.results {
                        if !res.rows.is_empty() {
                            res.scroll_v = (res.scroll_v + 1).min(res.rows.len().saturating_sub(1));
                        }
                        return check_trigger_infinite_scroll(app);
                    }
                    None
                }
                KeyCode::Char('k') | KeyCode::Up => {
                    if s.selected == 7 {
                        app.partition_scroll = app.partition_scroll.saturating_sub(1);
                    } else if s.selected == 8 {
                        app.schema_scroll = app.schema_scroll.saturating_sub(1);
                    } else if let Some(ref mut res) = s.results {
                        res.scroll_v = res.scroll_v.saturating_sub(1);
                    }
                    None
                }
                KeyCode::Char('l') | KeyCode::Right => {
                    if let Some(ref mut res) = s.results
                        && !res.columns.is_empty() {
                            res.scroll_h = (res.scroll_h + 1).min(res.columns.len().saturating_sub(1));
                        }
                    None
                }
                KeyCode::Char('h') | KeyCode::Left => {
                    if let Some(ref mut res) = s.results {
                        res.scroll_h = res.scroll_h.saturating_sub(1);
                    }
                    None
                }
                KeyCode::Esc => {
                    app.active_panel = ActivePanel::MenuPane;
                    None
                }
                KeyCode::Char('g') => {
                    if s.selected == 7 {
                        app.partition_scroll = 0;
                    } else if s.selected == 8 {
                        app.schema_scroll = 0;
                    } else if let Some(ref mut res) = s.results {
                        res.scroll_v = 0;
                        res.scroll_h = 0;
                    }
                    None
                }
                KeyCode::Char('G') => {
                    if s.selected == 7 {
                        app.partition_scroll = app.partition_tree_lines.len().saturating_sub(1);
                    } else if s.selected == 8 {
                        app.schema_scroll = app.vertical_schema_cols.len().saturating_sub(1);
                    } else if let Some(ref mut res) = s.results {
                        res.scroll_v = res.rows.len().saturating_sub(1);
                        return check_trigger_infinite_scroll(app);
                    }
                    None
                }
                KeyCode::Char('q') | KeyCode::Char(':') => {
                    if s.selected < ACTIONS.len() && matches!(ACTIONS[s.selected].2, Action::TableView) {
                        app.mode = Mode::QueryInput;
                    }
                    None
                }
                _ => None,
            },
        }
    } else {
        None
    }
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

pub fn extract_selected_text(app: &App, anchor: (u16, u16), current: (u16, u16)) -> String {
    let (term_width, term_height) = crossterm::terminal::size().unwrap_or((80, 24));
    if term_width == 0 || term_height == 0 {
        return String::new();
    }

    let bottom_y = term_height.saturating_sub(7);
    let border_x = ((term_width as u32 * app.main_panel_pct as u32) / 100) as u16;
    let height_right = bottom_y;
    let border_y = ((height_right as u32 * app.control_panel_split_pct as u32) / 100) as u16;

    let start_row = anchor.1.min(current.1);
    let end_row = anchor.1.max(current.1);

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

    if anchor.0 < border_x && current.0 < border_x {
        match &app.screen {
            Screen::Catalog(s) => {
                let mut lines = Vec::new();
                let inner_y = 4;
                for r in start_row..=end_row {
                    if r >= inner_y {
                        let idx = (r - inner_y) as usize;
                        if idx < s.items.len() {
                            lines.push(s.items[idx].clone());
                        }
                    }
                }
                return lines.join("\n");
            }
            Screen::Schema(s) => {
                let mut lines = Vec::new();
                let inner_y = 4;
                for r in start_row..=end_row {
                    if r >= inner_y {
                        let idx = (r - inner_y) as usize;
                        if idx < s.items.len() {
                            lines.push(s.items[idx].clone());
                        }
                    }
                }
                return lines.join("\n");
            }
            Screen::Table(s) => {
                let mut lines = Vec::new();
                let inner_y = 4;
                for r in start_row..=end_row {
                    if r >= inner_y {
                        let idx = (r - inner_y) as usize;
                        if idx < s.items.len() {
                            lines.push(s.items[idx].clone());
                        }
                    }
                }
                return lines.join("\n");
            }
            Screen::Actions(_) => {
                let mut lines = Vec::new();
                let inner_y = 4;
                for r in start_row..=end_row {
                    if r >= inner_y {
                        let idx = (r - inner_y) as usize;
                        if idx < ACTIONS.len() {
                            lines.push(ACTIONS[idx].1.to_string());
                        }
                    }
                }
                return lines.join("\n");
            }
            _ => {}
        }
    }

    if anchor.0 >= border_x && current.0 >= border_x {
        if start_row < border_y {
            let mut lines = Vec::new();
            let inner_y = 1;
            for r in start_row..=end_row {
                if r >= inner_y {
                    let idx = (r - inner_y) as usize + app.partition_scroll;
                    if idx < app.partition_tree_lines.len() {
                        lines.push(app.partition_tree_lines[idx].clone());
                    }
                }
            }
            return lines.join("\n");
        } else {
            let mut lines = Vec::new();
            let inner_y = border_y + 1;
            for r in start_row..=end_row {
                if r >= inner_y {
                    let idx = (r - inner_y) as usize + app.schema_scroll;
                    if idx < app.vertical_schema_cols.len() {
                        let col = &app.vertical_schema_cols[idx];
                        lines.push(format!("{} {}", col.name, col.data_type));
                    }
                }
            }
            return lines.join("\n");
        }
    }

    String::new()
}

pub fn dispatch_command(
    app: &mut App,
    cmd: Command,
    tx: &tokio::sync::mpsc::UnboundedSender<AsyncResult>,
) {
    match cmd {
        Command::Connect { url, user, password } => {
            let sql = queries::show_catalogs();
            let log_id = app.add_query_log(sql);
            app.loading = true;
            if let Screen::Connect(s) = &mut app.screen {
                s.loading = true;
                s.error = None;
            }
            let tx = tx.clone();
            tokio::spawn(async move {
                let client = TrinoClient::new(&url, &user);
                let res = client.fetch_catalogs().await.map_err(|e| e.to_string());
                let _ = tx.send(AsyncResult::Connect {
                    log_id,
                    url,
                    user,
                    password,
                    client,
                    result: res,
                });
            });
        }
        Command::FetchSchemas { catalog } => {
            let sql = queries::show_schemas(&catalog);
            let log_id = app.add_query_log(sql);
            app.loading = true;
            if let Some(client) = app.trino_client.clone() {
                let tx = tx.clone();
                tokio::spawn(async move {
                    let res = client.fetch_schemas(&catalog).await.map_err(|e| e.to_string());
                    let _ = tx.send(AsyncResult::FetchSchemas {
                        log_id,
                        catalog,
                        result: res,
                    });
                });
            }
        }
        Command::FetchTables { catalog, schema } => {
            let sql = queries::show_tables(&catalog, &schema);
            let log_id = app.add_query_log(sql);
            app.loading = true;
            if let Some(client) = app.trino_client.clone() {
                let tx = tx.clone();
                tokio::spawn(async move {
                    let res = client.fetch_tables(&catalog, &schema).await.map_err(|e| e.to_string());
                    let _ = tx.send(AsyncResult::FetchTables {
                        log_id,
                        catalog,
                        schema,
                        result: res,
                    });
                });
            }
        }
        Command::FetchTableMetadata { catalog, schema, table } => {
            if let Some(client) = app.trino_client.clone() {
                let part_query = queries::partitions(&catalog, &schema, &table);
                let part_log_id = app.add_query_log(part_query.clone());
                let desc_query = queries::info_schema_columns(&catalog, &schema, &table);
                let cols_log_id = app.add_query_log(desc_query.clone());
                app.loading = true;

                let tx = tx.clone();
                tokio::spawn(async move {
                    let partition_lines = match client.execute(&part_query).await {
                        Ok(res) if !res.data.is_empty() => {
                            let raw_lines: Vec<String> = res.data.into_iter().map(|r| r.join("/")).collect();
                            crate::tui::screens::partition_tree::build_tree_lines(&raw_lines)
                        }
                        _ => {
                            let show_create_query = queries::show_create(&catalog, &schema, &table);
                            let ddl_str = match client.execute(&show_create_query).await {
                                Ok(res2) => res2.data.first().and_then(|r| r.first()).cloned().unwrap_or_default(),
                                Err(_) => String::new(),
                            };
                            crate::tui::screens::partition_tree::build_tree_lines(&[ddl_str])
                        }
                    };

                    let columns = match client.execute(&desc_query).await {
                        Ok(res) => {
                            res.data.iter().enumerate().map(|(idx, r)| {
                                let name = r.first().cloned().unwrap_or_default();
                                let dtype = r.get(1).cloned().unwrap_or_default();
                                let is_nullable = r.get(2).cloned().unwrap_or_default();
                                let comment = r.get(3).cloned().unwrap_or_default();
                                let key_meta = if name.starts_with("_hoodie") {
                                    "Hudi Metadata".to_string()
                                } else if name.starts_with("$") || name.contains("iceberg") {
                                    "Iceberg Meta".to_string()
                                } else if is_nullable == "NO" {
                                    "PK".to_string()
                                } else {
                                    String::new()
                                };
                                VerticalColumn { index: idx + 1, name, data_type: dtype, key_meta, description: comment }
                            }).collect()
                        }
                        Err(_) => Vec::new(),
                    };

                    let _ = tx.send(AsyncResult::FetchTableMetadata {
                        partitions_log_id: part_log_id,
                        cols_log_id,
                        partition_lines,
                        columns,
                    });
                });
            }
        }
        Command::ExecuteQuery { query, is_paginated, catalog, schema, table } => {
            let log_id = app.add_query_log(query.clone());
            app.loading = true;
            let (query_buffer, query_cursor) = match &app.screen {
                Screen::Actions(a) => {
                    if let Some(ref r) = a.results {
                        (r.query_buffer.clone(), r.query_cursor)
                    } else {
                        (a.query_buffer.clone(), a.query_cursor)
                    }
                }
                _ => (query.clone(), query.len()),
            };

            let res_state = ResultsState {
                query_buffer: query_buffer.clone(),
                query_cursor,
                columns: Vec::new(),
                rows: vec![vec!["Loading...".to_string()]],
                scroll_v: 0,
                scroll_h: 0,
                loading: true,
                error: None,
                is_paginated,
                catalog: catalog.clone(),
                schema: schema.clone(),
                table: table.clone(),
                offset: 0,
                page_size: 100,
                is_fetching_next_page: false,
                has_more_rows: true,
                invalid_query_error: None,
                selection_anchor: None,
            };

            if let Screen::Actions(ref mut a) = app.screen {
                a.results = Some(res_state);
            } else {
                app.prev_screen = Some(Box::new(app.screen.clone()));
                app.screen = Screen::Actions(ActionState {
                    catalog: catalog.clone(),
                    schema: schema.clone(),
                    table: table.clone(),
                    selected: 0,
                    query_buffer: query_buffer.clone(),
                    query_cursor,
                    results: Some(res_state),
                });
            }

            if let Some(client) = app.trino_client.clone() {
                let tx = tx.clone();
                tokio::spawn(async move {
                    let res = client.execute(&query).await.map_err(|e| e.to_string());
                    let _ = tx.send(AsyncResult::ExecuteQuery {
                        log_id,
                        query_buffer,
                        query_cursor,
                        catalog,
                        schema,
                        table,
                        is_paginated,
                        result: res,
                    });
                });
            }
        }
        Command::FetchNextPage { catalog, schema, table, offset, limit } => {
            let query = queries::page_query(&catalog, &schema, &table, offset, limit);
            let log_id = app.add_query_log(query.clone());
            if let Screen::Actions(ref mut action_state) = app.screen
                && let Some(state) = action_state.results.as_mut()
            {
                state.is_fetching_next_page = true;
            }

            if let Some(client) = app.trino_client.clone() {
                let tx = tx.clone();
                tokio::spawn(async move {
                    let res = client.execute(&query).await.map_err(|e| e.to_string());
                    let _ = tx.send(AsyncResult::FetchNextPage {
                        log_id,
                        offset,
                        limit,
                        result: res,
                    });
                });
            }
        }
    }
}

pub fn handle_async_result(app: &mut App, result: AsyncResult) {
    match result {
        AsyncResult::Connect { log_id, url, user, password, client, result } => {
            app.loading = false;
            match result {
                Ok(catalogs) => {
                    app.complete_query_log_success(log_id, 15, catalogs.len());
                    app.config.url = url;
                    app.config.user = user;
                    app.config.password = password;
                    app.trino_client = Some(client);
                    app.catalogs = catalogs.iter().map(|c| c.trim().to_string()).collect();
                    app.screen = Screen::Catalog(CatalogState {
                        items: app.catalogs.clone(),
                        selected: 0,
                    });
                }
                Err(e) => {
                    error!(error = %e, "Connect failed");
                    app.complete_query_log_error(log_id, e.clone());
                    if let Screen::Connect(s) = &mut app.screen {
                        s.loading = false;
                        s.error = Some(format!("Connection failed: {e}"));
                    }
                }
            }
        }
        AsyncResult::FetchSchemas { log_id, catalog, result } => {
            app.loading = false;
            match result {
                Ok(schemas) => {
                    let trimmed: Vec<String> = schemas.iter().map(|s| s.trim().to_string()).collect();
                    app.complete_query_log_success(log_id, 25, trimmed.len());
                    app.schemas.insert(catalog.clone(), trimmed.clone());
                    app.screen = Screen::Schema(SchemaState {
                        catalog: catalog.clone(),
                        items: trimmed,
                        selected: 0,
                    });
                }
                Err(e) => {
                    error!(error = %e, "Fetch schemas failed");
                    app.complete_query_log_error(log_id, e);
                }
            }
        }
        AsyncResult::FetchTables { log_id, catalog, schema, result } => {
            app.loading = false;
            match result {
                Ok(tables) => {
                    let trimmed: Vec<String> = tables.iter().map(|t| t.trim().to_string()).collect();
                    app.complete_query_log_success(log_id, 35, trimmed.len());
                    app.tables.insert((catalog.clone(), schema.clone()), trimmed.clone());
                    app.screen = Screen::Table(TableState {
                        catalog: catalog.clone(),
                        schema: schema.clone(),
                        items: trimmed,
                        selected: 0,
                    });
                }
                Err(e) => {
                    error!(error = %e, "Fetch tables failed");
                    app.complete_query_log_error(log_id, e);
                }
            }
        }
        AsyncResult::FetchTableMetadata { partitions_log_id, cols_log_id, partition_lines, columns } => {
            app.loading = false;
            app.complete_query_log_success(partitions_log_id, 20, partition_lines.len());
            app.complete_query_log_success(cols_log_id, 20, columns.len());
            app.partition_tree_lines = partition_lines;
            app.vertical_schema_cols = columns;
        }
        AsyncResult::ExecuteQuery { log_id, query_buffer, query_cursor, catalog, schema, table, is_paginated, result } => {
            app.loading = false;
            match result {
                Ok(results) => {
                    app.complete_query_log_success(log_id, results.duration_ms, results.data.len());
                    let cols: Vec<String> = results.columns.iter().map(|c| c.name.clone()).collect();
                    let rows = results.data;
                    let has_more = if is_paginated { rows.len() >= 100 } else { false };
                    let res_state = ResultsState {
                        query_buffer,
                        query_cursor,
                        columns: cols,
                        rows,
                        scroll_v: 0,
                        scroll_h: 0,
                        loading: false,
                        error: None,
                        is_paginated,
                        catalog: catalog.clone(),
                        schema: schema.clone(),
                        table: table.clone(),
                        offset: 0,
                        page_size: 100,
                        is_fetching_next_page: false,
                        has_more_rows: has_more,
                        invalid_query_error: None,
                        selection_anchor: None,
                    };

                    let cur_selected = if let Screen::Actions(ref a) = app.screen { a.selected } else { 0 };
                    if let Screen::Actions(ref mut a) = app.screen {
                        a.results = Some(res_state);
                    } else {
                        let default_query = ACTIONS[0].2.build_query(&catalog, &schema, &table);
                        let query_len = default_query.len();
                        app.screen = Screen::Actions(ActionState {
                            catalog,
                            schema,
                            table,
                            selected: cur_selected,
                            query_buffer: default_query,
                            query_cursor: query_len,
                            results: Some(res_state),
                        });
                    }
                }
                Err(e) => {
                    error!(error = %e, "Execute query failed");
                    app.complete_query_log_error(log_id, e.clone());
                    let res_state = ResultsState {
                        query_buffer,
                        query_cursor,
                        columns: Vec::new(),
                        rows: vec![vec![format!("Error: {e}")]],
                        scroll_v: 0,
                        scroll_h: 0,
                        loading: false,
                        error: Some(e),
                        is_paginated: false,
                        catalog: catalog.clone(),
                        schema: schema.clone(),
                        table: table.clone(),
                        offset: 0,
                        page_size: 100,
                        is_fetching_next_page: false,
                        has_more_rows: false,
                        invalid_query_error: None,
                        selection_anchor: None,
                    };

                    let cur_selected = if let Screen::Actions(ref a) = app.screen { a.selected } else { 0 };
                    if let Screen::Actions(ref mut a) = app.screen {
                        a.results = Some(res_state);
                    } else {
                        let default_query = ACTIONS[0].2.build_query(&catalog, &schema, &table);
                        let query_len = default_query.len();
                        app.screen = Screen::Actions(ActionState {
                            catalog,
                            schema,
                            table,
                            selected: cur_selected,
                            query_buffer: default_query,
                            query_cursor: query_len,
                            results: Some(res_state),
                        });
                    }
                }
            }
        }
        AsyncResult::FetchNextPage { log_id, offset, limit, result } => {
            match result {
                Ok(results) => {
                    app.complete_query_log_success(log_id, results.duration_ms, results.data.len());
                    let new_rows = results.data;
                    let fetched_count = new_rows.len();
                    if let Screen::Actions(ref mut action_state) = app.screen
                        && let Some(state) = action_state.results.as_mut()
                    {
                        state.rows.extend(new_rows);
                        state.offset = offset;
                        state.is_fetching_next_page = false;
                        if fetched_count < limit {
                            state.has_more_rows = false;
                        }
                    }
                }
                Err(e) => {
                    error!(error = %e, "Fetch next page failed");
                    app.complete_query_log_error(log_id, e);
                    if let Screen::Actions(ref mut action_state) = app.screen
                        && let Some(state) = action_state.results.as_mut()
                    {
                        state.is_fetching_next_page = false;
                        state.has_more_rows = false;
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_results_state(query_buffer: &str) -> ResultsState {
        ResultsState {
            query_buffer: query_buffer.to_string(),
            query_cursor: query_buffer.len(),
            columns: Vec::new(),
            rows: Vec::new(),
            scroll_v: 0,
            scroll_h: 0,
            loading: false,
            error: None,
            is_paginated: true,
            catalog: "datalake".to_string(),
            schema: "some_db".to_string(),
            table: "orders".to_string(),
            offset: 0,
            page_size: 100,
            is_fetching_next_page: false,
            has_more_rows: true,
            invalid_query_error: None,
            selection_anchor: None,
        }
    }

    #[test]
    fn test_extract_from_tables() {
        assert_eq!(
            extract_from_tables("SELECT * FROM datalake.some_db.some_table WHERE id > 5"),
            vec!["datalake.some_db.some_table"]
        );
        assert_eq!(
            extract_from_tables("SELECT * FROM some_table"),
            vec!["some_table"]
        );
        assert_eq!(
            extract_from_tables("WHERE age > 20 ORDER BY id"),
            Vec::<String>::new()
        );
    }

    #[test]
    fn test_full_query_table_name() {
        let res = validate_and_build_query(
            "SELECT * FROM some_table WHERE status = 'ACTIVE'",
            "datalake",
            "some_db",
            "some_table",
        );
        assert!(res.is_ok());
        assert_eq!(
            res.unwrap(),
            "SELECT * FROM some_table WHERE status = 'ACTIVE'"
        );
    }

    #[test]
    fn test_full_query_qualified() {
        let res = validate_and_build_query(
            "SELECT * FROM datalake.some_db.some_table WHERE age > 10",
            "datalake",
            "some_db",
            "some_table",
        );
        assert!(res.is_ok());
        assert_eq!(
            res.unwrap(),
            "SELECT * FROM datalake.some_db.some_table WHERE age > 10"
        );
    }

    #[test]
    fn test_missing_from_clause() {
        let res = validate_and_build_query(
            "WHERE age > 10",
            "datalake",
            "some_db",
            "some_table",
        );
        assert!(res.is_err());
        assert!(res.unwrap_err().contains("Please enter a full SQL query"));
    }

    #[test]
    fn test_quoted_table_query() {
        let res = validate_and_build_query(
            "SELECT * FROM \"datalake\".\"some_db\".\"some_table\" OFFSET 0 LIMIT 100",
            "datalake",
            "some_db",
            "some_table",
        );
        assert!(res.is_ok());
        assert_eq!(
            res.unwrap(),
            "SELECT * FROM \"datalake\".\"some_db\".\"some_table\" OFFSET 0 LIMIT 100"
        );
    }

    #[test]
    fn test_partially_quoted_table_query() {
        let res = validate_and_build_query(
            "SELECT * FROM \"some_db\".\"some_table\"",
            "datalake",
            "some_db",
            "some_table",
        );
        assert!(res.is_ok());
        assert_eq!(
            res.unwrap(),
            "SELECT * FROM \"some_db\".\"some_table\""
        );
    }

    #[test]
    fn test_invalid_query_different_table() {
        let res = validate_and_build_query(
            "SELECT * FROM table_a WHERE age > 10",
            "datalake",
            "some_db",
            "some_table",
        );
        assert!(res.is_err());
        assert!(res.unwrap_err().contains("Query targets table 'table_a'"));
    }

    #[test]
    fn test_word_navigation() {
        let text = "SELECT * FROM orders";
        assert_eq!(prev_word_pos(text, 20), 14);
        assert_eq!(prev_word_pos(text, 14), 9);
        assert_eq!(next_word_pos(text, 0), 7);
        assert_eq!(next_word_pos(text, 7), 9);
    }

    #[test]
    fn test_mouse_index_calculation() {
        let len = 50;
        assert_eq!(query_text_index_from_mouse(8, 3, 0, 2, 40, len), 0);
        assert_eq!(query_text_index_from_mouse(18, 3, 0, 2, 40, len), 10);
    }

    #[test]
    fn test_selection_helpers_and_insert_replace() {
        let mut state = sample_results_state("SELECT * FROM orders");
        state.select_all();
        assert_eq!(query_selection_text(&state).as_deref(), Some("SELECT * FROM orders"));
        assert!(state.delete_selection());
        assert_eq!(state.query_buffer, "");
        assert_eq!(state.query_cursor, 0);

        let mut state = sample_results_state("SELECT * FROM orders");
        state.selection_anchor = Some(0);
        state.query_cursor = 6;
        insert_query_text(&mut state, "WITH");
        assert_eq!(state.query_buffer, "WITH * FROM orders");
        assert_eq!(state.query_cursor, 4);
    }

    #[test]
    fn test_wrap_text() {
        use crate::tui::screens::results::wrap_text;
        let wrapped = wrap_text("hello world this is a test of long text wrapping", 10);
        assert!(wrapped.len() > 1);
        for line in &wrapped {
            assert!(line.len() <= 10);
        }
    }
}
