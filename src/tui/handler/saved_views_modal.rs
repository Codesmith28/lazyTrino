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

use std::time::Instant;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::app::{ActionState, ActivePanel, App, Mode, Screen};
use crate::saved_views::{delete_saved_view, SavedView};
use crate::tui::handler::{Command, navigation};

pub fn open_saved_views_modal(app: &mut App) {
    let views = crate::saved_views::load_saved_views();
    app.saved_views_modal = Some(crate::app::SavedViewsModalState {
        views,
        selected: 0,
        scroll: 0,
        pending_delete: None,
    });
    app.mode = Mode::SavedViewsModal;
}

pub fn close_saved_views_modal(app: &mut App) {
    app.saved_views_modal = None;
    app.mode = Mode::Normal;
}

pub fn handle_saved_views_modal_key(app: &mut App, key: KeyEvent) -> Option<Command> {
    let code = navigation::normalize_key_code(key.code);

    if key.modifiers.contains(KeyModifiers::CONTROL) && code == KeyCode::Char('c') {
        app.should_quit = true;
        return None;
    }

    let modal = app.saved_views_modal.as_mut()?;

    // 1. If Delete Confirmation modal is active on top (ModalPreset::Small)
    if let Some(del_id) = modal.pending_delete.clone() {
        match code {
            KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter => {
                let _ = delete_saved_view(&del_id);
                modal.views.retain(|v| v.id != del_id);
                if modal.selected >= modal.views.len() && !modal.views.is_empty() {
                    modal.selected = modal.views.len() - 1;
                }
                modal.pending_delete = None;
                app.copied_toast = Some(("Saved view deleted".to_string(), Instant::now()));
            }
            KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                modal.pending_delete = None;
            }
            _ => {}
        }
        return None;
    }

    // 2. Main Saved Views Browser (ModalPreset::Large)
    match code {
        KeyCode::Esc | KeyCode::Char('q') => {
            close_saved_views_modal(app);
            None
        }
        KeyCode::Char('j') | KeyCode::Down => {
            if !modal.views.is_empty() {
                modal.selected = (modal.selected + 1).min(modal.views.len() - 1);
            }
            None
        }
        KeyCode::Char('k') | KeyCode::Up => {
            modal.selected = modal.selected.saturating_sub(1);
            None
        }
        KeyCode::Home | KeyCode::Char('g') => {
            modal.selected = 0;
            None
        }
        KeyCode::End | KeyCode::Char('G') => {
            if !modal.views.is_empty() {
                modal.selected = modal.views.len() - 1;
            }
            None
        }
        KeyCode::Char('d') | KeyCode::Delete => {
            if !modal.views.is_empty() {
                modal.pending_delete = Some(modal.views[modal.selected].id.clone());
            }
            None
        }
        KeyCode::Enter => {
            if modal.views.is_empty() {
                return None;
            }
            let view = modal.views[modal.selected].clone();
            restore_saved_view(app, view)
        }
        _ => None,
    }
}

pub fn restore_saved_view(app: &mut App, view: SavedView) -> Option<Command> {
    close_saved_views_modal(app);
    app.active_panel = ActivePanel::MainViewer;

    let catalog = view.catalog.clone();
    let schema = view.schema.clone();
    let table = view.table.clone();
    let action_idx = view.action_idx;
    let query_buf = view.query.clone();
    let query_len = query_buf.len();

    let mut action_state = ActionState {
        catalog: catalog.clone(),
        schema: schema.clone(),
        table: table.clone(),
        selected: action_idx,
        query_buffer: query_buf.clone(),
        query_cursor: query_len,
        pending_view_restore: Some(view.clone()),
        ..Default::default()
    };

    if !view.partition_path.is_empty() {
        action_state.drilldown = Some(crate::app::DrillDownState {
            partition_cols: Vec::new(),
            path: view.partition_path.clone(),
            levels_cache: Vec::new(),
            selected: 0,
            ..Default::default()
        });
    }

    app.screen = Screen::Actions(action_state);
    app.loading = true;

    Some(Command::ExecuteQuery {
        query: query_buf,
        catalog,
        schema,
        table,
        is_paginated: action_idx == 0,
        filters: view.partition_path,
    })
}

