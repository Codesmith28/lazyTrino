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

use tracing::{error, warn};

use crate::app::*;
use crate::config_file;
use crate::trino::client::TrinoClient;
use crate::trino::queries;

use super::{AsyncResult, Command};

/// Cap on the number of distinct values fetched per drill-down level, to
/// protect against extremely high-cardinality partition columns.
const DRILLDOWN_LEVEL_LIMIT: usize = 200;

pub fn dispatch_command(
    app: &mut App,
    cmd: Command,
    tx: &tokio::sync::mpsc::UnboundedSender<AsyncResult>,
) {
    match cmd {
        Command::Connect {
            url,
            user,
            password,
        } => {
            let sql = queries::show_catalogs();
            let log_id = app.add_query_log(sql);
            app.loading = true;
            if let Screen::Connect(s) = &mut app.screen {
                s.loading = true;
                s.error = None;
            }
            let tx = tx.clone();
            tokio::spawn(async move {
                let res = match TrinoClient::new(&url, &user) {
                    Ok(client) => match client.fetch_catalogs().await {
                        Ok(catalogs) => Ok((client, catalogs)),
                        Err(err) => Err(err),
                    },
                    Err(err) => Err(err),
                };
                let _ = tx.send(AsyncResult::Connect {
                    log_id,
                    url,
                    user,
                    password,
                    result: res,
                });
            });
        }
        Command::FetchSchemas { catalog } => {
            let sql = queries::show_schemas(&catalog);
            let log_id = app.add_query_log(sql);
            app.loading = true;
            if let Some(client) = app.trino_client.clone() {
                let tx = tx.clone();
                tokio::spawn(async move {
                    let res = client.fetch_schemas(&catalog).await;
                    let _ = tx.send(AsyncResult::FetchSchemas {
                        log_id,
                        catalog,
                        result: res,
                    });
                });
            }
        }
        Command::FetchTables { catalog, schema } => {
            let sql = queries::show_tables(&catalog, &schema);
            let log_id = app.add_query_log(sql);
            app.loading = true;
            if let Some(client) = app.trino_client.clone() {
                let tx = tx.clone();
                tokio::spawn(async move {
                    let res = client.fetch_tables(&catalog, &schema).await;
                    let _ = tx.send(AsyncResult::FetchTables {
                        log_id,
                        catalog,
                        schema,
                        result: res,
                    });
                });
            }
        }
        Command::FetchTableMetadata {
            catalog,
            schema,
            table,
        } => {
            if let Some(client) = app.trino_client.clone() {
                let show_create_query = queries::show_create(&catalog, &schema, &table);
                let show_create_log_id = app.add_query_log(show_create_query.clone());
                let desc_query = queries::info_schema_columns(&catalog, &schema, &table);
                let cols_log_id = app.add_query_log(desc_query.clone());
                app.loading = true;

                let tx = tx.clone();
                tokio::spawn(async move {
                    // The table's partition layout is always derived directly
                    // from the table's DDL (`SHOW CREATE TABLE`).
                    let (partitioned_by, location, ddl_text, show_create_error) =
                        match client.execute(&show_create_query).await {
                            Ok(res) => {
                                let ddl = res
                                    .data
                                    .first()
                                    .and_then(|r| r.first())
                                    .cloned()
                                    .unwrap_or_default();
                                let (cols, loc) =
                                    crate::tui::screens::partition_tree::parse_partitioned_by(&ddl);
                                (cols, loc, ddl, None)
                            }
                            Err(err) => (Vec::new(), String::new(), String::new(), Some(err)),
                        };

                    let _ = tx.send(AsyncResult::FetchTableDdl {
                        show_create_log_id,
                        partitioned_by: partitioned_by.clone(),
                        location: location.clone(),
                        ddl_text: ddl_text.clone(),
                        show_create_error,
                    });

                    let partition_lines = crate::tui::screens::partition_tree::parse_show_create_to_tree_lines(
                        &ddl_text,
                        Some(&location),
                    );

                    let (columns, columns_error) = match client.execute(&desc_query).await {
                        Ok(res) => {
                            let columns = res
                                .data
                                .iter()
                                .enumerate()
                                .map(|(idx, r)| {
                                    let name = r.first().cloned().unwrap_or_default();
                                    let dtype = r.get(1).cloned().unwrap_or_default();
                                    let is_nullable = r.get(2).cloned().unwrap_or_default();
                                    let comment = r.get(3).cloned().unwrap_or_default();
                                    let key_meta = if name.starts_with("_hoodie") {
                                        "Hudi Metadata".to_string()
                                    } else if name.starts_with("$") || name.contains("iceberg") {
                                        "Iceberg Meta".to_string()
                                    } else if crate::trino::queries::is_partition_column(
                                        &partitioned_by,
                                        &name,
                                    ) {
                                        "Partition Key".to_string()
                                    } else if is_nullable == "NO" {
                                        "PK".to_string()
                                    } else {
                                        String::new()
                                    };
                                    VerticalColumn {
                                        index: idx + 1,
                                        name,
                                        data_type: dtype,
                                        key_meta,
                                        description: comment,
                                    }
                                })
                                .collect();
                            (columns, None)
                        }
                        Err(err) => (Vec::new(), Some(err)),
                    };

                    let _ = tx.send(AsyncResult::FetchTableMetadata {
                        partitions_log_id: show_create_log_id,
                        cols_log_id,
                        partition_lines,
                        columns,
                        partitions_error: None,
                        columns_error,
                    });
                });
            }
        }
        Command::ExecuteQuery {
            query,
            is_paginated,
            catalog,
            schema,
            table,
            filters,
        } => {
            let log_id = app.add_query_log(query.clone());
            app.loading = true;
            // The query bar must always mirror exactly what's being
            // executed — whether the user typed it manually or a menu
            // action (Count, Partitions, Table View drill-down, etc.)
            // built it programmatically via `build_query`/
            // `filtered_page_query`. Previously this reused whatever text
            // happened to already be in the bar (e.g. the very first
            // default query from table entry), so selecting a different
            // action or drilling into a partition value fired a new query
            // without the bar ever updating to show it.
            let query_buffer = query.clone();
            let query_cursor = query_buffer.len();

            let res_state = ResultsState {
                query_buffer: query_buffer.clone(),
                query_cursor,
                columns: Vec::new(),
                rows: vec![vec!["Loading...".to_string()]],
                scroll_v: 0,
                scroll_h: 0,
                loading: true,
                error: None,
                is_paginated,
                catalog: catalog.clone(),
                schema: schema.clone(),
                table: table.clone(),
                offset: 0,
                page_size: 100,
                is_fetching_next_page: false,
                has_more_rows: true,
                invalid_query_error: None,
                selection_anchor: None,
                filters: filters.clone(),
            };

            if let Screen::Actions(ref mut a) = app.screen {
                a.results = Some(res_state);
            } else {
                app.prev_screen = Some(Box::new(app.screen.clone()));
                app.clear_mouse_selection();
                app.screen = Screen::Actions(ActionState {
                    catalog: catalog.clone(),
                    schema: schema.clone(),
                    table: table.clone(),
                    selected: 0,
                    query_buffer: query_buffer.clone(),
                    query_cursor,
                    results: Some(res_state),
                    ..Default::default()
                });
            }

            if let Some(client) = app.trino_client.clone() {
                let tx = tx.clone();
                tokio::spawn(async move {
                    let res = client.execute(&query).await;
                    let _ = tx.send(AsyncResult::ExecuteQuery {
                        log_id,
                        query_buffer,
                        query_cursor,
                        catalog,
                        schema,
                        table,
                        is_paginated,
                        filters,
                        result: res,
                    });
                });
            }
        }
        Command::FetchNextPage {
            catalog,
            schema,
            table,
            offset,
            limit,
            filters,
        } => {
            let safe_columns = crate::app::safe_select_columns(&app.vertical_schema_cols);
            let query = queries::filtered_page_query(
                &catalog,
                &schema,
                &table,
                &filters,
                offset,
                limit,
                &safe_columns,
            );
            let log_id = app.add_query_log(query.clone());
            if let Screen::Actions(ref mut action_state) = app.screen
                && let Some(state) = action_state.results.as_mut()
            {
                state.is_fetching_next_page = true;
            }

            if let Some(client) = app.trino_client.clone() {
                let tx = tx.clone();
                tokio::spawn(async move {
                    let res = client.execute(&query).await;
                    let _ = tx.send(AsyncResult::FetchNextPage {
                        log_id,
                        offset,
                        limit,
                        result: res,
                    });
                });
            }
        }
        Command::FetchPartitionLevel {
            catalog,
            schema,
            table,
            filters,
            column,
        } => {
            let query = queries::distinct_partition_values(
                &catalog,
                &schema,
                &table,
                &filters,
                &column,
                DRILLDOWN_LEVEL_LIMIT,
            );
            let log_id = app.add_query_log(query.clone());
            if let Screen::Actions(ref mut action_state) = app.screen
                && let Some(drilldown) = action_state.drilldown.as_mut()
            {
                drilldown.loading = true;
                drilldown.error = None;
            }

            if let Some(client) = app.trino_client.clone() {
                let tx = tx.clone();
                tokio::spawn(async move {
                    let res = client.execute(&query).await;
                    let _ = tx.send(AsyncResult::FetchPartitionLevel {
                        log_id,
                        filters,
                        column,
                        result: res,
                    });
                });
            }
        }
    }
}

