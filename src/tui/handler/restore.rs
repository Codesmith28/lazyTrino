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

//! Rebuilds a saved view the way a user would have reached it, so every pane
//! before and after it can be navigated:
//!
//! 1. schemas of the catalog, then tables of the schema (fetched if not
//!    cached, without jumping to those screens), with each list's cursor
//!    put on the saved item — so Back / Esc land on the right rows;
//! 2. the table, opened fresh (metadata, DDL, columns);
//! 3. the saved partition path, checked against the table's *current*
//!    partition columns (only the matching prefix is kept);
//! 4. the query — regenerated for that location, or the saved custom SQL;
//! 5. the grid (filters, hidden columns, sort) through the same code path
//!    the filter / column modals use.
//!
//! Each step that needs Trino returns a command; its async result calls back
//! into this module. Anything that cannot be restored exactly is reported in
//! the final notification. A key press or click while the plan is running
//! cancels it.

use crate::app::{
    Action, App, CatalogState, DrillDownState, NoticeKind, RestorePlan, RestoreStage, SchemaState,
    Screen, TableState,
};
use crate::saved_views::SavedView;
use crate::trino::queries;
use crate::view_location::{QuerySource, ViewLocation};

use super::Command;

/// Starts restoring `view`. Returns the first command to run, if any.
pub fn begin(app: &mut App, view: SavedView) -> Option<Command> {
    if app.catalogs.is_empty() {
        app.set_notice(
            NoticeKind::Error,
            "Connect to a cluster before opening a saved view",
        );
        return None;
    }
    app.clear_all_selections();
    app.search_query.clear();
    app.restore = Some(RestorePlan {
        view,
        stage: RestoreStage::Schemas,
        notes: Vec::new(),
    });
    advance(app)
}

/// Cancels a running restore because the user took over.
pub fn cancel(app: &mut App) {
    if let Some(plan) = app.restore.take() {
        app.set_notice(
            NoticeKind::Error,
            format!("Stopped restoring \"{}\"", plan.view.name),
        );
    }
}

/// Walks catalog → schema → table using the cached lists, fetching the first
/// one that is missing. Once all three are known, opens the table.
fn advance(app: &mut App) -> Option<Command> {
    let loc = app.restore.as_ref()?.view.location.clone();

    let Some(cat_idx) = position(&app.catalogs, &loc.catalog) else {
        return fail(
            app,
            Fallback::Catalogs,
            format!("catalog \"{}\" no longer exists", loc.catalog),
        );
    };
    app.last_selected_catalog = cat_idx;

    let Some(schemas) = app.schemas.get(&loc.catalog) else {
        set_stage(app, RestoreStage::Schemas);
        return Some(Command::FetchSchemas {
            catalog: loc.catalog,
        });
    };
    let Some(schema_idx) = position(schemas, &loc.schema) else {
        return fail(
            app,
            Fallback::Schemas,
            format!("schema \"{}\" no longer exists", loc.schema),
        );
    };
    app.last_selected_schema
        .insert(loc.catalog.clone(), schema_idx);

    let list_key = (loc.catalog.clone(), loc.schema.clone());
    let Some(tables) = app.tables.get(&list_key) else {
        set_stage(app, RestoreStage::Tables);
        return Some(Command::FetchTables {
            catalog: loc.catalog,
            schema: loc.schema,
        });
    };
    let Some(table_idx) = position(tables, &loc.table) else {
        return fail(
            app,
            Fallback::Tables,
            format!("table \"{}\" no longer exists", loc.table),
        );
    };
    app.last_selected_table.insert(list_key, table_idx);

    let key = (loc.catalog.clone(), loc.schema.clone(), loc.table.clone());
    app.last_selected_action
        .insert(key.clone(), loc.action.index());
    app.cached_action_states.remove(&key);

    set_stage(app, RestoreStage::Metadata);
    app.clear_mouse_selection();
    app.main_panel_pct = 15;
    let cmd =
        super::navigation::open_table(app, loc.catalog, loc.schema, loc.table, loc.action.index());
    app.set_active_panel(crate::app::ActivePanel::MainViewer);
    Some(cmd)
}

