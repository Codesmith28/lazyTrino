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
    App, ColumnFilter, FilterModalState, FilterMode, Mode, NumericFilter, NumericFocus,
    NumericOp, Screen, VerticalColumn,
};
use crate::tui::handler::Command;
use crate::tui::handler::navigation::normalize_key_code;

pub fn is_numeric_type(data_type: &str) -> bool {
    let t = data_type.trim().to_ascii_lowercase();
    t.starts_with("tinyint")
        || t.starts_with("smallint")
        || t.starts_with("integer")
        || t == "int"
        || t.starts_with("bigint")
        || t.starts_with("real")
        || t.starts_with("float")
        || t.starts_with("double")
        || t.starts_with("decimal")
        || t.starts_with("numeric")
}

pub fn is_numeric_column(
    col_name: &str,
    vertical_schema_cols: &[VerticalColumn],
    sample_values: &[String],
) -> bool {
    if let Some(col) = vertical_schema_cols
        .iter()
        .find(|c| c.name.eq_ignore_ascii_case(col_name))
        && !col.data_type.is_empty()
    {
        return is_numeric_type(&col.data_type);
    }
    let non_empty: Vec<&str> = sample_values
        .iter()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty() && !s.eq_ignore_ascii_case("null"))
        .collect();
    if non_empty.is_empty() {
        return false;
    }
    non_empty.iter().all(|s| s.parse::<f64>().is_ok())
}

pub fn open_filter_modal(app: &mut App) {
    if let Screen::Actions(ref mut action_state) = app.screen
        && let Some(ref mut res) = action_state.results
        && !res.columns.is_empty()
    {
        let col_idx = res.selected_col.min(res.columns.len().saturating_sub(1));
        let col_name = res.columns[col_idx].clone();

        // Ensure unfiltered_rows is populated
        if res.unfiltered_rows.is_none() {
            res.unfiltered_rows = Some(res.rows.clone());
        }

        // Collect distinct values for this column if not cached
        let values = res
            .col_distinct_values
            .entry(col_name.clone())
            .or_insert_with(|| {
                let mut vals = Vec::new();
                let rows_source = res.unfiltered_rows.as_ref().unwrap_or(&res.rows);
                for row in rows_source {
                    let val = row.get(col_idx).cloned().unwrap_or_default();
                    if !vals.contains(&val) {
                        vals.push(val);
                    }
                }
                vals.sort();
                vals
            })
            .clone();

        let is_numeric = is_numeric_column(&col_name, &app.vertical_schema_cols, &values);

        let (mode, selected_values, numeric_op, numeric_input) =
            match res.column_filters.get(&col_name) {
                Some(ColumnFilter::Numeric(num)) => (
                    FilterMode::Numeric,
                    values.iter().cloned().collect(),
                    num.op,
                    num.raw_value.clone(),
                ),
                Some(ColumnFilter::Values(existing)) => (
                    FilterMode::Values,
                    existing.clone(),
                    NumericOp::Lt,
                    String::new(),
                ),
                None => (
                    FilterMode::Values,
                    values.iter().cloned().collect(),
                    NumericOp::Lt,
                    String::new(),
                ),
            };

        app.filter_modal = Some(FilterModalState {
            column_index: col_idx,
            column_name: col_name,
            is_numeric_col: is_numeric,
            mode,
            values,
            selected_values,
            cursor: 0,
            scroll: 0,
            search_query: String::new(),
            is_searching: false,
            numeric_op,
            numeric_input,
            numeric_focus: NumericFocus::Value,
            numeric_error: None,
            last_op_char: None,
        });
        app.mode = Mode::FilterModal;
    }
}

pub fn handle_filter_modal_key(app: &mut App, key: KeyEvent) -> Option<Command> {
    let mode = app.filter_modal.as_ref().map(|m| m.mode)?;
    let is_numeric_col = app
        .filter_modal
        .as_ref()
        .map(|m| m.is_numeric_col)
        .unwrap_or(false);

    match mode {
        FilterMode::Numeric => handle_numeric_modal_key(app, key),
        FilterMode::Values => handle_values_modal_key(app, key, is_numeric_col),
    }
}

fn cycle_op_left(op: NumericOp) -> NumericOp {
    match op {
        NumericOp::Lt => NumericOp::Eq,
        NumericOp::Lte => NumericOp::Lt,
        NumericOp::Gt => NumericOp::Lte,
        NumericOp::Gte => NumericOp::Gt,
        NumericOp::Eq => NumericOp::Gte,
    }
}

fn cycle_op_right(op: NumericOp) -> NumericOp {
    match op {
        NumericOp::Lt => NumericOp::Lte,
        NumericOp::Lte => NumericOp::Gt,
        NumericOp::Gt => NumericOp::Gte,
        NumericOp::Gte => NumericOp::Eq,
        NumericOp::Eq => NumericOp::Lt,
    }
}