pub fn save_current_view(app: &mut App) {
    let Screen::Actions(ref a) = app.screen else {
        return;
    };

    let catalog = a.catalog.clone();
    let schema = a.schema.clone();
    let table = a.table.clone();
    let action_idx = a.selected;
    let query = if let Some(ref res) = a.results {
        res.query_buffer.clone()
    } else if !a.query_buffer.is_empty() {
        a.query_buffer.clone()
    } else {
        crate::app::ACTIONS[action_idx].2.build_query(&catalog, &schema, &table)
    };

    let (column_filters, hidden_columns, sort) = if let Some(ref res) = a.results {
        let filters: Vec<(String, crate::saved_views::StoredColumnFilter)> = res
            .column_filters
            .iter()
            .map(|(k, v)| (k.clone(), v.into()))
            .collect();
        let hidden: Vec<String> = res.hidden_columns.iter().cloned().collect();
        let s = res.sort.as_ref().map(|(c, d)| (c.clone(), (*d).into()));
        (filters, hidden, s)
    } else {
        (Vec::new(), Vec::new(), None)
    };

    let partition_path = if let Some(ref dd) = a.drilldown {
        dd.path.clone()
    } else {
        Vec::new()
    };

    let now_ts = crate::saved_views::current_timestamp_str();
    let epoch_millis = std::time::SystemTime::now()
        .duration_since(std::time::SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let id = format!("{catalog}_{schema}_{table}_{epoch_millis}");
    let name = format!("{catalog}.{schema}.{table}");

    let view = SavedView {
        id,
        name: name.clone(),
        catalog,
        schema,
        table,
        action_idx,
        query,
        column_filters,
        hidden_columns,
        sort,
        partition_path,
        saved_at: now_ts,
    };

    match crate::saved_views::add_saved_view(view) {
        Ok(_) => {
            app.copied_toast = Some((format!("Saved view: {name}"), Instant::now()));
        }
        Err(e) => {
            app.copied_toast = Some((format!("Failed to save view: {e}"), Instant::now()));
        }
    }
}

pub fn handle_saved_views_modal_mouse(
    app: &mut App,
    mouse: crossterm::event::MouseEvent,
    term_width: u16,
    term_height: u16,
) -> Option<Command> {
    use crossterm::event::{MouseButton, MouseEventKind};
    use ratatui::layout::Rect;

    let screen_area = Rect::new(0, 0, term_width, term_height);
    let modal = app.saved_views_modal.as_mut()?;

    // 1. If Delete Confirmation modal is active (modal_small)
    if let Some(del_id) = modal.pending_delete.clone() {
        let confirm_area = crate::tui::screens::modal::modal_small(screen_area);
        if let MouseEventKind::Down(MouseButton::Left) = mouse.kind {
            if mouse.column < confirm_area.x
                || mouse.column >= confirm_area.x + confirm_area.width
                || mouse.row < confirm_area.y
                || mouse.row >= confirm_area.y + confirm_area.height
            {
                // Clicked outside confirm modal -> cancel
                modal.pending_delete = None;
                return None;
            }
            // Check if clicked in buttons row (inner bottom row)
            let rel_y = mouse.row.saturating_sub(confirm_area.y);
            if (3..=6).contains(&rel_y) {
                let rel_x = mouse.column.saturating_sub(confirm_area.x);
                if rel_x < 24 {
                    // "Confirm" clicked
                    let _ = delete_saved_view(&del_id);
                    modal.views.retain(|v| v.id != del_id);
                    if modal.selected >= modal.views.len() && !modal.views.is_empty() {
                        modal.selected = modal.views.len() - 1;
                    }
                    modal.pending_delete = None;
                    app.copied_toast = Some(("Saved view deleted".to_string(), Instant::now()));
                } else {
                    // "Cancel" clicked
                    modal.pending_delete = None;
                }
            }
        }
        return None;
    }

    // 2. Main Saved Views Browser (modal_large)
    let popup_area = crate::tui::screens::modal::modal_large(screen_area);
    match mouse.kind {
        MouseEventKind::Down(MouseButton::Left) => {
            if mouse.column < popup_area.x
                || mouse.column >= popup_area.x + popup_area.width
                || mouse.row < popup_area.y
                || mouse.row >= popup_area.y + popup_area.height
            {
                close_saved_views_modal(app);
                return None;
            }

            let rel_y = mouse.row.saturating_sub(popup_area.y);
            // List starts at rel_y 1, each item has 3 lines (title, detail, spacer)
            if rel_y >= 1 && rel_y < popup_area.height.saturating_sub(2) {
                let item_idx = (rel_y.saturating_sub(1) / 3) as usize;
                if item_idx < modal.views.len() {
                    if modal.selected == item_idx {
                        let view = modal.views[item_idx].clone();
                        return restore_saved_view(app, view);
                    } else {
                        modal.selected = item_idx;
                    }
                }
            }
            None
        }
        MouseEventKind::ScrollUp => {
            modal.selected = modal.selected.saturating_sub(1);
            None
        }
        MouseEventKind::ScrollDown => {
            if !modal.views.is_empty() {
                modal.selected = (modal.selected + 1).min(modal.views.len() - 1);
            }
            None
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::SavedViewsModalState;
    use crate::saved_views::{StoredColumnFilter, StoredSortDirection};
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    fn make_test_app() -> App {
        App::new(crate::config::ConnectionConfig::default(), false)
    }

    fn sample_view(id: &str, name: &str) -> SavedView {
        SavedView {
            id: id.to_string(),
            name: name.to_string(),
            catalog: "iceberg".to_string(),
            schema: "sales".to_string(),
            table: "orders".to_string(),
            action_idx: 0,
            query: "SELECT * FROM iceberg.sales.orders".to_string(),
            column_filters: vec![("status".to_string(), StoredColumnFilter::Values(vec!["OK".to_string()]))],
            hidden_columns: vec!["internal_id".to_string()],
            sort: Some(("id".to_string(), StoredSortDirection::Asc)),
            partition_path: Vec::new(),
            saved_at: "2026-10-14 08:00:00 UTC".to_string(),
        }
    }

    #[test]
    fn test_saved_views_modal_navigation_and_closing() {
        let mut app = make_test_app();
        app.saved_views_modal = Some(SavedViewsModalState {
            views: vec![sample_view("1", "view_1"), sample_view("2", "view_2")],
            selected: 0,
            scroll: 0,
            pending_delete: None,
        });
        app.mode = Mode::SavedViewsModal;

        // Down/j
        let down_key = KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE);
        handle_saved_views_modal_key(&mut app, down_key);
        assert_eq!(app.saved_views_modal.as_ref().unwrap().selected, 1);

        // Up/k
        let up_key = KeyEvent::new(KeyCode::Char('k'), KeyModifiers::NONE);
        handle_saved_views_modal_key(&mut app, up_key);
        assert_eq!(app.saved_views_modal.as_ref().unwrap().selected, 0);

        // Esc closes modal
        let esc_key = KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE);
        handle_saved_views_modal_key(&mut app, esc_key);
        assert!(app.saved_views_modal.is_none());
        assert_eq!(app.mode, Mode::Normal);
    }

    #[test]
    fn test_saved_views_modal_delete_flow() {
        let mut app = make_test_app();
        app.saved_views_modal = Some(SavedViewsModalState {
            views: vec![sample_view("1", "view_1"), sample_view("2", "view_2")],
            selected: 0,
            scroll: 0,
            pending_delete: None,
        });

        // Press 'd' to initiate delete confirmation
        let d_key = KeyEvent::new(KeyCode::Char('d'), KeyModifiers::NONE);
        handle_saved_views_modal_key(&mut app, d_key);
        assert_eq!(
            app.saved_views_modal.as_ref().unwrap().pending_delete,
            Some("1".to_string())
        );

        // Press 'n' cancels deletion
        let n_key = KeyEvent::new(KeyCode::Char('n'), KeyModifiers::NONE);
        handle_saved_views_modal_key(&mut app, n_key);
        assert_eq!(app.saved_views_modal.as_ref().unwrap().pending_delete, None);
        assert_eq!(app.saved_views_modal.as_ref().unwrap().views.len(), 2);

        // Press 'd' again and press 'y' to confirm
        handle_saved_views_modal_key(&mut app, d_key);
        let y_key = KeyEvent::new(KeyCode::Char('y'), KeyModifiers::NONE);
        handle_saved_views_modal_key(&mut app, y_key);
        assert_eq!(app.saved_views_modal.as_ref().unwrap().pending_delete, None);
        assert_eq!(app.saved_views_modal.as_ref().unwrap().views.len(), 1);
        assert_eq!(app.saved_views_modal.as_ref().unwrap().views[0].id, "2");
        assert!(app.copied_toast.is_some());
    }

    #[test]
    fn test_restore_saved_view_populates_action_state_and_emits_command() {
        let mut app = make_test_app();
        let view = sample_view("v_xyz", "orders_view");
        let cmd = restore_saved_view(&mut app, view.clone());

        assert!(cmd.is_some());
        if let Some(Command::ExecuteQuery { query, catalog, schema, table, is_paginated, .. }) = cmd {
            assert_eq!(query, view.query);
            assert_eq!(catalog, "iceberg");
            assert_eq!(schema, "sales");
            assert_eq!(table, "orders");
            assert!(is_paginated);
        } else {
            panic!("Expected Command::ExecuteQuery");
        }

        if let Screen::Actions(a) = &app.screen {
            assert_eq!(a.pending_view_restore.as_ref().unwrap().id, "v_xyz");
        } else {
            panic!("Expected Screen::Actions");
        }
    }
}