/// A schema list (`schema == None`) or table list arrived. Returns
/// `Some(next command)` when it belonged to the running restore, in which
/// case the caller must not switch screens itself.
pub fn on_list_loaded(
    app: &mut App,
    catalog: &str,
    schema: Option<&str>,
) -> Option<Option<Command>> {
    let plan = app.restore.as_ref()?;
    let loc = &plan.view.location;
    let ours = match (plan.stage, schema) {
        (RestoreStage::Schemas, None) => loc.catalog == catalog,
        (RestoreStage::Tables, Some(s)) => loc.catalog == catalog && loc.schema == s,
        _ => false,
    };
    ours.then(|| advance(app))
}

/// A schema / table list failed to load. Returns true when the failure
/// ended the running restore.
pub fn on_list_failed(app: &mut App, catalog: &str, schema: Option<&str>, error: &str) -> bool {
    let Some(plan) = app.restore.as_ref() else {
        return false;
    };
    let loc = &plan.view.location;
    let (ours, fallback) = match (plan.stage, schema) {
        (RestoreStage::Schemas, None) => (loc.catalog == catalog, Fallback::Catalogs),
        (RestoreStage::Tables, Some(s)) => {
            (loc.catalog == catalog && loc.schema == s, Fallback::Schemas)
        }
        _ => (false, Fallback::Catalogs),
    };
    if ours {
        fail(app, fallback, format!("could not list it ({error})"));
    }
    ours
}

/// The table's DDL (and so its current partition layout) arrived. Returns
/// `Some(next command)` when the running restore took over from the
/// default "run the pending action" behaviour.
pub fn on_table_ddl(
    app: &mut App,
    catalog: &str,
    schema: &str,
    table: &str,
    partitioned_by: &[String],
) -> Option<Option<Command>> {
    let plan = app.restore.as_ref()?;
    let loc = plan.view.location.clone();
    if plan.stage != RestoreStage::Metadata
        || loc.catalog != catalog
        || loc.schema != schema
        || loc.table != table
    {
        return None;
    }
    let query = plan.view.query.clone();
    let Screen::Actions(a) = &mut app.screen else {
        app.restore = None;
        return None;
    };
    if a.catalog != catalog || a.schema != schema || a.table != table {
        app.restore = None;
        return None;
    }

    let custom = match &query {
        QuerySource::Custom { sql } => Some(sql.clone()),
        QuerySource::Generated => None,
    };

    match loc.action {
        Action::TableView => Some(restore_table_view(app, &loc, custom, partitioned_by)),
        Action::TableDDL => {
            a.pending_action = None;
            super::navigation::populate_table_ddl_results(a);
            finish(app);
            Some(None)
        }
        // These panes render from the column / partition metadata, which the
        // still-pending action picks up when it arrives.
        Action::Schema | Action::Partitions => {
            finish(app);
            Some(None)
        }
        Action::ShowStats | Action::Sample => {
            a.pending_action = None;
            let sql = custom
                .clone()
                .unwrap_or_else(|| loc.action.build_query(catalog, schema, table));
            a.custom_sql = custom;
            a.query_cursor = sql.len();
            a.query_buffer = sql.clone();
            set_stage(app, RestoreStage::Results);
            Some(Some(Command::ExecuteQuery {
                query: sql,
                is_paginated: false,
                catalog: catalog.to_string(),
                schema: schema.to_string(),
                table: table.to_string(),
                filters: Vec::new(),
            }))
        }
    }
}

