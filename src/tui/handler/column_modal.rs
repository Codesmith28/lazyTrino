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

        let cursor = res.selected_col.min(columns.len().saturating_sub(1));
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
        });
        app.mode = Mode::ColumnModal;
    }
}

pub fn handle_column_modal_key(app: &mut App, key: KeyEvent) -> Option<Command> {
    let code = normalize_key_code(key.code);
    match code {
        KeyCode::Char('j') | KeyCode::Down => {
            if let Some(ref mut m) = app.column_modal
                && !m.columns.is_empty()
            {
                m.cursor = (m.cursor + 1).min(m.columns.len() - 1);
                if m.cursor >= m.scroll + 10 {
                    m.scroll = m.cursor.saturating_sub(9);
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
        KeyCode::Char('g') => {
            if let Some(ref mut m) = app.column_modal {
                m.cursor = 0;
                m.scroll = 0;
            }
            None
        }
        KeyCode::Char('G') => {
            if let Some(ref mut m) = app.column_modal
                && !m.columns.is_empty()
            {
                m.cursor = m.columns.len() - 1;
                if m.cursor >= 10 {
                    m.scroll = m.cursor.saturating_sub(9);
                }
            }
            None
        }
        KeyCode::Char(' ') => {
            if let Some(ref mut m) = app.column_modal
                && let Some(col) = m.columns.get(m.cursor).cloned()
            {
                if m.selected_columns.contains(&col) {
                    // Prevent hiding all columns: require at least 1 visible
                    if m.selected_columns.len() > 1 {
                        m.selected_columns.remove(&col);
                    }
                } else {
                    m.selected_columns.insert(col);
                }
            }
            None
        }
        KeyCode::Char('a') | KeyCode::Char('A') => {
            if let Some(ref mut m) = app.column_modal {
                if m.selected_columns.len() == m.columns.len() {
                    // Deselect all except currently highlighted column
                    if let Some(cur) = m.columns.get(m.cursor).cloned() {
                        m.selected_columns.clear();
                        m.selected_columns.insert(cur);
                    }
                } else {
                    // Select all
                    m.selected_columns = m.columns.iter().cloned().collect();
                }
            }
            None
        }
        KeyCode::Esc => {
            app.mode = Mode::Normal;
            app.column_modal = None;
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
        if res.columns.get(res.selected_col).is_some_and(|c| res.hidden_columns.contains(c)) {
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
        res.scroll_h = 0;
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
                rows: vec![
                    vec![
                        "1".to_string(),
                        "active".to_string(),
                        "100".to_string(),
                        "2026-01-01".to_string(),
                    ],
                ],
                scroll_v: 0,
                scroll_h: 0,
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
        assert!(!app.column_modal.as_ref().unwrap().selected_columns.contains("amount"));
        assert_eq!(app.column_modal.as_ref().unwrap().selected_columns.len(), 3);

        // Toggle again to re-select
        handle_column_modal_key(&mut app, KeyEvent::from(KeyCode::Char(' ')));
        assert!(app.column_modal.as_ref().unwrap().selected_columns.contains("amount"));
        assert_eq!(app.column_modal.as_ref().unwrap().selected_columns.len(), 4);

        // 'a' toggles all off except current cursor item
        handle_column_modal_key(&mut app, KeyEvent::from(KeyCode::Char('a')));
        assert_eq!(app.column_modal.as_ref().unwrap().selected_columns.len(), 1);
        assert!(app.column_modal.as_ref().unwrap().selected_columns.contains("amount"));

        // 'a' again selects all
        handle_column_modal_key(&mut app, KeyEvent::from(KeyCode::Char('a')));
        assert_eq!(app.column_modal.as_ref().unwrap().selected_columns.len(), 4);

        // Navigation: 'G' moves to bottom, 'g' moves to top
        handle_column_modal_key(&mut app, KeyEvent::from(KeyCode::Char('G')));
        assert_eq!(app.column_modal.as_ref().unwrap().cursor, 3);
        handle_column_modal_key(&mut app, KeyEvent::from(KeyCode::Char('g')));
        assert_eq!(app.column_modal.as_ref().unwrap().cursor, 0);

        // Esc cancels without changes
        handle_column_modal_key(&mut self::sample_app_with_results(), KeyEvent::from(KeyCode::Esc));
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
    }
}
