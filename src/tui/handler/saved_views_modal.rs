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

use crate::app::{App, Mode, Screen};
use crate::saved_views::{GridState, SavedView, delete_saved_view};
use crate::tui::handler::{Command, navigation};
use crate::view_location::ViewLocation;

pub fn open_saved_views_modal(app: &mut App) {
    let views = match crate::saved_views::load_saved_views() {
        Ok(views) => views,
        Err(err) => {
            app.set_notice(
                crate::app::NoticeKind::Error,
                format!("Saved views unavailable: {err:#} (file left untouched)"),
            );
            return;
        }
    };
    app.saved_views_modal = Some(crate::app::SavedViewsModalState {
        views,
        selected: 0,
        scroll: 0,
        selected_ids: std::collections::HashSet::new(),
        pending_delete_ids: Vec::new(),
        current_url: app.config.url.clone(),
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
    if !modal.pending_delete_ids.is_empty() {
        match code {
            KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter => {
                let count = modal.pending_delete_ids.len();
                for id in &modal.pending_delete_ids {
                    let _ = delete_saved_view(id);
                }
                let to_del: std::collections::HashSet<_> =
                    modal.pending_delete_ids.drain(..).collect();
                modal.views.retain(|v| !to_del.contains(&v.id));
                modal.selected_ids.retain(|id| !to_del.contains(id));
                if modal.selected >= modal.views.len() && !modal.views.is_empty() {
                    modal.selected = modal.views.len() - 1;
                }
                let msg = if count == 1 {
                    "Saved view deleted".to_string()
                } else {
                    format!("{count} saved views deleted")
                };
                app.copied_toast = Some((msg, Instant::now()));
            }
            KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                modal.pending_delete_ids.clear();
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
        KeyCode::Char(' ') => {
            if let Some(view) = modal.views.get(modal.selected) {
                if modal.selected_ids.contains(&view.id) {
                    modal.selected_ids.remove(&view.id);
                } else {
                    modal.selected_ids.insert(view.id.clone());
                }
            }
            None
        }
        KeyCode::Char('a') | KeyCode::Char('A') => {
            if modal.selected_ids.len() == modal.views.len() {
                modal.selected_ids.clear();
            } else {
                modal.selected_ids = modal.views.iter().map(|v| v.id.clone()).collect();
            }
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
                if modal.selected_ids.is_empty() {
                    modal.pending_delete_ids = vec![modal.views[modal.selected].id.clone()];
                } else {
                    modal.pending_delete_ids = modal
                        .views
                        .iter()
                        .filter(|v| modal.selected_ids.contains(&v.id))
                        .map(|v| v.id.clone())
                        .collect();
                }
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

/// Rebuilds the saved view step by step so every pane before and after it
/// can be navigated (see `restore`).
pub fn restore_saved_view(app: &mut App, view: SavedView) -> Option<Command> {
    close_saved_views_modal(app);
    crate::tui::handler::restore::begin(app, view)
}

pub fn save_current_view(app: &mut App) {
    open_save_view_prompt(app);
}

pub fn open_save_view_prompt(app: &mut App) {
    let Some((location, query)) = ViewLocation::capture(app) else {
        app.copied_toast = Some((
            "Open a table view first to save it".to_string(),
            Instant::now(),
        ));
        return;
    };
    let grid = match &app.screen {
        Screen::Actions(a) => a.results.as_ref().map(GridState::capture),
        _ => None,
    }
    .unwrap_or_default();

    let epoch_millis = std::time::SystemTime::now()
        .duration_since(std::time::SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let id = format!(
        "{}_{}_{}_{epoch_millis}",
        location.catalog, location.schema, location.table
    );
    let default_name = format!(
        "{}.{}.{}",
        location.catalog, location.schema, location.table
    );
    let connection = (!app.config.url.is_empty()).then(|| app.config.url.clone());

    let view = SavedView {
        version: crate::saved_views::SAVED_VIEWS_VERSION,
        id,
        name: default_name.clone(),
        saved_at: crate::saved_views::current_timestamp_str(),
        connection,
        location,
        query,
        grid,
    };

    app.save_view_prompt = Some(crate::app::SaveViewPromptState {
        name_input: default_name,
        view,
    });
    app.mode = Mode::SaveViewPrompt;
}

pub fn close_save_view_prompt(app: &mut App) {
    app.save_view_prompt = None;
    app.mode = Mode::Normal;
}

pub fn handle_save_view_prompt_key(app: &mut App, key: KeyEvent) -> Option<Command> {
    let code = navigation::normalize_key_code(key.code);

    if key.modifiers.contains(KeyModifiers::CONTROL) && code == KeyCode::Char('c') {
        app.should_quit = true;
        return None;
    }

    match code {
        KeyCode::Esc => {
            close_save_view_prompt(app);
            None
        }
        KeyCode::Enter => {
            confirm_save_view_prompt(app);
            None
        }
        KeyCode::Backspace => {
            if let Some(ref mut prompt) = app.save_view_prompt {
                prompt.name_input.pop();
            }
            None
        }
        KeyCode::Char(c) => {
            if let Some(ref mut prompt) = app.save_view_prompt
                && prompt.name_input.len() < 60
            {
                prompt.name_input.push(c);
            }
            None
        }
        _ => None,
    }
}

pub fn confirm_save_view_prompt(app: &mut App) {
    if let Some(mut prompt) = app.save_view_prompt.take() {
        let final_name = prompt.name_input.trim();
        if !final_name.is_empty() {
            prompt.view.name = final_name.to_string();
        }
        let name = prompt.view.name.clone();
        match crate::saved_views::add_saved_view(prompt.view) {
            Ok(_) => {
                app.copied_toast = Some((format!("Saved view: {name}"), Instant::now()));
            }
            Err(e) => {
                app.set_notice(
                    crate::app::NoticeKind::Error,
                    format!("Failed to save view: {e:#}"),
                );
            }
        }
    }
    app.mode = Mode::Normal;
}

pub fn handle_save_view_prompt_mouse(
    app: &mut App,
    mouse: crossterm::event::MouseEvent,
    term_width: u16,
    term_height: u16,
) -> Option<Command> {
    use crossterm::event::{MouseButton, MouseEventKind};
    use ratatui::layout::Rect;

    let screen_area = Rect::new(0, 0, term_width, term_height);
    let popup_area = crate::tui::screens::modal::modal_small(screen_area);

    if let MouseEventKind::Down(MouseButton::Left) = mouse.kind {
        if mouse.column < popup_area.x
            || mouse.column >= popup_area.x + popup_area.width
            || mouse.row < popup_area.y
            || mouse.row >= popup_area.y + popup_area.height
        {
            close_save_view_prompt(app);
            return None;
        }

        let rel_y = mouse.row.saturating_sub(popup_area.y);
        if rel_y >= popup_area.height.saturating_sub(2) {
            let rel_x = mouse.column.saturating_sub(popup_area.x);
            if rel_x < 18 {
                confirm_save_view_prompt(app);
            } else {
                close_save_view_prompt(app);
            }
        }
    }
    None
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
    if !modal.pending_delete_ids.is_empty() {
        let confirm_area = crate::tui::screens::modal::modal_small(screen_area);
        if let MouseEventKind::Down(MouseButton::Left) = mouse.kind {
            if mouse.column < confirm_area.x
                || mouse.column >= confirm_area.x + confirm_area.width
                || mouse.row < confirm_area.y
                || mouse.row >= confirm_area.y + confirm_area.height
            {
                modal.pending_delete_ids.clear();
                return None;
            }
            let rel_y = mouse.row.saturating_sub(confirm_area.y);
            if (3..=6).contains(&rel_y) {
                let rel_x = mouse.column.saturating_sub(confirm_area.x);
                if rel_x < 24 {
                    // "Confirm" clicked
                    let count = modal.pending_delete_ids.len();
                    for id in &modal.pending_delete_ids {
                        let _ = delete_saved_view(id);
                    }
                    let to_del: std::collections::HashSet<_> =
                        modal.pending_delete_ids.drain(..).collect();
                    modal.views.retain(|v| !to_del.contains(&v.id));
                    modal.selected_ids.retain(|id| !to_del.contains(id));
                    if modal.selected >= modal.views.len() && !modal.views.is_empty() {
                        modal.selected = modal.views.len() - 1;
                    }
                    let msg = if count == 1 {
                        "Saved view deleted".to_string()
                    } else {
                        format!("{count} saved views deleted")
                    };
                    app.copied_toast = Some((msg, Instant::now()));
                } else {
                    // "Cancel" clicked
                    modal.pending_delete_ids.clear();
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
                    let rel_x = mouse.column.saturating_sub(popup_area.x);
                    if rel_x <= 8 {
                        // Clicked checkbox / indicator area -> toggle selection
                        let id = modal.views[item_idx].id.clone();
                        if modal.selected_ids.contains(&id) {
                            modal.selected_ids.remove(&id);
                        } else {
                            modal.selected_ids.insert(id);
                        }
                        modal.selected = item_idx;
                    } else if modal.selected == item_idx {
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
            version: crate::saved_views::SAVED_VIEWS_VERSION,
            id: id.to_string(),
            name: name.to_string(),
            saved_at: "2026-10-14 08:00:00 UTC".to_string(),
            connection: None,
            location: ViewLocation {
                catalog: "iceberg".to_string(),
                schema: "sales".to_string(),
                table: "orders".to_string(),
                action: crate::app::Action::TableView,
                partition_path: Vec::new(),
            },
            query: crate::view_location::QuerySource::Generated,
            grid: GridState {
                column_filters: vec![(
                    "status".to_string(),
                    StoredColumnFilter::Values(vec!["OK".to_string()]),
                )],
                hidden_columns: vec!["internal_id".to_string()],
                sort: Some(("id".to_string(), StoredSortDirection::Asc)),
            },
        }
    }

    #[test]
    fn test_saved_views_modal_navigation_and_closing() {
        let mut app = make_test_app();
        app.saved_views_modal = Some(SavedViewsModalState {
            views: vec![sample_view("1", "view_1"), sample_view("2", "view_2")],
            selected: 0,
            scroll: 0,
            selected_ids: std::collections::HashSet::new(),
            pending_delete_ids: Vec::new(),
            ..Default::default()
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
            selected_ids: std::collections::HashSet::new(),
            pending_delete_ids: Vec::new(),
            ..Default::default()
        });

        // Press 'd' to initiate delete confirmation
        let d_key = KeyEvent::new(KeyCode::Char('d'), KeyModifiers::NONE);
        handle_saved_views_modal_key(&mut app, d_key);
        assert_eq!(
            app.saved_views_modal.as_ref().unwrap().pending_delete_ids,
            vec!["1".to_string()]
        );

        // Press 'n' cancels deletion
        let n_key = KeyEvent::new(KeyCode::Char('n'), KeyModifiers::NONE);
        handle_saved_views_modal_key(&mut app, n_key);
        assert!(
            app.saved_views_modal
                .as_ref()
                .unwrap()
                .pending_delete_ids
                .is_empty()
        );
        assert_eq!(app.saved_views_modal.as_ref().unwrap().views.len(), 2);

        // Press 'd' again and press 'y' to confirm
        handle_saved_views_modal_key(&mut app, d_key);
        let y_key = KeyEvent::new(KeyCode::Char('y'), KeyModifiers::NONE);
        handle_saved_views_modal_key(&mut app, y_key);
        assert!(
            app.saved_views_modal
                .as_ref()
                .unwrap()
                .pending_delete_ids
                .is_empty()
        );
        assert_eq!(app.saved_views_modal.as_ref().unwrap().views.len(), 1);
        assert_eq!(app.saved_views_modal.as_ref().unwrap().views[0].id, "2");
        assert!(app.copied_toast.is_some());
    }

    #[test]
    fn test_saved_views_modal_multi_select_and_batch_delete() {
        let mut app = make_test_app();
        app.saved_views_modal = Some(SavedViewsModalState {
            views: vec![
                sample_view("1", "view_1"),
                sample_view("2", "view_2"),
                sample_view("3", "view_3"),
            ],
            selected: 0,
            scroll: 0,
            selected_ids: std::collections::HashSet::new(),
            pending_delete_ids: Vec::new(),
            ..Default::default()
        });

        // 1. Space toggles item 0
        let space_key = KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE);
        handle_saved_views_modal_key(&mut app, space_key);
        assert!(
            app.saved_views_modal
                .as_ref()
                .unwrap()
                .selected_ids
                .contains("1")
        );

        // 2. Move down and Space toggle item 1
        let down_key = KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE);
        handle_saved_views_modal_key(&mut app, down_key);
        handle_saved_views_modal_key(&mut app, space_key);
        assert_eq!(
            app.saved_views_modal.as_ref().unwrap().selected_ids.len(),
            2
        );

        // 3. Press 'd' - schedules both selected IDs for deletion
        let d_key = KeyEvent::new(KeyCode::Char('d'), KeyModifiers::NONE);
        handle_saved_views_modal_key(&mut app, d_key);
        assert_eq!(
            app.saved_views_modal.as_ref().unwrap().pending_delete_ids,
            vec!["1".to_string(), "2".to_string()]
        );

        // 4. Confirm deletion with 'y'
        let y_key = KeyEvent::new(KeyCode::Char('y'), KeyModifiers::NONE);
        handle_saved_views_modal_key(&mut app, y_key);
        let modal = app.saved_views_modal.as_ref().unwrap();
        assert_eq!(modal.views.len(), 1);
        assert_eq!(modal.views[0].id, "3");
        assert!(modal.selected_ids.is_empty());
        assert!(modal.pending_delete_ids.is_empty());

        // 5. 'a' toggles all
        let a_key = KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE);
        handle_saved_views_modal_key(&mut app, a_key);
        assert_eq!(
            app.saved_views_modal.as_ref().unwrap().selected_ids.len(),
            1
        );
        // Pressing 'a' again clears all
        handle_saved_views_modal_key(&mut app, a_key);
        assert!(
            app.saved_views_modal
                .as_ref()
                .unwrap()
                .selected_ids
                .is_empty()
        );
    }

    #[test]
    fn test_save_view_prompt_typing_and_confirmation() {
        let mut app = make_test_app();
        app.save_view_prompt = Some(crate::app::SaveViewPromptState {
            name_input: "test_view".to_string(),
            view: sample_view("v1", "test_view"),
        });
        app.mode = Mode::SaveViewPrompt;

        // Type char
        let char_key = KeyEvent::new(KeyCode::Char('_'), KeyModifiers::NONE);
        handle_save_view_prompt_key(&mut app, char_key);
        let char_key2 = KeyEvent::new(KeyCode::Char('2'), KeyModifiers::NONE);
        handle_save_view_prompt_key(&mut app, char_key2);
        assert_eq!(
            app.save_view_prompt.as_ref().unwrap().name_input,
            "test_view_2"
        );

        // Backspace
        let bs_key = KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE);
        handle_save_view_prompt_key(&mut app, bs_key);
        assert_eq!(
            app.save_view_prompt.as_ref().unwrap().name_input,
            "test_view_"
        );

        // Enter confirms save
        let enter_key = KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE);
        handle_save_view_prompt_key(&mut app, enter_key);
        assert!(app.save_view_prompt.is_none());
        assert_eq!(app.mode, Mode::Normal);
        assert!(app.copied_toast.is_some());
    }

    #[test]
    fn test_restore_saved_view_closes_the_modal_and_starts_the_plan() {
        let mut app = make_test_app();
        app.catalogs = vec!["iceberg".to_string()];
        app.saved_views_modal = Some(SavedViewsModalState::default());
        app.mode = Mode::SavedViewsModal;
        let cmd = restore_saved_view(&mut app, sample_view("v_xyz", "orders_view"));
        assert!(app.saved_views_modal.is_none());
        assert_eq!(app.mode, Mode::Normal);
        // schemas of the catalog are not cached yet: fetch them first
        assert!(matches!(cmd, Some(Command::FetchSchemas { ref catalog }) if catalog == "iceberg"));
        assert_eq!(app.restore.as_ref().unwrap().view.id, "v_xyz");
    }

    #[test]
    fn save_prompt_captures_location_query_and_grid() {
        let mut app = make_test_app();
        app.config.url = "https://trino.example".into();
        let mut res = crate::app::ResultsState::default();
        res.hidden_columns.insert("secret".into());
        app.screen = Screen::Actions(crate::app::ActionState {
            catalog: "c".into(),
            schema: "s".into(),
            table: "t".into(),
            selected: crate::app::Action::Sample.index(),
            custom_sql: Some("SELECT a FROM t".into()),
            results: Some(res),
            ..Default::default()
        });
        open_save_view_prompt(&mut app);
        let prompt = app.save_view_prompt.as_ref().unwrap();
        assert_eq!(prompt.name_input, "c.s.t");
        let v = &prompt.view;
        assert_eq!(v.location.action, crate::app::Action::Sample);
        assert_eq!(
            v.query,
            crate::view_location::QuerySource::Custom {
                sql: "SELECT a FROM t".into()
            }
        );
        assert_eq!(v.grid.hidden_columns, vec!["secret"]);
        assert_eq!(v.connection.as_deref(), Some("https://trino.example"));
    }
}
