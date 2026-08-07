use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::app::*;

use super::Command;

pub fn extract_from_tables(sql: &str) -> Vec<String> {
    let mut tables = Vec::new();
    let tokens: Vec<&str> = sql.split_whitespace().collect();
    let mut i = 0;
    while i < tokens.len() {
        let upper = tokens[i].to_uppercase();
        if (upper == "FROM" || upper == "JOIN") && i + 1 < tokens.len() {
            let next_token = tokens[i + 1];
            if !next_token.starts_with('(') {
                let clean_table =
                    next_token.trim_matches(|c| c == ',' || c == ';' || c == '(' || c == ')');
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
        return Ok(crate::trino::queries::page_query(
            catalog, schema, table, 0, 100,
        ));
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
            super::navigation::reset_list_selected_for_search(app);
        }
        KeyCode::Backspace => {
            app.search_query.pop();
            super::navigation::reset_list_selected_for_search(app);
        }
        KeyCode::Enter | KeyCode::Esc => {
            if key.code == KeyCode::Esc {
                app.search_query.clear();
                super::navigation::reset_list_selected_for_search(app);
            }
            app.mode = Mode::Normal;
            app.active_panel = ActivePanel::MainViewer;
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

pub(crate) fn copy_to_clipboard(text: &str) {
    if let Ok(mut board) = arboard::Clipboard::new() {
        let _ = board.set_text(text);
    }
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

pub(super) fn active_query_state_mut(app: &mut App) -> Option<&mut ResultsState> {
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

pub(super) fn active_query_buffer_len(app: &App) -> Option<usize> {
    match &app.screen {
        Screen::Actions(action_state)
            if action_state.selected < ACTIONS.len()
                && matches!(ACTIONS[action_state.selected].2, Action::TableView) =>
        {
            action_state
                .results
                .as_ref()
                .map(|state| state.query_buffer.len())
        }
        _ => None,
    }
}

pub(super) fn query_bar_layout(
    app: &App,
    term_width: u16,
    term_height: u16,
) -> Option<(u16, u16, u16, u16, usize)> {
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

pub(super) fn handle_query_input_mode(app: &mut App, key: KeyEvent) -> Option<Command> {
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
                        filters: Vec::new(),
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

    if exit_mode {
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
            filters: Vec::new(),
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
        let len = 50;
        assert_eq!(query_text_index_from_mouse(8, 3, 0, 2, 40, len), 0);
        assert_eq!(query_text_index_from_mouse(18, 3, 0, 2, 40, len), 10);
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
}
