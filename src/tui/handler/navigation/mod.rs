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

use crate::app::{ActivePanel, App, CatalogState, ConnectState, SchemaState, Screen, TableState};
use crate::tui::handler::Command;

pub mod actions;
pub mod clipboard;
pub mod connect_help;
pub mod drilldown;
pub mod list;

pub use actions::*;
pub use connect_help::*;
pub use list::*;

pub fn go_back(app: &mut App) {
    let logged_in = app.trino_client.is_some();
    if !matches!(app.screen, Screen::Help) {
        app.main_panel_pct = 60;
    }

    let next = match &app.screen {
        Screen::Help => {
            app.search_query.clear();
            app.prev_screen.take().map(|p| *p)
        }
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
            let idx = schemas
                .iter()
                .position(|c| c.trim() == schema_name.trim())
                .unwrap_or(0);
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
            let idx = tables
                .iter()
                .position(|t| t.trim() == s.table.trim())
                .unwrap_or(0);
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
        app.clear_mouse_selection();
        app.screen = s;
    }
}

pub fn help_keys(app: &mut App, key: KeyEvent) -> Option<Command> {
    let code = normalize_key_code(key.code);
    match code {
        KeyCode::Char('j') | KeyCode::Down => {
            app.help_scroll = app.help_scroll.saturating_add(1);
            if let Some(ref mut m) = app.help_modal {
                m.scroll = app.help_scroll;
            }
            None
        }
        KeyCode::Char('k') | KeyCode::Up => {
            app.help_scroll = app.help_scroll.saturating_sub(1);
            if let Some(ref mut m) = app.help_modal {
                m.scroll = app.help_scroll;
            }
            None
        }
        KeyCode::Char('g') => {
            app.help_scroll = 0;
            if let Some(ref mut m) = app.help_modal {
                m.scroll = 0;
            }
            None
        }
        KeyCode::Char('G') => {
            app.help_scroll = 100;
            if let Some(ref mut m) = app.help_modal {
                m.scroll = 100;
            }
            None
        }
        KeyCode::Char('q') | KeyCode::Char('?') | KeyCode::Esc => {
            app.help_modal = None;
            app.mode = crate::app::Mode::Normal;
            if matches!(app.screen, Screen::Help) {
                go_back(app);
            }
            None
        }
        _ => None,
    }
}

pub fn normalize_key_code(code: KeyCode) -> KeyCode {
    match code {
        KeyCode::Char('∆') => KeyCode::Char('j'),
        KeyCode::Char('˚') => KeyCode::Char('k'),
        KeyCode::Char('˙') => KeyCode::Char('h'),
        KeyCode::Char('¬') => KeyCode::Char('l'),
        KeyCode::Char('©') => KeyCode::Char('g'),
        _ => code,
    }
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
        if app.active_panel == ActivePanel::MainViewer && (is_h || is_l || is_left || is_right) {
            return false;
        }
        if is_h || is_left {
            app.set_active_panel(ActivePanel::MenuPane);
            return true;
        }
        if is_l || is_right || code == KeyCode::Tab {
            if app.active_panel == ActivePanel::MenuPane {
                app.set_active_panel(ActivePanel::MainViewer);
            } else {
                app.set_active_panel(ActivePanel::MenuPane);
            }
            return true;
        }
    }

    false
}

pub fn mod_list_selected(screen: &mut Screen, new_selected: usize) {
    match screen {
        Screen::Catalog(s) => s.selected = new_selected,
        Screen::Schema(s) => s.selected = new_selected,
        Screen::Table(s) => s.selected = new_selected,
        Screen::Actions(s) => s.selected = new_selected,
        _ => {}
    }
}

pub fn get_selected(screen: &Screen) -> Option<usize> {
    match screen {
        Screen::Catalog(s) => Some(s.selected),
        Screen::Schema(s) => Some(s.selected),
        Screen::Table(s) => Some(s.selected),
        Screen::Actions(s) => Some(s.selected),
        _ => None,
    }
}

#[cfg(test)]
pub mod tests {
    use super::drilldown::*;
    use super::*;
    use crate::app::{ACTIONS, Action, ActionState, DrillDownState, Mode, ResultsState, TableRecon};
    use crate::config::ConnectionConfig;
    use crate::trino::client::TrinoClient;
    use crate::trino::queries;
    use crate::tui::handler::Command;

    fn sample_config() -> ConnectionConfig {
        ConnectionConfig {
            url: "https://trino.example".to_string(),
            user: "analyst".to_string(),
            password: "secret".to_string(),
        }
    }

    fn sample_app(screen: Screen) -> App {
        let mut app = App::new(sample_config(), false);
        app.clear_mouse_selection();
        app.screen = screen;
        app
    }

    #[test]
    fn go_back_returns_to_connect_when_logged_out_on_catalog_screen() {
        let config = sample_config();
        let mut app = App::new(config.clone(), false);
        app.main_panel_pct = 25;
        app.clear_mouse_selection();
        app.screen = Screen::Catalog(CatalogState {
            items: vec!["system".to_string(), "tpch".to_string()],
            selected: 1,
        });

        go_back(&mut app);

        let Screen::Connect(state) = &app.screen else {
            panic!("expected connect screen");
        };
        assert_eq!(state.url, config.url);
        assert_eq!(state.user, config.user);
        assert_eq!(state.password, config.password);
        assert_eq!(app.main_panel_pct, 60);
    }

    #[test]
    fn go_back_is_noop_for_catalog_screen_when_logged_in() {
        let mut app = sample_app(Screen::Catalog(CatalogState {
            items: vec!["system".to_string(), "tpch".to_string()],
            selected: 1,
        }));
        app.trino_client = Some(TrinoClient::new("https://trino.example", "analyst").unwrap());

        go_back(&mut app);

        let Screen::Catalog(state) = &app.screen else {
            panic!("expected catalog screen");
        };
        assert_eq!(state.selected, 1);
        assert_eq!(app.main_panel_pct, 60);
    }

    #[test]
    fn go_back_restores_previous_list_selection() {
        let mut app = sample_app(Screen::Schema(SchemaState {
            catalog: "tpch".to_string(),
            items: vec!["tiny".to_string(), "sf1".to_string()],
            selected: 1,
        }));
        app.catalogs = vec!["system".to_string(), "tpch".to_string(), "hive".to_string()];

        go_back(&mut app);

        let Screen::Catalog(state) = &app.screen else {
            panic!("expected catalog screen");
        };
        assert_eq!(state.items, app.catalogs);
        assert_eq!(state.selected, 1);

        app.clear_mouse_selection();
        app.screen = Screen::Table(TableState {
            catalog: "tpch".to_string(),
            schema: "sf1".to_string(),
            items: vec!["customer".to_string(), "orders".to_string()],
            selected: 1,
        });
        app.schemas.insert(
            "tpch".to_string(),
            vec!["tiny".to_string(), "sf1".to_string()],
        );

        go_back(&mut app);

        let Screen::Schema(state) = &app.screen else {
            panic!("expected schema screen");
        };
        assert_eq!(state.catalog, "tpch");
        assert_eq!(state.items, vec!["tiny".to_string(), "sf1".to_string()]);
        assert_eq!(state.selected, 1);

        app.clear_mouse_selection();
        app.screen = Screen::Actions(ActionState {
            catalog: "tpch".to_string(),
            schema: "sf1".to_string(),
            table: "orders".to_string(),
            selected: 0,
            query_buffer: "SELECT * FROM orders".to_string(),
            query_cursor: 20,
            results: None,
            ..Default::default()
        });
        app.tables.insert(
            ("tpch".to_string(), "sf1".to_string()),
            vec!["customer".to_string(), "orders".to_string()],
        );

        go_back(&mut app);

        let Screen::Table(state) = &app.screen else {
            panic!("expected table screen");
        };
        assert_eq!(state.catalog, "tpch");
        assert_eq!(state.schema, "sf1");
        assert_eq!(
            state.items,
            vec!["customer".to_string(), "orders".to_string()]
        );
        assert_eq!(state.selected, 1);
    }