fn restore_table_view(
    app: &mut App,
    loc: &ViewLocation,
    custom: Option<String>,
    partitioned_by: &[String],
) -> Option<Command> {
    let (catalog, schema, table) = (&loc.catalog, &loc.schema, &loc.table);
    let depth = loc.matching_partition_depth(partitioned_by);
    if depth < loc.partition_path.len() {
        let kept = &loc.partition_path[..depth];
        let note = if partitioned_by.is_empty() {
            "the table is no longer partitioned".to_string()
        } else if kept.is_empty() {
            "the partition layout changed, opened the top level".to_string()
        } else {
            format!(
                "the partition layout changed, restored up to {}",
                kept.iter()
                    .map(|s| format!("{}={}", s.column, s.value))
                    .collect::<Vec<_>>()
                    .join(" › ")
            )
        };
        note_issue(app, note);
    }

    // Use the table's current spelling of each matched column.
    let path: Vec<(String, String)> = loc.partition_path[..depth]
        .iter()
        .zip(partitioned_by)
        .map(|(step, col)| (col.clone(), step.value.clone()))
        .collect();
    let safe_columns = crate::app::safe_select_columns(&app.vertical_schema_cols);

    let Screen::Actions(a) = &mut app.screen else {
        return None;
    };
    a.pending_action = None;
    a.results = None;

    if partitioned_by.is_empty() {
        a.drilldown = None;
        let sql = custom
            .clone()
            .unwrap_or_else(|| Action::TableView.build_query(catalog, schema, table));
        a.custom_sql = custom;
        a.query_cursor = sql.len();
        a.query_buffer = sql.clone();
        set_stage(app, RestoreStage::Results);
        return Some(Command::ExecuteQuery {
            query: sql,
            is_paginated: true,
            catalog: catalog.clone(),
            schema: schema.clone(),
            table: table.clone(),
            filters: Vec::new(),
        });
    }

    let at_leaf = path.len() == partitioned_by.len();
    // Walking down from the top fetches the table total on the way; a
    // restore starts below it, so ask for it separately.
    let table_total = (!path.is_empty()).then(|| Command::FetchPartitionCount {
        catalog: catalog.clone(),
        schema: schema.clone(),
        table: table.clone(),
        filters: Vec::new(),
    });
    let with_total = |cmd: Command| match table_total {
        Some(total) => Command::Batch(vec![cmd, total]),
        None => cmd,
    };
    a.drilldown = Some(DrillDownState {
        partition_cols: partitioned_by.to_vec(),
        path: path.clone(),
        loading: !at_leaf,
        table_count_loading: true,
        current_count_loading: true,
        ..Default::default()
    });

    if at_leaf {
        let sql = custom.clone().unwrap_or_else(|| {
            queries::filtered_page_query(
                catalog,
                schema,
                table,
                &path,
                0,
                100,
                &safe_columns,
                &a.column_types,
            )
        });
        a.custom_sql = custom;
        a.query_cursor = sql.len();
        a.query_buffer = sql.clone();
        set_stage(app, RestoreStage::Results);
        return Some(with_total(Command::ExecuteQuery {
            query: sql,
            is_paginated: true,
            catalog: catalog.clone(),
            schema: schema.clone(),
            table: table.clone(),
            filters: path,
        }));
    }

    // Browsing a partition level: show that level's values. (A results grid
    // is only shown at the leaf, so custom SQL / grid state do not apply.)
    let column = partitioned_by[path.len()].clone();
    let sql = queries::distinct_partition_values(
        catalog,
        schema,
        table,
        &path,
        &column,
        queries::DRILLDOWN_LEVEL_LIMIT,
        &a.column_types,
    );
    a.custom_sql = None;
    a.query_cursor = sql.len();
    a.query_buffer = sql;
    let cmd = with_total(Command::FetchPartitionLevel {
        catalog: catalog.clone(),
        schema: schema.clone(),
        table: table.clone(),
        filters: path,
        column,
    });
    finish(app);
    Some(cmd)
}

/// A query for the restored table finished. Applies the saved grid state on
/// success and ends the restore either way.
pub fn on_query_result(
    app: &mut App,
    catalog: &str,
    schema: &str,
    table: &str,
    error: Option<&str>,
) -> Option<Command> {
    let plan = app.restore.as_ref()?;
    let loc = &plan.view.location;
    if plan.stage != RestoreStage::Results
        || loc.catalog != catalog
        || loc.schema != schema
        || loc.table != table
    {
        return None;
    }
    if let Some(err) = error {
        let name = plan.view.name.clone();
        app.restore = None;
        app.set_notice(
            NoticeKind::Error,
            format!("Opened \"{name}\", but its query failed: {err}"),
        );
        return None;
    }

    let grid = plan.view.grid.clone();
    let mut cmd = None;
    if !grid.is_empty()
        && let Screen::Actions(a) = &mut app.screen
        && let Some(res) = a.results.as_mut()
    {
        let mut missing = Vec::new();
        for (col, filter) in &grid.column_filters {
            if res.columns.contains(col) {
                res.column_filters
                    .insert(col.clone(), filter.to_column_filter());
            } else {
                missing.push(col.clone());
            }
        }
        for col in &grid.hidden_columns {
            if res.columns.contains(col) {
                res.hidden_columns.insert(col.clone());
            }
        }
        if let Some((col, dir)) = &grid.sort {
            if let Some(pos) = res.columns.iter().position(|c| c == col) {
                res.sort = Some((col.clone(), (*dir).into()));
                res.selected_col = pos;
            } else {
                missing.push(col.clone());
            }
        }
        if let Some(name) = res.columns.get(res.selected_col)
            && !res.is_column_visible(name)
        {
            res.selected_col = res.first_visible_col();
        }
        if !missing.is_empty() {
            note_issue(
                app,
                format!("column(s) no longer present: {}", missing.join(", ")),
            );
        }
        // Same path as the filter / column modals: filter the loaded rows,
        // rebuild the SQL and re-run it.
        cmd = super::filter_modal::reapply_in_memory_filters(app);
    }
    finish(app);
    cmd
}

