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
use tracing::info;

use crate::app::*;
use crate::trino::client::TrinoClient;
use crate::trino::error::TrinoClientError;

pub mod applied_filters_modal;
pub mod column_modal;
mod commands;
pub mod export;
pub mod filter_modal;
pub mod help_modal;
mod mouse;
mod navigation;
mod query;
pub mod query_vim;
pub mod saved_views_modal;
mod screen_text;

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

/// The keyboard row selection of the focused list view, if it still applies.
pub fn live_line_selection(app: &App) -> Option<LineSelection> {
    navigation::line_select::live_selection(app)
}

pub fn selection_uses_screen_text(app: &App, anchor: (u16, u16), current: (u16, u16)) -> bool {
    mouse::selection_uses_screen_text(app, anchor, current)
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

pub fn handle_async_result(app: &mut App, result: AsyncResult) -> Option<Command> {
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
        /// Partition predicates baked into `query`, carried along so the
        /// resulting `ResultsState` (and any subsequent `FetchNextPage`)
        /// keeps using them. Empty for ordinary, non-drill-down queries.
        filters: Vec<(String, String)>,
    },
    FetchNextPage {
        catalog: String,
        schema: String,
        table: String,
        offset: usize,
        limit: usize,
        filters: Vec<(String, String)>,
    },
    /// Fetches the distinct values of the next unfixed partition column in
    /// a cd/ls-style drill-down, scoped by the partition predicates already
    /// fixed by the levels above it (`filters`). Used only for tables whose
    /// `partitioned_by` (from live `SHOW CREATE TABLE` recon) is non-empty.
    FetchPartitionLevel {
        catalog: String,
        schema: String,
        table: String,
        filters: Vec<(String, String)>,
        column: String,
    },
    FetchPartitionCount {
        catalog: String,
        schema: String,
        table: String,
        filters: Vec<(String, String)>,
    },
}

#[derive(Debug)]
pub enum AsyncResult {
    Connect {
        log_id: usize,
        url: String,
        user: String,
        password: String,
        result: Result<(TrinoClient, Vec<String>), TrinoClientError>,
    },
    FetchSchemas {
        log_id: usize,
        catalog: String,
        result: Result<Vec<String>, TrinoClientError>,
    },
    FetchTables {
        log_id: usize,
        catalog: String,
        schema: String,
        result: Result<Vec<String>, TrinoClientError>,
    },
    /// Sent as soon as `SHOW CREATE TABLE` returns — before the slower
    /// `$partitions` / `information_schema.columns` recon queries finish —
    /// so `Table DDL` and the partitioned-ness needed by `Table View` are
    /// available immediately instead of waiting on the full recon batch.
    FetchTableDdl {
        show_create_log_id: usize,
        catalog: String,
        schema: String,
        table: String,
        partitioned_by: Vec<String>,
        location: String,
        ddl_text: String,
        show_create_error: Option<TrinoClientError>,
    },
    FetchTableMetadata {
        partitions_log_id: usize,
        cols_log_id: usize,
        catalog: String,
        schema: String,
        table: String,
        partition_lines: Vec<String>,
        columns: Vec<VerticalColumn>,
        partitions_error: Option<TrinoClientError>,
        columns_error: Option<TrinoClientError>,
    },
    ExecuteQuery {
        log_id: usize,
        query_buffer: String,
        query_cursor: usize,
        catalog: String,
        schema: String,
        table: String,
        is_paginated: bool,
        filters: Vec<(String, String)>,
        result: Result<crate::trino::types::QueryResults, TrinoClientError>,
    },
    FetchNextPage {
        log_id: usize,
        catalog: String,
        schema: String,
        table: String,
        offset: usize,
        limit: usize,
        result: Result<crate::trino::types::QueryResults, TrinoClientError>,
    },
    FetchPartitionLevel {
        log_id: usize,
        catalog: String,
        schema: String,
        table: String,
        filters: Vec<(String, String)>,
        column: String,
        result: Result<crate::trino::types::QueryResults, TrinoClientError>,
    },
    FetchPartitionCount {
        log_id: usize,
        catalog: String,
        schema: String,
        table: String,
        filters: Vec<(String, String)>,
        result: Result<crate::trino::types::QueryResults, TrinoClientError>,
    },
}

