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

use crate::app::*;

use super::Command;

pub fn extract_from_tables(sql: &str) -> Vec<String> {
    let mut tables = Vec::new();
    let tokens: Vec<&str> = sql.split_whitespace().collect();
    let mut i = 0;
    while i < tokens.len() {
        let upper = tokens[i].to_uppercase();
        let is_table_intro = upper == "FROM"
            || upper == "JOIN"
            || upper == "TABLE"
            || upper == "FOR"
            || upper == "DESCRIBE"
            || upper == "DESC";
        if is_table_intro && i + 1 < tokens.len() {
            let next_token = tokens[i + 1];
            if !next_token.starts_with('(') {
                let clean_table =
                    next_token.trim_matches(|c| c == ',' || c == ';' || c == '(' || c == ')');
                let clean_upper = clean_table.to_uppercase();
                let sql_keywords = [
                    "SELECT", "WHERE", "GROUP", "HAVING", "ORDER", "LIMIT", "JOIN", "LEFT",
                    "RIGHT", "INNER", "OUTER", "CROSS", "FULL", "ON", "USING", "AS", "TABLE",
                    "FROM", "FOR", "VIEW",
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
        return Ok(crate::trino::queries::page_query(
            catalog, schema, table, 0, 100,
        ));
    }

    let full_target = format!("{catalog}.{schema}.{table}").to_lowercase();
    let schema_target = format!("{schema}.{table}").to_lowercase();
    let table_target = table.to_lowercase();

    let extracted_tables = extract_from_tables(trimmed);

    if extracted_tables.is_empty() {
        let lower = trimmed.to_lowercase();
        let stripped_lower = lower.replace(['"', '`', '\''], "");
        if stripped_lower.contains(&full_target)
            || stripped_lower.contains(&schema_target)
            || stripped_lower.contains(&table_target)
        {
            return Ok(trimmed.to_string());
        }

        return Err(format!(
            "Invalid query: Please enter a full SQL query targeting table '{table}' (e.g. SELECT * FROM {table} WHERE ...)."
        ));
    }

    for t in &extracted_tables {
        let normalized = t.replace(['"', '`', '\''], "").to_lowercase();
        let matches_table =
            normalized == table_target || normalized == schema_target || normalized == full_target;
        if !matches_table {
            return Err(format!(
                "Query targets table '{}', but current view scope is '{}.{}.{}'. Queries in this view must operate on table '{}'.",
                t, catalog, schema, table, table
            ));
        }
    }

    Ok(trimmed.to_string())
}

pub(super) fn handle_search_mode(app: &mut App, key: KeyEvent) -> Option<Command> {
    match key.code {
        KeyCode::Char(c) if !c.is_ascii_control() => {
            app.search_query.push(c);
            app.bind_search_to_pane();
            app.help_scroll = 0;
            super::navigation::reset_list_selected_for_search(app);
        }
        KeyCode::Backspace => {
            app.search_query.pop();
            app.help_scroll = 0;
            super::navigation::reset_list_selected_for_search(app);
        }
        KeyCode::Enter | KeyCode::Esc => {
            if key.code == KeyCode::Esc {
                app.search_query.clear();
                app.help_scroll = 0;
                super::navigation::reset_list_selected_for_search(app);
            }
            app.mode = Mode::Normal;
            app.set_active_panel(ActivePanel::MainViewer);
        }
        _ => {}
    }
    None
}

fn is_enter_key(code: KeyCode) -> bool {
    matches!(
        code,
        KeyCode::Enter | KeyCode::Char('\r') | KeyCode::Char('\n')
    )
}

fn prev_char_boundary(s: &str, cursor: usize) -> usize {
    if cursor == 0 {
        return 0;
    }
    let mut i = cursor.min(s.len()) - 1;
    while i > 0 && !s.is_char_boundary(i) {
        i -= 1;
    }
    i
}

fn next_char_boundary(s: &str, cursor: usize) -> usize {
    let len = s.len();
    if cursor >= len {
        return len;
    }
    let mut i = cursor + 1;
    while i < len && !s.is_char_boundary(i) {
        i += 1;
    }
    i
}

fn prev_word_pos(s: &str, cursor: usize) -> usize {
    let cursor = cursor.min(s.len());
    let mut i = cursor;
    while i > 0 {
        let prev = prev_char_boundary(s, i);
        if let Some(c) = s[prev..i].chars().next()
            && c.is_whitespace()
        {
            i = prev;
        } else {
            break;
        }
    }
    while i > 0 {
        let prev = prev_char_boundary(s, i);
        if let Some(c) = s[prev..i].chars().next()
            && !c.is_whitespace()
        {
            i = prev;
        } else {
            break;
        }
    }
    i
}

fn next_word_pos(s: &str, cursor: usize) -> usize {
    let len = s.len();
    let mut i = cursor.min(len);
    while i < len {
        let next = next_char_boundary(s, i);
        if let Some(c) = s[i..next].chars().next()
            && !c.is_whitespace()
        {
            i = next;
        } else {
            break;
        }
    }
    while i < len {
        let next = next_char_boundary(s, i);
        if let Some(c) = s[i..next].chars().next()
            && c.is_whitespace()
        {
            i = next;
        } else {
            break;
        }
    }
    i
}

pub(crate) fn try_copy_to_clipboard(text: &str) -> Result<(), String> {
    let mut board = arboard::Clipboard::new().map_err(|e| e.to_string())?;
    board.set_text(text).map_err(|e| e.to_string())
}

pub(crate) fn copy_to_clipboard(text: &str) {
    let _ = try_copy_to_clipboard(text);
}

pub(super) fn paste_from_clipboard() -> Option<String> {
    if let Ok(mut board) = arboard::Clipboard::new() {
        board.get_text().ok()
    } else {
        None
    }
}

pub(super) fn query_text_index_from_mouse(
    mouse_col: u16,
    mouse_row: u16,
    query_x_start: u16,
    query_y_start: u16,
    inner_w: usize,
    buffer: &str,
) -> usize {
    if inner_w == 0 || buffer.is_empty() {
        return 0;
    }
    let line0_cap = inner_w.saturating_sub(7);
    let chunks = crate::tui::draw::chunk_query_buffer(buffer, line0_cap, inner_w);
    let rel_y = mouse_row.saturating_sub(query_y_start + 1) as usize;
    let rel_x = mouse_col.saturating_sub(query_x_start + 1) as usize;

    if rel_y >= chunks.len() {
        return buffer.len();
    }
    let (start, end, chunk_str) = chunks[rel_y];
    let target_col = if rel_y == 0 {
        rel_x.saturating_sub(7)
    } else {
        rel_x
    };
    for (cur_char, (idx, _)) in chunk_str.char_indices().enumerate() {
        if cur_char >= target_col {
            return start + idx;
        }
    }
    end
}

pub(super) fn active_query_state_mut(app: &mut App) -> Option<&mut ResultsState> {
    match &mut app.screen {
        Screen::Actions(action_state) => {
            if action_state.results.is_none() {
                let is_paginated = action_state.selected == 0;
                action_state.results = Some(ResultsState {
                    query_buffer: action_state.query_buffer.clone(),
                    query_cursor: action_state.query_cursor,
                    catalog: action_state.catalog.clone(),
                    schema: action_state.schema.clone(),
                    table: action_state.table.clone(),
                    is_paginated,
                    ..Default::default()
                });
            } else if let Some(ref mut res) = action_state.results
                && res.query_buffer.is_empty()
                && !action_state.query_buffer.is_empty()
            {
                res.query_buffer = action_state.query_buffer.clone();
                res.query_cursor = action_state.query_cursor;
            }
            action_state.results.as_mut()
        }
        _ => None,
    }
}

pub(super) fn active_query_buffer(app: &App) -> Option<String> {
    match &app.screen {
        Screen::Actions(action_state) => {
            let is_partitions_or_schema = matches!(
                crate::app::ACTIONS
                    .get(action_state.selected)
                    .map(|(_, _, a)| a),
                Some(crate::app::Action::Partitions | crate::app::Action::Schema)
            );
            if !is_partitions_or_schema && let Some(ref res) = action_state.results {
                Some(res.query_buffer.clone())
            } else {
                Some(action_state.query_buffer.clone())
            }
        }
        _ => None,
    }
}

pub(super) fn query_bar_layout(
    app: &App,
    term_width: u16,
    term_height: u16,
) -> Option<(u16, u16, u16, u16, usize)> {
    let query_buf = active_query_buffer(app)?;
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

    let query_height = if matches!(app.mode, Mode::QueryInput) {
        let total_chars = 7 + query_buf.len();
        let lines = total_chars.div_ceil(inner_w);
        (lines as u16 + 2).clamp(3, 7)
    } else {
        3
    };

    if query_height > bottom_y {
        return None;
    }

    Some((query_x, 0, query_width, query_height, inner_w))
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
    super::query_vim::begin_insert_edit(state);
    state.delete_selection();
    state.query_buffer.insert_str(state.query_cursor, text);
    state.query_cursor += text.len();
    state.invalid_query_error = None;
}

fn query_selection_text(state: &ResultsState) -> Option<String> {
    let (start, end) = state.selection_range()?;
    Some(state.query_buffer[start..end].to_string())
}

/// Opens the query panel in a fresh insert session (a click on the panel while
/// it is already open keeps the current vim mode).
pub(super) fn begin_query_input(app: &mut App) {
    if app.mode != Mode::QueryInput
        && let Some(state) = active_query_state_mut(app)
    {
        state.vim.reset();
    }
    app.mode = Mode::QueryInput;
}

/// Removes `from..to` (byte offsets) from the query and puts the cursor at `from`.
fn delete_query_range(state: &mut ResultsState, from: usize, to: usize) {
    let (from, to) = (from.min(to), from.max(to).min(state.query_buffer.len()));
    if from < to {
        super::query_vim::begin_insert_edit(state);
        state.query_buffer.drain(from..to);
        state.query_cursor = from;
        state.clear_selection();
        state.invalid_query_error = None;
    }
}

pub(super) fn handle_query_input_mode(app: &mut App, key: KeyEvent) -> Option<Command> {
    let mut copied_text = None;
    let mut exit_mode = false;
    let mut command = None;
    let vim_ctx = {
        let (w, h) = crossterm::terminal::size().unwrap_or((120, 40));
        super::query_vim::Ctx {
            inner_w: query_bar_layout(app, w, h).map(|l| l.4).unwrap_or(80),
        }
    };

    {
        let Some(state) = active_query_state_mut(app) else {
            app.mode = Mode::Normal;
            return None;
        };

        let code = key.code;
        let selecting = key.modifiers.contains(KeyModifiers::SHIFT);
        // Option/Alt+arrows and Ctrl+arrows hop words; Cmd+arrows hop to the
        // line (query) ends. Terminals that send Option+arrow as ESC b / ESC f
        // arrive as Alt+b / Alt+f, handled below.
        let alt = key.modifiers.contains(KeyModifiers::ALT);
        let word_jump = key
            .modifiers
            .intersects(KeyModifiers::ALT | KeyModifiers::CONTROL);
        let line_jump = key.modifiers.contains(KeyModifiers::SUPER);
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
                KeyCode::Char('r') | KeyCode::Char('R')
                    if state.vim.mode != VimMode::Insert
                        && key.modifiers.contains(KeyModifiers::CONTROL) =>
                {
                    super::query_vim::redo(state);
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
                        filters: Vec::new(),
                    });
                }
                Err(err_msg) => {
                    state.invalid_query_error = Some(err_msg);
                }
            }
        } else if state.vim.mode != VimMode::Insert {
            let outcome =
                super::query_vim::handle_key(state, key, &vim_ctx, &mut || paste_from_clipboard());
            exit_mode = outcome.exit;
            copied_text = outcome.yanked;
        } else {
            match code {
                KeyCode::Esc => super::query_vim::enter_normal(state),
                KeyCode::Left => {
                    let new_cursor = if line_jump {
                        0
                    } else if word_jump {
                        prev_word_pos(&state.query_buffer, state.query_cursor)
                    } else {
                        prev_char_boundary(&state.query_buffer, state.query_cursor)
                    };
                    set_query_cursor(state, new_cursor, selecting);
                }
                KeyCode::Right => {
                    let new_cursor = if line_jump {
                        state.query_buffer.len()
                    } else if word_jump {
                        next_word_pos(&state.query_buffer, state.query_cursor)
                    } else {
                        next_char_boundary(&state.query_buffer, state.query_cursor)
                    };
                    set_query_cursor(state, new_cursor, selecting);
                }
                KeyCode::Up | KeyCode::Down => {
                    let new_cursor = if line_jump {
                        if code == KeyCode::Up {
                            0
                        } else {
                            state.query_buffer.len()
                        }
                    } else {
                        super::query_vim::move_rows(
                            &state.query_buffer,
                            state.query_cursor,
                            if code == KeyCode::Up { -1 } else { 1 },
                            vim_ctx.inner_w.max(8),
                        )
                    };
                    set_query_cursor(state, new_cursor, selecting);
                }
                KeyCode::Home => set_query_cursor(state, 0, selecting),
                KeyCode::End => set_query_cursor(state, state.query_buffer.len(), selecting),
                // Option+arrow on terminals that send ESC b / ESC f
                KeyCode::Char('b') | KeyCode::Char('B') if alt => {
                    let to = prev_word_pos(&state.query_buffer, state.query_cursor);
                    set_query_cursor(state, to, selecting);
                }
                KeyCode::Char('f') | KeyCode::Char('F') if alt => {
                    let to = next_word_pos(&state.query_buffer, state.query_cursor);
                    set_query_cursor(state, to, selecting);
                }
                KeyCode::Char('d') | KeyCode::Char('D') if alt => {
                    let to = next_word_pos(&state.query_buffer, state.query_cursor);
                    delete_query_range(state, state.query_cursor, to);
                }
                // Option+Backspace deletes a word, Cmd+Backspace to the start
                KeyCode::Backspace if alt || line_jump => {
                    if !state.delete_selection() {
                        let from = if line_jump {
                            0
                        } else {
                            prev_word_pos(&state.query_buffer, state.query_cursor)
                        };
                        delete_query_range(state, from, state.query_cursor);
                    }
                    state.invalid_query_error = None;
                }
                KeyCode::Backspace => {
                    super::query_vim::begin_insert_edit(state);
                    if !state.delete_selection() && state.query_cursor > 0 {
                        let prev = prev_char_boundary(&state.query_buffer, state.query_cursor);
                        state.query_buffer.drain(prev..state.query_cursor);
                        state.query_cursor = prev;
                    }
                    state.invalid_query_error = None;
                }
                KeyCode::Delete => {
                    super::query_vim::begin_insert_edit(state);
                    if !state.delete_selection() && state.query_cursor < state.query_buffer.len() {
                        let next = next_char_boundary(&state.query_buffer, state.query_cursor);
                        state.query_buffer.drain(state.query_cursor..next);
                    }
                    state.invalid_query_error = None;
                }
                KeyCode::Char(c)
                    if !command_modifier && !key.modifiers.contains(KeyModifiers::ALT) =>
                {
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

    if let Screen::Actions(ref mut a) = app.screen
        && let Some(ref r) = a.results
    {
        a.query_buffer = r.query_buffer.clone();
        a.query_cursor = r.query_cursor;
    }

    if exit_mode {
        if let Some(state) = active_query_state_mut(app) {
            state.vim.reset();
        }
        app.mode = Mode::Normal;
    }

    command
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
            scroll_v: std::cell::Cell::new(0),
            scroll_h: std::cell::Cell::new(0),
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
            filters: Vec::new(),
            total_count: None,
            total_count_loading: false,
            table_count: None,
            ..Default::default()
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
        let res = validate_and_build_query("WHERE age > 10", "datalake", "some_db", "some_table");
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
        assert_eq!(res.unwrap(), "SELECT * FROM \"some_db\".\"some_table\"");
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
        let text = "SELECT * FROM orders WHERE id > 100 AND status = 'ACTIVE'";
        assert_eq!(query_text_index_from_mouse(8, 3, 0, 2, 40, text), 0);
        assert_eq!(query_text_index_from_mouse(18, 3, 0, 2, 40, text), 10);
    }

    #[test]
    fn test_chunk_query_buffer_and_cursor_sync() {
        use crate::tui::draw::{chunk_query_buffer, cursor_line_and_col};
        let query = "SELECT * FROM iceberg.demo.lineitem_partitioned WHERE year(shipdate) = 1998 ORDER BY shipmode DESC";
        let inner_w = 40;
        let line0_cap = inner_w - 7;
        let chunks = chunk_query_buffer(query, line0_cap, inner_w);
        assert!(chunks.len() >= 3);
        assert_eq!(chunks[0].0, 0);
        assert_eq!(chunks[0].1, 33);
        assert_eq!(chunks[1].0, 33);
        assert_eq!(chunks[1].1, 73);

        // Cursor at index 0 is at line 0, column 7 (after prefix " SQL > ")
        assert_eq!(cursor_line_and_col(query, 0, line0_cap, inner_w, 7), (0, 7));

        // Cursor at index 33 (start of line 1) is at line 1, column 0
        assert_eq!(
            cursor_line_and_col(query, 33, line0_cap, inner_w, 7),
            (1, 0)
        );

        // Cursor at index 35 (line 1, char 2) is at line 1, column 2
        assert_eq!(
            cursor_line_and_col(query, 35, line0_cap, inner_w, 7),
            (1, 2)
        );

        // Cursor at index 73 (start of line 2) is at line 2, column 0
        assert_eq!(
            cursor_line_and_col(query, 73, line0_cap, inner_w, 7),
            (2, 0)
        );
    }

    #[test]
    fn test_selection_helpers_and_insert_replace() {
        let mut state = sample_results_state("SELECT * FROM orders");
        state.select_all();
        assert_eq!(
            query_selection_text(&state).as_deref(),
            Some("SELECT * FROM orders")
        );
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

    fn panel(query: &str) -> App {
        let mut app = App::new(crate::config::ConnectionConfig::default(), false);
        app.screen = Screen::Actions(ActionState {
            catalog: "c".to_string(),
            schema: "s".to_string(),
            table: "t".to_string(),
            selected: 0,
            query_buffer: query.to_string(),
            query_cursor: query.len(),
            ..Default::default()
        });
        begin_query_input(&mut app);
        app
    }

    fn send(app: &mut App, code: KeyCode, mods: KeyModifiers) {
        handle_query_input_mode(app, KeyEvent::new(code, mods));
    }

    fn typed(app: &mut App, text: &str) {
        for c in text.chars() {
            send(app, KeyCode::Char(c), KeyModifiers::NONE);
        }
    }

    fn query(app: &App) -> (String, usize) {
        let Screen::Actions(s) = &app.screen else {
            panic!()
        };
        let r = s.results.as_ref().unwrap();
        (r.query_buffer.clone(), r.query_cursor)
    }

    #[test]
    fn panel_opens_in_insert_and_esc_goes_normal_then_leaves() {
        let mut app = panel("SELECT 1");
        assert_eq!(app.query_vim_mode(), VimMode::Insert);
        typed(&mut app, "0");
        assert_eq!(query(&app).0, "SELECT 10");

        send(&mut app, KeyCode::Esc, KeyModifiers::NONE);
        assert_eq!(app.query_vim_mode(), VimMode::Normal);
        assert_eq!(app.mode, Mode::QueryInput);
        // vim keys edit, they do not type
        typed(&mut app, "x");
        assert_eq!(query(&app).0, "SELECT 1");

        send(&mut app, KeyCode::Esc, KeyModifiers::NONE);
        assert_eq!(app.mode, Mode::Normal);
        // a later open starts in insert again
        begin_query_input(&mut app);
        assert_eq!(app.query_vim_mode(), VimMode::Insert);
    }

    #[test]
    fn insert_mode_alt_cmd_and_ctrl_navigation() {
        let mut app = panel("SELECT * FROM orders");
        let word = |app: &mut App, code, mods| {
            send(app, code, mods);
            query(app).1
        };
        // Option/Alt+arrows and the ESC b / ESC f form
        assert_eq!(word(&mut app, KeyCode::Left, KeyModifiers::ALT), 14);
        assert_eq!(word(&mut app, KeyCode::Char('b'), KeyModifiers::ALT), 9);
        assert_eq!(word(&mut app, KeyCode::Char('f'), KeyModifiers::ALT), 14);
        assert_eq!(word(&mut app, KeyCode::Right, KeyModifiers::ALT), 20);
        assert_eq!(word(&mut app, KeyCode::Left, KeyModifiers::CONTROL), 14);
        // Cmd+arrows go to the ends
        assert_eq!(word(&mut app, KeyCode::Left, KeyModifiers::SUPER), 0);
        assert_eq!(word(&mut app, KeyCode::Right, KeyModifiers::SUPER), 20);
        assert_eq!(word(&mut app, KeyCode::Up, KeyModifiers::SUPER), 0);
        assert_eq!(word(&mut app, KeyCode::Down, KeyModifiers::SUPER), 20);
        // nothing was typed by those chords
        assert_eq!(query(&app).0, "SELECT * FROM orders");
    }

    #[test]
    fn insert_mode_word_and_line_deletion() {
        let mut app = panel("SELECT * FROM orders");
        send(&mut app, KeyCode::Backspace, KeyModifiers::ALT);
        assert_eq!(query(&app).0, "SELECT * FROM ");
        send(&mut app, KeyCode::Backspace, KeyModifiers::SUPER);
        assert_eq!(query(&app), (String::new(), 0));
        // and the whole edit session undoes from normal mode
        send(&mut app, KeyCode::Esc, KeyModifiers::NONE);
        typed(&mut app, "u");
        assert_eq!(query(&app).0, "SELECT * FROM orders");
    }

    #[test]
    fn normal_mode_navigation_matches_insert_mode_chords() {
        let mut app = panel("SELECT * FROM orders");
        send(&mut app, KeyCode::Esc, KeyModifiers::NONE);
        send(&mut app, KeyCode::Left, KeyModifiers::SUPER);
        assert_eq!(query(&app).1, 0);
        send(&mut app, KeyCode::Right, KeyModifiers::ALT);
        assert_eq!(query(&app).1, 7);
        send(&mut app, KeyCode::Char('f'), KeyModifiers::ALT);
        assert_eq!(query(&app).1, 9);
        send(&mut app, KeyCode::Right, KeyModifiers::SUPER);
        assert_eq!(query(&app).1, 19); // last character, not past it
        assert_eq!(query(&app).0, "SELECT * FROM orders");
    }

    #[test]
    fn yank_in_normal_mode_reports_text_and_ctrl_r_redoes() {
        let mut app = panel("SELECT 1");
        send(&mut app, KeyCode::Esc, KeyModifiers::NONE);
        typed(&mut app, "0dw");
        assert_eq!(query(&app).0, "1");
        typed(&mut app, "u");
        assert_eq!(query(&app).0, "SELECT 1");
        send(&mut app, KeyCode::Char('r'), KeyModifiers::CONTROL);
        assert_eq!(query(&app).0, "1");
    }

    #[test]
    fn enter_runs_the_query_from_any_vim_mode() {
        let mut app = panel("SELECT * FROM t");
        send(&mut app, KeyCode::Esc, KeyModifiers::NONE);
        let cmd = handle_query_input_mode(&mut app, KeyEvent::from(KeyCode::Enter));
        assert!(matches!(cmd, Some(Command::ExecuteQuery { .. })));
        assert_eq!(app.mode, Mode::Normal);
    }

    #[test]
    fn test_edit_query_in_sample_view() {
        let sample_query = "SELECT * FROM \"iceberg\".\"sales\".\"orders\" LIMIT 20";
        let mut app = App::new(
            crate::config::ConnectionConfig {
                url: "http://localhost:8080".to_string(),
                user: "test".to_string(),
                password: "".to_string(),
                query_cache_limit: 10,
            },
            false,
        );
        // Action index 4 is Sample Mode
        app.screen = Screen::Actions(ActionState {
            catalog: "iceberg".to_string(),
            schema: "sales".to_string(),
            table: "orders".to_string(),
            selected: 4,
            query_buffer: sample_query.to_string(),
            query_cursor: sample_query.len(),
            results: None,
            ..Default::default()
        });

        // Triggering 'q' enters QueryInput even in Sample view
        crate::tui::handler::navigation::actions_keys(
            &mut app,
            crossterm::event::KeyEvent::from(crossterm::event::KeyCode::Char('q')),
        );
        assert_eq!(app.mode, Mode::QueryInput);

        // active_query_state_mut initializes ResultsState from ActionState query_buffer
        let state = active_query_state_mut(&mut app).expect("state should be active");
        assert_eq!(state.query_buffer, sample_query);

        // Type additional text (e.g. backspace 20 and type 50)
        let backspaces = 2;
        for _ in 0..backspaces {
            let _ = handle_query_input_mode(
                &mut app,
                crossterm::event::KeyEvent::from(crossterm::event::KeyCode::Backspace),
            );
        }
        let _ = handle_query_input_mode(
            &mut app,
            crossterm::event::KeyEvent::from(crossterm::event::KeyCode::Char('5')),
        );
        let _ = handle_query_input_mode(
            &mut app,
            crossterm::event::KeyEvent::from(crossterm::event::KeyCode::Char('0')),
        );

        let cmd = handle_query_input_mode(
            &mut app,
            crossterm::event::KeyEvent::from(crossterm::event::KeyCode::Enter),
        );
        assert_eq!(app.mode, Mode::Normal);
        assert!(
            matches!(cmd, Some(Command::ExecuteQuery { query, .. }) if query.ends_with("LIMIT 50"))
        );
    }
}