#[derive(Clone, Copy)]
enum Fallback {
    Catalogs,
    Schemas,
    Tables,
}

/// Ends the restore at the deepest list that could be reached, with the
/// cursor on the parent of what was missing.
fn fail(app: &mut App, fallback: Fallback, reason: String) -> Option<Command> {
    let plan = app.restore.take()?;
    let loc = plan.view.location;
    app.clear_mouse_selection();
    app.main_panel_pct = 60;
    let catalog_screen = |app: &App| {
        Screen::Catalog(CatalogState {
            items: app.catalogs.clone(),
            selected: app
                .last_selected_catalog
                .min(app.catalogs.len().saturating_sub(1)),
        })
    };
    app.screen = match fallback {
        Fallback::Catalogs => catalog_screen(app),
        Fallback::Schemas => match app.schemas.get(&loc.catalog) {
            Some(items) => Screen::Schema(SchemaState {
                catalog: loc.catalog.clone(),
                items: items.clone(),
                selected: 0,
            }),
            None => catalog_screen(app),
        },
        Fallback::Tables => {
            let key = (loc.catalog.clone(), loc.schema.clone());
            match app.tables.get(&key) {
                Some(items) => Screen::Table(TableState {
                    catalog: loc.catalog.clone(),
                    schema: loc.schema.clone(),
                    items: items.clone(),
                    selected: 0,
                }),
                None => catalog_screen(app),
            }
        }
    };
    app.set_notice(
        NoticeKind::Error,
        format!("Couldn't open \"{}\": {reason}", plan.view.name),
    );
    None
}

fn finish(app: &mut App) {
    let Some(plan) = app.restore.take() else {
        return;
    };
    if plan.notes.is_empty() {
        app.set_notice(
            NoticeKind::Success,
            format!("Opened \"{}\"", plan.view.name),
        );
    } else {
        app.set_notice(
            NoticeKind::Error,
            format!("Opened \"{}\" — {}", plan.view.name, plan.notes.join("; ")),
        );
    }
}

fn note_issue(app: &mut App, note: String) {
    if let Some(plan) = app.restore.as_mut() {
        plan.notes.push(note);
    }
}

fn set_stage(app: &mut App, stage: RestoreStage) {
    if let Some(plan) = app.restore.as_mut() {
        plan.stage = stage;
    }
}

fn position(items: &[String], name: &str) -> Option<usize> {
    items.iter().position(|i| i.trim() == name.trim())
}

#[cfg(test)]
mod tests {
    //! Restores driven through the real async-result handlers, with the
    //! Trino responses simulated.
    use super::*;
    use crate::app::ResultsState;
    use crate::saved_views::{GridState, StoredColumnFilter, StoredSortDirection};
    use crate::trino::error::TrinoClientError;
    use crate::trino::types::{Column, QueryResults};
    use crate::tui::handler::navigation;
    use crate::tui::handler::{AsyncResult, handle_async_result};
    use crate::view_location::PartitionStep;

    const CAT: &str = "datalake";
    const SCH: &str = "tenant_1";
    const TBL: &str = "events";

    fn rows(cols: &[&str], data: &[&[&str]]) -> Result<QueryResults, TrinoClientError> {
        Ok(QueryResults {
            columns: cols
                .iter()
                .map(|c| Column {
                    name: c.to_string(),
                    type_name: String::new(),
                })
                .collect(),
            data: data
                .iter()
                .map(|r| r.iter().map(|v| v.to_string()).collect())
                .collect(),
            duration_ms: 1,
        })
    }

