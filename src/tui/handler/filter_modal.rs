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

use crate::app::{App, FilterModalState, Mode, Screen};
use crate::tui::handler::Command;
use crate::tui::handler::navigation::normalize_key_code;

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

        // Determine currently selected values
        let selected_values = if let Some(existing) = res.column_filters.get(&col_name) {
            existing.clone()
        } else {
            values.iter().cloned().collect()
        };

        app.filter_modal = Some(FilterModalState {
            column_index: col_idx,
            column_name: col_name,
            values,
            selected_values,
            cursor: 0,
            scroll: 0,
            search_query: String::new(),
            is_searching: false,
        });
        app.mode = Mode::FilterModal;
    }
}

pub fn handle_filter_modal_key(app: &mut App, key: KeyEvent) -> Option<Command> {
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
                }
                return None;
            }
            KeyCode::Enter => {
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
        KeyCode::Char('g') => {
            if let Some(ref mut m) = app.filter_modal {
                m.cursor = 0;
                m.scroll = 0;
            }
            None
        }
        KeyCode::Char('G') => {
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
            if let Some(ref mut m) = app.filter_modal
                && !m.search_query.is_empty()
            {
                m.search_query.clear();
                m.cursor = 0;
                m.scroll = 0;
            } else {
                app.mode = Mode::Normal;
                app.filter_modal = None;
            }
            None
        }
        KeyCode::Enter => apply_filter_modal(app),
        _ => None,
    }
}

pub fn apply_filter_modal(app: &mut App) -> Option<Command> {
    let modal = app.filter_modal.take()?;
    app.mode = Mode::Normal;

    if let Screen::Actions(ref mut action_state) = app.screen
        && let Some(ref mut res) = action_state.results
    {
        // 1. Update column_filters
        if modal.selected_values.len() == modal.values.len() {
            res.column_filters.remove(&modal.column_name);
        } else {
            res.column_filters
                .insert(modal.column_name.clone(), modal.selected_values.clone());
        }

        // 2. Filter in-memory rows from unfiltered_rows
        if res.unfiltered_rows.is_none() {
            res.unfiltered_rows = Some(res.rows.clone());
        }
        let unfiltered = res.unfiltered_rows.as_ref().unwrap();

        let mut filtered_rows = Vec::new();
        for row in unfiltered {
            let mut matches_all = true;
            for (col_name, selected_set) in &res.column_filters {
                if let Some(col_idx) = res.columns.iter().position(|c| c == col_name) {
                    let val = row.get(col_idx).cloned().unwrap_or_default();
                    if !selected_set.contains(&val) {
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
}