    #[test]
    fn go_back_restores_prev_screen_from_help() {
        let previous_screen = Screen::Table(TableState {
            catalog: "tpch".to_string(),
            schema: "sf1".to_string(),
            items: vec!["customer".to_string(), "orders".to_string()],
            selected: 0,
        });
        let mut app = sample_app(Screen::Help);
        app.prev_screen = Some(Box::new(previous_screen));

        go_back(&mut app);

        let Screen::Table(state) = &app.screen else {
            panic!("expected restored table screen");
        };
        assert_eq!(state.catalog, "tpch");
        assert_eq!(state.schema, "sf1");
        assert_eq!(
            state.items,
            vec!["customer".to_string(), "orders".to_string()]
        );
        assert_eq!(state.selected, 0);
        assert!(app.prev_screen.is_none());
    }

    #[test]
    fn test_select_current_item_with_search_filter() {
        let mut app = sample_app(Screen::Table(TableState {
            catalog: "datalake".to_string(),
            schema: "default".to_string(),
            items: vec![
                "activity".to_string(),
                "policy".to_string(),
                "events".to_string(),
                "enableTenants".to_string(),
            ],
            selected: 0,
        }));
        app.search_query = "e".to_string();

        let cmd = select_current_item(&mut app);
        assert!(matches!(cmd, Some(Command::FetchTableMetadata { .. })));

        let Screen::Actions(state) = &app.screen else {
            panic!("expected Screen::Actions");
        };
        assert_eq!(state.table, "events");
        assert!(state.metadata_loading);
    }

    fn drilldown_action_state(partition_cols: Vec<&str>) -> ActionState {
        ActionState {
            catalog: "datalake".to_string(),
            schema: "tenant".to_string(),
            table: "events".to_string(),
            metadata: Some(TableRecon {
                partitioned_by: partition_cols.iter().map(|s| s.to_string()).collect(),
                location: "s3://bucket/events/".to_string(),
                ddl_text: "CREATE TABLE events (...)".to_string(),
            }),
            ..Default::default()
        }
    }

    #[test]
    fn trigger_action_blocks_partitions_and_schema_while_metadata_loading() {
        let mut app = sample_app(Screen::Actions(ActionState {
            metadata_loading: true,
            ..drilldown_action_state(vec!["date"])
        }));

        let partitions_idx = ACTIONS
            .iter()
            .position(|(_, _, a)| *a == Action::Partitions)
            .unwrap();
        let schema_idx = ACTIONS
            .iter()
            .position(|(_, _, a)| *a == Action::Schema)
            .unwrap();
        for idx in [partitions_idx, schema_idx] {
            let cmd = trigger_action(&mut app, idx);
            assert!(
                cmd.is_none(),
                "action {idx} should be blocked while loading"
            );
        }
    }

    #[test]
    fn trigger_action_blocks_table_view_and_ddl_while_ddl_loading() {
        let mut app = sample_app(Screen::Actions(ActionState {
            ddl_loading: true,
            ..drilldown_action_state(vec!["date"])
        }));

        for idx in [0usize, 1] {
            let cmd = trigger_action(&mut app, idx);
            assert!(
                cmd.is_none(),
                "action {idx} should be blocked while ddl loading"
            );
        }
    }

    #[test]
    fn trigger_action_table_ddl_uses_cached_recon_ddl_without_query() {
        let mut app = sample_app(Screen::Actions(drilldown_action_state(vec!["date"])));

        let cmd = trigger_action(&mut app, 1);
        assert!(
            cmd.is_none(),
            "Table DDL should render from cached recon, not fire a query"
        );

        let Screen::Actions(state) = &app.screen else {
            panic!("expected actions screen");
        };
        let results = state.results.as_ref().expect("results should be populated");
        assert_eq!(
            results.rows,
            vec![vec!["CREATE TABLE events (...)".to_string()]]
        );
        let expected_ddl_query = "SHOW CREATE TABLE \"datalake\".\"tenant\".\"events\"";
        assert_eq!(state.query_buffer, expected_ddl_query);
        assert_eq!(results.query_buffer, expected_ddl_query);
    }

    #[test]
    fn trigger_action_table_view_runs_direct_query_for_unpartitioned_table() {
        let mut app = sample_app(Screen::Actions(drilldown_action_state(vec![])));

        let cmd = trigger_action(&mut app, 0);
        let Some(Command::ExecuteQuery {
            is_paginated,
            filters,
            ..
        }) = cmd
        else {
            panic!("expected ExecuteQuery for unpartitioned table view");
        };
        assert!(is_paginated);
        assert!(filters.is_empty());

        let Screen::Actions(state) = &app.screen else {
            panic!("expected actions screen");
        };
        assert!(state.drilldown.is_none());
    }

    #[test]
    fn trigger_action_table_view_starts_drilldown_for_partitioned_table() {
        let mut app = sample_app(Screen::Actions(drilldown_action_state(vec![
            "date",
            "service",
            "account_id",
        ])));

        let cmd = trigger_action(&mut app, 0);
        let Some(Command::FetchPartitionLevel {
            column, filters, ..
        }) = cmd
        else {
            panic!("expected FetchPartitionLevel to start drilldown");
        };
        assert_eq!(column, "date");
        assert!(filters.is_empty());

        let Screen::Actions(state) = &app.screen else {
            panic!("expected actions screen");
        };
        let dd = state
            .drilldown
            .as_ref()
            .expect("drilldown should be initialized");
        assert_eq!(dd.partition_cols, vec!["date", "service", "account_id"]);
        assert!(dd.path.is_empty());
        assert!(dd.loading);
    }

    #[test]
    fn drilldown_drill_into_selected_advances_to_next_level() {
        let mut s = drilldown_action_state(vec!["date", "service"]);
        s.drilldown = Some(DrillDownState {
            partition_cols: vec!["date".to_string(), "service".to_string()],
            path: Vec::new(),
            levels_cache: vec![vec!["2026-08-06".to_string(), "2026-08-05".to_string()]],
            selected: 0,
            loading: false,
            error: None,
            truncated: false,
            ..Default::default()
        });

        let cmd = drilldown_drill_into_selected(&mut s, &[]);
        let Some(Command::FetchPartitionLevel {
            column, filters, ..
        }) = cmd
        else {
            panic!("expected FetchPartitionLevel for next level");
        };
        assert_eq!(column, "service");
        assert_eq!(
            filters,
            vec![("date".to_string(), "2026-08-06".to_string())]
        );

        let dd = s.drilldown.as_ref().unwrap();
        assert_eq!(
            dd.path,
            vec![("date".to_string(), "2026-08-06".to_string())]
        );
        assert!(dd.loading);
        assert_eq!(dd.selected, 0);
        let expected_level_query = queries::distinct_partition_values(
            "datalake",
            "tenant",
            "events",
            &[("date".to_string(), "2026-08-06".to_string())],
            "service",
            200,
        );
        assert_eq!(s.query_buffer, expected_level_query);
    }