/// `Cmd+C` (kitty protocol: Super) or `Ctrl+Shift+C`.
fn is_native_copy_chord(code: KeyCode, mods: KeyModifiers) -> bool {
    let is_c = matches!(code, KeyCode::Char('c') | KeyCode::Char('C'));
    is_c && (mods.contains(KeyModifiers::SUPER)
        || (mods.contains(KeyModifiers::CONTROL)
            && (mods.contains(KeyModifiers::SHIFT) || code == KeyCode::Char('C'))))
}

pub fn handle_key_sync(app: &mut App, key: KeyEvent) -> Option<Command> {
    preserve_public_api_symbols();
    app.sync_search_scope();

    // The painted mouse selection is positional, so any key that could move
    // content invalidates it. Esc (deselect), `y` and the native copy chords
    // act on it instead.
    if app.mouse_selection_committed {
        let code = navigation::normalize_key_code(key.code);
        let keeps_selection = code == KeyCode::Esc
            || is_native_copy_chord(code, key.modifiers)
            || (code == KeyCode::Char('y') && key.modifiers.is_empty());
        if !keeps_selection {
            app.mouse_selection_committed = false;
        }
    }

    // Guard against mode desynchronization: if any modal is currently visible,
    // key events MUST be handled by that modal and never leak to background panes.
    if app.filter_modal.is_some() {
        if app.mode != Mode::FilterModal {
            app.mode = Mode::FilterModal;
        }
        return filter_modal::handle_filter_modal_key(app, key);
    }
    if app.column_modal.is_some() {
        if app.mode != Mode::ColumnModal {
            app.mode = Mode::ColumnModal;
        }
        return column_modal::handle_column_modal_key(app, key);
    }
    if app.help_modal.is_some() {
        if app.mode != Mode::HelpModal {
            app.mode = Mode::HelpModal;
        }
        return help_modal::handle_help_modal_key(app, key);
    }
    if app.saved_views_modal.is_some() {
        if app.mode != Mode::SavedViewsModal {
            app.mode = Mode::SavedViewsModal;
        }
        return saved_views_modal::handle_saved_views_modal_key(app, key);
    }
    if app.save_view_prompt.is_some() {
        if app.mode != Mode::SaveViewPrompt {
            app.mode = Mode::SaveViewPrompt;
        }
        return saved_views_modal::handle_save_view_prompt_key(app, key);
    }
    if app.applied_filters_modal.is_some() {
        if app.mode != Mode::AppliedFiltersModal {
            app.mode = Mode::AppliedFiltersModal;
        }
        return applied_filters_modal::handle_applied_filters_modal_key(app, key);
    }

    if app.export_modal.is_some() {
        if app.mode != Mode::ExportModal {
            app.mode = Mode::ExportModal;
        }
        return export::handle_export_modal_key(app, key);
    }

    match app.mode {
        Mode::ExportModal => return export::handle_export_modal_key(app, key),
        Mode::HelpModal => return help_modal::handle_help_modal_key(app, key),
        Mode::FilterModal => return filter_modal::handle_filter_modal_key(app, key),
        Mode::ColumnModal => return column_modal::handle_column_modal_key(app, key),
        Mode::SavedViewsModal => return saved_views_modal::handle_saved_views_modal_key(app, key),
        Mode::SaveViewPrompt => return saved_views_modal::handle_save_view_prompt_key(app, key),
        Mode::AppliedFiltersModal => {
            return applied_filters_modal::handle_applied_filters_modal_key(app, key);
        }
        Mode::QueryInput => return query::handle_query_input_mode(app, key),
        Mode::Search => return query::handle_search_mode(app, key),
        Mode::Normal => {}
    }

    let code = navigation::normalize_key_code(key.code);

    // Native copy shortcuts. Must run before the Ctrl+C quit check: with the
    // kitty keyboard protocol, Ctrl+Shift+C arrives as Ctrl+C plus Shift.
    // (Terminals that handle Cmd+C / Ctrl+Shift+C themselves never forward
    // them; the drag-to-select auto-copy and `y` cover those.)
    if is_native_copy_chord(code, key.modifiers) {
        navigation::clipboard::copy_current_selection(app);
        return None;
    }

    if code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
        info!("User pressed Ctrl+C, quitting...");
        app.should_quit = true;
        return None;
    }

    if code == KeyCode::Char('s') && key.modifiers.contains(KeyModifiers::CONTROL) {
        saved_views_modal::open_save_view_prompt(app);
        return None;
    }

    if (code == KeyCode::Char('o') && key.modifiers.contains(KeyModifiers::CONTROL))
        || (code == KeyCode::Char('B') && !key.modifiers.contains(KeyModifiers::CONTROL))
    {
        saved_views_modal::open_saved_views_modal(app);
        return None;
    }

    if matches!(app.screen, Screen::Connect(_)) {
        return navigation::connect_keys(app, key);
    }

    if code == KeyCode::Char('e') && key.modifiers.contains(KeyModifiers::CONTROL) {
        export::open_export_modal(app);
        return None;
    }

    // Esc deselects first; it only falls through to its normal meaning
    // (back / focus menu pane) when nothing is selected.
    navigation::line_select::prune(app);
    if code == KeyCode::Esc && app.has_active_selection() {
        app.number_buffer.clear();
        app.clear_all_selections();
        return None;
    }

    if navigation::handle_pane_focus_keys(app, key) {
        return None;
    }

    if navigation::line_select::handle_key(app, key) {
        return None;
    }

    if code == KeyCode::Char('?') {
        if app.help_modal.is_some() || matches!(app.screen, Screen::Help) {
            help_modal::close_help_modal(app);
            if matches!(app.screen, Screen::Help) {
                navigation::go_back(app);
            }
            return None;
        }
        help_modal::open_help_modal(app);
        return None;
    }

    if code == KeyCode::Char('/')
        && matches!(
            app.screen,
            Screen::Catalog(_) | Screen::Schema(_) | Screen::Table(_) | Screen::Help
        )
    {
        app.mode = Mode::Search;
        return None;
    }

    if code == KeyCode::Esc {
        app.number_buffer.clear();
        let is_in_table = matches!(app.screen, Screen::Actions(_));
        if is_in_table && app.active_panel == ActivePanel::MainViewer {
            app.set_active_panel(ActivePanel::MenuPane);
            return None;
        } else if matches!(app.screen, Screen::Help) && !app.search_query.is_empty() {
            app.search_query.clear();
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
        && ((c.is_ascii_digit() && c != '0') || (c == '0' && !app.number_buffer.is_empty()))
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
        Screen::Help => navigation::help_keys(app, key),
    }
}

