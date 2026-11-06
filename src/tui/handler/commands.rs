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

pub use crate::trino::queries::DRILLDOWN_LEVEL_LIMIT;

/// Adds the query to the Executed Queries log, flagged when the client will
/// answer it from its cache instead of sending it to Trino.
fn log_query(app: &mut App, sql: String) -> usize {
    let cached = app
        .trino_client
        .as_ref()
        .is_some_and(|client| client.is_cached(&sql));
    app.add_query_log_with_cache(sql, cached)
}

pub fn dispatch_command(
    app: &mut App,
    cmd: Command,
    tx: &tokio::sync::mpsc::UnboundedSender<AsyncResult>,
) {
    match cmd {
        Command::Batch(cmds) => {
            for cmd in cmds {
                dispatch_command(app, cmd, tx);
            }
        }
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
            let cache_limit = app.config.query_cache_limit;
            let tx = tx.clone();
            tokio::spawn(async move {
                let res = match TrinoClient::with_cache_limit(&url, &user, cache_limit) {
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
        Command::FetchCatalogs => {
            // Metadata lookups never use the query cache, so they are logged
            // as plain queries.
            let log_id = app.add_query_log(queries::show_catalogs());
            app.loading = true;
            if let Some(client) = app.trino_client.clone() {
                let tx = tx.clone();
                tokio::spawn(async move {
                    let res = client.fetch_catalogs().await;
                    let _ = tx.send(AsyncResult::FetchCatalogs {
                        log_id,
                        result: res,
                    });
                });
            }
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
                        match client.execute_fresh(&show_create_query).await {
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
                        catalog: catalog.clone(),
                        schema: schema.clone(),
                        table: table.clone(),
                        partitioned_by: partitioned_by.clone(),
                        location: location.clone(),
                        ddl_text: ddl_text.clone(),
                        show_create_error,
                    });

                    let partition_lines =
                        crate::tui::screens::partition_tree::parse_show_create_to_tree_lines(
                            &ddl_text,
                            Some(&location),
                        );

                    let (columns, columns_error) = match client.execute_fresh(&desc_query).await {
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
                                        is_nullable,
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
                        catalog: catalog.clone(),
                        schema: schema.clone(),
                        table: table.clone(),
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
            fresh,
        } => {
            // A bare `SELECT *` over this table (Sample, unpartitioned Table
            // View, or the same text typed in the query bar) fails outright
            // if any map/row column's Parquet encoding disagrees with the
            // catalog. Read only the columns that are safe, as filtered
            // views already do; the query bar shows the rewritten SQL.
            let query = match &app.screen {
                Screen::Actions(a)
                    if a.catalog == catalog && a.schema == schema && a.table == table =>
                {
                    let safe = crate::app::safe_select_columns(&app.vertical_schema_cols);
                    if safe.len() < app.vertical_schema_cols.len() {
                        queries::star_to_columns(&query, &catalog, &schema, &table, &safe)
                            .unwrap_or(query)
                    } else {
                        query
                    }
                }
                _ => query,
            };
            let log_id = if fresh {
                app.add_query_log(query.clone())
            } else {
                log_query(app, query.clone())
            };
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

            let cached_table_count = if let Screen::Actions(ref a) = app.screen {
                a.drilldown.as_ref().and_then(|d| d.table_count)
            } else {
                None
            };
            let cached_total = if let Screen::Actions(ref a) = app.screen {
                a.drilldown
                    .as_ref()
                    .and_then(|d| d.counts_cache.get(&filters).copied())
            } else {
                None
            };

            // The grid is rebuilt for the new query, but the filter modal's
            // full value lists (slow to fetch) still apply to the same table
            // and partition, so keep them.
            let (kept_distinct, kept_full, kept_truncated) = match &app.screen {
                Screen::Actions(a)
                    if a.catalog == catalog && a.schema == schema && a.table == table =>
                {
                    a.results
                        .as_ref()
                        .map(|r| r.fetched_values_for(&filters))
                        .unwrap_or_default()
                }
                _ => Default::default(),
            };

            let res_state = ResultsState {
                col_distinct_values: kept_distinct,
                col_values_full: kept_full,
                col_values_truncated: kept_truncated,
                query_buffer: query_buffer.clone(),
                query_cursor,
                columns: Vec::new(),
                rows: vec![vec!["Loading...".to_string()]],
                scroll_v: std::cell::Cell::new(0),
                scroll_h: std::cell::Cell::new(0),
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
                total_count: cached_total,
                total_count_loading: is_paginated && cached_total.is_none(),
                table_count: cached_table_count,
                ..Default::default()
            };

            if let Screen::Actions(ref mut a) = app.screen {
                a.query_buffer = query_buffer.clone();
                a.query_cursor = query_cursor;
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
                let tx_page = tx.clone();
                let client_page = client.clone();
                let query_buf = query_buffer;
                let cat = catalog.clone();
                let sch = schema.clone();
                let tbl = table.clone();
                let filts = filters.clone();
                tokio::spawn(async move {
                    let res = if fresh {
                        client_page.execute_fresh(&query).await
                    } else {
                        client_page.execute(&query).await
                    };
                    let _ = tx_page.send(AsyncResult::ExecuteQuery {
                        log_id,
                        query_buffer: query_buf,
                        query_cursor,
                        catalog: cat,
                        schema: sch,
                        table: tbl,
                        is_paginated,
                        filters: filts,
                        result: res,
                    });
                });

                if is_paginated && cached_total.is_none() {
                    let count_query = queries::filtered_count(
                        &catalog,
                        &schema,
                        &table,
                        &filters,
                        &table_column_types(app, &catalog, &schema, &table),
                    );
                    let count_log_id = app.add_query_log(count_query.clone());
                    let tx_cnt = tx.clone();
                    let client_cnt = client;
                    let cat_cnt = catalog;
                    let sch_cnt = schema;
                    let tbl_cnt = table;
                    let filts_cnt = filters;
                    tokio::spawn(async move {
                        let res = client_cnt.execute_fresh(&count_query).await;
                        let _ = tx_cnt.send(AsyncResult::FetchPartitionCount {
                            log_id: count_log_id,
                            catalog: cat_cnt,
                            schema: sch_cnt,
                            table: tbl_cnt,
                            filters: filts_cnt,
                            result: res,
                        });
                    });
                }
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
            let query = next_page_query(app, &catalog, &schema, &table, offset, limit, &filters);
            let log_id = log_query(app, query.clone());
            if let Screen::Actions(ref mut action_state) = app.screen
                && let Some(state) = action_state.results.as_mut()
            {
                state.is_fetching_next_page = true;
                let new_limit = offset + limit;
                let updated = queries::update_query_limit(&state.query_buffer, new_limit);
                let prev_len = state.query_buffer.len();
                if state.query_cursor == prev_len {
                    state.query_cursor = updated.len();
                } else {
                    state.query_cursor = state.query_cursor.min(updated.len());
                }
                state.query_buffer = updated.clone();
                action_state.query_buffer = updated;
                action_state.query_cursor = state.query_cursor;
                if let Some(ref base) = state.base_query {
                    state.base_query = Some(queries::update_query_limit(base, new_limit));
                }
            }

            if let Some(client) = app.trino_client.clone() {
                let tx = tx.clone();
                let catalog = catalog.clone();
                let schema = schema.clone();
                let table = table.clone();
                tokio::spawn(async move {
                    let res = client.execute(&query).await;
                    let _ = tx.send(AsyncResult::FetchNextPage {
                        log_id,
                        catalog,
                        schema,
                        table,
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
            let types = table_column_types(app, &catalog, &schema, &table);
            let query = queries::distinct_partition_values(
                &catalog,
                &schema,
                &table,
                &filters,
                &column,
                DRILLDOWN_LEVEL_LIMIT,
                &types,
            );
            let log_id = app.add_query_log(query.clone());

            let need_count = if let Screen::Actions(ref action_state) = app.screen {
                action_state
                    .drilldown
                    .as_ref()
                    .map(|d| !d.counts_cache.contains_key(&filters))
                    .unwrap_or(true)
            } else {
                true
            };

            if let Screen::Actions(ref mut action_state) = app.screen
                && action_state.catalog == catalog
                && action_state.schema == schema
                && action_state.table == table
            {
                let query_len = query.len();
                action_state.query_buffer = query.clone();
                action_state.query_cursor = query_len;
                action_state.results = None;
                if let Some(drilldown) = action_state.drilldown.as_mut() {
                    drilldown.loading = true;
                    drilldown.error = None;
                    if need_count {
                        drilldown.current_count_loading = true;
                        if filters.is_empty() {
                            drilldown.table_count_loading = true;
                        }
                    }
                }
            }

            if let Some(client) = app.trino_client.clone() {
                // Task 1: Fetch distinct partition values for next level
                let tx1 = tx.clone();
                let client1 = client.clone();
                let cat1 = catalog.clone();
                let sch1 = schema.clone();
                let tbl1 = table.clone();
                let filts1 = filters.clone();
                let col1 = column.clone();
                tokio::spawn(async move {
                    let res = client1.execute_fresh(&query).await;
                    let _ = tx1.send(AsyncResult::FetchPartitionLevel {
                        log_id,
                        catalog: cat1,
                        schema: sch1,
                        table: tbl1,
                        filters: filts1,
                        column: col1,
                        result: res,
                    });
                });

                // Task 2: Fetch count for current level in parallel
                if need_count {
                    let count_query =
                        queries::filtered_count(&catalog, &schema, &table, &filters, &types);
                    let count_log_id = app.add_query_log(count_query.clone());
                    let tx2 = tx.clone();
                    let client2 = client;
                    let cat2 = catalog;
                    let sch2 = schema;
                    let tbl2 = table;
                    let filts2 = filters;
                    tokio::spawn(async move {
                        let res = client2.execute_fresh(&count_query).await;
                        let _ = tx2.send(AsyncResult::FetchPartitionCount {
                            log_id: count_log_id,
                            catalog: cat2,
                            schema: sch2,
                            table: tbl2,
                            filters: filts2,
                            result: res,
                        });
                    });
                }
            }
        }
        Command::FetchPartitionCount {
            catalog,
            schema,
            table,
            filters,
        } => {
            let count_query = queries::filtered_count(
                &catalog,
                &schema,
                &table,
                &filters,
                &table_column_types(app, &catalog, &schema, &table),
            );
            let count_log_id = app.add_query_log(count_query.clone());
            if let Some(client) = app.trino_client.clone() {
                let tx = tx.clone();
                tokio::spawn(async move {
                    let res = client.execute_fresh(&count_query).await;
                    let _ = tx.send(AsyncResult::FetchPartitionCount {
                        log_id: count_log_id,
                        catalog,
                        schema,
                        table,
                        filters,
                        result: res,
                    });
                });
            }
        }
        Command::FetchColumnValues {
            catalog,
            schema,
            table,
            filters,
            column,
        } => {
            let query = queries::distinct_column_values(
                &catalog,
                &schema,
                &table,
                &filters,
                &column,
                // One more than shown, to tell "exactly the limit" from "more".
                queries::COLUMN_VALUES_LIMIT + 1,
                &table_column_types(app, &catalog, &schema, &table),
            );
            let log_id = app.add_query_log(query.clone());
            if let Some(client) = app.trino_client.clone() {
                let tx = tx.clone();
                tokio::spawn(async move {
                    let res = client.execute_fresh(&query).await;
                    let _ = tx.send(AsyncResult::FetchColumnValues {
                        log_id,
                        catalog,
                        schema,
                        table,
                        column,
                        result: res,
                    });
                });
            }
        }
    }
}

/// The query for the page of rows starting at `offset` of the grid on screen.
///
/// It is the grid's own query moved to that page, so the rows line up with
/// the columns already shown and honour its column filters, hidden columns
/// and any custom SQL. Without a grid to copy, the table's plain page query.
fn next_page_query(
    app: &App,
    catalog: &str,
    schema: &str,
    table: &str,
    offset: usize,
    limit: usize,
    filters: &[(String, String)],
) -> String {
    if let Screen::Actions(a) = &app.screen
        && a.catalog == catalog
        && a.schema == schema
        && a.table == table
        && let Some(res) = a.results.as_ref()
        && !res.query_buffer.trim().is_empty()
    {
        return queries::query_page(&res.query_buffer, offset, limit);
    }
    let safe_columns = crate::app::safe_select_columns(&app.vertical_schema_cols);
    queries::filtered_page_query(
        catalog,
        schema,
        table,
        filters,
        offset,
        limit,
        &safe_columns,
        &table_column_types(app, catalog, schema, table),
    )
}

/// Known column types of the table on screen (empty for any other table).
fn table_column_types(app: &App, catalog: &str, schema: &str, table: &str) -> queries::ColumnTypes {
    match &app.screen {
        Screen::Actions(a) if a.catalog == catalog && a.schema == schema && a.table == table => {
            a.column_types.clone()
        }
        _ => queries::ColumnTypes::default(),
    }
}

pub fn handle_async_result(app: &mut App, result: AsyncResult) -> Option<Command> {
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
                    let selected = app
                        .last_selected_catalog
                        .min(app.catalogs.len().saturating_sub(1));
                    app.screen = Screen::Catalog(CatalogState {
                        items: app.catalogs.clone(),
                        selected,
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
        AsyncResult::FetchCatalogs { log_id, result } => {
            app.loading = false;
            match result {
                Ok(catalogs) => {
                    app.complete_query_log_success(log_id, 15, catalogs.len());
                    app.catalogs = catalogs.iter().map(|c| c.trim().to_string()).collect();
                    if let Screen::Catalog(s) = &mut app.screen {
                        s.items = app.catalogs.clone();
                        s.selected = s.selected.min(s.items.len().saturating_sub(1));
                    }
                }
                Err(e) => {
                    error!(error = %e, "Fetch catalogs failed");
                    app.complete_query_log_error(log_id, e.to_string());
                    app.set_notice(
                        crate::app::NoticeKind::Error,
                        format!("Could not refresh catalogs: {e}"),
                    );
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
                    if let Some(next) = super::restore::on_list_loaded(app, &catalog, None) {
                        return next;
                    }
                    app.clear_mouse_selection();
                    // Reloading the list already on screen keeps the cursor.
                    let saved_selected = match &app.screen {
                        Screen::Schema(s) if s.catalog == catalog => s.selected,
                        _ => app.last_selected_schema.get(&catalog).copied().unwrap_or(0),
                    };
                    let selected = saved_selected.min(trimmed.len().saturating_sub(1));
                    app.screen = Screen::Schema(SchemaState {
                        catalog: catalog.clone(),
                        items: trimmed,
                        selected,
                    });
                }
                Err(e) => {
                    error!(error = %e, "Fetch schemas failed");
                    app.complete_query_log_error(log_id, e.to_string());
                    super::restore::on_list_failed(app, &catalog, None, &e.to_string());
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
                    if let Some(next) = super::restore::on_list_loaded(app, &catalog, Some(&schema))
                    {
                        return next;
                    }
                    app.clear_mouse_selection();
                    let saved_selected = match &app.screen {
                        Screen::Table(s) if s.catalog == catalog && s.schema == schema => {
                            s.selected
                        }
                        _ => app
                            .last_selected_table
                            .get(&(catalog.clone(), schema.clone()))
                            .copied()
                            .unwrap_or(0),
                    };
                    let selected = saved_selected.min(trimmed.len().saturating_sub(1));
                    app.screen = Screen::Table(TableState {
                        catalog: catalog.clone(),
                        schema: schema.clone(),
                        items: trimmed,
                        selected,
                    });
                }
                Err(e) => {
                    error!(error = %e, "Fetch tables failed");
                    app.complete_query_log_error(log_id, e.to_string());
                    super::restore::on_list_failed(app, &catalog, Some(&schema), &e.to_string());
                }
            }
        }
        AsyncResult::FetchTableDdl {
            show_create_log_id,
            catalog,
            schema,
            table,
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
            if let Screen::Actions(ref mut action_state) = app.screen
                && action_state.catalog == catalog
                && action_state.schema == schema
                && action_state.table == table
            {
                for (col, ty) in
                    crate::tui::screens::partition_tree::parse_ddl_column_types(&ddl_text)
                {
                    action_state.column_types.insert(&col, &ty);
                }
                action_state.metadata = Some(TableRecon {
                    partitioned_by: partitioned_by.clone(),
                    location,
                    ddl_text,
                });
                action_state.ddl_loading = false;
            }
            if let Some(next) =
                super::restore::on_table_ddl(app, &catalog, &schema, &table, &partitioned_by)
            {
                return next;
            }
            if let Screen::Actions(ref mut action_state) = app.screen
                && action_state.catalog == catalog
                && action_state.schema == schema
                && action_state.table == table
            {
                let table_view_idx = Action::TableView.index();
                let should_run_table_view = action_state.pending_action == Some(table_view_idx)
                    || (action_state.selected == table_view_idx
                        && action_state.results.is_none()
                        && action_state.drilldown.is_none());

                if should_run_table_view {
                    action_state.pending_action = None;
                    if partitioned_by.is_empty() {
                        let query = Action::TableView.build_query(&catalog, &schema, &table);
                        let query_len = query.len();
                        action_state.query_buffer = query.clone();
                        action_state.query_cursor = query_len;
                        return Some(Command::ExecuteQuery {
                            fresh: false,
                            query,
                            is_paginated: true,
                            catalog,
                            schema,
                            table,
                            filters: Vec::new(),
                        });
                    } else {
                        let first_col = partitioned_by[0].clone();
                        let query = crate::trino::queries::distinct_partition_values(
                            &catalog,
                            &schema,
                            &table,
                            &[],
                            &first_col,
                            DRILLDOWN_LEVEL_LIMIT,
                            &action_state.column_types,
                        );
                        let query_len = query.len();
                        action_state.query_buffer = query;
                        action_state.query_cursor = query_len;
                        action_state.drilldown = Some(DrillDownState {
                            partition_cols: partitioned_by,
                            path: Vec::new(),
                            levels_cache: Vec::new(),
                            selected: 0,
                            loading: true,
                            error: None,
                            truncated: false,
                            table_count: None,
                            table_count_loading: true,
                            counts_cache: std::collections::HashMap::new(),
                            current_count_loading: true,
                            path_cursors: std::collections::HashMap::new(),
                            focus_value: None,
                        });
                        return Some(Command::FetchPartitionLevel {
                            catalog,
                            schema,
                            table,
                            filters: Vec::new(),
                            column: first_col,
                        });
                    }
                } else if matches!(
                    ACTIONS.get(action_state.selected).map(|(_, _, a)| a),
                    Some(Action::TableDDL)
                ) && action_state.results.is_none()
                {
                    action_state.pending_action = None;
                    super::navigation::populate_table_ddl_results(action_state);
                }
            }
        }
        AsyncResult::FetchTableMetadata {
            partitions_log_id,
            cols_log_id,
            catalog,
            schema,
            table,
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
            if let Screen::Actions(ref mut action_state) = app.screen
                && action_state.catalog == catalog
                && action_state.schema == schema
                && action_state.table == table
            {
                for col in &columns {
                    action_state
                        .column_types
                        .insert_if_absent(&col.name, &col.data_type);
                }
                app.partition_tree_lines = partition_lines;
                app.vertical_schema_cols = columns;
                action_state.metadata_loading = false;
                action_state.pending_action = None;

                // All Table View waits for the column list so its first
                // page can skip columns Trino can't read (see
                // `safe_select_columns`); it starts now that they're here.
                if action_state.selected == Action::AllTableView.index()
                    && action_state.results.is_none()
                {
                    let query = Action::AllTableView.build_query(&catalog, &schema, &table);
                    action_state.query_cursor = query.len();
                    action_state.query_buffer = query.clone();
                    return Some(Command::ExecuteQuery {
                        fresh: false,
                        query,
                        is_paginated: true,
                        catalog,
                        schema,
                        table,
                        filters: Vec::new(),
                    });
                }
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
            let target = (catalog.clone(), schema.clone(), table.clone());
            let mut applied_to_table = false;
            let mut query_error: Option<String> = None;
            match result {
                Ok(results) => {
                    app.complete_query_log_success(log_id, results.duration_ms, results.data.len());
                    let cols: Vec<String> =
                        results.columns.iter().map(|c| c.name.clone()).collect();
                    if let Screen::Actions(ref mut a) = app.screen
                        && a.catalog == catalog
                        && a.schema == schema
                        && a.table == table
                    {
                        for c in &results.columns {
                            a.column_types.insert_if_absent(&c.name, &c.type_name);
                        }
                    }
                    let rows = results.data;
                    let has_more = if is_paginated {
                        rows.len() >= 100
                    } else {
                        false
                    };
                    let existing_results = if let Screen::Actions(ref a) = app.screen {
                        a.results.as_ref()
                    } else {
                        None
                    };
                    let cached_total = existing_results.and_then(|r| r.total_count).or_else(|| {
                        if let Screen::Actions(ref a) = app.screen {
                            a.drilldown
                                .as_ref()
                                .and_then(|d| d.counts_cache.get(&filters).copied())
                        } else {
                            None
                        }
                    });
                    let cached_table = existing_results.and_then(|r| r.table_count).or_else(|| {
                        if let Screen::Actions(ref a) = app.screen {
                            a.drilldown.as_ref().and_then(|d| d.table_count)
                        } else {
                            None
                        }
                    });
                    let total_loading = existing_results
                        .map(|r| r.total_count_loading)
                        .unwrap_or(is_paginated && cached_total.is_none());

                    let prev_col = existing_results.map(|r| r.selected_col).unwrap_or(0);
                    let sel_col = prev_col.min(cols.len().saturating_sub(1));
                    let base_q = existing_results.and_then(|r| r.base_query.clone());
                    let col_filts = existing_results
                        .map(|r| r.column_filters.clone())
                        .unwrap_or_default();
                    // Full per-column value lists belong to the partition they
                    // were fetched for; a different one starts over.
                    let same_scope = existing_results.is_some_and(|r| r.filters == filters);
                    let mut distinct_vals = existing_results
                        .map(|r| r.col_distinct_values.clone())
                        .unwrap_or_default();
                    if let Some(r) = existing_results.filter(|_| !same_scope) {
                        distinct_vals.retain(|col, _| !r.col_values_full.contains(col));
                    }
                    let values_full = existing_results
                        .filter(|_| same_scope)
                        .map(|r| r.col_values_full.clone())
                        .unwrap_or_default();
                    let values_truncated = existing_results
                        .filter(|_| same_scope)
                        .map(|r| r.col_values_truncated.clone())
                        .unwrap_or_default();
                    let hidden_cols = existing_results
                        .filter(|r| !r.columns.is_empty())
                        .map(|r| r.hidden_columns.clone())
                        .unwrap_or_else(|| {
                            cols.iter()
                                .filter(|c| c.starts_with('_'))
                                .cloned()
                                .collect()
                        });
                    let unfilt = existing_results
                        .and_then(|r| r.unfiltered_rows.clone())
                        .or_else(|| {
                            if col_filts.is_empty() {
                                Some(rows.clone())
                            } else {
                                None
                            }
                        });

                    let mut res_state = ResultsState {
                        query_buffer,
                        query_cursor,
                        columns: cols,
                        rows,
                        scroll_v: std::cell::Cell::new(0),
                        scroll_h: std::cell::Cell::new(0),
                        selected_row: 0,
                        selected_col: sel_col,
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
                        total_count: cached_total,
                        total_count_loading: total_loading,
                        table_count: cached_table,
                        base_query: base_q,
                        unfiltered_rows: unfilt,
                        column_filters: col_filts,
                        col_distinct_values: distinct_vals,
                        col_values_full: values_full,
                        col_values_truncated: values_truncated,
                        hidden_columns: hidden_cols,
                        ..Default::default()
                    };

                    // Keep the grid's sort across re-runs of the same view
                    // (filters, hidden columns, custom SQL), like its filters.
                    if let Some((col, dir)) = existing_results.and_then(|r| r.sort.clone())
                        && res_state.columns.contains(&col)
                    {
                        res_state.sort = Some((col, dir));
                    }

                    if let Some(col_name) = res_state.columns.get(res_state.selected_col)
                        && !res_state.is_column_visible(col_name)
                    {
                        res_state.selected_col = res_state.first_visible_col();
                    }

                    let cur_selected = if let Screen::Actions(ref a) = app.screen {
                        a.selected
                    } else {
                        0
                    };
                    if let Screen::Actions(ref mut a) = app.screen {
                        if a.catalog == catalog && a.schema == schema && a.table == table {
                            let mut res = res_state;
                            res.auto_flatten(&app.vertical_schema_cols);
                            if res.sort.is_some() {
                                res.unsorted_rows = Some(res.rows.clone());
                                res.apply_sort(&app.vertical_schema_cols);
                            }
                            a.query_buffer = res.query_buffer.clone();
                            a.query_cursor = res.query_cursor;
                            a.results = Some(res);
                            applied_to_table = true;
                        }
                    } else {
                        let default_query =
                            Action::TableView.build_query(&catalog, &schema, &table);
                        let query_len = default_query.len();
                        app.clear_mouse_selection();
                        let mut res_state = res_state;
                        res_state.auto_flatten(&app.vertical_schema_cols);
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
                    query_error = Some(err.clone());
                    let res_state = ResultsState {
                        query_buffer,
                        query_cursor,
                        columns: Vec::new(),
                        rows: vec![vec![format!("Error: {err}")]],
                        scroll_v: std::cell::Cell::new(0),
                        scroll_h: std::cell::Cell::new(0),
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
                        total_count: None,
                        total_count_loading: false,
                        table_count: None,
                        ..Default::default()
                    };

                    let cur_selected = if let Screen::Actions(ref a) = app.screen {
                        a.selected
                    } else {
                        0
                    };
                    if let Screen::Actions(ref mut a) = app.screen {
                        if a.catalog == catalog && a.schema == schema && a.table == table {
                            a.query_buffer = res_state.query_buffer.clone();
                            a.query_cursor = res_state.query_cursor;
                            a.results = Some(res_state);
                            applied_to_table = true;
                        }
                    } else {
                        let default_query =
                            Action::TableView.build_query(&catalog, &schema, &table);
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
            if applied_to_table {
                return super::restore::on_query_result(
                    app,
                    &target.0,
                    &target.1,
                    &target.2,
                    query_error.as_deref(),
                );
            }
        }
        AsyncResult::FetchNextPage {
            log_id,
            catalog,
            schema,
            table,
            offset,
            limit,
            result,
        } => match result {
            Ok(results) => {
                app.complete_query_log_success(log_id, results.duration_ms, results.data.len());
                let new_rows = results.data;
                let fetched_count = new_rows.len();
                if let Screen::Actions(ref mut action_state) = app.screen
                    && action_state.catalog == catalog
                    && action_state.schema == schema
                    && action_state.table == table
                    && let Some(state) = action_state.results.as_mut()
                {
                    state.extend_rows_with_flattening(new_rows);
                    if state.sort.is_some() {
                        state.apply_sort(&app.vertical_schema_cols);
                    }
                    state.offset = offset;
                    state.is_fetching_next_page = false;
                    if fetched_count < limit {
                        state.has_more_rows = false;
                    }
                    let actual_limit = state.rows.len();
                    let updated = queries::update_query_limit(&state.query_buffer, actual_limit);
                    let prev_len = state.query_buffer.len();
                    if state.query_cursor == prev_len {
                        state.query_cursor = updated.len();
                    } else {
                        state.query_cursor = state.query_cursor.min(updated.len());
                    }
                    state.query_buffer = updated.clone();
                    action_state.query_buffer = updated;
                    action_state.query_cursor = state.query_cursor;
                    if let Some(ref base) = state.base_query {
                        state.base_query = Some(queries::update_query_limit(base, actual_limit));
                    }
                }
            }
            Err(e) => {
                error!(error = %e, "Fetch next page failed");
                app.complete_query_log_error(log_id, e.to_string());
                if let Screen::Actions(ref mut action_state) = app.screen
                    && action_state.catalog == catalog
                    && action_state.schema == schema
                    && action_state.table == table
                    && let Some(state) = action_state.results.as_mut()
                {
                    state.is_fetching_next_page = false;
                    state.has_more_rows = false;
                    let actual_limit = state.rows.len();
                    let updated = queries::update_query_limit(&state.query_buffer, actual_limit);
                    let prev_len = state.query_buffer.len();
                    if state.query_cursor == prev_len {
                        state.query_cursor = updated.len();
                    } else {
                        state.query_cursor = state.query_cursor.min(updated.len());
                    }
                    state.query_buffer = updated.clone();
                    action_state.query_buffer = updated;
                    action_state.query_cursor = state.query_cursor;
                    if let Some(ref base) = state.base_query {
                        state.base_query = Some(queries::update_query_limit(base, actual_limit));
                    }
                }
            }
        },
        AsyncResult::FetchPartitionLevel {
            log_id,
            catalog,
            schema,
            table,
            filters,
            column,
            result,
        } => match result {
            Ok(results) => {
                app.complete_query_log_success(log_id, results.duration_ms, results.data.len());
                let truncated = results.data.len() >= DRILLDOWN_LEVEL_LIMIT;
                // The type Trino reports for the listed expression is the
                // most reliable one (it covers transforms like `day(ts)`).
                let level_type = results
                    .columns
                    .first()
                    .map(|c| c.type_name.clone())
                    .unwrap_or_default();
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
                    && action_state.catalog == catalog
                    && action_state.schema == schema
                    && action_state.table == table
                    && let Some(drilldown) = action_state.drilldown.as_mut()
                    // Only apply if this result still matches where the user currently
                    // is in the drill-down (guards against stale in-flight responses
                    // after the user has already navigated elsewhere).
                    && drilldown.path == filters
                    && drilldown.next_column() == Some(column.as_str())
                {
                    // `levels_cache[d]` holds the values for `path[..d]`. A
                    // restored view starts deep in the hierarchy with no
                    // ancestor levels loaded, so pad instead of pushing.
                    action_state.column_types.insert(&column, &level_type);
                    let drilldown = action_state.drilldown.as_mut().unwrap();
                    let depth = drilldown.depth();
                    if drilldown.levels_cache.len() <= depth {
                        drilldown.levels_cache.resize(depth + 1, Vec::new());
                    }
                    let remembered = drilldown.path_cursors.get(&drilldown.path).copied();
                    drilldown.selected = drilldown
                        .focus_value
                        .take()
                        .and_then(|v| values.iter().position(|x| *x == v))
                        .or(remembered)
                        .unwrap_or(0)
                        .min(values.len().saturating_sub(1));
                    drilldown.levels_cache[depth] = values;
                    drilldown.loading = false;
                    drilldown.truncated = truncated;
                    drilldown.error = None;

                    return super::navigation::actions::maybe_fetch_selected_partition_count(
                        action_state,
                    );
                }
            }
            Err(e) => {
                error!(error = %e, "Fetch partition level failed");
                let err = e.to_string();
                app.complete_query_log_error(log_id, err.clone());
                if let Screen::Actions(ref mut action_state) = app.screen
                    && action_state.catalog == catalog
                    && action_state.schema == schema
                    && action_state.table == table
                    && let Some(drilldown) = action_state.drilldown.as_mut()
                    && drilldown.path == filters
                    && drilldown.next_column() == Some(column.as_str())
                {
                    drilldown.loading = false;
                    drilldown.error = Some(err);
                }
            }
        },
        AsyncResult::FetchColumnValues {
            log_id,
            catalog,
            schema,
            table,
            column,
            result,
        } => {
            let outcome = match result {
                Ok(results) => {
                    app.complete_query_log_success(log_id, results.duration_ms, results.data.len());
                    let mut values: Vec<String> = results
                        .data
                        .into_iter()
                        .filter_map(|mut r| (!r.is_empty()).then(|| r.remove(0)))
                        .collect();
                    let truncated = values.len() > queries::COLUMN_VALUES_LIMIT;
                    values.truncate(queries::COLUMN_VALUES_LIMIT);
                    values.sort();
                    values.dedup();
                    Ok((values, truncated))
                }
                Err(err) => {
                    error!(error = %err, column = %column, "Fetch column values failed");
                    app.complete_query_log_error(log_id, err.to_string());
                    Err(err.to_string())
                }
            };
            super::filter_modal::column_values_loaded(
                app, &catalog, &schema, &table, &column, outcome,
            );
        }
        AsyncResult::FetchPartitionCount {
            log_id,
            catalog,
            schema,
            table,
            filters,
            result,
        } => match result {
            Ok(results) => {
                app.complete_query_log_success(log_id, results.duration_ms, 1);
                let count_opt = results
                    .data
                    .first()
                    .and_then(|r| r.first())
                    .and_then(|s| s.trim().parse::<usize>().ok());

                if let Screen::Actions(ref mut action_state) = app.screen
                    && action_state.catalog == catalog
                    && action_state.schema == schema
                    && action_state.table == table
                {
                    if let Some(drilldown) = action_state.drilldown.as_mut() {
                        if let Some(count) = count_opt {
                            drilldown.counts_cache.insert(filters.clone(), count);
                            if filters.is_empty() {
                                drilldown.table_count = Some(count);
                                drilldown.table_count_loading = false;
                            }
                        }
                        if drilldown.path == filters {
                            drilldown.current_count_loading = false;
                        }
                    }
                    if let Some(res) = action_state.results.as_mut() {
                        if res.filters == filters {
                            res.total_count = count_opt;
                            res.total_count_loading = false;
                        }
                        if filters.is_empty() && count_opt.is_some() {
                            res.table_count = count_opt;
                        }
                    }
                }
            }
            Err(e) => {
                error!(error = %e, "Fetch partition count failed");
                app.complete_query_log_error(log_id, e.to_string());
                if let Screen::Actions(ref mut action_state) = app.screen
                    && action_state.catalog == catalog
                    && action_state.schema == schema
                    && action_state.table == table
                {
                    if let Some(drilldown) = action_state.drilldown.as_mut() {
                        if filters.is_empty() {
                            drilldown.table_count_loading = false;
                        }
                        if drilldown.path == filters {
                            drilldown.current_count_loading = false;
                        }
                    }
                    if let Some(res) = action_state.results.as_mut()
                        && res.filters == filters
                    {
                        res.total_count_loading = false;
                    }
                }
            }
        },
    }
    None
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
            query_cache_limit: crate::config::DEFAULT_QUERY_CACHE_LIMIT,
        }
    }

    fn grid_with_fetched_status_values(partition: Vec<(String, String)>) -> App {
        let mut app = App::new(sample_config(), false);
        app.clear_mouse_selection();
        let mut res = crate::app::ResultsState {
            is_paginated: true,
            filters: partition,
            columns: vec!["status".into(), "id".into()],
            ..Default::default()
        };
        res.col_values_full.insert("status".to_string());
        res.col_values_truncated.insert("status".to_string());
        res.col_distinct_values
            .insert("status".to_string(), vec!["a".into(), "b".into()]);
        // A list built from loaded rows only: not worth keeping.
        res.col_distinct_values
            .insert("id".to_string(), vec!["1".into()]);
        app.screen = Screen::Actions(ActionState {
            catalog: "c".into(),
            schema: "s".into(),
            table: "t".into(),
            selected: Action::AllTableView.index(),
            results: Some(res),
            ..Default::default()
        });
        app
    }

    fn rerun(app: &mut App, filters: Vec<(String, String)>) {
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
        dispatch_command(
            app,
            Command::ExecuteQuery {
                fresh: false,
                query: "SELECT status, id FROM c.s.t".into(),
                is_paginated: true,
                catalog: "c".into(),
                schema: "s".into(),
                table: "t".into(),
                filters,
            },
            &tx,
        );
    }

    #[test]
    fn rerunning_a_grid_keeps_its_fetched_filter_values() {
        let mut app = grid_with_fetched_status_values(Vec::new());
        rerun(&mut app, Vec::new());
        let Screen::Actions(a) = &app.screen else {
            panic!()
        };
        let res = a.results.as_ref().unwrap();
        assert!(res.col_values_full.contains("status"));
        assert!(res.col_values_truncated.contains("status"));
        assert_eq!(res.col_distinct_values["status"], vec!["a", "b"]);
        assert!(!res.col_distinct_values.contains_key("id"));
    }

    #[test]
    fn another_partition_does_not_inherit_fetched_values() {
        let mut app = grid_with_fetched_status_values(Vec::new());
        rerun(
            &mut app,
            vec![("date".to_string(), "2026-08-06".to_string())],
        );
        let Screen::Actions(a) = &app.screen else {
            panic!()
        };
        let res = a.results.as_ref().unwrap();
        assert!(res.col_values_full.is_empty());
        assert!(res.col_distinct_values.is_empty());
    }

    #[test]
    fn the_next_page_of_a_filtered_grid_keeps_its_filters_and_columns() {
        let mut app = App::new(sample_config(), false);
        app.clear_mouse_selection();
        // What the grid ran after a column filter and hidden `_hoodie_*`
        // columns: fewer columns and a WHERE, shown with the rows loaded so far.
        let grid_query = "SELECT \"account_name\", \"event_type\" FROM \"c\".\"s\".\"t\" \
                          WHERE \"event_type\" = 'viewed' OFFSET 0 LIMIT 100";
        app.screen = Screen::Actions(ActionState {
            catalog: "c".into(),
            schema: "s".into(),
            table: "t".into(),
            selected: Action::AllTableView.index(),
            results: Some(crate::app::ResultsState {
                is_paginated: true,
                query_buffer: grid_query.into(),
                ..Default::default()
            }),
            ..Default::default()
        });
        app.vertical_schema_cols = vec![
            vcol(1, "_hoodie_commit_time", "varchar"),
            vcol(2, "account_name", "varchar"),
            vcol(3, "event_type", "varchar"),
        ];

        assert_eq!(
            next_page_query(&app, "c", "s", "t", 100, 100, &[]),
            "SELECT \"account_name\", \"event_type\" FROM \"c\".\"s\".\"t\" \
             WHERE \"event_type\" = 'viewed' OFFSET 100 LIMIT 100"
        );
    }

    #[test]
    fn the_next_page_without_a_grid_is_the_plain_table_page() {
        let mut app = App::new(sample_config(), false);
        app.clear_mouse_selection();
        app.vertical_schema_cols = vec![
            vcol(1, "a", "varchar"),
            vcol(2, "m", "map(varchar, varchar)"),
        ];
        assert_eq!(
            next_page_query(&app, "c", "s", "t", 100, 100, &[]),
            "SELECT \"a\" FROM \"c\".\"s\".\"t\" OFFSET 100 LIMIT 100"
        );
    }

    fn vcol(index: usize, name: &str, data_type: &str) -> crate::app::VerticalColumn {
        crate::app::VerticalColumn {
            index,
            name: name.to_string(),
            data_type: data_type.to_string(),
            is_nullable: "YES".to_string(),
            key_meta: String::new(),
            description: String::new(),
        }
    }

    #[test]
    fn all_table_view_starts_when_column_metadata_arrives() {
        let mut app = App::new(sample_config(), false);
        app.clear_mouse_selection();
        app.screen = Screen::Actions(ActionState {
            catalog: "iceberg".to_string(),
            schema: "sales".to_string(),
            table: "orders".to_string(),
            selected: Action::AllTableView.index(),
            pending_action: Some(Action::AllTableView.index()),
            metadata_loading: true,
            ..Default::default()
        });

        let cmd = handle_async_result(
            &mut app,
            AsyncResult::FetchTableMetadata {
                partitions_log_id: 1,
                cols_log_id: 2,
                catalog: "iceberg".to_string(),
                schema: "sales".to_string(),
                table: "orders".to_string(),
                partition_lines: Vec::new(),
                columns: vec![vcol(1, "id", "bigint")],
                partitions_error: None,
                columns_error: None,
            },
        );
        assert!(matches!(
            cmd,
            Some(Command::ExecuteQuery {
                is_paginated: true,
                ..
            })
        ));
    }

    #[test]
    fn execute_query_skips_map_columns_for_a_plain_select_star() {
        let mut app = App::new(sample_config(), false);
        app.clear_mouse_selection();
        app.screen = Screen::Actions(ActionState {
            catalog: "datalake".to_string(),
            schema: "tenant".to_string(),
            table: "events".to_string(),
            selected: Action::AllTableView.index(),
            ..Default::default()
        });
        app.vertical_schema_cols = vec![
            vcol(1, "event_type", "varchar"),
            vcol(2, "policy_id", "map(varchar, varchar)"),
            vcol(3, "timestamp", "bigint"),
        ];

        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
        dispatch_command(
            &mut app,
            Command::ExecuteQuery {
                fresh: false,
                query: Action::AllTableView.build_query("datalake", "tenant", "events"),
                is_paginated: true,
                catalog: "datalake".to_string(),
                schema: "tenant".to_string(),
                table: "events".to_string(),
                filters: Vec::new(),
            },
            &tx,
        );

        let Screen::Actions(state) = &app.screen else {
            panic!("expected actions screen");
        };
        let expected = "SELECT \"event_type\", \"timestamp\" FROM \"datalake\".\"tenant\".\"events\" OFFSET 0 LIMIT 100";
        assert_eq!(state.query_buffer, expected);
        assert_eq!(state.results.as_ref().unwrap().query_buffer, expected);
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
            selected: Action::TableDDL.index(),
            ddl_loading: true,
            ..Default::default()
        });

        handle_async_result(
            &mut app,
            AsyncResult::FetchTableDdl {
                show_create_log_id: 1,
                catalog: "iceberg".to_string(),
                schema: "sales".to_string(),
                table: "orders".to_string(),
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
            selected: crate::app::Action::TableView.index(),
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
                fresh: false,
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

    #[test]
    fn fetch_partition_count_updates_drilldown_and_results_state() {
        let mut app = App::new(sample_config(), false);
        app.clear_mouse_selection();
        let mut dd = DrillDownState {
            partition_cols: vec!["date".to_string(), "service".to_string()],
            path: vec![("date".to_string(), "2026-08-06".to_string())],
            table_count: None,
            table_count_loading: true,
            current_count_loading: true,
            ..Default::default()
        };
        dd.counts_cache = std::collections::HashMap::new();

        app.screen = Screen::Actions(ActionState {
            catalog: "iceberg".to_string(),
            schema: "sales".to_string(),
            table: "orders".to_string(),
            drilldown: Some(dd),
            results: Some(ResultsState {
                query_buffer: "".to_string(),
                query_cursor: 0,
                columns: Vec::new(),
                rows: Vec::new(),
                scroll_v: std::cell::Cell::new(0),
                scroll_h: std::cell::Cell::new(0),
                loading: false,
                error: None,
                is_paginated: true,
                catalog: "iceberg".to_string(),
                schema: "sales".to_string(),
                table: "orders".to_string(),
                offset: 0,
                page_size: 100,
                is_fetching_next_page: false,
                has_more_rows: false,
                invalid_query_error: None,
                selection_anchor: None,
                filters: vec![("date".to_string(), "2026-08-06".to_string())],
                total_count: None,
                total_count_loading: true,
                table_count: None,
                ..Default::default()
            }),
            ..Default::default()
        });

        let log_id = app.add_query_log("SELECT COUNT(*)".to_string());
        let count_results = crate::trino::types::QueryResults {
            columns: vec![crate::trino::types::Column {
                name: "total_records".to_string(),
                type_name: String::new(),
            }],
            data: vec![vec!["45120".to_string()]],
            duration_ms: 35,
        };

        handle_async_result(
            &mut app,
            AsyncResult::FetchPartitionCount {
                log_id,
                catalog: "iceberg".to_string(),
                schema: "sales".to_string(),
                table: "orders".to_string(),
                filters: vec![("date".to_string(), "2026-08-06".to_string())],
                result: Ok(count_results),
            },
        );

        let Screen::Actions(state) = &app.screen else {
            panic!("expected actions screen");
        };
        let dd = state.drilldown.as_ref().unwrap();
        assert_eq!(
            dd.counts_cache
                .get(&vec![("date".to_string(), "2026-08-06".to_string())]),
            Some(&45120)
        );
        assert!(!dd.current_count_loading);

        let res = state.results.as_ref().unwrap();
        assert_eq!(res.total_count, Some(45120));
        assert!(!res.total_count_loading);

        // Also test root/table count (filters: empty)
        let log_id2 = app.add_query_log("SELECT COUNT(*)".to_string());
        let table_count_results = crate::trino::types::QueryResults {
            columns: vec![crate::trino::types::Column {
                name: "total_records".to_string(),
                type_name: String::new(),
            }],
            data: vec![vec!["1234567".to_string()]],
            duration_ms: 50,
        };

        handle_async_result(
            &mut app,
            AsyncResult::FetchPartitionCount {
                log_id: log_id2,
                catalog: "iceberg".to_string(),
                schema: "sales".to_string(),
                table: "orders".to_string(),
                filters: Vec::new(),
                result: Ok(table_count_results),
            },
        );

        let Screen::Actions(state) = &app.screen else {
            panic!("expected actions screen");
        };
        let dd = state.drilldown.as_ref().unwrap();
        assert_eq!(dd.table_count, Some(1234567));
        assert!(!dd.table_count_loading);
    }

    #[test]
    fn fetch_next_page_incrementally_updates_query_bar_limit() {
        let mut app = App::new(sample_config(), false);
        app.clear_mouse_selection();

        let initial_rows: Vec<Vec<String>> = (0..100)
            .map(|i| vec![i.to_string(), format!("item_{i}")])
            .collect();

        let initial_query = "SELECT * FROM iceberg.sales.orders OFFSET 0 LIMIT 100".to_string();

        app.screen = Screen::Actions(ActionState {
            catalog: "iceberg".to_string(),
            schema: "sales".to_string(),
            table: "orders".to_string(),
            query_buffer: initial_query.clone(),
            query_cursor: initial_query.len(),
            results: Some(ResultsState {
                query_buffer: initial_query.clone(),
                query_cursor: initial_query.len(),
                columns: vec!["id".to_string(), "name".to_string()],
                rows: initial_rows,
                scroll_v: std::cell::Cell::new(0),
                scroll_h: std::cell::Cell::new(0),
                loading: false,
                error: None,
                is_paginated: true,
                catalog: "iceberg".to_string(),
                schema: "sales".to_string(),
                table: "orders".to_string(),
                offset: 0,
                page_size: 100,
                is_fetching_next_page: false,
                has_more_rows: true,
                ..Default::default()
            }),
            ..Default::default()
        });

        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();

        // 1. Dispatching next page increases the limit incrementally in query bar
        dispatch_command(
            &mut app,
            Command::FetchNextPage {
                catalog: "iceberg".to_string(),
                schema: "sales".to_string(),
                table: "orders".to_string(),
                offset: 100,
                limit: 100,
                filters: Vec::new(),
            },
            &tx,
        );

        let Screen::Actions(action_state) = &app.screen else {
            panic!("expected actions screen");
        };
        let res = action_state.results.as_ref().unwrap();
        assert_eq!(
            res.query_buffer,
            "SELECT * FROM iceberg.sales.orders OFFSET 0 LIMIT 200"
        );
        assert_eq!(
            action_state.query_buffer,
            "SELECT * FROM iceberg.sales.orders OFFSET 0 LIMIT 200"
        );

        // 2. Full page arrives -> limit stays 200, rows extend to 200
        let next_100_rows: Vec<Vec<String>> = (100..200)
            .map(|i| vec![i.to_string(), format!("item_{i}")])
            .collect();
        let log_id = app.add_query_log("page 2".to_string());
        handle_async_result(
            &mut app,
            AsyncResult::FetchNextPage {
                log_id,
                catalog: "iceberg".to_string(),
                schema: "sales".to_string(),
                table: "orders".to_string(),
                offset: 100,
                limit: 100,
                result: Ok(crate::trino::types::QueryResults {
                    columns: vec![
                        crate::trino::types::Column {
                            name: "id".to_string(),
                            type_name: String::new(),
                        },
                        crate::trino::types::Column {
                            name: "name".to_string(),
                            type_name: String::new(),
                        },
                    ],
                    data: next_100_rows,
                    duration_ms: 40,
                }),
            },
        );

        let Screen::Actions(action_state) = &app.screen else {
            panic!("expected actions screen");
        };
        let res = action_state.results.as_ref().unwrap();
        assert_eq!(res.rows.len(), 200);
        assert_eq!(
            res.query_buffer,
            "SELECT * FROM iceberg.sales.orders OFFSET 0 LIMIT 200"
        );

        // 3. Dispatching page 3 updates limit to 300
        dispatch_command(
            &mut app,
            Command::FetchNextPage {
                catalog: "iceberg".to_string(),
                schema: "sales".to_string(),
                table: "orders".to_string(),
                offset: 200,
                limit: 100,
                filters: Vec::new(),
            },
            &tx,
        );

        let Screen::Actions(action_state) = &app.screen else {
            panic!("expected actions screen");
        };
        let res = action_state.results.as_ref().unwrap();
        assert_eq!(
            res.query_buffer,
            "SELECT * FROM iceberg.sales.orders OFFSET 0 LIMIT 300"
        );

        // 4. End of table (only 35 rows returned) -> refines limit to 235
        let last_35_rows: Vec<Vec<String>> = (200..235)
            .map(|i| vec![i.to_string(), format!("item_{i}")])
            .collect();
        let log_id2 = app.add_query_log("page 3".to_string());
        handle_async_result(
            &mut app,
            AsyncResult::FetchNextPage {
                log_id: log_id2,
                catalog: "iceberg".to_string(),
                schema: "sales".to_string(),
                table: "orders".to_string(),
                offset: 200,
                limit: 100,
                result: Ok(crate::trino::types::QueryResults {
                    columns: vec![
                        crate::trino::types::Column {
                            name: "id".to_string(),
                            type_name: String::new(),
                        },
                        crate::trino::types::Column {
                            name: "name".to_string(),
                            type_name: String::new(),
                        },
                    ],
                    data: last_35_rows,
                    duration_ms: 25,
                }),
            },
        );

        let Screen::Actions(action_state) = &app.screen else {
            panic!("expected actions screen");
        };
        let res = action_state.results.as_ref().unwrap();
        assert_eq!(res.rows.len(), 235);
        assert!(!res.has_more_rows);
        assert_eq!(
            res.query_buffer,
            "SELECT * FROM iceberg.sales.orders OFFSET 0 LIMIT 235"
        );
        assert_eq!(
            action_state.query_buffer,
            "SELECT * FROM iceberg.sales.orders OFFSET 0 LIMIT 235"
        );
    }

    #[test]
    fn fetch_table_ddl_auto_dispatches_table_view_for_unpartitioned_table_when_pending() {
        let mut app = App::new(sample_config(), false);
        app.screen = Screen::Actions(ActionState {
            catalog: "hive".to_string(),
            schema: "default".to_string(),
            table: "events".to_string(),
            selected: crate::app::Action::TableView.index(),
            pending_action: Some(0),
            ddl_loading: true,
            ..Default::default()
        });

        let cmd = handle_async_result(
            &mut app,
            AsyncResult::FetchTableDdl {
                show_create_log_id: 1,
                catalog: "hive".to_string(),
                schema: "default".to_string(),
                table: "events".to_string(),
                partitioned_by: Vec::new(),
                location: "s3://bucket/events".to_string(),
                ddl_text: "CREATE TABLE hive.default.events (id bigint)".to_string(),
                show_create_error: None,
            },
        );

        assert!(matches!(
            cmd,
            Some(Command::ExecuteQuery {
                catalog,
                schema,
                table,
                is_paginated: true,
                ..
            }) if catalog == "hive" && schema == "default" && table == "events"
        ));
        let Screen::Actions(s) = &app.screen else {
            panic!()
        };
        assert_eq!(s.pending_action, None);
        assert!(!s.ddl_loading);
    }

    #[test]
    fn fetch_table_ddl_auto_dispatches_drilldown_for_partitioned_table_when_pending() {
        let mut app = App::new(sample_config(), false);
        app.screen = Screen::Actions(ActionState {
            catalog: "hive".to_string(),
            schema: "default".to_string(),
            table: "events_part".to_string(),
            selected: crate::app::Action::TableView.index(),
            pending_action: Some(0),
            ddl_loading: true,
            ..Default::default()
        });

        let cmd = handle_async_result(
            &mut app,
            AsyncResult::FetchTableDdl {
                show_create_log_id: 1,
                catalog: "hive".to_string(),
                schema: "default".to_string(),
                table: "events_part".to_string(),
                partitioned_by: vec!["dt".to_string(), "region".to_string()],
                location: "s3://bucket/events_part".to_string(),
                ddl_text:
                    "CREATE TABLE hive.default.events_part (id bigint, dt varchar, region varchar)"
                        .to_string(),
                show_create_error: None,
            },
        );

        assert!(matches!(
            cmd,
            Some(Command::FetchPartitionLevel {
                catalog,
                schema,
                table,
                column,
                filters,
            }) if catalog == "hive" && schema == "default" && table == "events_part" && column == "dt" && filters.is_empty()
        ));
        let Screen::Actions(s) = &app.screen else {
            panic!()
        };
        assert_eq!(s.pending_action, None);
        assert!(!s.ddl_loading);
        assert!(s.drilldown.is_some());
    }

    #[test]
    fn execute_query_hides_hudi_internal_columns_by_default_and_selects_first_visible() {
        let mut app = App::new(sample_config(), false);
        app.screen = Screen::Actions(ActionState {
            catalog: "hudi".to_string(),
            schema: "analytics".to_string(),
            table: "users".to_string(),
            selected: crate::app::Action::TableView.index(),
            ..Default::default()
        });

        handle_async_result(
            &mut app,
            AsyncResult::ExecuteQuery {
                log_id: 1,
                query_buffer: "SELECT * FROM hudi.analytics.users".to_string(),
                query_cursor: 34,
                catalog: "hudi".to_string(),
                schema: "analytics".to_string(),
                table: "users".to_string(),
                is_paginated: true,
                filters: Vec::new(),
                result: Ok(crate::trino::types::QueryResults {
                    columns: vec![
                        crate::trino::types::Column {
                            name: "_hoodie_commit_time".to_string(),
                            type_name: String::new(),
                        },
                        crate::trino::types::Column {
                            name: "_hoodie_record_key".to_string(),
                            type_name: String::new(),
                        },
                        crate::trino::types::Column {
                            name: "user_id".to_string(),
                            type_name: String::new(),
                        },
                        crate::trino::types::Column {
                            name: "username".to_string(),
                            type_name: String::new(),
                        },
                    ],
                    data: vec![vec![
                        "20261014120000".to_string(),
                        "key1".to_string(),
                        "101".to_string(),
                        "alice".to_string(),
                    ]],
                    duration_ms: 10,
                }),
            },
        );

        let Screen::Actions(s) = &app.screen else {
            panic!()
        };
        let res = s.results.as_ref().unwrap();
        assert!(res.hidden_columns.contains("_hoodie_commit_time"));
        assert!(res.hidden_columns.contains("_hoodie_record_key"));
        assert!(!res.hidden_columns.contains("user_id"));
        assert!(!res.hidden_columns.contains("username"));
        // First visible column is user_id at index 2
        assert_eq!(res.selected_col, 2);
    }

    #[test]
    fn fetch_partition_level_auto_dispatches_count_for_selected_item_zero() {
        let mut app = App::new(sample_config(), false);
        app.screen = Screen::Actions(ActionState {
            catalog: "lake".to_string(),
            schema: "default".to_string(),
            table: "logs".to_string(),
            selected: crate::app::Action::TableView.index(),
            drilldown: Some(DrillDownState {
                partition_cols: vec!["dt".to_string()],
                path: Vec::new(),
                levels_cache: Vec::new(),
                selected: 0,
                loading: true,
                error: None,
                truncated: false,
                table_count: None,
                table_count_loading: true,
                counts_cache: std::collections::HashMap::new(),
                current_count_loading: true,
                path_cursors: std::collections::HashMap::new(),
                focus_value: None,
            }),
            ..Default::default()
        });

        let cmd = handle_async_result(
            &mut app,
            AsyncResult::FetchPartitionLevel {
                log_id: 1,
                catalog: "lake".to_string(),
                schema: "default".to_string(),
                table: "logs".to_string(),
                filters: Vec::new(),
                column: "dt".to_string(),
                result: Ok(crate::trino::types::QueryResults {
                    columns: vec![crate::trino::types::Column {
                        name: "dt".to_string(),
                        type_name: String::new(),
                    }],
                    data: vec![
                        vec!["2026-10-14".to_string()],
                        vec!["2026-10-13".to_string()],
                    ],
                    duration_ms: 20,
                }),
            },
        );

        assert!(matches!(
            cmd,
            Some(Command::FetchPartitionCount {
                catalog,
                schema,
                table,
                filters,
            }) if catalog == "lake"
                && schema == "default"
                && table == "logs"
                && filters == vec![("dt".to_string(), "2026-10-14".to_string())]
        ));
    }
}