    fn connected_app() -> App {
        let mut app = App::new(crate::config::ConnectionConfig::default(), false);
        app.catalogs = vec!["system".into(), CAT.into()];
        app.screen = Screen::Catalog(CatalogState {
            items: app.catalogs.clone(),
            selected: 0,
        });
        app
    }

    /// Same app with the schema and table lists already cached.
    fn browsed_app() -> App {
        let mut app = connected_app();
        app.schemas
            .insert(CAT.into(), vec!["default".into(), SCH.into()]);
        app.tables
            .insert((CAT.into(), SCH.into()), vec!["users".into(), TBL.into()]);
        app
    }

    fn view(
        action: Action,
        path: &[(&str, &str)],
        query: QuerySource,
        grid: GridState,
    ) -> SavedView {
        SavedView {
            version: crate::saved_views::SAVED_VIEWS_VERSION,
            id: "id".into(),
            name: "my view".into(),
            saved_at: String::new(),
            connection: None,
            location: ViewLocation {
                catalog: CAT.into(),
                schema: SCH.into(),
                table: TBL.into(),
                action,
                partition_path: path
                    .iter()
                    .map(|(c, v)| PartitionStep {
                        column: c.to_string(),
                        value: v.to_string(),
                    })
                    .collect(),
            },
            query,
            grid,
        }
    }

    fn ddl(app: &mut App, partitioned_by: &[&str]) -> Option<Command> {
        handle_async_result(
            app,
            AsyncResult::FetchTableDdl {
                show_create_log_id: 0,
                catalog: CAT.into(),
                schema: SCH.into(),
                table: TBL.into(),
                partitioned_by: partitioned_by.iter().map(|s| s.to_string()).collect(),
                location: String::new(),
                ddl_text: "CREATE TABLE events (...)".into(),
                show_create_error: None,
            },
        )
    }

    fn query_result(
        app: &mut App,
        sql: &str,
        filters: Vec<(String, String)>,
        result: Result<QueryResults, TrinoClientError>,
    ) -> Option<Command> {
        handle_async_result(
            app,
            AsyncResult::ExecuteQuery {
                log_id: 0,
                query_buffer: sql.into(),
                query_cursor: sql.len(),
                catalog: CAT.into(),
                schema: SCH.into(),
                table: TBL.into(),
                is_paginated: true,
                filters,
                result,
            },
        )
    }

    fn level(
        app: &mut App,
        filters: Vec<(String, String)>,
        column: &str,
        values: &[&[&str]],
    ) -> Option<Command> {
        handle_async_result(
            app,
            AsyncResult::FetchPartitionLevel {
                log_id: 0,
                catalog: CAT.into(),
                schema: SCH.into(),
                table: TBL.into(),
                filters,
                column: column.into(),
                result: rows(&[column], values),
            },
        )
    }

    fn actions(app: &App) -> &crate::app::ActionState {
        match &app.screen {
            Screen::Actions(a) => a,
            _ => panic!("expected the table screen"),
        }
    }

    /// The main command of a restore step; a restore that starts below the
    /// top of the partition hierarchy also asks for the table total.
    fn main_cmd(cmd: Option<Command>) -> Option<Command> {
        match cmd {
            Some(Command::Batch(mut cmds)) => {
                assert!(matches!(
                    cmds.get(1),
                    Some(Command::FetchPartitionCount { filters, .. }) if filters.is_empty()
                ));
                Some(cmds.remove(0))
            }
            other => other,
        }
    }

    fn notice(app: &App) -> (NoticeKind, String) {
        let n = app.notice_toast.as_ref().expect("a notice");
        (n.kind, n.text.clone())
    }

    fn pairs(p: &[(&str, &str)]) -> Vec<(String, String)> {
        p.iter()
            .map(|(c, v)| (c.to_string(), v.to_string()))
            .collect()
    }