#[cfg(test)]
mod router_tests {
    //! Key-binding tests that go through `handle_key_sync` (the real router),
    //! so global handlers that shadow per-screen ones are exercised too.
    use super::*;
    use crate::config::ConnectionConfig;
    use crossterm::event::{KeyEvent, KeyModifiers as M};

    fn press(app: &mut App, code: KeyCode, mods: M) {
        handle_key_sync(app, KeyEvent::new(code, mods));
    }

    fn schema_app() -> App {
        let mut app = App::new(ConnectionConfig::default(), false);
        app.catalogs = vec!["hive".into()];
        app.screen = Screen::Schema(SchemaState {
            catalog: "hive".into(),
            items: vec!["a".into(), "b".into(), "c".into()],
            selected: 1,
        });
        app
    }

    fn grid_app() -> App {
        let res = ResultsState {
            columns: vec!["a".into(), "b".into()],
            rows: vec![vec!["1".into(), "2".into()], vec!["3".into(), "4".into()]],
            ..Default::default()
        };
        let mut app = App::new(ConnectionConfig::default(), false);
        app.screen = Screen::Actions(ActionState {
            catalog: "c".into(),
            schema: "s".into(),
            table: "t".into(),
            selected: 0,
            results: Some(res),
            ..Default::default()
        });
        app.active_panel = ActivePanel::MainViewer;
        app
    }

