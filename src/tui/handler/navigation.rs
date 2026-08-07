use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::app::*;

use super::{Command, export};

pub(super) fn check_trigger_infinite_scroll(app: &mut App) -> Option<Command> {
    if let Screen::Actions(ref mut action_state) = app.screen
        && let Some(state) = action_state.results.as_mut()
        && state.is_paginated
        && !state.is_fetching_next_page
        && state.has_more_rows
        && state.scroll_v + 15 >= state.rows.len()
    {
        state.is_fetching_next_page = true;
        let offset = state.rows.len();
        return Some(Command::FetchNextPage {
            catalog: state.catalog.clone(),
            schema: state.schema.clone(),
            table: state.table.clone(),
            offset,
            limit: state.page_size,
            filters: state.filters.clone(),
        });
    }
    None
}

pub(super) fn filter_items<'a>(items: &'a [String], search: &str) -> Vec<&'a String> {
    items
        .iter()
        .filter(|name| search.is_empty() || name.to_lowercase().contains(&search.to_lowercase()))
        .collect()
}

pub(super) fn extract_list_labels(app: &App) -> Option<Vec<String>> {
    match &app.screen {
        Screen::Catalog(s) => Some(
            filter_items(&s.items, &app.search_query)
                .into_iter()
                .map(|x| x.trim().to_string())
                .collect(),
        ),
        Screen::Schema(s) => Some(
            filter_items(&s.items, &app.search_query)
                .into_iter()
                .map(|x| x.trim().to_string())
                .collect(),
        ),
        Screen::Table(s) => Some(
            filter_items(&s.items, &app.search_query)
                .into_iter()
                .map(|x| x.trim().to_string())
                .collect(),
        ),
        Screen::Actions(_) => Some(ACTIONS.iter().map(|(_, l, _)| l.to_string()).collect()),
        _ => None,
    }
}

pub(super) fn get_selected_item_label(app: &App) -> Option<String> {
    match &app.screen {
        Screen::Catalog(s) => {
            let filtered = filter_items(&s.items, &app.search_query);
            if filtered.is_empty() {
                None
            } else {
                let idx = s.selected.min(filtered.len() - 1);
                Some(filtered[idx].trim().to_string())
            }
        }
        Screen::Schema(s) => {
            let filtered = filter_items(&s.items, &app.search_query);
            if filtered.is_empty() {
                None
            } else {
                let idx = s.selected.min(filtered.len() - 1);
                Some(filtered[idx].trim().to_string())
            }
        }
        Screen::Table(s) => {
            let filtered = filter_items(&s.items, &app.search_query);
            if filtered.is_empty() {
                None
            } else {
                let idx = s.selected.min(filtered.len() - 1);
                Some(filtered[idx].trim().to_string())
            }
        }
        _ => None,
    }
}

pub(super) fn reset_list_selected_for_search(app: &mut App) {
    if let Some(items) = extract_list_labels(app) {
        if !items.is_empty() {
            if let Some(s) = get_selected(&app.screen)
                && s >= items.len()
            {
                mod_list_selected(&mut app.screen, items.len() - 1);
            }
        } else {
            mod_list_selected(&mut app.screen, 0);
        }
    }
}

pub(super) fn mod_list_selected(screen: &mut Screen, new_selected: usize) {
    match screen {
        Screen::Catalog(s) => s.selected = new_selected,
        Screen::Schema(s) => s.selected = new_selected,
        Screen::Table(s) => s.selected = new_selected,
        Screen::Actions(s) => s.selected = new_selected,
        _ => {}
    }
}

pub(super) fn get_selected(screen: &Screen) -> Option<usize> {
    match screen {
        Screen::Catalog(s) => Some(s.selected),
        Screen::Schema(s) => Some(s.selected),
        Screen::Table(s) => Some(s.selected),
        Screen::Actions(s) => Some(s.selected),
        _ => None,
    }
}