    #[test]
    fn partition_level_view_restores_with_every_pane_navigable() {
        // The reported case: saved while browsing `account_id` values under
        // date › service, restored from the catalog list with nothing cached.
        let mut app = connected_app();
        let path = [("date", "2026-10-13"), ("service", "onedrivebiz")];
        let cmd = begin(
            &mut app,
            view(
                Action::TableView,
                &path,
                QuerySource::Generated,
                GridState::default(),
            ),
        );
        assert!(matches!(cmd, Some(Command::FetchSchemas { ref catalog }) if catalog == CAT));

        let cmd = handle_async_result(
            &mut app,
            AsyncResult::FetchSchemas {
                log_id: 0,
                catalog: CAT.into(),
                result: Ok(vec!["default".into(), SCH.into()]),
            },
        );
        assert!(matches!(cmd, Some(Command::FetchTables { .. })));
        assert!(
            matches!(app.screen, Screen::Catalog(_)),
            "lists load in the background"
        );

        let cmd = handle_async_result(
            &mut app,
            AsyncResult::FetchTables {
                log_id: 0,
                catalog: CAT.into(),
                schema: SCH.into(),
                result: Ok(vec!["users".into(), TBL.into()]),
            },
        );
        assert!(matches!(cmd, Some(Command::FetchTableMetadata { ref table, .. }) if table == TBL));
        assert_eq!(actions(&app).selected, Action::TableView.index());

        let cmd = main_cmd(ddl(&mut app, &["date", "service", "account_id"]));
        match cmd {
            Some(Command::FetchPartitionLevel {
                filters, column, ..
            }) => {
                assert_eq!(filters, pairs(&path));
                assert_eq!(column, "account_id");
            }
            _ => panic!("expected the account_id level"),
        }
        assert!(app.restore.is_none());
        assert_eq!(notice(&app).0, NoticeKind::Success);
        level(&mut app, pairs(&path), "account_id", &[&["a1"], &["a2"]]);
        let dd = actions(&app).drilldown.as_ref().unwrap();
        assert_eq!(dd.levels_cache.get(2).map(Vec::len), Some(2));

        // Up one level: never loaded, so it is fetched and the cursor lands
        // on the value we came from.
        match navigation::actions_keys(
            &mut app,
            crossterm::event::KeyEvent::from(crossterm::event::KeyCode::Char('<')),
        ) {
            Some(Command::FetchPartitionLevel {
                filters, column, ..
            }) => {
                assert_eq!(filters, pairs(&path[..1]));
                assert_eq!(column, "service");
            }
            _ => panic!("expected the service level"),
        }
        level(
            &mut app,
            pairs(&path[..1]),
            "service",
            &[&["auth"], &["onedrivebiz"], &["smb3"]],
        );
        assert_eq!(actions(&app).drilldown.as_ref().unwrap().selected, 1);

        // And back out through the lists, each on the right row.
        navigation::go_back(&mut app);
        let Screen::Table(t) = &app.screen else {
            panic!("expected the table list")
        };
        assert_eq!((t.items.len(), t.selected), (2, 1));
        navigation::go_back(&mut app);
        let Screen::Schema(s) = &app.screen else {
            panic!("expected the schema list")
        };
        assert_eq!((s.items.len(), s.selected), (2, 1));
        navigation::go_back(&mut app);
        let Screen::Catalog(c) = &app.screen else {
            panic!("expected the catalog list")
        };
        assert_eq!(c.selected, 1);
    }

    #[test]
    fn leaf_view_reruns_the_generated_query_and_reapplies_the_grid() {
        let mut app = browsed_app();
        app.trino_client = Some(
            crate::trino::client::TrinoClient::new("https://trino.example", "analyst").unwrap(),
        );
        let grid = GridState {
            column_filters: vec![(
                "status".into(),
                StoredColumnFilter::Values(vec!["OK".into()]),
            )],
            hidden_columns: vec!["internal_id".into()],
            sort: Some(("id".into(), StoredSortDirection::Desc)),
        };
        let cmd = begin(
            &mut app,
            view(
                Action::TableView,
                &[("dt", "2026-01-01")],
                QuerySource::Generated,
                grid,
            ),
        );
        assert!(matches!(cmd, Some(Command::FetchTableMetadata { .. })));

        let path = pairs(&[("dt", "2026-01-01")]);
        let expected = queries::filtered_page_query(
            CAT,
            SCH,
            TBL,
            &path,
            0,
            100,
            &[],
            &crate::trino::queries::ColumnTypes::default(),
        );
        match main_cmd(ddl(&mut app, &["dt"])) {
            Some(Command::ExecuteQuery {
                query,
                filters,
                is_paginated,
                ..
            }) => {
                assert_eq!(query, expected);
                assert_eq!(filters, path);
                assert!(is_paginated);
            }
            _ => panic!("expected the leaf query"),
        }
        assert!(actions(&app).drilldown.as_ref().unwrap().is_leaf());

        let cmd = query_result(
            &mut app,
            &expected,
            path.clone(),
            rows(
                &["id", "status", "internal_id"],
                &[&["1", "OK", "x"], &["2", "BAD", "y"], &["3", "OK", "z"]],
            ),
        );
        // the filters are re-run through Trino like the filter modal does
        assert!(matches!(cmd, Some(Command::ExecuteQuery { .. })));
        let res: &ResultsState = actions(&app).results.as_ref().unwrap();
        assert!(res.column_filters.contains_key("status"));
        assert!(res.hidden_columns.contains("internal_id"));
        assert!(res.query_buffer.contains("status"), "{}", res.query_buffer);
        assert_eq!(res.rows.len(), 2);
        assert_eq!(res.rows[0][0], "3", "sorted by id, descending");
        assert!(app.restore.is_none());
        assert_eq!(notice(&app).0, NoticeKind::Success);
    }