fn handle_numeric_modal_key(app: &mut App, key: KeyEvent) -> Option<Command> {
    match key.code {
        KeyCode::Esc => {
            app.mode = Mode::Normal;
            app.filter_modal = None;
            None
        }
        KeyCode::Tab | KeyCode::BackTab => {
            if let Some(ref mut m) = app.filter_modal {
                m.mode = FilterMode::Values;
            }
            None
        }
        KeyCode::Enter => apply_filter_modal(app),
        KeyCode::Char('c') | KeyCode::Char('C') => {
            if let Some(ref mut m) = app.filter_modal {
                m.numeric_input.clear();
                m.numeric_error = None;
            }
            None
        }
        KeyCode::Up | KeyCode::Char('k') => {
            if let Some(ref mut m) = app.filter_modal {
                m.numeric_focus = NumericFocus::Operator;
            }
            None
        }
        KeyCode::Down | KeyCode::Char('j') => {
            if let Some(ref mut m) = app.filter_modal {
                m.numeric_focus = NumericFocus::Value;
            }
            None
        }
        KeyCode::Left => {
            if let Some(ref mut m) = app.filter_modal {
                m.last_op_char = None;
                m.numeric_op = cycle_op_left(m.numeric_op);
                if let Some((_, rest)) = NumericOp::from_str_prefix(&m.numeric_input) {
                    m.numeric_input = format!("{} {}", m.numeric_op.as_str(), rest);
                }
                m.numeric_error = None;
            }
            None
        }
        KeyCode::Right => {
            if let Some(ref mut m) = app.filter_modal {
                m.last_op_char = None;
                m.numeric_op = cycle_op_right(m.numeric_op);
                if let Some((_, rest)) = NumericOp::from_str_prefix(&m.numeric_input) {
                    m.numeric_input = format!("{} {}", m.numeric_op.as_str(), rest);
                }
                m.numeric_error = None;
            }
            None
        }
        KeyCode::Backspace => {
            if let Some(ref mut m) = app.filter_modal {
                m.last_op_char = None;
                m.numeric_input.pop();
                m.numeric_error = None;
                if let Some((op, _)) = NumericOp::from_str_prefix(&m.numeric_input) {
                    m.numeric_op = op;
                }
            }
            None
        }
        KeyCode::Delete => {
            if let Some(ref mut m) = app.filter_modal {
                m.last_op_char = None;
                m.numeric_input.clear();
                m.numeric_error = None;
            }
            None
        }
        KeyCode::Char(c) => {
            if let Some(ref mut m) = app.filter_modal {
                if m.numeric_focus == NumericFocus::Operator {
                    m.last_op_char = None;
                    match c {
                        'h' => {
                            m.numeric_op = cycle_op_left(m.numeric_op);
                            return None;
                        }
                        'l' => {
                            m.numeric_op = cycle_op_right(m.numeric_op);
                            return None;
                        }
                        '1' => {
                            m.numeric_op = NumericOp::Lt;
                            return None;
                        }
                        '2' => {
                            m.numeric_op = NumericOp::Lte;
                            return None;
                        }
                        '3' => {
                            m.numeric_op = NumericOp::Gt;
                            return None;
                        }
                        '4' => {
                            m.numeric_op = NumericOp::Gte;
                            return None;
                        }
                        '5' => {
                            m.numeric_op = NumericOp::Eq;
                            return None;
                        }
                        'j' => {
                            m.numeric_focus = NumericFocus::Value;
                            return None;
                        }
                        _ => {
                            m.numeric_focus = NumericFocus::Value;
                        }
                    }
                }

                if c == '<' {
                    m.numeric_op = NumericOp::Lt;
                    m.last_op_char = Some('<');
                    if let Some((_, rest)) = NumericOp::from_str_prefix(&m.numeric_input) {
                        m.numeric_input = format!("< {}", rest);
                    }
                    m.numeric_error = None;
                } else if c == '>' {
                    m.numeric_op = NumericOp::Gt;
                    m.last_op_char = Some('>');
                    if let Some((_, rest)) = NumericOp::from_str_prefix(&m.numeric_input) {
                        m.numeric_input = format!("> {}", rest);
                    }
                    m.numeric_error = None;
                } else if c == '=' {
                    if m.last_op_char == Some('<') {
                        m.numeric_op = NumericOp::Lte;
                        m.last_op_char = None;
                    } else if m.last_op_char == Some('>') {
                        m.numeric_op = NumericOp::Gte;
                        m.last_op_char = None;
                    } else {
                        m.numeric_op = NumericOp::Eq;
                        m.last_op_char = Some('=');
                    }
                    if let Some((_, rest)) = NumericOp::from_str_prefix(&m.numeric_input) {
                        m.numeric_input = format!("{} {}", m.numeric_op.as_str(), rest);
                    }
                    m.numeric_error = None;
                } else if c.is_ascii_digit() || c == '.' || c == '-' || c == ' ' {
                    m.last_op_char = None;
                    if c != ' ' || !m.numeric_input.is_empty() {
                        m.numeric_input.push(c);
                        m.numeric_error = None;
                        m.numeric_focus = NumericFocus::Value;
                        if let Some((op, _)) = NumericOp::from_str_prefix(&m.numeric_input) {
                            m.numeric_op = op;
                        }
                    }
                }
            }
            None
        }
        _ => None,
    }
}