    #[test]
    fn drilldown_drill_into_selected_switches_to_leaf_query_on_last_column() {
        let mut s = drilldown_action_state(vec!["date"]);
        s.drilldown = Some(DrillDownState {
            partition_cols: vec!["date".to_string()],
            path: Vec::new(),
            levels_cache: vec![vec!["2026-08-06".to_string()]],
            selected: 0,
            loading: false,
            error: None,
            truncated: false,
            ..Default::default()
        });

        let cmd = drilldown_drill_into_selected(&mut s, &[]);
        let Some(Command::ExecuteQuery {
            is_paginated,
            filters,
            query,
            ..
        }) = cmd
        else {
            panic!("expected leaf ExecuteQuery");
        };
        assert!(is_paginated);
        assert_eq!(
            filters,
            vec![("date".to_string(), "2026-08-06".to_string())]
        );
        assert!(query.contains("WHERE date = '2026-08-06'"));
        assert!(s.results.is_none());
        assert_eq!(s.query_buffer, query);
    }

    #[test]
    fn drilldown_go_up_pops_path_and_clears_leaf_results() {
        let mut s = drilldown_action_state(vec!["date", "service"]);
        s.drilldown = Some(DrillDownState {
            partition_cols: vec!["date".to_string(), "service".to_string()],
            path: vec![("date".to_string(), "2026-08-06".to_string())],
            levels_cache: vec![vec!["2026-08-06".to_string()], vec!["smb3".to_string()]],
            selected: 3,
            loading: false,
            error: None,
            truncated: false,
            ..Default::default()
        });
        s.results = Some(sample_leaf_results_state());

        drilldown_go_up(&mut s);

        let dd = s.drilldown.as_ref().unwrap();
        assert!(dd.path.is_empty());
        assert_eq!(dd.selected, 0);
        assert!(s.results.is_none());
        let expected_parent_query = queries::distinct_partition_values(
            "datalake",
            "tenant",
            "events",
            &[],
            "date",
            200,
        );
        assert_eq!(s.query_buffer, expected_parent_query);
    }

    #[test]
    fn drilldown_go_up_is_noop_at_top_level() {
        let mut s = drilldown_action_state(vec!["date"]);
        s.drilldown = Some(DrillDownState {
            partition_cols: vec!["date".to_string()],
            path: Vec::new(),
            levels_cache: vec![vec!["2026-08-06".to_string()]],
            selected: 0,
            loading: false,
            error: None,
            truncated: false,
            ..Default::default()
        });

        drilldown_go_up(&mut s);

        let dd = s.drilldown.as_ref().unwrap();
        assert!(dd.path.is_empty());
        assert_eq!(dd.selected, 0);
    }

    fn sample_leaf_results_state() -> ResultsState {
        ResultsState {
            query_buffer: String::new(),
            query_cursor: 0,
            columns: vec!["event_type".to_string()],
            rows: vec![vec!["created".to_string()]],
            scroll_v: std::cell::Cell::new(0),
            scroll_h: std::cell::Cell::new(0),
            loading: false,
            error: None,
            is_paginated: true,
            catalog: "datalake".to_string(),
            schema: "tenant".to_string(),
            table: "events".to_string(),
            offset: 0,
            page_size: 100,
            is_fetching_next_page: false,
            has_more_rows: false,
            invalid_query_error: None,
            selection_anchor: None,
            filters: vec![("date".to_string(), "2026-08-06".to_string())],
            total_count: None,
            total_count_loading: false,
            table_count: None,
            ..Default::default()
        }
    }

    #[test]
    fn actions_keys_menu_navigation_resets_results_pane() {
        let mut app = sample_app(Screen::Actions(ActionState {
            catalog: "iceberg".to_string(),
            schema: "sales".to_string(),
            table: "orders".to_string(),
            selected: 0,
            results: Some(sample_leaf_results_state()),
            ..Default::default()
        }));
        app.set_active_panel(ActivePanel::MenuPane);

        actions_keys(&mut app, KeyEvent::from(KeyCode::Char('j')));

        let Screen::Actions(state) = &app.screen else {
            panic!("expected actions screen");
        };
        assert_eq!(state.selected, 1);
        assert!(
            state.results.is_none(),
            "moving menu selection should reset results pane"
        );
        assert_eq!(
            state.query_buffer,
            "SHOW CREATE TABLE \"iceberg\".\"sales\".\"orders\""
        );
    }

    #[test]
    fn actions_keys_menu_navigation_syncs_query_buffer_for_each_action() {
        let mut app = sample_app(Screen::Actions(ActionState {
            catalog: "iceberg".to_string(),
            schema: "sales".to_string(),
            table: "orders".to_string(),
            selected: 0,
            ..Default::default()
        }));
        app.set_active_panel(ActivePanel::MenuPane);

        for (expected_idx, (_, _, action)) in ACTIONS.iter().enumerate().skip(1) {
            actions_keys(&mut app, KeyEvent::from(KeyCode::Char('j')));
            let Screen::Actions(state) = &app.screen else {
                panic!("expected actions screen");
            };
            assert_eq!(state.selected, expected_idx);
            let expected_query = action.build_query("iceberg", "sales", "orders");
            assert_eq!(state.query_buffer, expected_query);
        }

        for (expected_idx, (_, _, action)) in ACTIONS.iter().enumerate().take(ACTIONS.len() - 1).rev() {
            actions_keys(&mut app, KeyEvent::from(KeyCode::Char('k')));
            let Screen::Actions(state) = &app.screen else {
                panic!("expected actions screen");
            };
            assert_eq!(state.selected, expected_idx);
            let expected_query = action.build_query("iceberg", "sales", "orders");
            assert_eq!(state.query_buffer, expected_query);
        }
    }

    #[test]
    fn trigger_action_partitions_and_schema_sync_query_buffer_and_clear_results() {
        let mut app = sample_app(Screen::Actions(ActionState {
            catalog: "iceberg".to_string(),
            schema: "sales".to_string(),
            table: "orders".to_string(),
            selected: 0,
            results: Some(sample_leaf_results_state()),
            ..Default::default()
        }));

        let partitions_idx = ACTIONS
            .iter()
            .position(|(_, _, a)| *a == Action::Partitions)
            .unwrap();
        let schema_idx = ACTIONS
            .iter()
            .position(|(_, _, a)| *a == Action::Schema)
            .unwrap();

        // Trigger Partitions
        let cmd = trigger_action(&mut app, partitions_idx);
        assert!(cmd.is_none());
        let Screen::Actions(state) = &app.screen else {
            panic!("expected actions screen");
        };
        assert_eq!(state.selected, partitions_idx);
        assert!(state.results.is_none());
        assert_eq!(
            state.query_buffer,
            "SHOW CREATE TABLE \"iceberg\".\"sales\".\"orders\""
        );

        // Put stale results back
        if let Screen::Actions(ref mut state) = app.screen {
            state.results = Some(sample_leaf_results_state());
        }

        // Trigger Schema
        let cmd = trigger_action(&mut app, schema_idx);
        assert!(cmd.is_none());
        let Screen::Actions(state) = &app.screen else {
            panic!("expected actions screen");
        };
        assert_eq!(state.selected, schema_idx);
        assert!(state.results.is_none());
        assert_eq!(
            state.query_buffer,
            "DESCRIBE \"iceberg\".\"sales\".\"orders\""
        );
    }