    #[test]
    fn a_changed_partition_layout_keeps_the_matching_prefix_and_says_so() {
        let mut app = browsed_app();
        begin(
            &mut app,
            view(
                Action::TableView,
                &[("date", "2026-10-13"), ("service", "smb")],
                QuerySource::Generated,
                GridState::default(),
            ),
        );
        match main_cmd(ddl(&mut app, &["date", "region"])) {
            Some(Command::FetchPartitionLevel {
                filters, column, ..
            }) => {
                assert_eq!(filters, pairs(&[("date", "2026-10-13")]));
                assert_eq!(column, "region");
            }
            _ => panic!("expected the region level"),
        }
        let (kind, text) = notice(&app);
        assert_eq!(kind, NoticeKind::Error);
        assert!(text.contains("partition layout changed"), "{text}");
        assert!(text.contains("date=2026-10-13"), "{text}");
    }

    #[test]
    fn custom_sql_on_a_table_that_is_no_longer_partitioned() {
        let mut app = browsed_app();
        let sql = "SELECT id FROM \"datalake\".\"tenant_1\".\"events\" WHERE id > 3";
        begin(
            &mut app,
            view(
                Action::TableView,
                &[("dt", "x")],
                QuerySource::Custom { sql: sql.into() },
                GridState::default(),
            ),
        );
        match ddl(&mut app, &[]) {
            Some(Command::ExecuteQuery { query, filters, .. }) => {
                assert_eq!(query, sql);
                assert!(filters.is_empty());
            }
            _ => panic!("expected the custom query"),
        }
        assert_eq!(actions(&app).custom_sql.as_deref(), Some(sql));
        assert!(actions(&app).drilldown.is_none());
        query_result(&mut app, sql, Vec::new(), rows(&["id"], &[&["4"]]));
        let (kind, text) = notice(&app);
        assert_eq!(kind, NoticeKind::Error);
        assert!(text.contains("no longer partitioned"), "{text}");
    }

    #[test]
    fn other_actions_restore_through_their_own_paths() {
        let mut app = browsed_app();
        begin(
            &mut app,
            view(
                Action::Sample,
                &[],
                QuerySource::Generated,
                GridState::default(),
            ),
        );
        match ddl(&mut app, &["dt"]) {
            Some(Command::ExecuteQuery {
                query,
                is_paginated,
                ..
            }) => {
                assert_eq!(query, queries::sample(CAT, SCH, TBL));
                assert!(!is_paginated);
            }
            _ => panic!("expected the sample query"),
        }

        let mut app = browsed_app();
        begin(
            &mut app,
            view(
                Action::TableDDL,
                &[],
                QuerySource::Generated,
                GridState::default(),
            ),
        );
        assert!(ddl(&mut app, &[]).is_none());
        assert!(actions(&app).results.is_some(), "DDL shown from metadata");
        assert!(app.restore.is_none());
    }