    #[test]
    fn search_stays_in_its_pane_and_is_dropped_on_drill_or_back() {
        let mut app = schema_app();
        press(&mut app, KeyCode::Char('/'), M::NONE);
        press(&mut app, KeyCode::Char('b'), M::NONE);
        press(&mut app, KeyCode::Enter, M::NONE); // close the search bar
        assert_eq!(app.search_query, "b");

        // another pane (drill-down) shows its own, unfiltered list
        app.screen = Screen::Table(TableState {
            catalog: "hive".into(),
            schema: "b".into(),
            items: vec!["t1".into(), "t2".into()],
            selected: 0,
        });
        press(&mut app, KeyCode::Down, M::NONE);
        assert!(app.search_query.is_empty());

        // and the previous pane is not filtered when coming back
        app.screen = schema_app().screen;
        press(&mut app, KeyCode::Down, M::NONE);
        assert!(app.search_query.is_empty());
    }

    #[test]
    fn search_survives_keys_within_the_same_pane() {
        let mut app = schema_app();
        press(&mut app, KeyCode::Char('/'), M::NONE);
        press(&mut app, KeyCode::Char('a'), M::NONE);
        press(&mut app, KeyCode::Enter, M::NONE);
        press(&mut app, KeyCode::Down, M::NONE);
        press(&mut app, KeyCode::Up, M::NONE);
        assert_eq!(app.search_query, "a");
    }

    fn in_visual(app: &App) -> bool {
        matches!(&app.screen, Screen::Actions(s)
            if s.results.as_ref().is_some_and(|r| r.is_in_visual_mode()))
    }

    #[test]
    fn esc_clears_grid_visual_mode_before_leaving_the_pane() {
        let mut app = grid_app();
        press(&mut app, KeyCode::Char('V'), M::SHIFT);
        assert!(in_visual(&app));

        press(&mut app, KeyCode::Esc, M::NONE);
        assert!(!in_visual(&app), "first Esc deselects");
        assert_eq!(app.active_panel, ActivePanel::MainViewer, "and stays put");

        press(&mut app, KeyCode::Esc, M::NONE);
        assert_eq!(
            app.active_panel,
            ActivePanel::MenuPane,
            "second Esc is the usual Esc"
        );
    }

    #[test]
    fn esc_clears_list_selection_before_going_back() {
        let mut app = schema_app();
        press(&mut app, KeyCode::Char('V'), M::SHIFT);
        press(&mut app, KeyCode::Char('j'), M::NONE);
        assert!(app.line_selection.is_some());

        press(&mut app, KeyCode::Esc, M::NONE);
        assert!(app.line_selection.is_none());
        assert!(
            matches!(app.screen, Screen::Schema(_)),
            "still on the schema list"
        );

        press(&mut app, KeyCode::Esc, M::NONE);
        assert!(
            matches!(app.screen, Screen::Catalog(_)),
            "now Esc goes back"
        );
    }

    #[test]
    fn esc_is_not_swallowed_by_an_invisible_click_anchor() {
        let mut app = schema_app();
        // a bare click leaves anchors but nothing visible to deselect
        app.mouse_selection_anchor = Some((3, 3));
        app.mouse_selection_current = Some((3, 3));
        press(&mut app, KeyCode::Esc, M::NONE);
        assert!(matches!(app.screen, Screen::Catalog(_)));
    }