    #[test]
    fn actions_keys_h_and_l_move_columns_in_generic_results() {
        let mut res = sample_leaf_results_state();
        res.columns = vec!["col_a".to_string(), "col_b".to_string()];
        res.rows = vec![vec!["a".to_string(), "b".to_string()]];
        let mut app = sample_app(Screen::Actions(ActionState {
            catalog: "iceberg".to_string(),
            schema: "sales".to_string(),
            table: "orders".to_string(),
            selected: 3,
            results: Some(res),
            ..Default::default()
        }));
        app.set_active_panel(ActivePanel::MainViewer);

        actions_keys(&mut app, KeyEvent::from(KeyCode::Char('l')));
        let Screen::Actions(state) = &app.screen else {
            panic!("expected actions screen");
        };
        assert_eq!(state.results.as_ref().unwrap().selected_col, 1);

        actions_keys(&mut app, KeyEvent::from(KeyCode::Char('h')));
        let Screen::Actions(state) = &app.screen else {
            panic!("expected actions screen");
        };
        assert_eq!(state.results.as_ref().unwrap().selected_col, 0);
    }

    #[test]
    fn actions_keys_less_than_and_greater_than_are_no_ops_in_generic_results() {
        let mut app = sample_app(Screen::Actions(ActionState {
            catalog: "iceberg".to_string(),
            schema: "sales".to_string(),
            table: "orders".to_string(),
            selected: 3,
            results: Some(sample_leaf_results_state()),
            ..Default::default()
        }));
        app.set_active_panel(ActivePanel::MainViewer);

        let cmd = actions_keys(&mut app, KeyEvent::from(KeyCode::Char('>')));
        assert!(cmd.is_none());
        let Screen::Actions(state) = &app.screen else {
            panic!("expected actions screen");
        };
        assert_eq!(state.results.as_ref().unwrap().scroll_h.get(), 0);
        assert!(matches!(app.active_panel, ActivePanel::MainViewer));

        let cmd = actions_keys(&mut app, KeyEvent::from(KeyCode::Char('<')));
        assert!(cmd.is_none());
        assert!(matches!(app.active_panel, ActivePanel::MainViewer));
    }

    #[test]
    fn actions_keys_c_no_longer_triggers_copy_only_y_does() {
        let mut app = sample_app(Screen::Actions(ActionState {
            catalog: "iceberg".to_string(),
            schema: "sales".to_string(),
            table: "orders".to_string(),
            selected: 3,
            results: Some(sample_leaf_results_state()),
            ..Default::default()
        }));
        app.set_active_panel(ActivePanel::MainViewer);

        actions_keys(&mut app, KeyEvent::from(KeyCode::Char('c')));
        assert!(app.copied_toast.is_none(), "'c' should not copy anymore");

        actions_keys(&mut app, KeyEvent::from(KeyCode::Char('y')));
        assert!(app.copied_toast.is_some(), "'y' should still copy");
    }

    #[test]
    fn actions_keys_leaf_grid_lt_goes_up_partition_and_h_l_move_columns() {
        let mut res = sample_leaf_results_state();
        res.columns = vec!["col_a".to_string(), "col_b".to_string()];
        res.rows = vec![vec!["a".to_string(), "b".to_string()]];
        let mut app = sample_app(Screen::Actions(ActionState {
            selected: 0,
            drilldown: Some(DrillDownState {
                partition_cols: vec!["date".to_string()],
                path: vec![("date".to_string(), "2026-08-06".to_string())],
                levels_cache: vec![vec!["2026-08-06".to_string()]],
                selected: 0,
                loading: false,
                error: None,
                truncated: false,
                ..Default::default()
            }),
            results: Some(res),
            ..drilldown_action_state(vec!["date"])
        }));
        app.set_active_panel(ActivePanel::MainViewer);

        // 'l' moves column right in leaf grid
        actions_keys(&mut app, KeyEvent::from(KeyCode::Char('l')));
        let Screen::Actions(state) = &app.screen else {
            panic!("expected actions screen");
        };
        assert_eq!(state.results.as_ref().unwrap().selected_col, 1);
        assert!(state.drilldown.as_ref().unwrap().is_leaf());

        // '>' is no-op at leaf
        actions_keys(&mut app, KeyEvent::from(KeyCode::Char('>')));
        let Screen::Actions(state) = &app.screen else {
            panic!("expected actions screen");
        };
        assert!(state.drilldown.as_ref().unwrap().is_leaf());

        // 'h' moves column left in leaf grid
        actions_keys(&mut app, KeyEvent::from(KeyCode::Char('h')));
        let Screen::Actions(state) = &app.screen else {
            panic!("expected actions screen");
        };
        assert_eq!(state.results.as_ref().unwrap().selected_col, 0);

        // '<' goes up one partition level
        actions_keys(&mut app, KeyEvent::from(KeyCode::Char('<')));
        let Screen::Actions(state) = &app.screen else {
            panic!("expected actions screen");
        };
        assert!(state.drilldown.as_ref().unwrap().path.is_empty());
        assert!(state.results.is_none());
    }

    #[test]
    fn actions_keys_drilldown_non_leaf_gt_drills_down_and_lt_goes_up() {
        let mut app = sample_app(Screen::Actions(ActionState {
            selected: 0,
            drilldown: Some(DrillDownState {
                partition_cols: vec!["date".to_string(), "service".to_string()],
                path: vec![("date".to_string(), "2026-08-06".to_string())],
                levels_cache: vec![
                    vec!["2026-08-06".to_string()],
                    vec!["api".to_string(), "worker".to_string()],
                ],
                selected: 1,
                loading: false,
                error: None,
                truncated: false,
                ..Default::default()
            }),
            results: None,
            ..drilldown_action_state(vec!["date", "service"])
        }));
        app.set_active_panel(ActivePanel::MainViewer);

        // '>' drills into the selected partition ("worker")
        let cmd = actions_keys(&mut app, KeyEvent::from(KeyCode::Char('>')));
        assert!(cmd.is_some());
        let Screen::Actions(state) = &app.screen else {
            panic!("expected actions screen");
        };
        assert_eq!(
            state.drilldown.as_ref().unwrap().path,
            vec![
                ("date".to_string(), "2026-08-06".to_string()),
                ("service".to_string(), "worker".to_string()),
            ]
        );

        // '<' pops the path and goes up
        actions_keys(&mut app, KeyEvent::from(KeyCode::Char('<')));
        let Screen::Actions(state) = &app.screen else {
            panic!("expected actions screen");
        };
        assert_eq!(
            state.drilldown.as_ref().unwrap().path,
            vec![("date".to_string(), "2026-08-06".to_string())]
        );
    }