pub fn handle_async_result(app: &mut App, result: AsyncResult) {
    match result {
        AsyncResult::Connect {
            log_id,
            url,
            user,
            password,
            result,
        } => {
            app.loading = false;
            match result {
                Ok((client, catalogs)) => {
                    app.complete_query_log_success(log_id, 15, catalogs.len());
                    app.config.url = url;
                    app.config.user = user;
                    app.config.password = password;
                    if let Err(error) =
                        config_file::save_last_used(&app.config.url, &app.config.user)
                    {
                        warn!(error = %error, "Failed to persist last used connection");
                    }
                    app.trino_client = Some(client);
                    app.catalogs = catalogs.iter().map(|c| c.trim().to_string()).collect();
                    app.clear_mouse_selection();
                    app.screen = Screen::Catalog(CatalogState {
                        items: app.catalogs.clone(),
                        selected: 0,
                    });
                }
                Err(e) => {
                    error!(error = %e, "Connect failed");
                    let err = e.to_string();
                    app.complete_query_log_error(log_id, err.clone());
                    if let Screen::Connect(s) = &mut app.screen {
                        s.loading = false;
                        s.error = Some(format!("Connection failed: {err}"));
                    }
                }
            }
        }
        AsyncResult::FetchSchemas {
            log_id,
            catalog,
            result,
        } => {
            app.loading = false;
            match result {
                Ok(schemas) => {
                    let trimmed: Vec<String> =
                        schemas.iter().map(|s| s.trim().to_string()).collect();
                    app.complete_query_log_success(log_id, 25, trimmed.len());
                    app.schemas.insert(catalog.clone(), trimmed.clone());
                    app.clear_mouse_selection();
                    app.screen = Screen::Schema(SchemaState {
                        catalog: catalog.clone(),
                        items: trimmed,
                        selected: 0,
                    });
                }
                Err(e) => {
                    error!(error = %e, "Fetch schemas failed");
                    app.complete_query_log_error(log_id, e.to_string());
                }
            }
        }
        AsyncResult::FetchTables {
            log_id,
            catalog,
            schema,
            result,
        } => {
            app.loading = false;
            match result {
                Ok(tables) => {
                    let trimmed: Vec<String> =
                        tables.iter().map(|t| t.trim().to_string()).collect();
                    app.complete_query_log_success(log_id, 35, trimmed.len());
                    app.tables
                        .insert((catalog.clone(), schema.clone()), trimmed.clone());
                    app.clear_mouse_selection();
                    app.screen = Screen::Table(TableState {
                        catalog: catalog.clone(),
                        schema: schema.clone(),
                        items: trimmed,
                        selected: 0,
                    });
                }
                Err(e) => {
                    error!(error = %e, "Fetch tables failed");
                    app.complete_query_log_error(log_id, e.to_string());
                }
            }
        }
        AsyncResult::FetchTableDdl {
            show_create_log_id,
            partitioned_by,
            location,
            ddl_text,
            show_create_error,
        } => {
            if let Some(err) = show_create_error {
                error!(error = %err, "Fetch table DDL (SHOW CREATE TABLE) failed");
                app.complete_query_log_error(show_create_log_id, err.to_string());
            } else {
                app.complete_query_log_success(show_create_log_id, 20, 1);
            }
            if let Screen::Actions(ref mut action_state) = app.screen {
                action_state.metadata = Some(TableRecon {
                    partitioned_by,
                    location,
                    ddl_text,
                });
                action_state.ddl_loading = false;
                // If the user already selected Table DDL while recon was
                // still in flight (blocked, see `trigger_action`), the menu
                // switched to it but nothing was ever populated since the
                // cached DDL wasn't ready yet. Now that it just landed,
                // render it immediately instead of leaving the pane stuck
                // on the loading spinner until the user re-presses `c`.
                if matches!(
                    ACTIONS.get(action_state.selected).map(|(_, _, a)| a),
                    Some(Action::TableDDL)
                ) && action_state.results.is_none()
                {
                    super::navigation::populate_table_ddl_results(action_state);
                }
            }
        }
        AsyncResult::FetchTableMetadata {
            partitions_log_id,
            cols_log_id,
            partition_lines,
            columns,
            partitions_error,
            columns_error,
        } => {
            app.loading = false;
            if let Some(err) = partitions_error {
                error!(error = %err, "Fetch table partitions failed");
                app.complete_query_log_error(partitions_log_id, err.to_string());
            } else {
                app.complete_query_log_success(partitions_log_id, 20, partition_lines.len());
            }
            if let Some(err) = columns_error {
                error!(error = %err, "Fetch table columns failed");
                app.complete_query_log_error(cols_log_id, err.to_string());
            } else {
                app.complete_query_log_success(cols_log_id, 20, columns.len());
            }
            app.partition_tree_lines = partition_lines;
            app.vertical_schema_cols = columns;
            if let Screen::Actions(ref mut action_state) = app.screen {
                action_state.metadata_loading = false;
            }
        }
        AsyncResult::ExecuteQuery {
            log_id,
            query_buffer,
            query_cursor,
            catalog,
            schema,
            table,
            is_paginated,
            filters,
            result,
        } => {
            app.loading = false;
            match result {
                Ok(results) => {
                    app.complete_query_log_success(log_id, results.duration_ms, results.data.len());
                    let cols: Vec<String> =
                        results.columns.iter().map(|c| c.name.clone()).collect();
                    let rows = results.data;
                    let has_more = if is_paginated {
                        rows.len() >= 100
                    } else {
                        false
                    };
                    let res_state = ResultsState {
                        query_buffer,
                        query_cursor,
                        columns: cols,
                        rows,
                        scroll_v: 0,
                        scroll_h: 0,
                        loading: false,
                        error: None,
                        is_paginated,
                        catalog: catalog.clone(),
                        schema: schema.clone(),
                        table: table.clone(),
                        offset: 0,
                        page_size: 100,
                        is_fetching_next_page: false,
                        has_more_rows: has_more,
                        invalid_query_error: None,
                        selection_anchor: None,
                        filters: filters.clone(),
                    };

                    let cur_selected = if let Screen::Actions(ref a) = app.screen {
                        a.selected
                    } else {
                        0
                    };
                    if let Screen::Actions(ref mut a) = app.screen {
                        a.results = Some(res_state);
                    } else {
                        let default_query = ACTIONS[0].2.build_query(&catalog, &schema, &table);
                        let query_len = default_query.len();
                        app.clear_mouse_selection();
                        app.screen = Screen::Actions(ActionState {
                            catalog,
                            schema,
                            table,
                            selected: cur_selected,
                            query_buffer: default_query,
                            query_cursor: query_len,
                            results: Some(res_state),
                            ..Default::default()
                        });
                    }
                }
                Err(e) => {
                    error!(error = %e, "Execute query failed");
                    let err = e.to_string();
                    app.complete_query_log_error(log_id, err.clone());
                    let res_state = ResultsState {
                        query_buffer,
                        query_cursor,
                        columns: Vec::new(),
                        rows: vec![vec![format!("Error: {err}")]],
                        scroll_v: 0,
                        scroll_h: 0,
                        loading: false,
                        error: Some(err),
                        is_paginated: false,
                        catalog: catalog.clone(),
                        schema: schema.clone(),
                        table: table.clone(),
                        offset: 0,
                        page_size: 100,
                        is_fetching_next_page: false,
                        has_more_rows: false,
                        invalid_query_error: None,
                        selection_anchor: None,
                        filters,
                    };

                    let cur_selected = if let Screen::Actions(ref a) = app.screen {
                        a.selected
                    } else {
                        0
                    };
                    if let Screen::Actions(ref mut a) = app.screen {
                        a.results = Some(res_state);
                    } else {
                        let default_query = ACTIONS[0].2.build_query(&catalog, &schema, &table);
                        let query_len = default_query.len();
                        app.clear_mouse_selection();
                        app.screen = Screen::Actions(ActionState {
                            catalog,
                            schema,
                            table,
                            selected: cur_selected,
                            query_buffer: default_query,
                            query_cursor: query_len,
                            results: Some(res_state),
                            ..Default::default()
                        });
                    }
                }
            }
        }
        AsyncResult::FetchNextPage {
            log_id,
            offset,
            limit,
            result,
        } => match result {
            Ok(results) => {
                app.complete_query_log_success(log_id, results.duration_ms, results.data.len());
                let new_rows = results.data;
                let fetched_count = new_rows.len();
                if let Screen::Actions(ref mut action_state) = app.screen
                    && let Some(state) = action_state.results.as_mut()
                {
                    state.rows.extend(new_rows);
                    state.offset = offset;
                    state.is_fetching_next_page = false;
                    if fetched_count < limit {
                        state.has_more_rows = false;
                    }
                }
            }
            Err(e) => {
                error!(error = %e, "Fetch next page failed");
                app.complete_query_log_error(log_id, e.to_string());
                if let Screen::Actions(ref mut action_state) = app.screen
                    && let Some(state) = action_state.results.as_mut()
                {
                    state.is_fetching_next_page = false;
                    state.has_more_rows = false;
                }
            }
        },
        AsyncResult::FetchPartitionLevel {
            log_id,
            filters,
            column,
            result,
        } => match result {
            Ok(results) => {
                app.complete_query_log_success(log_id, results.duration_ms, results.data.len());
                let truncated = results.data.len() >= DRILLDOWN_LEVEL_LIMIT;
                let values: Vec<String> = results
                    .data
                    .into_iter()
                    .filter_map(|mut r| {
                        if r.is_empty() {
                            None
                        } else {
                            Some(r.remove(0))
                        }
                    })
                    .collect();
                if let Screen::Actions(ref mut action_state) = app.screen
                    && let Some(drilldown) = action_state.drilldown.as_mut()
                    // Only apply if this result still matches where the user currently
                    // is in the drill-down (guards against stale in-flight responses
                    // after the user has already navigated elsewhere).
                    && drilldown.path == filters
                    && drilldown.next_column() == Some(column.as_str())
                {
                    let depth = drilldown.depth();
                    if depth < drilldown.levels_cache.len() {
                        drilldown.levels_cache[depth] = values;
                    } else {
                        drilldown.levels_cache.push(values);
                    }
                    drilldown.selected = 0;
                    drilldown.loading = false;
                    drilldown.truncated = truncated;
                    drilldown.error = None;
                }
            }
            Err(e) => {
                error!(error = %e, "Fetch partition level failed");
                let err = e.to_string();
                app.complete_query_log_error(log_id, err.clone());
                if let Screen::Actions(ref mut action_state) = app.screen
                    && let Some(drilldown) = action_state.drilldown.as_mut()
                    && drilldown.path == filters
                    && drilldown.next_column() == Some(column.as_str())
                {
                    drilldown.loading = false;
                    drilldown.error = Some(err);
                }
            }
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::ActionState;
    use crate::config::ConnectionConfig;

    fn sample_config() -> ConnectionConfig {
        ConnectionConfig {
            url: "http://localhost:8080".to_string(),
            user: "user".to_string(),
            password: String::new(),
        }
    }

    #[test]
    fn fetch_table_ddl_auto_populates_results_when_ddl_view_was_pending() {
        let mut app = App::new(sample_config(), false);
        // Simulate the user pressing Table DDL ('c', action idx 1) while
        // recon was still in flight: `trigger_action` selects the action
        // and switches focus but leaves `results` empty since `metadata`
        // isn't populated yet.
        app.clear_mouse_selection();
        app.screen = Screen::Actions(ActionState {
            catalog: "iceberg".to_string(),
            schema: "sales".to_string(),
            table: "orders".to_string(),
            selected: 1,
            ddl_loading: true,
            ..Default::default()
        });

        handle_async_result(
            &mut app,
            AsyncResult::FetchTableDdl {
                show_create_log_id: 1,
                partitioned_by: vec!["date".to_string()],
                location: "s3://bucket/orders".to_string(),
                ddl_text: "CREATE TABLE orders (...)".to_string(),
                show_create_error: None,
            },
        );

        let Screen::Actions(state) = &app.screen else {
            panic!("expected actions screen");
        };
        assert!(!state.ddl_loading);
        let results = state
            .results
            .as_ref()
            .expect("Table DDL results should be auto-populated once recon lands");
        assert_eq!(
            results.rows,
            vec![vec!["CREATE TABLE orders (...)".to_string()]]
        );
    }

    #[test]
    fn execute_query_bar_always_mirrors_the_actual_query_being_run() {
        // Regression test: the query bar must show exactly the query that
        // was fired — whether typed manually or built programmatically by
        // a menu action (Count/Partitions/Table View/etc.) — never stale
        // text left over from a previous query_buffer.
        let mut app = App::new(sample_config(), false);
        app.clear_mouse_selection();
        app.screen = Screen::Actions(ActionState {
            catalog: "iceberg".to_string(),
            schema: "sales".to_string(),
            table: "orders".to_string(),
            selected: 0,
            // Stale text that must NOT leak into the new query bar.
            query_buffer: "SELECT * FROM orders LIMIT 10".to_string(),
            query_cursor: 30,
            ..Default::default()
        });

        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
        let programmatic_query = "SELECT count(*) FROM iceberg.sales.orders".to_string();
        dispatch_command(
            &mut app,
            Command::ExecuteQuery {
                query: programmatic_query.clone(),
                is_paginated: false,
                catalog: "iceberg".to_string(),
                schema: "sales".to_string(),
                table: "orders".to_string(),
                filters: Vec::new(),
            },
            &tx,
        );

        let Screen::Actions(state) = &app.screen else {
            panic!("expected actions screen");
        };
        let results = state.results.as_ref().expect("results should be set");
        assert_eq!(results.query_buffer, programmatic_query);
        assert_eq!(results.query_cursor, programmatic_query.len());
    }
}