fn handle_values_modal_key(app: &mut App, key: KeyEvent, is_numeric_col: bool) -> Option<Command> {
    let is_searching = app
        .filter_modal
        .as_ref()
        .map(|m| m.is_searching)
        .unwrap_or(false);

    if is_searching {
        match key.code {
            KeyCode::Esc => {
                if let Some(ref mut m) = app.filter_modal {
                    m.is_searching = false;
                    m.search_query.clear();
                    m.cursor = 0;
                    m.scroll = 0;
                }
                return None;
            }
            KeyCode::Enter => {
                if is_numeric_col
                    && let Some(m) = app.filter_modal.as_ref()
                    && let Some((_op, num_str)) = NumericOp::from_str_prefix(&m.search_query)
                    && num_str.parse::<f64>().is_ok()
                {
                    return apply_filter_modal(app);
                }
                if let Some(ref mut m) = app.filter_modal {
                    m.is_searching = false;
                }
                return None;
            }
            KeyCode::Backspace => {
                if let Some(ref mut m) = app.filter_modal {
                    m.search_query.pop();
                    m.cursor = 0;
                    m.scroll = 0;
                }
                return None;
            }
            KeyCode::Char(c) => {
                if let Some(ref mut m) = app.filter_modal {
                    m.search_query.push(c);
                    m.cursor = 0;
                    m.scroll = 0;
                }
                return None;
            }
            _ => return None,
        }
    }

    let code = normalize_key_code(key.code);
    match code {
        KeyCode::Tab | KeyCode::BackTab if is_numeric_col => {
            if let Some(ref mut m) = app.filter_modal {
                m.mode = FilterMode::Numeric;
                if let Some((op, num_str)) = NumericOp::from_str_prefix(&m.search_query) {
                    m.numeric_op = op;
                    m.numeric_input = num_str.to_string();
                    m.search_query.clear();
                }
            }
            None
        }
        KeyCode::Char(c) if is_numeric_col && (c == '<' || c == '>' || c == '=' || c == '2') => {
            if let Some(ref mut m) = app.filter_modal {
                m.mode = FilterMode::Numeric;
                if c == '<' {
                    m.numeric_op = NumericOp::Lt;
                } else if c == '>' {
                    m.numeric_op = NumericOp::Gt;
                } else if c == '=' {
                    m.numeric_op = NumericOp::Eq;
                }
            }
            None
        }
        KeyCode::Char('/') => {
            if let Some(ref mut m) = app.filter_modal {
                m.is_searching = true;
            }
            None
        }
        KeyCode::Char('j') | KeyCode::Down => {
            if let Some(ref mut m) = app.filter_modal {
                let matched = m.filtered_indices();
                if !matched.is_empty() {
                    m.cursor = (m.cursor + 1).min(matched.len() - 1);
                    if m.cursor >= m.scroll + 10 {
                        m.scroll = m.cursor.saturating_sub(9);
                    }
                }
            }
            None
        }
        KeyCode::Char('k') | KeyCode::Up => {
            if let Some(ref mut m) = app.filter_modal {
                m.cursor = m.cursor.saturating_sub(1);
                if m.cursor < m.scroll {
                    m.scroll = m.cursor;
                }
            }
            None
        }
        KeyCode::PageDown | KeyCode::Char('d') | KeyCode::Char('f')
            if code == KeyCode::PageDown
                || key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL) =>
        {
            if let Some(ref mut m) = app.filter_modal {
                let matched = m.filtered_indices();
                if !matched.is_empty() {
                    m.cursor = (m.cursor + 10).min(matched.len() - 1);
                    if m.cursor >= m.scroll + 10 {
                        m.scroll = m.cursor.saturating_sub(9);
                    }
                }
            }
            None
        }
        KeyCode::PageUp | KeyCode::Char('u') | KeyCode::Char('b')
            if code == KeyCode::PageUp
                || key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL) =>
        {
            if let Some(ref mut m) = app.filter_modal {
                m.cursor = m.cursor.saturating_sub(10);
                if m.cursor < m.scroll {
                    m.scroll = m.cursor;
                }
            }
            None
        }
        KeyCode::Char('g') | KeyCode::Home => {
            if let Some(ref mut m) = app.filter_modal {
                m.cursor = 0;
                m.scroll = 0;
            }
            None
        }
        KeyCode::Char('G') | KeyCode::End => {
            if let Some(ref mut m) = app.filter_modal {
                let matched = m.filtered_indices();
                if !matched.is_empty() {
                    m.cursor = matched.len() - 1;
                    if m.cursor >= 10 {
                        m.scroll = m.cursor.saturating_sub(9);
                    }
                }
            }
            None
        }
        KeyCode::Char(' ') => {
            if let Some(ref mut m) = app.filter_modal {
                let matched = m.filtered_indices();
                if let Some(&orig_idx) = matched.get(m.cursor)
                    && let Some(val) = m.values.get(orig_idx).cloned()
                {
                    if m.selected_values.contains(&val) {
                        m.selected_values.remove(&val);
                    } else {
                        m.selected_values.insert(val);
                    }
                }
            }
            None
        }
        KeyCode::Char('a') | KeyCode::Char('A') => {
            if let Some(ref mut m) = app.filter_modal {
                if m.selected_values.len() == m.values.len() {
                    m.selected_values.clear();
                } else {
                    m.selected_values = m.values.iter().cloned().collect();
                }
            }
            None
        }
        KeyCode::Esc => {
            app.mode = Mode::Normal;
            app.filter_modal = None;
            None
        }
        KeyCode::Enter => apply_filter_modal(app),
        _ => None,
    }
}