    #[test]
    fn connect_pane_allows_typing_digits_and_symbols_without_triggering_shortcuts() {
        let mut app = sample_app(Screen::Connect(ConnectState {
            url: "http://localhost:".to_string(),
            user: "trino_".to_string(),
            password: "pass_".to_string(),
            focused: 0,
            loading: false,
            error: None,
        }));

        // Type digits into URL field (focused = 0)
        crate::tui::handler::handle_key_sync(&mut app, KeyEvent::from(KeyCode::Char('8')));
        crate::tui::handler::handle_key_sync(&mut app, KeyEvent::from(KeyCode::Char('0')));
        crate::tui::handler::handle_key_sync(&mut app, KeyEvent::from(KeyCode::Char('8')));
        crate::tui::handler::handle_key_sync(&mut app, KeyEvent::from(KeyCode::Char('0')));
        // Type / into URL
        crate::tui::handler::handle_key_sync(&mut app, KeyEvent::from(KeyCode::Char('/')));

        let Screen::Connect(ref state) = app.screen else {
            panic!("expected connect screen");
        };
        assert_eq!(state.url, "http://localhost:8080/");
        assert!(
            app.number_buffer.is_empty(),
            "number_buffer must not be populated on connect screen"
        );
        assert!(
            matches!(app.mode, Mode::Normal),
            "typing / on connect screen must not enter Search mode"
        );

        // Move focus to user (focused = 1)
        crate::tui::handler::handle_key_sync(&mut app, KeyEvent::from(KeyCode::Tab));
        let Screen::Connect(ref state) = app.screen else {
            panic!("expected connect screen");
        };
        assert_eq!(state.focused, 1);

        // Type digits into username
        crate::tui::handler::handle_key_sync(&mut app, KeyEvent::from(KeyCode::Char('1')));
        crate::tui::handler::handle_key_sync(&mut app, KeyEvent::from(KeyCode::Char('2')));
        crate::tui::handler::handle_key_sync(&mut app, KeyEvent::from(KeyCode::Char('3')));

        let Screen::Connect(ref state) = app.screen else {
            panic!("expected connect screen");
        };
        assert_eq!(state.user, "trino_123");

        // Move focus to password (focused = 2) via Down arrow
        crate::tui::handler::handle_key_sync(&mut app, KeyEvent::from(KeyCode::Down));
        let Screen::Connect(ref state) = app.screen else {
            panic!("expected connect screen");
        };
        assert_eq!(state.focused, 2);

        // Type digits and special characters into password
        crate::tui::handler::handle_key_sync(&mut app, KeyEvent::from(KeyCode::Char('4')));
        crate::tui::handler::handle_key_sync(&mut app, KeyEvent::from(KeyCode::Char('5')));
        crate::tui::handler::handle_key_sync(&mut app, KeyEvent::from(KeyCode::Char('?')));

        let Screen::Connect(ref state) = app.screen else {
            panic!("expected connect screen");
        };
        assert_eq!(state.password, "pass_45?");
        assert!(
            !matches!(app.screen, Screen::Help),
            "typing ? on connect screen must not open Help"
        );
    }

    #[test]
    fn test_switching_from_table_ddl_to_table_view_fires_query_and_clears_ddl_results() {
        let mut app = sample_app(Screen::Actions(ActionState {
            catalog: "iceberg".to_string(),
            schema: "sales".to_string(),
            table: "orders".to_string(),
            selected: 1, // Table DDL
            metadata: Some(TableRecon {
                partitioned_by: Vec::new(),
                location: "s3://bucket/orders".to_string(),
                ddl_text: "CREATE TABLE orders (id bigint)".to_string(),
            }),
            results: Some(ResultsState {
                query_buffer: String::new(),
                query_cursor: 0,
                columns: vec!["Create Table".to_string()],
                rows: vec![vec!["CREATE TABLE orders (id bigint)".to_string()]],
                scroll_v: std::cell::Cell::new(0),
                scroll_h: std::cell::Cell::new(0),
                loading: false,
                error: None,
                is_paginated: false,
                catalog: "iceberg".to_string(),
                schema: "sales".to_string(),
                table: "orders".to_string(),
                offset: 0,
                page_size: 100,
                is_fetching_next_page: false,
                has_more_rows: false,
                invalid_query_error: None,
                selection_anchor: None,
                filters: Vec::new(),
                total_count: None,
                total_count_loading: false,
                table_count: None,
                ..Default::default()
            }),
            ..Default::default()
        }));

        // User triggers Table View (action idx 0)
        let cmd = trigger_action(&mut app, 0);
        let Some(Command::ExecuteQuery {
            is_paginated,
            catalog,
            schema,
            table,
            ..
        }) = cmd
        else {
            panic!("expected ExecuteQuery when switching from Table DDL to Table View");
        };
        assert!(is_paginated);
        assert_eq!(catalog, "iceberg");
        assert_eq!(schema, "sales");
        assert_eq!(table, "orders");

        let Screen::Actions(ref state) = app.screen else {
            panic!("expected actions screen");
        };
        assert_eq!(state.selected, 0);
        assert!(
            state.results.is_none(),
            "stale DDL results must be cleared on transition to Table View"
        );
    }

    #[test]
    fn actions_keys_cell_navigation_and_filter_modal_trigger() {
        let mut app = sample_app(Screen::Actions(ActionState {
            catalog: "iceberg".to_string(),
            schema: "sales".to_string(),
            table: "orders".to_string(),
            selected: 0,
            results: Some(ResultsState {
                query_buffer: "SELECT id, status, amount FROM orders".to_string(),
                query_cursor: 37,
                columns: vec!["id".to_string(), "status".to_string(), "amount".to_string()],
                rows: vec![
                    vec!["1".to_string(), "active".to_string(), "100".to_string()],
                    vec!["2".to_string(), "pending".to_string(), "200".to_string()],
                    vec!["3".to_string(), "inactive".to_string(), "300".to_string()],
                ],
                scroll_v: std::cell::Cell::new(0),
                scroll_h: std::cell::Cell::new(0),
                selected_row: 0,
                selected_col: 0,
                ..Default::default()
            }),
            ..Default::default()
        }));
        app.set_active_panel(ActivePanel::MainViewer);

        // Move cursor down (row 0 -> 1)
        actions_keys(&mut app, KeyEvent::from(KeyCode::Char('j')));
        let Screen::Actions(state) = &app.screen else {
            panic!("expected actions screen");
        };
        assert_eq!(state.results.as_ref().unwrap().selected_row, 1);

        // Move cursor right (l: col 0 -> 1)
        actions_keys(&mut app, KeyEvent::from(KeyCode::Char('l')));
        let Screen::Actions(state) = &app.screen else {
            panic!("expected actions screen");
        };
        assert_eq!(state.results.as_ref().unwrap().selected_col, 1);

        // Move cursor right again (l: col 1 -> 2)
        actions_keys(&mut app, KeyEvent::from(KeyCode::Char('l')));
        let Screen::Actions(state) = &app.screen else {
            panic!("expected actions screen");
        };
        assert_eq!(state.results.as_ref().unwrap().selected_col, 2);

        // Move cursor left (h: col 2 -> 1)
        actions_keys(&mut app, KeyEvent::from(KeyCode::Char('h')));
        let Screen::Actions(state) = &app.screen else {
            panic!("expected actions screen");
        };
        assert_eq!(state.results.as_ref().unwrap().selected_col, 1);

        // Move cursor up (k: row 1 -> 0)
        actions_keys(&mut app, KeyEvent::from(KeyCode::Char('k')));
        let Screen::Actions(state) = &app.screen else {
            panic!("expected actions screen");
        };
        assert_eq!(state.results.as_ref().unwrap().selected_row, 0);

        // Press 'F' while selected_col is 1 ("status")
        actions_keys(&mut app, KeyEvent::from(KeyCode::Char('F')));
        assert!(matches!(app.mode, Mode::FilterModal));
        let modal = app
            .filter_modal
            .as_ref()
            .expect("filter modal should be opened");
        assert_eq!(modal.column_index, 1);
        assert_eq!(modal.column_name, "status");
    }

