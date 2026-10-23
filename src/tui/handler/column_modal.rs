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
use std::collections::HashSet;

use crate::app::{App, ColumnModalState, Mode, Screen};
use crate::tui::handler::Command;
use crate::tui::handler::navigation::normalize_key_code;

pub fn open_column_modal(app: &mut App) {
    if let Screen::Actions(ref mut action_state) = app.screen
        && let Some(ref res) = action_state.results
        && !res.columns.is_empty()
    {
        let columns = res.columns.clone();
        let selected_columns: HashSet<String> = columns
            .iter()
            .filter(|c| !res.hidden_columns.contains(*c))
            .cloned()
            .collect();

        let root_columns: Vec<String> = columns
            .iter()
            .filter(|c| !res.flattened_fields.iter().any(|f| &f.full_name == *c))
            .cloned()
            .collect();
        let tree_nodes =
            crate::trino::flatten::build_column_tree(&root_columns, &res.flattened_fields);

        let cur_col_name = res.columns.get(res.selected_col).map(|s| s.as_str());
        let cursor = if let Some(name) = cur_col_name {
            tree_nodes.iter().position(|n| n.name == name).unwrap_or(0)
        } else {
            res.selected_col.min(tree_nodes.len().saturating_sub(1))
        };
        let scroll = if cursor >= 10 {
            cursor.saturating_sub(9)
        } else {
            0
        };

        app.column_modal = Some(ColumnModalState {
            columns,
            selected_columns,
            cursor,
            scroll,
            search_query: String::new(),
            is_searching: false,
            tree_nodes,
        });
        app.mode = Mode::ColumnModal;
    }
}