    #[test]
    fn esc_clears_an_in_progress_mouse_drag_selection() {
        let mut app = schema_app();
        app.mouse_selection_anchor = Some((1, 1));
        app.mouse_selection_current = Some((5, 4));
        app.is_selecting_text = true;
        press(&mut app, KeyCode::Esc, M::NONE);
        assert!(!app.is_selecting_text && !app.has_mouse_selection());
        assert!(matches!(app.screen, Screen::Schema(_)));
    }

    #[test]
    fn stale_list_selection_does_not_swallow_esc() {
        let mut app = schema_app();
        press(&mut app, KeyCode::Char('V'), M::SHIFT);
        // async refresh replaces the list while the selection is anchored
        if let Screen::Schema(s) = &mut app.screen {
            s.items.push("d".into());
        }
        press(&mut app, KeyCode::Esc, M::NONE);
        assert!(
            matches!(app.screen, Screen::Catalog(_)),
            "stale selection was pruned"
        );
    }

    #[test]
    fn ctrl_e_opens_modal_which_owns_the_keyboard_and_mouse() {
        let mut app = schema_app();
        press(&mut app, KeyCode::Char('e'), M::CONTROL);
        assert_eq!(app.mode, Mode::ExportModal);

        // keys must not leak to the list behind
        press(&mut app, KeyCode::Char('j'), M::NONE);
        press(&mut app, KeyCode::Char('V'), M::SHIFT);
        press(&mut app, KeyCode::Char('l'), M::NONE);
        assert!(matches!(&app.screen, Screen::Schema(s) if s.selected == 1));
        assert!(app.line_selection.is_none());

        // Ctrl+E again toggles it closed, and must not export anything
        press(&mut app, KeyCode::Char('e'), M::CONTROL);
        assert!(app.export_modal.is_none());
        assert_eq!(app.mode, Mode::Normal);
        assert!(app.notice_toast.is_none(), "closing must not copy/export");

        press(&mut app, KeyCode::Char('e'), M::CONTROL);
        press(&mut app, KeyCode::Esc, M::NONE);
        assert_eq!(app.mode, Mode::Normal);
        assert!(app.export_modal.is_none());
        assert!(matches!(app.screen, Screen::Schema(_)));
    }

    #[test]
    fn ctrl_e_is_inert_on_screens_with_nothing_to_export() {
        let mut app = App::new(ConnectionConfig::default(), false); // Connect screen
        press(&mut app, KeyCode::Char('e'), M::CONTROL);
        assert!(app.export_modal.is_none());
        assert_eq!(app.mode, Mode::Normal);
    }

    #[test]
    fn native_copy_chords_never_quit_but_plain_ctrl_c_does() {
        for (code, mods) in [
            (KeyCode::Char('c'), M::SUPER),
            (KeyCode::Char('c'), M::CONTROL | M::SHIFT),
            (KeyCode::Char('C'), M::CONTROL | M::SHIFT),
            (KeyCode::Char('C'), M::CONTROL),
        ] {
            let mut app = App::new(ConnectionConfig::default(), false);
            press(&mut app, code, mods);
            assert!(!app.should_quit, "{code:?} {mods:?} must not quit");
            assert!(
                app.notice_toast.is_some(),
                "{code:?} {mods:?} should report"
            );
        }
        let mut app = App::new(ConnectionConfig::default(), false);
        press(&mut app, KeyCode::Char('c'), M::CONTROL);
        assert!(app.should_quit);
    }

    #[test]
    fn menu_pane_action_hotkeys_still_win_over_selection_keys() {
        // `v` selects only in the main viewer; in the menu it still runs Table View.
        let mut app = grid_app();
        app.active_panel = ActivePanel::MenuPane;
        press(&mut app, KeyCode::Char('v'), M::NONE);
        assert!(app.line_selection.is_none());
        assert!(!in_visual(&app));
    }
}