    #[test]
    fn actions_keys_column_visibility_modal_and_visible_column_navigation() {
        let mut hidden = std::collections::HashSet::new();
        hidden.insert("status".to_string()); // hide middle column
        let mut app = sample_app(Screen::Actions(ActionState {
            catalog: "iceberg".to_string(),
            schema: "sales".to_string(),
            table: "orders".to_string(),
            selected: 0,
            results: Some(ResultsState {
                query_buffer: "SELECT id, status, amount FROM orders".to_string(),
                query_cursor: 37,
                columns: vec!["id".to_string(), "status".to_string(), "amount".to_string()],
                rows: vec![
                    vec!["1".to_string(), "active".to_string(), "100".to_string()],
                ],
                scroll_v: std::cell::Cell::new(0),
                scroll_h: std::cell::Cell::new(0),
                selected_row: 0,
                selected_col: 0, // starts at 'id' (col 0)
                hidden_columns: hidden,
                ..Default::default()
            }),
            ..Default::default()
        }));
        app.set_active_panel(ActivePanel::MainViewer);

        // Move cursor right (l): should skip hidden 'status' (col 1) and jump to 'amount' (col 2)
        actions_keys(&mut app, KeyEvent::from(KeyCode::Char('l')));
        let Screen::Actions(state) = &app.screen else {
            panic!("expected actions screen");
        };
        assert_eq!(state.results.as_ref().unwrap().selected_col, 2);

        // Move cursor left (h): should skip hidden 'status' (col 1) and jump back to 'id' (col 0)
        actions_keys(&mut app, KeyEvent::from(KeyCode::Char('h')));
        let Screen::Actions(state) = &app.screen else {
            panic!("expected actions screen");
        };
        assert_eq!(state.results.as_ref().unwrap().selected_col, 0);

        // Press 'C' -> opens column visibility modal
        actions_keys(&mut app, KeyEvent::from(KeyCode::Char('C')));
        assert!(matches!(app.mode, Mode::ColumnModal));
        let modal = app
            .column_modal
            .as_ref()
            .expect("column modal should be opened");
        assert_eq!(modal.columns, vec!["id", "status", "amount"]);
        assert!(modal.selected_columns.contains("id"));
        assert!(!modal.selected_columns.contains("status"));
        assert!(modal.selected_columns.contains("amount"));
    }

    #[test]
    fn test_help_keys_scroll_and_close() {
        let mut app = sample_app(Screen::Help);
        app.prev_screen = Some(Box::new(Screen::Table(TableState {
            catalog: "iceberg".to_string(),
            schema: "sales".to_string(),
            items: vec!["orders".to_string()],
            selected: 0,
        })));
        assert_eq!(app.help_scroll, 0);

        help_keys(&mut app, KeyEvent::from(KeyCode::Char('j')));
        assert_eq!(app.help_scroll, 1);

        help_keys(&mut app, KeyEvent::from(KeyCode::Char('k')));
        assert_eq!(app.help_scroll, 0);

        help_keys(&mut app, KeyEvent::from(KeyCode::Char('G')));
        assert_eq!(app.help_scroll, 100);

        help_keys(&mut app, KeyEvent::from(KeyCode::Char('g')));
        assert_eq!(app.help_scroll, 0);

        help_keys(&mut app, KeyEvent::from(KeyCode::Char('q')));
        assert!(matches!(app.screen, Screen::Table(_)));
    }

    #[test]
    fn test_actions_keys_page_down_and_up_in_table_view() {
        let mut app = sample_app(Screen::Actions(ActionState {
            selected: 0,
            results: Some(ResultsState {
                rows: (0..50).map(|i| vec![format!("{i}")]).collect(),
                columns: vec!["id".to_string()],
                selected_row: 0,
                selected_col: 0,
                is_paginated: true,
                ..Default::default()
            }),
            ..Default::default()
        }));
        app.active_panel = ActivePanel::MainViewer;

        // Page down
        actions_keys(&mut app, KeyEvent::from(KeyCode::PageDown));
        let Screen::Actions(state) = &app.screen else { panic!() };
        assert_eq!(state.results.as_ref().unwrap().selected_row, 15);

        // Page down again
        actions_keys(&mut app, KeyEvent::from(KeyCode::PageDown));
        let Screen::Actions(state) = &app.screen else { panic!() };
        assert_eq!(state.results.as_ref().unwrap().selected_row, 30);

        // Page up
        actions_keys(&mut app, KeyEvent::from(KeyCode::PageUp));
        let Screen::Actions(state) = &app.screen else { panic!() };
        assert_eq!(state.results.as_ref().unwrap().selected_row, 15);

        // Ctrl+d (page down)
        let ctrl_d = KeyEvent::new(KeyCode::Char('d'), crossterm::event::KeyModifiers::CONTROL);
        actions_keys(&mut app, ctrl_d);
        let Screen::Actions(state) = &app.screen else { panic!() };
        assert_eq!(state.results.as_ref().unwrap().selected_row, 30);

        // Ctrl+u (page up)
        let ctrl_u = KeyEvent::new(KeyCode::Char('u'), crossterm::event::KeyModifiers::CONTROL);
        actions_keys(&mut app, ctrl_u);
        let Screen::Actions(state) = &app.screen else { panic!() };
        assert_eq!(state.results.as_ref().unwrap().selected_row, 15);
    }