pub(super) fn update_number_buffer(app: &mut App, ch: char) {
    if ch.is_ascii_digit() {
        let mut buf = app.number_buffer.clone();
        buf.push(ch);
        let num: usize = buf.parse().unwrap_or(0);
        if let Some(items) = extract_list_labels(app) {
            if num <= items.len() && num > 0 {
                app.number_buffer = buf;
            } else {
                app.number_buffer = ch.to_string();
            }
        }
    }
}

pub(super) fn jump_to_number(app: &mut App) {
    if app.number_buffer.is_empty() {
        return;
    }
    let num: usize = app.number_buffer.parse().unwrap_or(1);
    if let Some(items) = extract_list_labels(app)
        && num > 0
        && num <= items.len()
    {
        mod_list_selected(&mut app.screen, num - 1);
    }
    app.number_buffer.clear();
}

pub(super) fn go_back(app: &mut App) {
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

/// macOS terminals can emit Option+hjkl/g as special glyphs when Option is not configured as Meta.
/// Remap those characters back to plain vim-style motion keys so Alt/Option navigation still works.
pub(super) fn normalize_key_code(code: KeyCode) -> KeyCode {
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

/// Builds the `Table DDL` results view straight from the cached recon
/// `ddl_text` (`SHOW CREATE TABLE`, fetched once on table entry) — never
/// re-queries Trino.
pub(in crate::tui::handler) fn populate_table_ddl_results(s: &mut ActionState) {
    let Some(meta) = s.metadata.as_ref() else {
        return;
    };
    let ddl_text = meta.ddl_text.clone();
    s.results = Some(ResultsState {
        query_buffer: String::new(),
        query_cursor: 0,
        columns: vec!["Create Table".to_string()],
        rows: vec![vec![ddl_text]],
        scroll_v: 0,
        scroll_h: 0,
        loading: false,
        error: None,
        is_paginated: false,
        catalog: s.catalog.clone(),
        schema: s.schema.clone(),
        table: s.table.clone(),
        offset: 0,
        page_size: 100,
        is_fetching_next_page: false,
        has_more_rows: false,
        invalid_query_error: None,
        selection_anchor: None,
        filters: Vec::new(),
    });
}

pub fn trigger_action(app: &mut App, action_idx: usize) -> Option<Command> {
    if action_idx >= ACTIONS.len() {
        return None;
    }
    if let Screen::Actions(ref mut s) = app.screen {
        let action = &ACTIONS[action_idx].2;

        // `Partitions` and `Schema` depend on the slower `$partitions` /
        // `information_schema.columns` recon queries. `Table View` and
        // `Table DDL` only need the DDL/partition-layout half of recon
        // (`SHOW CREATE TABLE`), which resolves first — block each action
        // only on the specific recon phase it actually needs, rather than
        // making DDL/Table View wait on the slower two queries too. The
        // action is still selected (and the menu switches to it) so the
        // main viewer shows a loading spinner instead of doing nothing;
        // once the relevant recon result lands, the pending view is
        // auto-populated (see `AsyncResult::FetchTableDdl`/
        // `FetchTableMetadata` handlers).
        if s.metadata_loading && matches!(action, Action::Partitions | Action::Schema) {
            s.selected = action_idx;
            if app.active_panel != ActivePanel::MainViewer {
                app.mouse_selection_anchor = None;
                app.mouse_selection_current = None;
            }
            app.active_panel = ActivePanel::MainViewer;
            return None;
        }
        if s.ddl_loading && matches!(action, Action::TableView | Action::TableDDL) {
            s.selected = action_idx;
            if app.active_panel != ActivePanel::MainViewer {
                app.mouse_selection_anchor = None;
                app.mouse_selection_current = None;
            }
            app.active_panel = ActivePanel::MainViewer;
            return None;
        }

        s.selected = action_idx;
        if app.active_panel != ActivePanel::MainViewer {
            app.mouse_selection_anchor = None;
            app.mouse_selection_current = None;
        }
        app.active_panel = ActivePanel::MainViewer;
        match action {
            Action::Partitions | Action::Schema => {
                // Recon already populated `app.partition_tree_lines` /
                // `app.vertical_schema_cols` on table entry — nothing to
                // fetch, just switch to displaying them.
                None
            }
            Action::TableView => {
                // Reentrancy guard: if Table View is already active for this
                // table (a drill-down in progress/already resolved, or the
                // leaf record view already fetched/loading), re-selecting the
                // same action must not blow away the existing state and fire
                // a brand-new duplicate query. This is what caused the same
                // `SELECT DISTINCT ...` query to appear twice in the log —
                // the same logical Table View trigger firing back-to-back
                // (e.g. terminal key-repeat delivering two Press events, or
                // Enter/`v` being processed on consecutive polls) each
                // started a fresh drill-down from scratch.
                if s.drilldown.is_some() || s.results.is_some() {
                    return None;
                }
                let partition_cols = s
                    .metadata
                    .as_ref()
                    .map(|m| m.partitioned_by.clone())
                    .unwrap_or_default();
                if partition_cols.is_empty() {
                    // Unpartitioned table: unchanged direct full query.
                    let query = action.build_query(&s.catalog, &s.schema, &s.table);
                    s.results = None;
                    s.drilldown = None;
                    Some(Command::ExecuteQuery {
                        query,
                        is_paginated: true,
                        catalog: s.catalog.clone(),
                        schema: s.schema.clone(),
                        table: s.table.clone(),
                        filters: Vec::new(),
                    })
                } else {
                    // Partitioned table: never fire an unfiltered `SELECT *`
                    // (that's what produces "Failed to generate splits").
                    // Start a cd/ls-style drill-down at the first partition
                    // column instead.
                    let first_col = partition_cols[0].clone();
                    s.results = None;
                    s.drilldown = Some(DrillDownState {
                        partition_cols,
                        path: Vec::new(),
                        levels_cache: Vec::new(),
                        selected: 0,
                        loading: true,
                        error: None,
                        truncated: false,
                    });
                    Some(Command::FetchPartitionLevel {
                        catalog: s.catalog.clone(),
                        schema: s.schema.clone(),
                        table: s.table.clone(),
                        filters: Vec::new(),
                        column: first_col,
                    })
                }
            }
            Action::TableDDL => {
                // Recon already fetched `SHOW CREATE TABLE` on table entry —
                // reuse the cached DDL text instead of re-querying Trino.
                if s.metadata.is_some() {
                    populate_table_ddl_results(s);
                    None
                } else {
                    // Recon hasn't populated metadata (e.g. it errored) —
                    // fall back to firing the query directly.
                    let query = action.build_query(&s.catalog, &s.schema, &s.table);
                    s.results = None;
                    Some(Command::ExecuteQuery {
                        query,
                        is_paginated: false,
                        catalog: s.catalog.clone(),
                        schema: s.schema.clone(),
                        table: s.table.clone(),
                        filters: Vec::new(),
                    })
                }
            }
            _ => {
                let query = action.build_query(&s.catalog, &s.schema, &s.table);
                s.results = None;
                Some(Command::ExecuteQuery {
                    query,
                    is_paginated: false,
                    catalog: s.catalog.clone(),
                    schema: s.schema.clone(),
                    table: s.table.clone(),
                    filters: Vec::new(),
                })
            }
        }
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ConnectionConfig;
    use crate::trino::client::TrinoClient;

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

        // Selected index 0 in filtered list ["events", "enableTenants"] should yield "events"
        // and kick off table-entry recon (SHOW CREATE TABLE + info schema).
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

        // Action index 1 is Table DDL ('c').
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
            "moving the menu selection should reset the main view pane"
        );
    }

    #[test]
    fn actions_keys_less_than_greater_than_scroll_columns_in_generic_results() {
        let mut app = sample_app(Screen::Actions(ActionState {
            catalog: "iceberg".to_string(),
            schema: "sales".to_string(),
            table: "orders".to_string(),
            selected: 3, // ShowStats: a generic (non-drilldown) results view
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
            selected: 3, // ShowStats: a generic (non-drilldown) results view
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
        // `h` is not `at_leaf` here (not in a Table View drill-down), so it
        // must not go up a partition level or scroll — just a no-op.
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

        // `c` must not trigger a copy toast anymore in MainViewer.
        actions_keys(&mut app, KeyEvent::from(KeyCode::Char('c')));
        assert!(app.copied_toast.is_none(), "'c' should not copy anymore");

        // `y` still does.
        actions_keys(&mut app, KeyEvent::from(KeyCode::Char('y')));
        assert!(app.copied_toast.is_some(), "'y' should still copy");
    }

    #[test]
    fn actions_keys_leaf_grid_h_goes_up_partition_l_is_no_op_and_lt_gt_scroll() {
        let mut app = sample_app(Screen::Actions(ActionState {
            selected: 0, // TableView
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

        // `l` at the leaf is a no-op (nothing to drill into further).
        actions_keys(&mut app, KeyEvent::from(KeyCode::Char('l')));
        let Screen::Actions(state) = &app.screen else {
            panic!("expected actions screen");
        };
        assert!(state.drilldown.as_ref().unwrap().is_leaf());

        // `>` still scrolls columns at the leaf.
        actions_keys(&mut app, KeyEvent::from(KeyCode::Char('>')));
        let Screen::Actions(state) = &app.screen else {
            panic!("expected actions screen");
        };
        let _ = state.results.as_ref().unwrap().scroll_h; // no panic; single column clamps to 0

        // `h` at the leaf goes up a partition level (back to browsing, not a
        // scroll).
        actions_keys(&mut app, KeyEvent::from(KeyCode::Char('h')));
        let Screen::Actions(state) = &app.screen else {
            panic!("expected actions screen");
        };
        assert!(state.drilldown.as_ref().unwrap().path.is_empty());
        assert!(state.results.is_none());
    }
}

fn select_current_item(app: &mut App) -> Option<Command> {
    match &app.screen {
        Screen::Catalog(_) => {
            let catalog = get_selected_item_label(app)?;
            if app.schemas.contains_key(&catalog) {
                let items = app.schemas[&catalog]
                    .iter()
                    .map(|x| x.trim().to_string())
                    .collect();
                app.clear_mouse_selection();
                app.screen = Screen::Schema(SchemaState {
                    catalog,
                    items,
                    selected: 0,
                });
                None
            } else {
                Some(Command::FetchSchemas { catalog })
            }
        }
        Screen::Schema(s) => {
            let schema = get_selected_item_label(app)?;
            let catalog = s.catalog.trim().to_string();
            if app.tables.contains_key(&(catalog.clone(), schema.clone())) {
                let items = app.tables[&(catalog.clone(), schema.clone())]
                    .iter()
                    .map(|x| x.trim().to_string())
                    .collect();
                app.clear_mouse_selection();
                app.screen = Screen::Table(TableState {
                    catalog,
                    schema,
                    items,
                    selected: 0,
                });
                None
            } else {
                Some(Command::FetchTables { catalog, schema })
            }
        }
        Screen::Table(s) => {
            let table = get_selected_item_label(app)?;
            let catalog = s.catalog.clone();
            let schema = s.schema.clone();
            app.main_panel_pct = 15;
            app.set_active_panel(ActivePanel::MenuPane);
            app.partition_tree_lines.clear();
            app.vertical_schema_cols.clear();
            let default_query = ACTIONS[0].2.build_query(&catalog, &schema, &table);
            let query_len = default_query.len();
            app.clear_mouse_selection();
            app.screen = Screen::Actions(ActionState {
                catalog: catalog.clone(),
                schema: schema.clone(),
                table: table.clone(),
                selected: 0,
                query_buffer: default_query,
                query_cursor: query_len,
                results: None,
                ddl_loading: true,
                metadata_loading: true,
                ..Default::default()
            });
            // Fire the grouped table-entry recon (SHOW CREATE TABLE +
            // information_schema.columns) immediately, before any menu
            // action is selected, so Partitions/Schema/Table View have
            // their data ready (or already in flight) as soon as the user
            // reaches for them.
            Some(Command::FetchTableMetadata {
                catalog,
                schema,
                table,
            })
        }
        Screen::Actions(s) => {
            let idx = s.selected;
            trigger_action(app, idx)
        }
        _ => None,
    }
}

pub(super) fn connect_keys(app: &mut App, key: KeyEvent) -> Option<Command> {
    let state_loading = match &app.screen {
        Screen::Connect(s) => s.loading,
        _ => return None,
    };
    if state_loading {
        return None;
    }
    let state = match &mut app.screen {
        Screen::Connect(s) => s,
        _ => return None,
    };
    match key.code {
        KeyCode::Tab | KeyCode::Char('\t') => {
            state.focused = (state.focused + 1) % 3;
        }
        KeyCode::Backspace => match state.focused {
            0 => {
                state.url.pop();
            }
            1 => {
                state.user.pop();
            }
            2 => {
                state.password.pop();
            }
            _ => {}
        },
        KeyCode::Char(c) => match state.focused {
            0 => state.url.push(c),
            1 => state.user.push(c),
            2 => state.password.push(c),
            _ => {}
        },
        KeyCode::Enter if !state.url.is_empty() && !state.user.is_empty() => {
            let url = state.url.clone();
            let user = state.user.clone();
            let password = state.password.clone();
            state.loading = true;
            return Some(Command::Connect {
                url,
                user,
                password,
            });
        }
        _ => {}
    }
    None
}

pub(super) fn copy_active_pane_content(app: &mut App) {
    let mut text_to_copy = String::new();

    if let (Some(anchor), Some(current)) = (app.mouse_selection_anchor, app.mouse_selection_current)
    {
        text_to_copy = super::mouse::extract_selected_text(app, anchor, current);
    }

    if text_to_copy.is_empty() {
        match &app.screen {
            Screen::Catalog(s) => {
                text_to_copy = filter_items(&s.items, &app.search_query)
                    .iter()
                    .map(|x| x.trim())
                    .collect::<Vec<_>>()
                    .join("\n");
            }
            Screen::Schema(s) => {
                text_to_copy = filter_items(&s.items, &app.search_query)
                    .iter()
                    .map(|x| x.trim())
                    .collect::<Vec<_>>()
                    .join("\n");
            }
            Screen::Table(s) => {
                text_to_copy = filter_items(&s.items, &app.search_query)
                    .iter()
                    .map(|x| x.trim())
                    .collect::<Vec<_>>()
                    .join("\n");
            }
            Screen::Actions(s) => {
                if app.active_panel == ActivePanel::MenuPane {
                    text_to_copy = ACTIONS
                        .iter()
                        .map(|(_, l, _)| l.to_string())
                        .collect::<Vec<_>>()
                        .join("\n");
                } else if app.active_panel == ActivePanel::MainViewer {
                    if s.selected == 6 {
                        text_to_copy = app.partition_tree_lines.join("\n");
                    } else if s.selected == 7 {
                        text_to_copy = app
                            .vertical_schema_cols
                            .iter()
                            .map(|col| {
                                format!(
                                    "{}\t{}\t{}\t{}",
                                    col.name, col.data_type, col.key_meta, col.description
                                )
                            })
                            .collect::<Vec<_>>()
                            .join("\n");
                    } else {
                        export::copy_results_to_clipboard(app);
                        return;
                    }
                }
            }
            _ => {}
        }
    }

    if !text_to_copy.is_empty() {
        super::query::copy_to_clipboard(&text_to_copy);
        app.copied_toast = Some((toast_summary(&text_to_copy), std::time::Instant::now()));
    }
}

/// Builds a short, single-line human summary for the "Copied to clipboard"
/// toast. Multi-line/tab-separated copies (e.g. whole columns or schema
/// tables) must not be mashed together into an unreadable run of
/// characters — show the line/row count instead of raw truncated content
/// in that case, and only preview raw text for genuinely single-line copies.
fn toast_summary(text: &str) -> String {
    let line_count = text.lines().count();
    if line_count > 1 {
        format!("{line_count} lines")
    } else {
        text.chars().take(40).collect()
    }
}

pub(super) fn handle_list_navigation_keys(app: &mut App, key: KeyEvent) -> Option<Command> {
    let code = normalize_key_code(key.code);
    match code {
        KeyCode::Char('j') | KeyCode::Down => {
            if let Some(items) = extract_list_labels(app)
                && !items.is_empty()
                && let Some(s) = get_selected(&app.screen)
            {
                mod_list_selected(&mut app.screen, (s + 1).min(items.len() - 1));
            }
            None
        }
        KeyCode::Char('k') | KeyCode::Up => {
            if let Some(s) = get_selected(&app.screen) {
                mod_list_selected(&mut app.screen, s.saturating_sub(1));
            }
            None
        }
        KeyCode::Char('l') | KeyCode::Right | KeyCode::Enter => select_current_item(app),
        KeyCode::Char('h') | KeyCode::Left | KeyCode::Esc => {
            app.number_buffer.clear();
            go_back(app);
            None
        }
        KeyCode::Char('g') => {
            mod_list_selected(&mut app.screen, 0);
            None
        }
        KeyCode::Char('G') => {
            if let Some(items) = extract_list_labels(app)
                && !items.is_empty()
            {
                mod_list_selected(&mut app.screen, items.len() - 1);
            }
            None
        }
        KeyCode::Char('y') | KeyCode::Char('c') => {
            copy_active_pane_content(app);
            None
        }
        _ => None,
    }
}

pub(super) fn catalog_keys(app: &mut App, key: KeyEvent) -> Option<Command> {
    handle_list_navigation_keys(app, key)
}

pub(super) fn schema_keys(app: &mut App, key: KeyEvent) -> Option<Command> {
    handle_list_navigation_keys(app, key)
}

pub(super) fn table_keys(app: &mut App, key: KeyEvent) -> Option<Command> {
    handle_list_navigation_keys(app, key)
}

/// Advances the drill-down by fixing the currently highlighted value for
/// the level being browsed (the `cd` step). If more partition columns
/// remain, dispatches the next `SELECT DISTINCT` level fetch; once every
/// partition column has a fixed value, switches to the leaf record view by
/// dispatching a filtered, paginated query instead of ever firing an
/// unfiltered `SELECT *` against the (potentially huge/broken) partition
/// tree.
fn drilldown_drill_into_selected(s: &mut ActionState, safe_columns: &[String]) -> Option<Command> {
    let catalog = s.catalog.clone();
    let schema = s.schema.clone();
    let table = s.table.clone();
    let dd = s.drilldown.as_mut()?;
    if dd.loading || dd.is_leaf() {
        return None;
    }
    let depth = dd.depth();
    let value = dd.levels_cache.get(depth)?.get(dd.selected)?.clone();
    let column = dd.partition_cols.get(depth)?.clone();
    dd.path.push((column, value));
    let next_depth = dd.depth();

    if next_depth >= dd.partition_cols.len() {
        let filters = dd.path.clone();
        s.results = None;
        Some(Command::ExecuteQuery {
            query: crate::trino::queries::filtered_page_query(
                &catalog,
                &schema,
                &table,
                &filters,
                0,
                100,
                safe_columns,
            ),
            is_paginated: true,
            catalog,
            schema,
            table,
            filters,
        })
    } else {
        let next_column = dd.partition_cols[next_depth].clone();
        let filters = dd.path.clone();
        dd.loading = true;
        dd.selected = 0;
        Some(Command::FetchPartitionLevel {
            catalog,
            schema,
            table,
            filters,
            column: next_column,
        })
    }
}

/// Pops back up one partition level (the `cd ..` step): from the leaf
/// record view back to the last distinct-value list, or from one
/// distinct-value list back to its parent. Uses the cached level data —
/// no re-fetch needed. No-ops at the top level (empty path), per spec.
fn drilldown_go_up(s: &mut ActionState) {
    let had_leaf_results = s.results.is_some();
    match s.drilldown.as_mut() {
        Some(dd) if !dd.path.is_empty() => {
            dd.path.pop();
            dd.selected = 0;
        }
        _ => return,
    }
    if had_leaf_results {
        s.results = None;
    }
}

pub(super) fn actions_keys(app: &mut App, key: KeyEvent) -> Option<Command> {
    let code = normalize_key_code(key.code);

    if let KeyCode::Char(c) = code
        && let Some(pos) = ACTIONS.iter().position(|(k, _, _)| *k == c)
    {
        return trigger_action(app, pos);
    }

    let safe_columns = crate::app::safe_select_columns(&app.vertical_schema_cols);
    if let Screen::Actions(ref mut s) = app.screen {
        match app.active_panel {
            ActivePanel::MenuPane => match code {
                KeyCode::Char('j') | KeyCode::Down => {
                    s.selected = (s.selected + 1) % ACTIONS.len();
                    s.results = None;
                    None
                }
                KeyCode::Char('k') | KeyCode::Up => {
                    s.selected = if s.selected == 0 {
                        ACTIONS.len() - 1
                    } else {
                        s.selected - 1
                    };
                    s.results = None;
                    None
                }
                KeyCode::Enter | KeyCode::Char('l') | KeyCode::Right => {
                    let idx = s.selected;
                    trigger_action(app, idx)
                }
                KeyCode::Char('h') | KeyCode::Left | KeyCode::Esc => {
                    go_back(app);
                    None
                }
                KeyCode::Char('y') => {
                    copy_active_pane_content(app);
                    None
                }
                _ => None,
            },
            ActivePanel::MainViewer => {
                let in_drilldown = s.drilldown.is_some()
                    && s.selected < ACTIONS.len()
                    && matches!(ACTIONS[s.selected].2, Action::TableView);
                let at_leaf =
                    in_drilldown && s.drilldown.as_ref().map(|d| d.is_leaf()).unwrap_or(false);

                if in_drilldown && !at_leaf {
                    // Browsing a level of distinct partition values (the
                    // `ls` step of the cd/ls-style drill-down) — no
                    // `ResultsState` exists yet at this point.
                    match code {
                        KeyCode::Char('j') | KeyCode::Down => {
                            if let Some(dd) = s.drilldown.as_mut() {
                                let depth = dd.depth();
                                if let Some(items) = dd.levels_cache.get(depth)
                                    && !items.is_empty()
                                {
                                    dd.selected = (dd.selected + 1).min(items.len() - 1);
                                }
                            }
                            None
                        }
                        KeyCode::Char('k') | KeyCode::Up => {
                            if let Some(dd) = s.drilldown.as_mut() {
                                dd.selected = dd.selected.saturating_sub(1);
                            }
                            None
                        }
                        KeyCode::Char('g') => {
                            if let Some(dd) = s.drilldown.as_mut() {
                                dd.selected = 0;
                            }
                            None
                        }
                        KeyCode::Char('G') => {
                            if let Some(dd) = s.drilldown.as_mut() {
                                let depth = dd.depth();
                                if let Some(items) = dd.levels_cache.get(depth) {
                                    dd.selected = items.len().saturating_sub(1);
                                }
                            }
                            None
                        }
                        KeyCode::Enter | KeyCode::Char('l') | KeyCode::Right => {
                            drilldown_drill_into_selected(s, &safe_columns)
                        }
                        KeyCode::Char('h') | KeyCode::Left => {
                            drilldown_go_up(s);
                            None
                        }
                        KeyCode::Char('y') => {
                            copy_active_pane_content(app);
                            None
                        }
                        _ => None,
                    }
                } else {
                    match code {
                        KeyCode::Char('j') | KeyCode::Down => {
                            if s.selected == 6 {
                                let max_lines = app.partition_tree_lines.len().saturating_sub(1);
                                app.partition_scroll = (app.partition_scroll + 1).min(max_lines);
                            } else if s.selected == 7 {
                                let max_cols = app.vertical_schema_cols.len().saturating_sub(1);
                                app.schema_scroll = (app.schema_scroll + 1).min(max_cols);
                            } else if let Some(ref mut res) = s.results {
                                if !res.rows.is_empty() {
                                    res.scroll_v =
                                        (res.scroll_v + 1).min(res.rows.len().saturating_sub(1));
                                }
                                return check_trigger_infinite_scroll(app);
                            }
                            None
                        }
                        KeyCode::Char('k') | KeyCode::Up => {
                            if s.selected == 6 {
                                app.partition_scroll = app.partition_scroll.saturating_sub(1);
                            } else if s.selected == 7 {
                                app.schema_scroll = app.schema_scroll.saturating_sub(1);
                            } else if let Some(ref mut res) = s.results {
                                res.scroll_v = res.scroll_v.saturating_sub(1);
                            }
                            None
                        }
                        KeyCode::Char('l') | KeyCode::Right => {
                            // `l` is purely hierarchical now — no-op in
                            // result-viewing contexts (use `<`/`>` to
                            // scroll columns instead).
                            None
                        }
                        KeyCode::Char('<') => {
                            if let Some(ref mut res) = s.results {
                                res.scroll_h = res.scroll_h.saturating_sub(1);
                            }
                            None
                        }
                        KeyCode::Char('>') => {
                            if let Some(ref mut res) = s.results
                                && !res.columns.is_empty()
                            {
                                res.scroll_h =
                                    (res.scroll_h + 1).min(res.columns.len().saturating_sub(1));
                            }
                            None
                        }
                        KeyCode::Char('h') | KeyCode::Left => {
                            // In a partitioned Table View's leaf record grid, `h`
                            // goes back up one partition level. Elsewhere it's
                            // a no-op — horizontal scrolling lives on `<`/`>`.
                            if at_leaf {
                                drilldown_go_up(s);
                            }
                            None
                        }
                        KeyCode::Esc => {
                            app.set_active_panel(ActivePanel::MenuPane);
                            None
                        }
                        KeyCode::Char('g') => {
                            if s.selected == 6 {
                                app.partition_scroll = 0;
                            } else if s.selected == 7 {
                                app.schema_scroll = 0;
                            } else if let Some(ref mut res) = s.results {
                                res.scroll_v = 0;
                                res.scroll_h = 0;
                            }
                            None
                        }
                        KeyCode::Char('G') => {
                            if s.selected == 6 {
                                app.partition_scroll =
                                    app.partition_tree_lines.len().saturating_sub(1);
                            } else if s.selected == 7 {
                                app.schema_scroll =
                                    app.vertical_schema_cols.len().saturating_sub(1);
                            } else if let Some(ref mut res) = s.results {
                                res.scroll_v = res.rows.len().saturating_sub(1);
                                return check_trigger_infinite_scroll(app);
                            }
                            None
                        }
                        KeyCode::Char('q') | KeyCode::Char(':') => {
                            if s.selected < ACTIONS.len()
                                && matches!(ACTIONS[s.selected].2, Action::TableView)
                            {
                                app.mode = Mode::QueryInput;
                            }
                            None
                        }
                        KeyCode::Char('y') => {
                            copy_active_pane_content(app);
                            None
                        }
                        KeyCode::Char('Y')
                            if s.selected < ACTIONS.len()
                                && !matches!(
                                    ACTIONS[s.selected].2,
                                    Action::Partitions | Action::Schema
                                ) =>
                        {
                            export::export_results_to_csv_file(app);
                            None
                        }
                        _ => None,
                    }
                }
            }
        }
    } else {
        None
    }
}