pub fn apply_filter_modal(app: &mut App) -> Option<Command> {
    let modal = app.filter_modal.take()?;

    let new_filter = match modal.mode {
        FilterMode::Numeric => {
            match modal.parse_numeric_filter() {
                Ok(Some((op, value, raw_value))) => {
                    Some(ColumnFilter::Numeric(NumericFilter {
                        op,
                        value,
                        raw_value,
                    }))
                }
                Ok(None) => None,
                Err(err) => {
                    let mut m = modal;
                    m.numeric_error = Some(err);
                    app.filter_modal = Some(m);
                    app.mode = Mode::FilterModal;
                    return None;
                }
            }
        }
        FilterMode::Values => {
            if modal.is_numeric_col
                && let Some((op, num_str)) = NumericOp::from_str_prefix(&modal.search_query)
                && let Ok(val) = num_str.parse::<f64>()
            {
                Some(ColumnFilter::Numeric(NumericFilter {
                    op,
                    value: val,
                    raw_value: num_str.to_string(),
                }))
            } else if modal.selected_values.len() == modal.values.len() {
                None
            } else {
                Some(ColumnFilter::Values(modal.selected_values))
            }
        }
    };

    app.mode = Mode::Normal;

    if let Screen::Actions(ref mut action_state) = app.screen
        && let Some(ref mut res) = action_state.results
    {
        // 1. Update column_filters
        match new_filter {
            Some(filter) => {
                res.column_filters.insert(modal.column_name.clone(), filter);
            }
            None => {
                res.column_filters.remove(&modal.column_name);
            }
        }

        // 2. Filter in-memory rows from unfiltered_rows
        if res.unfiltered_rows.is_none() {
            res.unfiltered_rows = Some(res.rows.clone());
        }
        let unfiltered = res.unfiltered_rows.as_ref().unwrap();

        let mut filtered_rows = Vec::new();
        for row in unfiltered {
            let mut matches_all = true;
            for (col_name, filter) in &res.column_filters {
                if let Some(col_idx) = res.columns.iter().position(|c| c == col_name) {
                    let val = row.get(col_idx).cloned().unwrap_or_default();
                    if !filter.matches(&val) {
                        matches_all = false;
                        break;
                    }
                }
            }
            if matches_all {
                filtered_rows.push(row.clone());
            }
        }
        res.rows = filtered_rows;
        if res.sort.is_some() {
            res.unsorted_rows = Some(res.rows.clone());
            res.apply_sort(&app.vertical_schema_cols);
        } else {
            res.unsorted_rows = None;
        }
        res.selected_row = 0;
        res.scroll_v.set(0);

        // 3. Update query in query cell
        if res.base_query.is_none() {
            res.base_query = Some(res.query_buffer.clone());
        }
        let base_q = res.base_query.as_ref().unwrap();

        let updated_query = crate::trino::queries::build_query_with_all_filters(
            base_q,
            &res.columns,
            &res.hidden_columns,
            &res.column_filters,
            &res.col_distinct_values,
        );

        res.query_buffer = updated_query.clone();
        res.query_cursor = updated_query.len();
        action_state.query_buffer = updated_query.clone();
        action_state.query_cursor = updated_query.len();

        // 4. Return ExecuteQuery if Trino client is present
        if app.trino_client.is_some() {
            return Some(Command::ExecuteQuery {
                query: updated_query,
                is_paginated: res.is_paginated,
                catalog: res.catalog.clone(),
                schema: res.schema.clone(),
                table: res.table.clone(),
                filters: res.filters.clone(),
            });
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{ActionState, ResultsState};

    fn sample_app_with_results() -> App {
        let config = crate::config::ConnectionConfig {
            url: "https://trino.example".to_string(),
            user: "analyst".to_string(),
            password: "secret".to_string(),
        };
        let mut app = App::new(config, false);
        app.screen = Screen::Actions(ActionState {
            catalog: "memory".to_string(),
            schema: "default".to_string(),
            table: "users".to_string(),
            selected: 0,
            results: Some(ResultsState {
                query_buffer: "SELECT id, status FROM users".to_string(),
                query_cursor: 28,
                columns: vec!["id".to_string(), "status".to_string()],
                rows: vec![
                    vec!["1".to_string(), "active".to_string()],
                    vec!["2".to_string(), "pending".to_string()],
                    vec!["3".to_string(), "active".to_string()],
                    vec!["4".to_string(), "inactive".to_string()],
                ],
                scroll_v: std::cell::Cell::new(0),
                scroll_h: std::cell::Cell::new(0),
                selected_row: 0,
                selected_col: 1, // cursor on 'status' column
                ..Default::default()
            }),
            ..Default::default()
        });
        app
    }

    #[test]
    fn test_open_filter_modal_populates_values_and_defaults_all_selected() {
        let mut app = sample_app_with_results();
        open_filter_modal(&mut app);

        assert!(matches!(app.mode, Mode::FilterModal));
        let modal = app.filter_modal.as_ref().expect("filter modal should be open");
        assert_eq!(modal.column_index, 1);
        assert_eq!(modal.column_name, "status");
        assert_eq!(modal.values, vec!["active", "inactive", "pending"]);
        assert_eq!(modal.selected_values.len(), 3);
        assert!(modal.selected_values.contains("active"));
        assert!(modal.selected_values.contains("inactive"));
        assert!(modal.selected_values.contains("pending"));
        assert_eq!(modal.cursor, 0);
    }

    #[test]
    fn test_filter_modal_keys_navigation_and_toggle() {
        let mut app = sample_app_with_results();
        open_filter_modal(&mut app);

        // Move down: cursor moves from 0 to 1
        handle_filter_modal_key(&mut app, KeyEvent::from(KeyCode::Char('j')));
        assert_eq!(app.filter_modal.as_ref().unwrap().cursor, 1);

        // Toggle at cursor 1 ("inactive")
        handle_filter_modal_key(&mut app, KeyEvent::from(KeyCode::Char(' ')));
        assert!(!app.filter_modal.as_ref().unwrap().selected_values.contains("inactive"));
        assert!(app.filter_modal.as_ref().unwrap().selected_values.contains("active"));
        assert!(app.filter_modal.as_ref().unwrap().selected_values.contains("pending"));

        // Toggle again to re-enable
        handle_filter_modal_key(&mut app, KeyEvent::from(KeyCode::Char(' ')));
        assert!(app.filter_modal.as_ref().unwrap().selected_values.contains("inactive"));

        // 'a' toggles all off when all are selected
        handle_filter_modal_key(&mut app, KeyEvent::from(KeyCode::Char('a')));
        assert!(app.filter_modal.as_ref().unwrap().selected_values.is_empty());

        // 'a' toggles all on when none are selected
        handle_filter_modal_key(&mut app, KeyEvent::from(KeyCode::Char('a')));
        assert_eq!(app.filter_modal.as_ref().unwrap().selected_values.len(), 3);

        // 'G' moves to bottom, 'g' moves to top
        handle_filter_modal_key(&mut app, KeyEvent::from(KeyCode::Char('G')));
        assert_eq!(app.filter_modal.as_ref().unwrap().cursor, 2);
        handle_filter_modal_key(&mut app, KeyEvent::from(KeyCode::Char('g')));
        assert_eq!(app.filter_modal.as_ref().unwrap().cursor, 0);

        // Esc cancels without applying
        handle_filter_modal_key(&mut app, KeyEvent::from(KeyCode::Esc));
        assert!(app.filter_modal.is_none());
        assert!(matches!(app.mode, Mode::Normal));
    }

    #[test]
    fn test_apply_filter_modal_updates_rows_and_query() {
        let mut app = sample_app_with_results();
        open_filter_modal(&mut app);

        // Deselect "inactive" and "pending", leaving only "active"
        // Cursor 0 is "active". Move to cursor 1 ("inactive") and toggle off
        handle_filter_modal_key(&mut app, KeyEvent::from(KeyCode::Char('j')));
        handle_filter_modal_key(&mut app, KeyEvent::from(KeyCode::Char(' ')));
        // Move to cursor 2 ("pending") and toggle off
        handle_filter_modal_key(&mut app, KeyEvent::from(KeyCode::Char('j')));
        handle_filter_modal_key(&mut app, KeyEvent::from(KeyCode::Char(' ')));

        // Apply with Enter
        let cmd = handle_filter_modal_key(&mut app, KeyEvent::from(KeyCode::Enter));
        assert!(cmd.is_none()); // app.trino_client is None
        assert!(app.filter_modal.is_none());
        assert!(matches!(app.mode, Mode::Normal));

        let Screen::Actions(action_state) = &app.screen else {
            panic!("expected actions screen");
        };
        let res = action_state.results.as_ref().unwrap();

        // Should filter rows to only active (id 1 and 3)
        assert_eq!(res.rows.len(), 2);
        assert_eq!(res.rows[0], vec!["1", "active"]);
        assert_eq!(res.rows[1], vec!["3", "active"]);

        // Query buffer should be updated with WHERE clause
        assert_eq!(
            res.query_buffer,
            "SELECT id, status FROM users WHERE \"status\" = 'active'"
        );
        assert_eq!(
            action_state.query_buffer,
            "SELECT id, status FROM users WHERE \"status\" = 'active'"
        );
    }

    #[test]
    fn test_reselecting_all_values_clears_filter_and_restores_rows() {
        let mut app = sample_app_with_results();
        open_filter_modal(&mut app);

        // Deselect "inactive"
        handle_filter_modal_key(&mut app, KeyEvent::from(KeyCode::Char('j')));
        handle_filter_modal_key(&mut app, KeyEvent::from(KeyCode::Char(' ')));
        handle_filter_modal_key(&mut app, KeyEvent::from(KeyCode::Enter));

        let Screen::Actions(action_state) = &app.screen else {
            panic!("expected actions screen");
        };
        let res = action_state.results.as_ref().unwrap();
        assert_eq!(res.rows.len(), 3);

        // Open modal again
        open_filter_modal(&mut app);
        // Toggle "inactive" back on
        handle_filter_modal_key(&mut app, KeyEvent::from(KeyCode::Char('j')));
        handle_filter_modal_key(&mut app, KeyEvent::from(KeyCode::Char(' ')));
        // Apply with Enter
        handle_filter_modal_key(&mut app, KeyEvent::from(KeyCode::Enter));

        let Screen::Actions(action_state) = &app.screen else {
            panic!("expected actions screen");
        };
        let res = action_state.results.as_ref().unwrap();
        // All 4 rows restored
        assert_eq!(res.rows.len(), 4);
        assert_eq!(res.query_buffer, "SELECT id, status FROM users");
    }

    #[test]
    fn test_search_then_a_deselects_entire_list_not_just_visible() {
        let mut app = sample_app_with_results();
        open_filter_modal(&mut app);

        // Values: "active", "inactive", "pending" (all 3 selected)
        assert_eq!(app.filter_modal.as_ref().unwrap().selected_values.len(), 3);

        // Search for "inactive" to limit visible results
        handle_filter_modal_key(&mut app, KeyEvent::from(KeyCode::Char('/')));
        for c in "inactive".chars() {
            handle_filter_modal_key(&mut app, KeyEvent::from(KeyCode::Char(c)));
        }
        handle_filter_modal_key(&mut app, KeyEvent::from(KeyCode::Enter));

        let m = app.filter_modal.as_ref().unwrap();
        assert_eq!(m.filtered_indices().len(), 1); // Only "inactive" is visible

        // Press 'a' to filter out
        handle_filter_modal_key(&mut app, KeyEvent::from(KeyCode::Char('a')));

        // Must deselect all values across the ENTIRE list, not just "inactive"
        let m = app.filter_modal.as_ref().unwrap();
        assert!(
            m.selected_values.is_empty(),
            "expected entire list to be deselected, but had {:?}",
            m.selected_values
        );

        // Press 'a' again to select all values across the entire list
        handle_filter_modal_key(&mut app, KeyEvent::from(KeyCode::Char('a')));
        let m = app.filter_modal.as_ref().unwrap();
        assert_eq!(m.selected_values.len(), 3);
    }

    #[test]
    fn test_is_numeric_column_detection() {
        // Values based
        assert!(is_numeric_column("num", &[], &["1".to_string(), "2".to_string(), "3.5".to_string()]));
        assert!(!is_numeric_column("txt", &[], &["foo".to_string(), "bar".to_string()]));
        assert!(!is_numeric_column("empty", &[], &[]));
        assert!(is_numeric_column("with_null", &[], &["null".to_string(), "10".to_string()]));

        // Schema based
        let schema_col = VerticalColumn {
            index: 0,
            name: "price".to_string(),
            data_type: "decimal(10, 2)".to_string(),
            is_nullable: "YES".to_string(),
            key_meta: String::new(),
            description: String::new(),
        };
        assert!(is_numeric_column("price", &[schema_col], &[]));

        let text_col = VerticalColumn {
            index: 0,
            name: "price".to_string(),
            data_type: "varchar".to_string(),
            is_nullable: "YES".to_string(),
            key_meta: String::new(),
            description: String::new(),
        };
        assert!(!is_numeric_column("price", &[text_col], &[]));
    }

    #[test]
    fn test_numeric_filter_all_operators() {
        // Test <, <=, >, >=, = on column "id" (values 1, 2, 3, 4)
        for (target_op_keys, input_str, expected_rows, expected_sql_op) in [
            (vec![KeyCode::Char('<')], "3", vec!["1", "2"], "<"),
            (vec![KeyCode::Char('<'), KeyCode::Right], "2", vec!["1", "2"], "<="),
            (vec![KeyCode::Char('>')], "2", vec!["3", "4"], ">"),
            (vec![KeyCode::Char('>'), KeyCode::Right], "3", vec!["3", "4"], ">="),
            (vec![KeyCode::Char('=')], "2", vec!["2"], "="),
        ] {
            let mut app = sample_app_with_results();
            // Cursor on column 0 ("id")
            if let Screen::Actions(ref mut a) = app.screen {
                a.results.as_mut().unwrap().selected_col = 0;
            }
            open_filter_modal(&mut app);
            assert!(app.filter_modal.as_ref().unwrap().is_numeric_col);

            // Switch to Numeric mode via Tab
            handle_filter_modal_key(&mut app, KeyEvent::from(KeyCode::Tab));
            assert_eq!(app.filter_modal.as_ref().unwrap().mode, FilterMode::Numeric);

            // Select operator via key sequence
            for k in target_op_keys {
                handle_filter_modal_key(&mut app, KeyEvent::from(k));
            }

            // Type number into value input
            for c in input_str.chars() {
                handle_filter_modal_key(&mut app, KeyEvent::from(KeyCode::Char(c)));
            }

            // Apply with Enter
            handle_filter_modal_key(&mut app, KeyEvent::from(KeyCode::Enter));
            assert!(app.filter_modal.is_none());

            let Screen::Actions(action_state) = &app.screen else {
                panic!("expected actions screen");
            };
            let res = action_state.results.as_ref().unwrap();
            let row_ids: Vec<&str> = res.rows.iter().map(|r| r[0].as_str()).collect();
            assert_eq!(row_ids, expected_rows, "mismatch for op {}", expected_sql_op);
            assert_eq!(
                res.query_buffer,
                format!("SELECT id, status FROM users WHERE \"id\" {expected_sql_op} {input_str}")
            );
        }
    }

    #[test]
    fn test_numeric_filter_search_box_shorthand() {
        let mut app = sample_app_with_results();
        if let Screen::Actions(ref mut a) = app.screen {
            a.results.as_mut().unwrap().selected_col = 0; // "id" column
        }
        open_filter_modal(&mut app);

        // In Values mode, enter search mode:
        handle_filter_modal_key(&mut app, KeyEvent::from(KeyCode::Char('/')));
        for c in "> 2".chars() {
            handle_filter_modal_key(&mut app, KeyEvent::from(KeyCode::Char(c)));
        }
        // Press Enter to apply
        handle_filter_modal_key(&mut app, KeyEvent::from(KeyCode::Enter));

        let Screen::Actions(action_state) = &app.screen else {
            panic!("expected actions screen");
        };
        let res = action_state.results.as_ref().unwrap();
        let row_ids: Vec<&str> = res.rows.iter().map(|r| r[0].as_str()).collect();
        assert_eq!(row_ids, vec!["3", "4"]);
        assert_eq!(
            res.query_buffer,
            "SELECT id, status FROM users WHERE \"id\" > 2"
        );
    }

    #[test]
    fn test_numeric_filter_reopen_and_clear() {
        let mut app = sample_app_with_results();
        if let Screen::Actions(ref mut a) = app.screen {
            a.results.as_mut().unwrap().selected_col = 0;
        }
        open_filter_modal(&mut app);
        handle_filter_modal_key(&mut app, KeyEvent::from(KeyCode::Tab));
        for c in "2".chars() {
            handle_filter_modal_key(&mut app, KeyEvent::from(KeyCode::Char(c)));
        }
        handle_filter_modal_key(&mut app, KeyEvent::from(KeyCode::Enter));

        // Reopen modal: should be in Numeric mode with op Lt and value "2"
        open_filter_modal(&mut app);
        let m = app.filter_modal.as_ref().unwrap();
        assert_eq!(m.mode, FilterMode::Numeric);
        assert_eq!(m.numeric_op, NumericOp::Lt);
        assert_eq!(m.numeric_input, "2");

        // Clear filter with 'c' then Enter
        handle_filter_modal_key(&mut app, KeyEvent::from(KeyCode::Char('c')));
        handle_filter_modal_key(&mut app, KeyEvent::from(KeyCode::Enter));

        let Screen::Actions(action_state) = &app.screen else {
            panic!("expected actions screen");
        };
        let res = action_state.results.as_ref().unwrap();
        assert_eq!(res.rows.len(), 4);
        assert_eq!(res.query_buffer, "SELECT id, status FROM users");
        assert!(!res.column_filters.contains_key("id"));
    }

    #[test]
    fn test_numeric_filter_operator_cycling() {
        let mut app = sample_app_with_results();
        if let Screen::Actions(ref mut a) = app.screen {
            a.results.as_mut().unwrap().selected_col = 0;
        }
        open_filter_modal(&mut app);
        handle_filter_modal_key(&mut app, KeyEvent::from(KeyCode::Tab));

        // Default op is Lt (<)
        assert_eq!(app.filter_modal.as_ref().unwrap().numeric_op, NumericOp::Lt);

        // Move to operator and cycle right
        handle_filter_modal_key(&mut app, KeyEvent::from(KeyCode::Char('k')));
        handle_filter_modal_key(&mut app, KeyEvent::from(KeyCode::Right));
        assert_eq!(app.filter_modal.as_ref().unwrap().numeric_op, NumericOp::Lte);

        handle_filter_modal_key(&mut app, KeyEvent::from(KeyCode::Right));
        assert_eq!(app.filter_modal.as_ref().unwrap().numeric_op, NumericOp::Gt);

        handle_filter_modal_key(&mut app, KeyEvent::from(KeyCode::Right));
        assert_eq!(app.filter_modal.as_ref().unwrap().numeric_op, NumericOp::Gte);

        handle_filter_modal_key(&mut app, KeyEvent::from(KeyCode::Right));
        assert_eq!(app.filter_modal.as_ref().unwrap().numeric_op, NumericOp::Eq);

        handle_filter_modal_key(&mut app, KeyEvent::from(KeyCode::Right));
        assert_eq!(app.filter_modal.as_ref().unwrap().numeric_op, NumericOp::Lt);

        // Cycle left
        handle_filter_modal_key(&mut app, KeyEvent::from(KeyCode::Left));
        assert_eq!(app.filter_modal.as_ref().unwrap().numeric_op, NumericOp::Eq);
    }

    #[test]
    fn test_numeric_filter_direct_typing_compound_operators() {
        let mut app = sample_app_with_results();
        if let Screen::Actions(ref mut a) = app.screen {
            a.results.as_mut().unwrap().selected_col = 0; // "id" column
        }
        open_filter_modal(&mut app);
        handle_filter_modal_key(&mut app, KeyEvent::from(KeyCode::Tab)); // Switch to numeric

        // Type "<= 2" directly: '<', '=', '2'
        handle_filter_modal_key(&mut app, KeyEvent::from(KeyCode::Char('<')));
        assert_eq!(app.filter_modal.as_ref().unwrap().numeric_op, NumericOp::Lt);
        handle_filter_modal_key(&mut app, KeyEvent::from(KeyCode::Char('=')));
        assert_eq!(app.filter_modal.as_ref().unwrap().numeric_op, NumericOp::Lte);
        handle_filter_modal_key(&mut app, KeyEvent::from(KeyCode::Char('2')));
        assert_eq!(app.filter_modal.as_ref().unwrap().numeric_input, "2");

        // Apply
        handle_filter_modal_key(&mut app, KeyEvent::from(KeyCode::Enter));
        assert!(app.filter_modal.is_none());

        let Screen::Actions(action_state) = &app.screen else {
            panic!("expected actions screen");
        };
        let res = action_state.results.as_ref().unwrap();
        let row_ids: Vec<&str> = res.rows.iter().map(|r| r[0].as_str()).collect();
        assert_eq!(row_ids, vec!["1", "2"]);
        assert_eq!(
            res.query_buffer,
            "SELECT id, status FROM users WHERE \"id\" <= 2"
        );
    }

    #[test]
    fn test_numeric_filter_esc_dismisses_immediately() {
        let mut app = sample_app_with_results();
        open_filter_modal(&mut app);
        handle_filter_modal_key(&mut app, KeyEvent::from(KeyCode::Tab));
        assert!(app.filter_modal.is_some());
        assert_eq!(app.mode, Mode::FilterModal);

        // Pressing Esc once must close the modal immediately and restore Mode::Normal
        handle_filter_modal_key(&mut app, KeyEvent::from(KeyCode::Esc));
        assert!(app.filter_modal.is_none());
        assert_eq!(app.mode, Mode::Normal);
    }

    #[test]
    fn test_filter_modal_key_sync_guard_recovers_desynced_mode() {
        let mut app = sample_app_with_results();
        open_filter_modal(&mut app);
        assert!(app.filter_modal.is_some());

        // Simulate an external mode desynchronization (e.g. mouse click or bug)
        app.mode = Mode::Normal;

        // handle_key_sync should detect app.filter_modal.is_some() and route Esc to filter modal
        crate::tui::handler::handle_key_sync(&mut app, KeyEvent::from(KeyCode::Esc));
        assert!(app.filter_modal.is_none(), "Filter modal must be closed by Esc");
        assert_eq!(app.mode, Mode::Normal);
    }

    #[test]
    fn test_filter_modal_values_mode_esc_behavior() {
        let mut app = sample_app_with_results();
        open_filter_modal(&mut app);

        // In values mode, enter search mode
        handle_filter_modal_key(&mut app, KeyEvent::from(KeyCode::Char('/')));
        handle_filter_modal_key(&mut app, KeyEvent::from(KeyCode::Char('a')));
        assert!(app.filter_modal.as_ref().unwrap().is_searching);
        assert_eq!(app.filter_modal.as_ref().unwrap().search_query, "a");

        // Esc clears search query and exits search mode
        handle_filter_modal_key(&mut app, KeyEvent::from(KeyCode::Esc));
        assert!(!app.filter_modal.as_ref().unwrap().is_searching);
        assert!(app.filter_modal.as_ref().unwrap().search_query.is_empty());
        assert!(app.filter_modal.is_some());

        // Next Esc immediately closes the modal
        handle_filter_modal_key(&mut app, KeyEvent::from(KeyCode::Esc));
        assert!(app.filter_modal.is_none());
        assert_eq!(app.mode, Mode::Normal);
    }
}