    #[test]
    fn test_actions_keys_horizontal_navigation_in_table_view() {
        let cols: Vec<String> = (0..20).map(|i| format!("col_{i}")).collect();
        let mut app = sample_app(Screen::Actions(ActionState {
            selected: 0,
            results: Some(ResultsState {
                rows: vec![vec!["val".to_string()]],
                columns: cols,
                selected_row: 0,
                selected_col: 0,
                ..Default::default()
            }),
            ..Default::default()
        }));
        app.active_panel = ActivePanel::MainViewer;

        // 1. Ctrl+Right moves 5 columns right
        let ctrl_right = KeyEvent::new(KeyCode::Right, crossterm::event::KeyModifiers::CONTROL);
        actions_keys(&mut app, ctrl_right);
        let Screen::Actions(state) = &app.screen else { panic!() };
        assert_eq!(state.results.as_ref().unwrap().selected_col, 5);

        // 2. Ctrl+l moves 5 columns right
        let ctrl_l = KeyEvent::new(KeyCode::Char('l'), crossterm::event::KeyModifiers::CONTROL);
        actions_keys(&mut app, ctrl_l);
        let Screen::Actions(state) = &app.screen else { panic!() };
        assert_eq!(state.results.as_ref().unwrap().selected_col, 10);

        // 3. Ctrl+Left moves 5 columns left
        let ctrl_left = KeyEvent::new(KeyCode::Left, crossterm::event::KeyModifiers::CONTROL);
        actions_keys(&mut app, ctrl_left);
        let Screen::Actions(state) = &app.screen else { panic!() };
        assert_eq!(state.results.as_ref().unwrap().selected_col, 5);

        // 4. End jumps to last column (19)
        actions_keys(&mut app, KeyEvent::from(KeyCode::End));
        let Screen::Actions(state) = &app.screen else { panic!() };
        assert_eq!(state.results.as_ref().unwrap().selected_col, 19);

        // 5. Home jumps to first column (0)
        actions_keys(&mut app, KeyEvent::from(KeyCode::Home));
        let Screen::Actions(state) = &app.screen else { panic!() };
        assert_eq!(state.results.as_ref().unwrap().selected_col, 0);

        // 6. '$' jumps to last column
        actions_keys(&mut app, KeyEvent::from(KeyCode::Char('$')));
        let Screen::Actions(state) = &app.screen else { panic!() };
        assert_eq!(state.results.as_ref().unwrap().selected_col, 19);

        // 7. '0' jumps to first column
        actions_keys(&mut app, KeyEvent::from(KeyCode::Char('0')));
        let Screen::Actions(state) = &app.screen else { panic!() };
        assert_eq!(state.results.as_ref().unwrap().selected_col, 0);

        // 8. Shift+Right moves 5 columns right
        let shift_right = KeyEvent::new(KeyCode::Right, crossterm::event::KeyModifiers::SHIFT);
        actions_keys(&mut app, shift_right);
        let Screen::Actions(state) = &app.screen else { panic!() };
        assert_eq!(state.results.as_ref().unwrap().selected_col, 5);

        // 9. 'L' moves 5 columns right
        actions_keys(&mut app, KeyEvent::from(KeyCode::Char('L')));
        let Screen::Actions(state) = &app.screen else { panic!() };
        assert_eq!(state.results.as_ref().unwrap().selected_col, 10);

        // 10. 'H' moves 5 columns left
        actions_keys(&mut app, KeyEvent::from(KeyCode::Char('H')));
        let Screen::Actions(state) = &app.screen else { panic!() };
        assert_eq!(state.results.as_ref().unwrap().selected_col, 5);

        // 11. Shift+Left moves 5 columns left
        let shift_left = KeyEvent::new(KeyCode::Left, crossterm::event::KeyModifiers::SHIFT);
        actions_keys(&mut app, shift_left);
        let Screen::Actions(state) = &app.screen else { panic!() };
        assert_eq!(state.results.as_ref().unwrap().selected_col, 0);

        // 12. ']' moves 1 column right and '[' moves 1 column left
        actions_keys(&mut app, KeyEvent::from(KeyCode::Char(']')));
        let Screen::Actions(state) = &app.screen else { panic!() };
        assert_eq!(state.results.as_ref().unwrap().selected_col, 1);

        actions_keys(&mut app, KeyEvent::from(KeyCode::Char('[')));
        let Screen::Actions(state) = &app.screen else { panic!() };
        assert_eq!(state.results.as_ref().unwrap().selected_col, 0);

        // 13. '>' moves 5 columns right and '<' moves 5 columns left
        actions_keys(&mut app, KeyEvent::from(KeyCode::Char('>')));
        let Screen::Actions(state) = &app.screen else { panic!() };
        assert_eq!(state.results.as_ref().unwrap().selected_col, 5);

        actions_keys(&mut app, KeyEvent::from(KeyCode::Char('<')));
        let Screen::Actions(state) = &app.screen else { panic!() };
        assert_eq!(state.results.as_ref().unwrap().selected_col, 0);

        // 14. Test global handle_key_sync with '0' when number_buffer is empty
        actions_keys(&mut app, KeyEvent::from(KeyCode::Char('>'))); // col 5
        assert!(app.number_buffer.is_empty());
        crate::tui::handler::handle_key_sync(&mut app, KeyEvent::from(KeyCode::Char('0')));
        let Screen::Actions(state) = &app.screen else { panic!() };
        assert_eq!(state.results.as_ref().unwrap().selected_col, 0, "'0' with empty buffer must jump to column 0");
        assert!(app.number_buffer.is_empty(), "number_buffer must remain empty when '0' is pressed alone");
    }

    #[test]
    fn actions_keys_s_and_shift_s_toggle_sorting_not_schema() {
        let mut app = sample_app(Screen::Actions(ActionState {
            catalog: "iceberg".to_string(),
            schema: "sales".to_string(),
            table: "orders".to_string(),
            selected: 0,
            results: Some(sample_leaf_results_state()),
            ..Default::default()
        }));
        app.set_active_panel(ActivePanel::MainViewer);

        // Press 'S': should toggle sort to Desc (NOT switch to Schema!)
        actions_keys(&mut app, KeyEvent::from(KeyCode::Char('S')));
        let Screen::Actions(state) = &app.screen else { panic!() };
        assert_eq!(state.selected, 0, "Selected menu action must remain TableView (0)");
        assert_eq!(
            state.results.as_ref().unwrap().sort,
            Some(("event_type".to_string(), crate::app::SortDirection::Desc))
        );

        // Press 'S' again: Asc
        actions_keys(&mut app, KeyEvent::from(KeyCode::Char('S')));
        let Screen::Actions(state) = &app.screen else { panic!() };
        assert_eq!(
            state.results.as_ref().unwrap().sort,
            Some(("event_type".to_string(), crate::app::SortDirection::Asc))
        );

        // Press 's' (lowercase): None
        actions_keys(&mut app, KeyEvent::from(KeyCode::Char('s')));
        let Screen::Actions(state) = &app.screen else { panic!() };
        assert_eq!(state.results.as_ref().unwrap().sort, None);
    }

    #[test]
    fn actions_keys_reworded_lowercase_hotkeys_trigger_actions() {
        let mut app = sample_app(Screen::Actions(ActionState {
            catalog: "iceberg".to_string(),
            schema: "sales".to_string(),
            table: "orders".to_string(),
            selected: 0,
            ..Default::default()
        }));

        // All menu actions have lowercase hotkeys
        for (key, _, _) in crate::app::ACTIONS {
            assert!(key.is_lowercase(), "Key '{key}' must be lowercase");
        }

        let schema_idx = crate::app::ACTIONS
            .iter()
            .position(|(_, _, a)| *a == crate::app::Action::Schema)
            .unwrap();
        let partitions_idx = crate::app::ACTIONS
            .iter()
            .position(|(_, _, a)| *a == crate::app::Action::Partitions)
            .unwrap();
        let stats_idx = crate::app::ACTIONS
            .iter()
            .position(|(_, _, a)| *a == crate::app::Action::ShowStats)
            .unwrap();
        let sample_idx = crate::app::ACTIONS
            .iter()
            .position(|(_, _, a)| *a == crate::app::Action::Sample)
            .unwrap();

        // 'e' triggers Schema
        actions_keys(&mut app, KeyEvent::from(KeyCode::Char('e')));
        let Screen::Actions(state) = &app.screen else { panic!() };
        assert_eq!(state.selected, schema_idx);

        // 'i' is no longer assigned to any action
        if let Screen::Actions(ref mut s) = app.screen {
            s.selected = 0;
        }
        actions_keys(&mut app, KeyEvent::from(KeyCode::Char('i')));
        let Screen::Actions(state) = &app.screen else { panic!() };
        assert_eq!(state.selected, 0, "'i' should not trigger any action");

        // 'p' triggers Partitions
        actions_keys(&mut app, KeyEvent::from(KeyCode::Char('p')));
        let Screen::Actions(state) = &app.screen else { panic!() };
        assert_eq!(state.selected, partitions_idx);

        // 't' triggers Show Stats
        actions_keys(&mut app, KeyEvent::from(KeyCode::Char('t')));
        let Screen::Actions(state) = &app.screen else { panic!() };
        assert_eq!(state.selected, stats_idx);

        // 'm' triggers Sample Mode
        actions_keys(&mut app, KeyEvent::from(KeyCode::Char('m')));
        let Screen::Actions(state) = &app.screen else { panic!() };
        assert_eq!(state.selected, sample_idx);
    }

