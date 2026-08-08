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
    app.main_panel_pct = 60;

    let next = match &app.screen {
        Screen::Help => app.prev_screen.take().map(|p| *p),
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
    use crate::app::{ActionState, DrillDownState, Mode, ResultsState, TableRecon};
    use crate::config::ConnectionConfig;
    use crate::trino::client::TrinoClient;
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

        for idx in [7, 8] {
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
        });
        s.results = Some(sample_leaf_results_state());

        drilldown_go_up(&mut s);

        let dd = s.drilldown.as_ref().unwrap();
        assert!(dd.path.is_empty());
        assert_eq!(dd.selected, 0);
        assert!(s.results.is_none());
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
            scroll_v: 0,
            scroll_h: 0,
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
    }

    #[test]
    fn actions_keys_less_than_greater_than_scroll_columns_in_generic_results() {
        let mut app = sample_app(Screen::Actions(ActionState {
            catalog: "iceberg".to_string(),
            schema: "sales".to_string(),
            table: "orders".to_string(),
            selected: 3,
            results: Some(sample_leaf_results_state()),
            ..Default::default()
        }));
        app.set_active_panel(ActivePanel::MainViewer);

        actions_keys(&mut app, KeyEvent::from(KeyCode::Char('>')));
        let Screen::Actions(state) = &app.screen else {
            panic!("expected actions screen");
        };
        assert_eq!(
            state.results.as_ref().unwrap().scroll_h,
            0,
            "single column can't scroll past 0"
        );

        actions_keys(&mut app, KeyEvent::from(KeyCode::Char('<')));
        let Screen::Actions(state) = &app.screen else {
            panic!("expected actions screen");
        };
        assert_eq!(state.results.as_ref().unwrap().scroll_h, 0);
    }

    #[test]
    fn actions_keys_h_and_l_are_no_ops_in_generic_results() {
        let mut app = sample_app(Screen::Actions(ActionState {
            catalog: "iceberg".to_string(),
            schema: "sales".to_string(),
            table: "orders".to_string(),
            selected: 3,
            results: Some(sample_leaf_results_state()),
            ..Default::default()
        }));
        app.set_active_panel(ActivePanel::MainViewer);

        let cmd = actions_keys(&mut app, KeyEvent::from(KeyCode::Char('l')));
        assert!(cmd.is_none());
        let Screen::Actions(state) = &app.screen else {
            panic!("expected actions screen");
        };
        assert_eq!(state.results.as_ref().unwrap().scroll_h, 0);
        assert!(matches!(app.active_panel, ActivePanel::MainViewer));

        let cmd = actions_keys(&mut app, KeyEvent::from(KeyCode::Char('h')));
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
    fn actions_keys_leaf_grid_h_goes_up_partition_l_is_no_op_and_lt_gt_scroll() {
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
            }),
            results: Some(sample_leaf_results_state()),
            ..drilldown_action_state(vec!["date"])
        }));
        app.set_active_panel(ActivePanel::MainViewer);

        actions_keys(&mut app, KeyEvent::from(KeyCode::Char('l')));
        let Screen::Actions(state) = &app.screen else {
            panic!("expected actions screen");
        };
        assert!(state.drilldown.as_ref().unwrap().is_leaf());

        actions_keys(&mut app, KeyEvent::from(KeyCode::Char('>')));
        let Screen::Actions(state) = &app.screen else {
            panic!("expected actions screen");
        };
        let _ = state.results.as_ref().unwrap().scroll_h;

        actions_keys(&mut app, KeyEvent::from(KeyCode::Char('h')));
        let Screen::Actions(state) = &app.screen else {
            panic!("expected actions screen");
        };
        assert!(state.drilldown.as_ref().unwrap().path.is_empty());
        assert!(state.results.is_none());
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
                scroll_v: 0,
                scroll_h: 0,
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
}