pub fn handle_column_modal_key(app: &mut App, key: KeyEvent) -> Option<Command> {
    let is_searching = app
        .column_modal
        .as_ref()
        .map(|m| m.is_searching)
        .unwrap_or(false);

    if is_searching {
        match key.code {
            KeyCode::Esc => {
                if let Some(ref mut m) = app.column_modal {
                    m.is_searching = false;
                }
                return None;
            }
            KeyCode::Enter => {
                if let Some(ref mut m) = app.column_modal {
                    m.is_searching = false;
                }
                return None;
            }
            KeyCode::Backspace => {
                if let Some(ref mut m) = app.column_modal {
                    m.search_query.pop();
                    m.cursor = 0;
                    m.scroll = 0;
                }
                return None;
            }
            KeyCode::Char(c) => {
                if let Some(ref mut m) = app.column_modal {
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
        KeyCode::Char('/') => {
            if let Some(ref mut m) = app.column_modal {
                m.is_searching = true;
            }
            None
        }
        KeyCode::Char('j') | KeyCode::Down => {
            if let Some(ref mut m) = app.column_modal {
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
            if let Some(ref mut m) = app.column_modal {
                m.cursor = m.cursor.saturating_sub(1);
                if m.cursor < m.scroll {
                    m.scroll = m.cursor;
                }
            }
            None
        }
        KeyCode::PageDown | KeyCode::Char('d') | KeyCode::Char('f')
            if code == KeyCode::PageDown
                || key
                    .modifiers
                    .contains(crossterm::event::KeyModifiers::CONTROL) =>
        {
            if let Some(ref mut m) = app.column_modal {
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
                || key
                    .modifiers
                    .contains(crossterm::event::KeyModifiers::CONTROL) =>
        {
            if let Some(ref mut m) = app.column_modal {
                m.cursor = m.cursor.saturating_sub(10);
                if m.cursor < m.scroll {
                    m.scroll = m.cursor;
                }
            }
            None
        }
        KeyCode::Char('g') | KeyCode::Home => {
            if let Some(ref mut m) = app.column_modal {
                m.cursor = 0;
                m.scroll = 0;
            }
            None
        }
        KeyCode::Char('G') | KeyCode::End => {
            if let Some(ref mut m) = app.column_modal {
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
            if let Some(ref mut m) = app.column_modal {
                let matched = m.filtered_indices();
                if let Some(&orig_idx) = matched.get(m.cursor) {
                    if !m.tree_nodes.is_empty() {
                        let prev_selected = m.selected_columns.clone();
                        crate::trino::flatten::toggle_tree_node_selection(
                            &mut m.selected_columns,
                            &m.tree_nodes,
                            orig_idx,
                        );
                        // Prevent hiding all columns: require at least 1 visible
                        if m.selected_columns.is_empty() {
                            m.selected_columns = prev_selected;
                        }
                    } else if let Some(col) = m.columns.get(orig_idx).cloned() {
                        if m.selected_columns.contains(&col) {
                            // Prevent hiding all columns: require at least 1 visible
                            if m.selected_columns.len() > 1 {
                                m.selected_columns.remove(&col);
                            }
                        } else {
                            m.selected_columns.insert(col);
                        }
                    }
                }
            }
            None
        }
        KeyCode::Char('a') | KeyCode::Char('A') => {
            if let Some(ref mut m) = app.column_modal {
                let cur_orig_idx = m.filtered_indices().get(m.cursor).copied();
                if !m.tree_nodes.is_empty() {
                    crate::trino::flatten::toggle_all_tree_selection(
                        &mut m.selected_columns,
                        &m.tree_nodes,
                        &m.columns,
                        cur_orig_idx,
                    );
                } else if m.selected_columns.len() == m.columns.len() {
                    let cur_col = if let Some(orig_idx) = cur_orig_idx {
                        m.columns.get(orig_idx).cloned()
                    } else {
                        m.columns.get(m.cursor).cloned()
                    };
                    m.selected_columns.clear();
                    if let Some(cur) = cur_col {
                        m.selected_columns.insert(cur);
                    }
                } else {
                    m.selected_columns = m.columns.iter().cloned().collect();
                }
            }
            None
        }
        KeyCode::Esc => {
            if let Some(ref mut m) = app.column_modal
                && !m.search_query.is_empty()
            {
                m.search_query.clear();
                m.cursor = 0;
                m.scroll = 0;
            } else {
                app.mode = Mode::Normal;
                app.column_modal = None;
            }
            None
        }
        KeyCode::Enter => apply_column_modal(app),
        _ => None,
    }
}

pub fn apply_column_modal(app: &mut App) -> Option<Command> {
    let modal = app.column_modal.take()?;
    app.mode = Mode::Normal;

    if let Screen::Actions(ref mut action_state) = app.screen
        && let Some(ref mut res) = action_state.results
    {
        // 1. Update hidden_columns: all columns not in selected_columns
        res.hidden_columns = res
            .columns
            .iter()
            .filter(|c| !modal.selected_columns.contains(*c))
            .cloned()
            .collect();

        // 2. Adjust selected_col if currently pointing to a hidden column
        if res
            .columns
            .get(res.selected_col)
            .is_some_and(|c| res.hidden_columns.contains(c))
        {
            let vis = res.visible_columns();
            if !vis.is_empty() {
                let closest = vis
                    .iter()
                    .min_by_key(|(idx, _)| (*idx as isize - res.selected_col as isize).abs())
                    .map(|(idx, _)| *idx)
                    .unwrap_or(0);
                res.selected_col = closest;
            }
        }
        if let Some((ref sort_col, _)) = res.sort
            && res.hidden_columns.contains(sort_col)
        {
            res.sort = None;
            res.apply_sort(&app.vertical_schema_cols);
        }
        res.scroll_h.set(0);

        // 3. Update query in query cell
        if res.base_query.is_none() {
            res.base_query = Some(res.query_buffer.clone());
        }
        let base_q = res.base_query.as_ref().unwrap();

        let expr_map = res.column_expr_map();
        let updated_query = crate::trino::queries::build_query_with_all_filters_and_overrides(
            base_q,
            &res.columns,
            &res.hidden_columns,
            &res.column_filters,
            &res.col_distinct_values,
            Some(&expr_map),
            Some(&action_state.column_types),
        );

        res.query_buffer = updated_query.clone();
        res.query_cursor = updated_query.len();
        action_state.query_buffer = updated_query.clone();
        action_state.query_cursor = updated_query.len();
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
            query_cache_limit: crate::config::DEFAULT_QUERY_CACHE_LIMIT,
        };
        let mut app = App::new(config, false);
        app.screen = Screen::Actions(ActionState {
            catalog: "memory".to_string(),
            schema: "default".to_string(),
            table: "orders".to_string(),
            selected: 0,
            results: Some(ResultsState {
                query_buffer: "SELECT id, status, amount, created_at FROM orders".to_string(),
                query_cursor: 50,
                columns: vec![
                    "id".to_string(),
                    "status".to_string(),
                    "amount".to_string(),
                    "created_at".to_string(),
                ],
                rows: vec![vec![
                    "1".to_string(),
                    "active".to_string(),
                    "100".to_string(),
                    "2026-01-01".to_string(),
                ]],
                scroll_v: std::cell::Cell::new(0),
                scroll_h: std::cell::Cell::new(0),
                selected_row: 0,
                selected_col: 1, // cursor on 'status'
                ..Default::default()
            }),
            ..Default::default()
        });
        app
    }

    #[test]
    fn test_open_column_modal_populates_columns_and_defaults_all_selected() {
        let mut app = sample_app_with_results();
        open_column_modal(&mut app);

        assert!(matches!(app.mode, Mode::ColumnModal));
        let modal = app.column_modal.as_ref().expect("modal should be open");
        assert_eq!(modal.columns, vec!["id", "status", "amount", "created_at"]);
        assert_eq!(modal.selected_columns.len(), 4);
        assert_eq!(modal.cursor, 1); // cursor matches selected_col
    }

    #[test]
    fn test_column_modal_keys_navigation_and_toggle() {
        let mut app = sample_app_with_results();
        open_column_modal(&mut app);

        // Cursor starts at 1. Move down to 2 ("amount")
        handle_column_modal_key(&mut app, KeyEvent::from(KeyCode::Char('j')));
        assert_eq!(app.column_modal.as_ref().unwrap().cursor, 2);

        // Toggle at cursor 2 ("amount") -> deselect
        handle_column_modal_key(&mut app, KeyEvent::from(KeyCode::Char(' ')));
        assert!(
            !app.column_modal
                .as_ref()
                .unwrap()
                .selected_columns
                .contains("amount")
        );
        assert_eq!(app.column_modal.as_ref().unwrap().selected_columns.len(), 3);

        // Toggle again to re-select
        handle_column_modal_key(&mut app, KeyEvent::from(KeyCode::Char(' ')));
        assert!(
            app.column_modal
                .as_ref()
                .unwrap()
                .selected_columns
                .contains("amount")
        );
        assert_eq!(app.column_modal.as_ref().unwrap().selected_columns.len(), 4);

        // 'a' toggles all off except current cursor item
        handle_column_modal_key(&mut app, KeyEvent::from(KeyCode::Char('a')));
        assert_eq!(app.column_modal.as_ref().unwrap().selected_columns.len(), 1);
        assert!(
            app.column_modal
                .as_ref()
                .unwrap()
                .selected_columns
                .contains("amount")
        );

        // 'a' again selects all
        handle_column_modal_key(&mut app, KeyEvent::from(KeyCode::Char('a')));
        assert_eq!(app.column_modal.as_ref().unwrap().selected_columns.len(), 4);

        // Navigation: 'G' moves to bottom, 'g' moves to top
        handle_column_modal_key(&mut app, KeyEvent::from(KeyCode::Char('G')));
        assert_eq!(app.column_modal.as_ref().unwrap().cursor, 3);
        handle_column_modal_key(&mut app, KeyEvent::from(KeyCode::Char('g')));
        assert_eq!(app.column_modal.as_ref().unwrap().cursor, 0);

        // Esc cancels without changes
        handle_column_modal_key(
            &mut self::sample_app_with_results(),
            KeyEvent::from(KeyCode::Esc),
        );
    }

    #[test]
    fn test_apply_column_modal_hides_deselected_columns() {
        let mut app = sample_app_with_results();
        open_column_modal(&mut app);

        // Cursor starts at 1 ("status"). Toggle off "status"
        handle_column_modal_key(&mut app, KeyEvent::from(KeyCode::Char(' ')));
        // Apply with Enter
        handle_column_modal_key(&mut app, KeyEvent::from(KeyCode::Enter));

        assert!(app.column_modal.is_none());
        assert!(matches!(app.mode, Mode::Normal));

        let Screen::Actions(action_state) = &app.screen else {
            panic!("expected actions screen");
        };
        let res = action_state.results.as_ref().unwrap();
        assert!(res.hidden_columns.contains("status"));
        assert!(!res.hidden_columns.contains("id"));
        assert!(!res.hidden_columns.contains("amount"));
        assert!(!res.hidden_columns.contains("created_at"));

        // Since 'status' was selected_col (1) and was hidden, selected_col should adjust to nearest visible column
        assert!(res.selected_col == 0 || res.selected_col == 2);

        // Query buffers reflect visible columns
        assert_eq!(
            res.query_buffer,
            "SELECT \"id\", \"amount\", \"created_at\" FROM orders"
        );
        assert_eq!(
            action_state.query_buffer,
            "SELECT \"id\", \"amount\", \"created_at\" FROM orders"
        );
    }

    #[test]
    fn test_column_modal_search_filtering() {
        let mut app = sample_app_with_results();
        open_column_modal(&mut app);

        // Press '/' to start searching
        handle_column_modal_key(&mut app, KeyEvent::from(KeyCode::Char('/')));
        assert!(app.column_modal.as_ref().unwrap().is_searching);

        // Type 'st'
        handle_column_modal_key(&mut app, KeyEvent::from(KeyCode::Char('s')));
        handle_column_modal_key(&mut app, KeyEvent::from(KeyCode::Char('t')));
        {
            let m = app.column_modal.as_ref().unwrap();
            assert_eq!(m.search_query, "st");
            let matched = m.filtered_indices();
            assert_eq!(matched.len(), 1);
            assert_eq!(m.columns[matched[0]], "status");
        }

        // Press Enter to finish search
        handle_column_modal_key(&mut app, KeyEvent::from(KeyCode::Enter));
        assert!(!app.column_modal.as_ref().unwrap().is_searching);

        // Space toggles the filtered item (status)
        handle_column_modal_key(&mut app, KeyEvent::from(KeyCode::Char(' ')));
        assert!(
            !app.column_modal
                .as_ref()
                .unwrap()
                .selected_columns
                .contains("status")
        );

        // Press Esc to clear search
        handle_column_modal_key(&mut app, KeyEvent::from(KeyCode::Esc));
        assert!(app.column_modal.as_ref().unwrap().search_query.is_empty());

        // Press Esc again to exit modal
        handle_column_modal_key(&mut app, KeyEvent::from(KeyCode::Esc));
        assert!(app.column_modal.is_none());
        assert_eq!(app.mode, Mode::Normal);
    }

    #[test]
    fn test_search_then_a_deselects_entire_column_list_not_just_visible() {
        let mut app = sample_app_with_results();
        open_column_modal(&mut app);

        // All 4 columns selected
        assert_eq!(app.column_modal.as_ref().unwrap().selected_columns.len(), 4);

        // Search for "stat" (only "status" matches)
        handle_column_modal_key(&mut app, KeyEvent::from(KeyCode::Char('/')));
        for c in "stat".chars() {
            handle_column_modal_key(&mut app, KeyEvent::from(KeyCode::Char(c)));
        }
        handle_column_modal_key(&mut app, KeyEvent::from(KeyCode::Enter));

        let m = app.column_modal.as_ref().unwrap();
        assert_eq!(m.filtered_indices().len(), 1); // Only "status" visible

        // Press 'a': deselects all columns in the entire table except current cursor ("status")
        handle_column_modal_key(&mut app, KeyEvent::from(KeyCode::Char('a')));

        let m = app.column_modal.as_ref().unwrap();
        assert_eq!(m.selected_columns.len(), 1);
        assert!(m.selected_columns.contains("status"));
        assert!(!m.selected_columns.contains("id"));
        assert!(!m.selected_columns.contains("amount"));
        assert!(!m.selected_columns.contains("created_at"));

        // Press 'a' again: selects all 4 columns across the entire table
        handle_column_modal_key(&mut app, KeyEvent::from(KeyCode::Char('a')));
        let m = app.column_modal.as_ref().unwrap();
        assert_eq!(m.selected_columns.len(), 4);
    }
}