    #[test]
    fn test_vim_visual_line_mode_selection_and_yank() {
        let mut res = sample_leaf_results_state();
        res.columns = vec!["col1".to_string(), "col2".to_string()];
        res.rows = vec![
            vec!["r0c0".to_string(), "r0c1".to_string()],
            vec!["r1c0".to_string(), "r1c1".to_string()],
            vec!["r2c0".to_string(), "r2c1".to_string()],
        ];
        let mut app = sample_app(Screen::Actions(ActionState {
            catalog: "iceberg".to_string(),
            schema: "sales".to_string(),
            table: "orders".to_string(),
            selected: 0,
            results: Some(res),
            ..Default::default()
        }));
        app.set_active_panel(ActivePanel::MainViewer);

        // 1. Shift+V enters visual line mode
        actions_keys(&mut app, KeyEvent::from(KeyCode::Char('V')));
        let Screen::Actions(state) = &app.screen else { panic!() };
        let res = state.results.as_ref().unwrap();
        assert_eq!(res.visual_mode, crate::app::VisualMode::Line { anchor_row: 0 });

        // 2. 'j' moves cursor down to row 1, expanding selection to rows 0..=1
        actions_keys(&mut app, KeyEvent::from(KeyCode::Char('j')));
        let Screen::Actions(state) = &app.screen else { panic!() };
        let res = state.results.as_ref().unwrap();
        assert_eq!(res.selected_row, 1);
        assert!(res.is_cell_selected(0, 0));
        assert!(res.is_cell_selected(0, 1));
        assert!(res.is_cell_selected(1, 0));
        assert!(res.is_cell_selected(1, 1));
        assert!(!res.is_cell_selected(2, 0));

        // 3. 'y' yanks the 2 rows as TSV and exits visual mode
        actions_keys(&mut app, KeyEvent::from(KeyCode::Char('y')));
        let Screen::Actions(state) = &app.screen else { panic!() };
        let res = state.results.as_ref().unwrap();
        assert_eq!(res.visual_mode, crate::app::VisualMode::None);
        let toast = app.copied_toast.as_ref().unwrap().0.clone();
        assert!(toast.contains("2 selected row(s) (CSV)"));
    }

    #[test]
    fn test_vim_visual_block_mode_selection_and_swap_ends() {
        let mut res = sample_leaf_results_state();
        res.columns = vec!["col0".to_string(), "col1".to_string(), "col2".to_string()];
        res.rows = vec![
            vec!["r0c0".to_string(), "r0c1".to_string(), "r0c2".to_string()],
            vec!["r1c0".to_string(), "r1c1".to_string(), "r1c2".to_string()],
            vec!["r2c0".to_string(), "r2c1".to_string(), "r2c2".to_string()],
        ];
        let mut app = sample_app(Screen::Actions(ActionState {
            catalog: "iceberg".to_string(),
            schema: "sales".to_string(),
            table: "orders".to_string(),
            selected: 0,
            results: Some(res),
            ..Default::default()
        }));
        app.set_active_panel(ActivePanel::MainViewer);

        // 1. Ctrl+v enters visual block mode
        let ctrl_v = KeyEvent::new(KeyCode::Char('v'), crossterm::event::KeyModifiers::CONTROL);
        actions_keys(&mut app, ctrl_v);
        let Screen::Actions(state) = &app.screen else { panic!() };
        let res = state.results.as_ref().unwrap();
        assert_eq!(
            res.visual_mode,
            crate::app::VisualMode::Block {
                anchor_row: 0,
                anchor_col: 0
            }
        );

        // 2. 'j' moves down and 'l' moves right to (1, 1)
        actions_keys(&mut app, KeyEvent::from(KeyCode::Char('j')));
        actions_keys(&mut app, KeyEvent::from(KeyCode::Char('l')));
        let Screen::Actions(state) = &app.screen else { panic!() };
        let res = state.results.as_ref().unwrap();
        assert_eq!(res.selected_row, 1);
        assert_eq!(res.selected_col, 1);
        assert!(res.is_cell_selected(0, 0));
        assert!(res.is_cell_selected(0, 1));
        assert!(res.is_cell_selected(1, 0));
        assert!(res.is_cell_selected(1, 1));
        assert!(!res.is_cell_selected(0, 2));

        // 3. 'o' swaps anchor and cursor
        actions_keys(&mut app, KeyEvent::from(KeyCode::Char('o')));
        let Screen::Actions(state) = &app.screen else { panic!() };
        let res = state.results.as_ref().unwrap();
        assert_eq!(res.selected_row, 0);
        assert_eq!(res.selected_col, 0);
        assert_eq!(
            res.visual_mode,
            crate::app::VisualMode::Block {
                anchor_row: 1,
                anchor_col: 1
            }
        );

        // 4. Esc exits visual mode but stays in MainViewer
        actions_keys(&mut app, KeyEvent::from(KeyCode::Esc));
        let Screen::Actions(state) = &app.screen else { panic!() };
        let res = state.results.as_ref().unwrap();
        assert_eq!(res.visual_mode, crate::app::VisualMode::None);
        assert_eq!(app.active_panel, ActivePanel::MainViewer);

        // 5. Esc again focuses MenuPane
        actions_keys(&mut app, KeyEvent::from(KeyCode::Esc));
        assert_eq!(app.active_panel, ActivePanel::MenuPane);
    }

    #[test]
    fn test_vim_yank_whole_row_with_uppercase_y() {
        let mut res = sample_leaf_results_state();
        res.columns = vec!["col0".to_string(), "col1".to_string()];
        res.rows = vec![
            vec!["r0c0".to_string(), "r0c1".to_string()],
            vec!["r1c0".to_string(), "r1c1".to_string()],
        ];
        res.selected_row = 1;
        let mut app = sample_app(Screen::Actions(ActionState {
            catalog: "iceberg".to_string(),
            schema: "sales".to_string(),
            table: "orders".to_string(),
            selected: 0,
            results: Some(res),
            ..Default::default()
        }));
        app.set_active_panel(ActivePanel::MainViewer);

        // 'Y' yanks the whole visible row 1
        actions_keys(&mut app, KeyEvent::from(KeyCode::Char('Y')));
        let toast = app.copied_toast.as_ref().unwrap().0.clone();
        assert!(toast.contains("1 visible row(s) (CSV)"));
    }

    #[test]
    fn test_shift_arrow_keys_enter_and_extend_visual_modes() {
        let mut res = sample_leaf_results_state();
        res.columns = vec!["c0".to_string(), "c1".to_string(), "c2".to_string()];
        res.rows = vec![
            vec!["0".to_string(), "1".to_string(), "2".to_string()],
            vec!["3".to_string(), "4".to_string(), "5".to_string()],
        ];
        let mut app = sample_app(Screen::Actions(ActionState {
            catalog: "iceberg".to_string(),
            schema: "sales".to_string(),
            table: "orders".to_string(),
            selected: 0,
            results: Some(res),
            ..Default::default()
        }));
        app.set_active_panel(ActivePanel::MainViewer);

        // Shift+Down enters visual line mode
        let shift_down = KeyEvent::new(KeyCode::Down, crossterm::event::KeyModifiers::SHIFT);
        actions_keys(&mut app, shift_down);
        let Screen::Actions(state) = &app.screen else { panic!() };
        let res = state.results.as_ref().unwrap();
        assert_eq!(res.visual_mode, crate::app::VisualMode::Line { anchor_row: 0 });
        assert_eq!(res.selected_row, 1);

        // Esc exits visual mode
        actions_keys(&mut app, KeyEvent::from(KeyCode::Esc));

        // Shift+Right enters visual block mode
        let shift_right = KeyEvent::new(KeyCode::Right, crossterm::event::KeyModifiers::SHIFT);
        actions_keys(&mut app, shift_right);
        let Screen::Actions(state) = &app.screen else { panic!() };
        let res = state.results.as_ref().unwrap();
        assert_eq!(
            res.visual_mode,
            crate::app::VisualMode::Block {
                anchor_row: 1,
                anchor_col: 0
            }
        );
    }
}