    #[test]
    fn a_missing_table_stops_at_its_schema_list() {
        let mut app = browsed_app();
        app.tables
            .insert((CAT.into(), SCH.into()), vec!["users".into()]);
        let cmd = begin(
            &mut app,
            view(
                Action::TableView,
                &[],
                QuerySource::Generated,
                GridState::default(),
            ),
        );
        assert!(cmd.is_none());
        assert!(app.restore.is_none());
        let Screen::Table(t) = &app.screen else {
            panic!("expected the table list")
        };
        assert_eq!(t.schema, SCH);
        let (kind, text) = notice(&app);
        assert_eq!(kind, NoticeKind::Error);
        assert!(text.contains("no longer exists"), "{text}");
    }

    #[test]
    fn a_failed_query_ends_the_restore_with_an_error() {
        let mut app = browsed_app();
        begin(
            &mut app,
            view(
                Action::TableView,
                &[],
                QuerySource::Generated,
                GridState::default(),
            ),
        );
        let Some(Command::ExecuteQuery { query, .. }) = ddl(&mut app, &[]) else {
            panic!("expected the table query")
        };
        query_result(
            &mut app,
            &query,
            Vec::new(),
            Err(TrinoClientError::QueryError {
                message: "access denied".into(),
            }),
        );
        assert!(app.restore.is_none());
        let (kind, text) = notice(&app);
        assert_eq!(kind, NoticeKind::Error);
        assert!(text.contains("access denied"), "{text}");
    }

    #[test]
    fn any_key_cancels_a_running_restore() {
        let mut app = connected_app();
        begin(
            &mut app,
            view(
                Action::TableView,
                &[],
                QuerySource::Generated,
                GridState::default(),
            ),
        );
        assert!(app.restore.is_some());
        crate::tui::handler::handle_key_sync(
            &mut app,
            crossterm::event::KeyEvent::from(crossterm::event::KeyCode::Down),
        );
        assert!(app.restore.is_none());
        assert!(notice(&app).1.contains("Stopped restoring"));
    }

    #[test]
    fn numeric_looking_varchar_partition_values_are_quoted_after_restore() {
        let mut app = browsed_app();
        let path = [("date", "2026-10-13"), ("service", "onedrivebiz")];
        begin(
            &mut app,
            view(
                Action::TableView,
                &path,
                QuerySource::Generated,
                GridState::default(),
            ),
        );
        handle_async_result(
            &mut app,
            AsyncResult::FetchTableDdl {
                show_create_log_id: 0,
                catalog: CAT.into(),
                schema: SCH.into(),
                table: TBL.into(),
                partitioned_by: vec!["date".into(), "service".into(), "account_id".into()],
                location: String::new(),
                ddl_text: "CREATE TABLE events (\n   date varchar COMMENT '',\n   service varchar COMMENT '',\n   account_id varchar COMMENT ''\n)".into(),
                show_create_error: None,
            },
        );
        assert_eq!(
            actions(&app).column_types.get("account_id"),
            Some("varchar")
        );

        // the level result reports the listed column's type too
        handle_async_result(
            &mut app,
            AsyncResult::FetchPartitionLevel {
                log_id: 0,
                catalog: CAT.into(),
                schema: SCH.into(),
                table: TBL.into(),
                filters: pairs(&path),
                column: "account_id".into(),
                result: Ok(QueryResults {
                    columns: vec![Column {
                        name: "account_id".into(),
                        type_name: "varchar".into(),
                    }],
                    data: vec![vec!["66349".into()]],
                    duration_ms: 1,
                }),
            },
        );

        let Screen::Actions(a) = &mut app.screen else {
            unreachable!()
        };
        let Some(Command::ExecuteQuery { query, filters, .. }) =
            navigation::drilldown::drilldown_drill_into_selected(a, &[])
        else {
            panic!("expected the leaf query")
        };
        assert!(query.contains("account_id = '66349'"), "{query}");
        let count = queries::filtered_count(CAT, SCH, TBL, &filters, &a.column_types);
        assert!(count.ends_with("account_id = '66349'"), "{count}");
    }

    #[test]
    fn restoring_needs_a_connection() {
        let mut app = App::new(crate::config::ConnectionConfig::default(), false);
        let cmd = begin(
            &mut app,
            view(
                Action::TableView,
                &[],
                QuerySource::Generated,
                GridState::default(),
            ),
        );
        assert!(cmd.is_none());
        assert!(app.restore.is_none());
        assert_eq!(notice(&app).0, NoticeKind::Error);
    }
}
