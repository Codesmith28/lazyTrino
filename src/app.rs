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

use std::collections::HashMap;

use crate::config::ConnectionConfig;
use crate::trino::client::TrinoClient;

#[derive(Clone, Debug, PartialEq)]
pub enum QueryStatus {
    Running,
    Success,
    Error,
}

#[derive(Clone, Debug)]
pub struct QueryLogEntry {
    pub id: usize,
    pub sql: String,
    pub status: QueryStatus,
    pub duration_ms: Option<u64>,
    pub row_count: Option<usize>,
    pub error_msg: Option<String>,
}

#[derive(Clone, Debug)]
pub struct VerticalColumn {
    pub index: usize,
    pub name: String,
    pub data_type: String,
    pub key_meta: String,
    pub description: String,
}

/// Returns the subset of column names safe to include in a `SELECT` list
/// for row-level drill-down queries. Trino/Parquet can fail an entire
/// query with an "Unsupported ... Parquet column" error when a `map` or
/// `row` (struct) typed column's on-disk encoding doesn't match what the
/// catalog metadata declares — a schema-drift issue seen in some Hudi
/// tables. Excluding those columns lets the rest of the row still be
/// readable instead of the whole query failing. Falls back to an empty
/// list (meaning "use `SELECT *`") when there's no cached schema yet.
pub fn safe_select_columns(columns: &[VerticalColumn]) -> Vec<String> {
    if columns.is_empty() {
        return Vec::new();
    }
    let safe: Vec<String> = columns
        .iter()
        .filter(|c| {
            let t = c.data_type.trim().to_ascii_lowercase();
            !(t.starts_with("map(") || t.starts_with("row(") || t == "map" || t == "row")
        })
        .map(|c| c.name.clone())
        .collect();
    // If everything got filtered out (unexpected), fall back to `SELECT *`
    // rather than sending an empty/invalid column list.
    if safe.is_empty() {
        Vec::new()
    } else {
        safe
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ActivePanel {
    MenuPane,
    MainViewer,
}

#[derive(Clone)]
pub struct ConnectState {
    pub url: String,
    pub user: String,
    pub password: String,
    pub focused: usize,
    pub loading: bool,
    pub error: Option<String>,
}

impl Default for ConnectState {
    fn default() -> Self {
        let default_config = ConnectionConfig::default();
        Self {
            url: default_config.url,
            user: default_config.user,
            password: String::new(),
            focused: 0,
            loading: false,
            error: None,
        }
    }
}

#[derive(Clone)]
pub struct CatalogState {
    pub items: Vec<String>,
    pub selected: usize,
}

#[derive(Clone)]
pub struct SchemaState {
    pub catalog: String,
    pub items: Vec<String>,
    pub selected: usize,
}

#[derive(Clone)]
pub struct TableState {
    pub catalog: String,
    pub schema: String,
    pub items: Vec<String>,
    pub selected: usize,
}

/// Ordered partition-column layout + storage location for a table, parsed
/// once (per table) from a live `SHOW CREATE TABLE` response during table
/// entry recon. `partitioned_by` is empty for unpartitioned tables. This is
/// always derived dynamically from that table's own DDL — never hardcoded
/// by table/schema name.
#[derive(Clone, Debug, Default)]
pub struct TableRecon {
    pub partitioned_by: Vec<String>,
    pub location: String,
    /// Raw `SHOW CREATE TABLE` DDL text fetched during recon. Cached here so
    /// `Action::TableDDL` can render it instantly without re-querying Trino.
    pub ddl_text: String,
}

/// Tracks cd/ls-style drill-down progress through a partitioned table's
/// hierarchy: which partition columns exist (from `TableRecon`), which
/// values have been fixed so far (`path`), and the distinct values shown
/// at each previously-visited depth (`levels_cache`, so navigating back up
/// with `h` doesn't need to re-query Trino).
#[derive(Clone, Debug, Default)]
pub struct DrillDownState {
    pub partition_cols: Vec<String>,
    pub path: Vec<(String, String)>,
    pub levels_cache: Vec<Vec<String>>,
    pub selected: usize,
    pub loading: bool,
    pub error: Option<String>,
    pub truncated: bool,
}

impl DrillDownState {
    /// Depth (0-based) of the level currently being displayed, i.e. how
    /// many partition columns have already been fixed.
    pub fn depth(&self) -> usize {
        self.path.len()
    }

    /// True once every partition column has a fixed value and the caller
    /// should switch to the leaf record view instead of another distinct
    /// value list.
    pub fn is_leaf(&self) -> bool {
        self.path.len() >= self.partition_cols.len()
    }

    /// The next partition column to fetch distinct values for, if any
    /// levels remain.
    pub fn next_column(&self) -> Option<&str> {
        self.partition_cols.get(self.path.len()).map(|s| s.as_str())
    }
}

#[derive(Clone, Default)]
pub struct ActionState {
    pub catalog: String,
    pub schema: String,
    pub table: String,
    pub selected: usize,
    pub query_buffer: String,
    pub query_cursor: usize,
    pub results: Option<ResultsState>,
    pub metadata: Option<TableRecon>,
    /// True until `SHOW CREATE TABLE` (the DDL/partition-layout part of
    /// recon) resolves. Only gates `Table DDL` and `Table View`, which only
    /// need `metadata`, not the slower `$partitions`/info-schema queries.
    pub ddl_loading: bool,
    /// True until the `$partitions` and `information_schema.columns` recon
    /// queries resolve. Gates `Partitions` and `Schema`.
    pub metadata_loading: bool,
    pub drilldown: Option<DrillDownState>,
}

#[derive(Clone)]
pub struct ResultsState {
    pub query_buffer: String,
    pub query_cursor: usize,
    pub columns: Vec<String>,
    pub rows: Vec<Vec<String>>,
    pub scroll_v: usize,
    pub scroll_h: usize,
    pub loading: bool,
    pub error: Option<String>,
    pub is_paginated: bool,
    pub catalog: String,
    pub schema: String,
    pub table: String,
    pub offset: usize,
    pub page_size: usize,
    pub is_fetching_next_page: bool,
    pub has_more_rows: bool,
    pub invalid_query_error: Option<String>,
    pub selection_anchor: Option<usize>,
    /// Partition predicates (`col`, `value`) baked into this view's query,
    /// e.g. when viewing leaf-level records reached via partition
    /// drill-down. Empty for ordinary (non-drill-down) queries. Threaded
    /// through to `FetchNextPage` so infinite scroll keeps the same
    /// predicate on subsequent pages.
    pub filters: Vec<(String, String)>,
}

impl ResultsState {
    pub fn selection_range(&self) -> Option<(usize, usize)> {
        if let Some(anchor) = self.selection_anchor
            && anchor != self.query_cursor
        {
            let start = anchor.min(self.query_cursor);
            let end = anchor.max(self.query_cursor);
            return Some((start, end));
        }
        None
    }

    pub fn delete_selection(&mut self) -> bool {
        if let Some((start, end)) = self.selection_range() {
            let start = start.min(self.query_buffer.len());
            let end = end.min(self.query_buffer.len());
            if start < end {
                self.query_buffer.drain(start..end);
                self.query_cursor = start;
                self.selection_anchor = None;
                return true;
            }
        }
        false
    }

    pub fn clear_selection(&mut self) {
        self.selection_anchor = None;
    }

    pub fn select_all(&mut self) {
        self.selection_anchor = Some(0);
        self.query_cursor = self.query_buffer.len();
    }
}

// `ActionState` (which embeds the optional `ResultsState` with query text/rows) is
// substantially larger than the other variants. `Screen` is only ever held as a single
// top-level app field (never collected into large vectors or hot loops), so boxing the
// large variant would add pattern-matching overhead across handler.rs/mod.rs for no
// measurable benefit.
#[allow(clippy::large_enum_variant)]
#[derive(Clone)]
pub enum Screen {
    Connect(ConnectState),
    Catalog(CatalogState),
    Schema(SchemaState),
    Table(TableState),
    Actions(ActionState),
    Help,
}

pub enum Mode {
    Normal,
    Search,
    QueryInput,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Action {
    TableView,
    TableDDL,
    InfoSchema,
    ShowStats,
    Count,
    Sample,
    Partitions,
    Schema,
}

pub const ACTIONS: &[(char, &str, Action)] = &[
    ('v', "Table View Mode", Action::TableView),
    ('c', "Table DDL", Action::TableDDL),
    ('i', "Info Schema", Action::InfoSchema),
    ('s', "Show Stats", Action::ShowStats),
    ('n', "Count", Action::Count),
    ('p', "Sample Mode (20 rows)", Action::Sample),
    ('P', "Partitions", Action::Partitions),
    ('S', "Schema", Action::Schema),
];

impl Action {
    pub fn build_query(&self, catalog: &str, schema: &str, table: &str) -> String {
        match self {
            Action::TableView => crate::trino::queries::page_query(catalog, schema, table, 0, 100),
            Action::TableDDL => crate::trino::queries::show_create(catalog, schema, table),
            Action::InfoSchema => {
                crate::trino::queries::info_schema_columns(catalog, schema, table)
            }
            Action::ShowStats => crate::trino::queries::show_stats(catalog, schema, table),
            Action::Count => crate::trino::queries::count(catalog, schema, table),
            Action::Sample => crate::trino::queries::sample(catalog, schema, table),
            Action::Partitions => crate::trino::queries::partitions(catalog, schema, table),
            Action::Schema => crate::trino::queries::describe(catalog, schema, table),
        }
    }
}

pub struct App {
    pub screen: Screen,
    pub prev_screen: Option<Box<Screen>>,
    pub mode: Mode,
    pub should_quit: bool,
    pub number_buffer: String,
    pub search_query: String,
    pub loading: bool,
    pub trino_client: Option<TrinoClient>,
    pub config: ConnectionConfig,
    pub catalogs: Vec<String>,
    pub schemas: HashMap<String, Vec<String>>,
    pub tables: HashMap<(String, String), Vec<String>>,
    pub frame_count: u64,
    pub query_logs: Vec<QueryLogEntry>,
    pub active_panel: ActivePanel,
    pub partition_tree_lines: Vec<String>,
    pub partition_scroll: usize,
    pub vertical_schema_cols: Vec<VerticalColumn>,
    pub schema_scroll: usize,
    pub auto_connect: bool,
    pub main_panel_pct: u16,
    pub control_panel_split_pct: u16,
    pub is_dragging_resizer: bool,
    pub is_dragging_query_select: bool,
    pub query_inspector_scroll: usize,
    pub mouse_selection_anchor: Option<(u16, u16)>,
    pub mouse_selection_current: Option<(u16, u16)>,
    pub is_selecting_text: bool,
    pub copied_toast: Option<(String, std::time::Instant)>,
}

impl App {
    pub fn new(config: ConnectionConfig, auto_connect: bool) -> Self {
        Self {
            screen: Screen::Connect(ConnectState {
                url: config.url.clone(),
                user: config.user.clone(),
                password: config.password.clone(),
                focused: 0,
                loading: false,
                error: None,
            }),
            prev_screen: None,
            mode: Mode::Normal,
            should_quit: false,
            number_buffer: String::new(),
            search_query: String::new(),
            loading: false,
            trino_client: None,
            config,
            catalogs: Vec::new(),
            schemas: HashMap::new(),
            tables: HashMap::new(),
            frame_count: 0,
            query_logs: Vec::new(),
            active_panel: ActivePanel::MainViewer,
            partition_tree_lines: Vec::new(),
            partition_scroll: 0,
            vertical_schema_cols: Vec::new(),
            schema_scroll: 0,
            auto_connect,
            main_panel_pct: 60,
            control_panel_split_pct: 40,
            is_dragging_resizer: false,
            is_dragging_query_select: false,
            query_inspector_scroll: 0,
            mouse_selection_anchor: None,
            mouse_selection_current: None,
            is_selecting_text: false,
            copied_toast: None,
        }
    }

    pub fn add_query_log(&mut self, sql: String) -> usize {
        let id = self.query_logs.len() + 1;
        self.query_logs.push(QueryLogEntry {
            id,
            sql,
            status: QueryStatus::Running,
            duration_ms: None,
            row_count: None,
            error_msg: None,
        });
        id
    }

    pub fn complete_query_log_success(&mut self, id: usize, duration_ms: u64, row_count: usize) {
        if let Some(entry) = self.query_logs.iter_mut().find(|e| e.id == id) {
            entry.status = QueryStatus::Success;
            entry.duration_ms = Some(duration_ms);
            entry.row_count = Some(row_count);
        }
    }

    pub fn complete_query_log_error(&mut self, id: usize, err: String) {
        if let Some(entry) = self.query_logs.iter_mut().find(|e| e.id == id) {
            entry.status = QueryStatus::Error;
            entry.error_msg = Some(err);
        }
    }

    pub fn is_area_mouse_selected(&self, area_x: u16, area_w: u16, row_y: u16) -> bool {
        if let (Some((a_x, a_y)), Some((c_x, c_y))) =
            (self.mouse_selection_anchor, self.mouse_selection_current)
        {
            let start_y = a_y.min(c_y);
            let end_y = a_y.max(c_y);
            let start_x = a_x.min(c_x);
            let end_x = a_x.max(c_x);

            let y_match = row_y >= start_y && row_y <= end_y;
            let x_match = area_x < end_x && area_x.saturating_add(area_w) > start_x;
            y_match && x_match
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ConnectionConfig;
    use crate::trino::queries;

    fn sample_config() -> ConnectionConfig {
        ConnectionConfig {
            url: "https://trino.example".to_string(),
            user: "analyst".to_string(),
            password: "secret".to_string(),
        }
    }

    #[test]
    fn app_new_initializes_connect_screen_and_defaults() {
        let config = sample_config();
        let app = App::new(config.clone(), true);

        let Screen::Connect(state) = &app.screen else {
            panic!("expected connect screen");
        };
        assert_eq!(state.url, config.url);
        assert_eq!(state.user, config.user);
        assert_eq!(state.password, config.password);
        assert_eq!(state.focused, 0);
        assert!(!state.loading);
        assert!(state.error.is_none());

        assert!(app.prev_screen.is_none());
        assert!(matches!(app.mode, Mode::Normal));
        assert!(!app.should_quit);
        assert!(app.number_buffer.is_empty());
        assert!(app.search_query.is_empty());
        assert!(!app.loading);
        assert!(app.trino_client.is_none());
        assert_eq!(app.config, config);
        assert!(app.catalogs.is_empty());
        assert!(app.schemas.is_empty());
        assert!(app.tables.is_empty());
        assert_eq!(app.frame_count, 0);
        assert!(app.query_logs.is_empty());
        assert_eq!(app.active_panel, ActivePanel::MainViewer);
        assert!(app.partition_tree_lines.is_empty());
        assert_eq!(app.partition_scroll, 0);
        assert!(app.vertical_schema_cols.is_empty());
        assert_eq!(app.schema_scroll, 0);
        assert!(app.auto_connect);
        assert_eq!(app.main_panel_pct, 60);
        assert_eq!(app.control_panel_split_pct, 40);
        assert!(!app.is_dragging_resizer);
        assert!(!app.is_dragging_query_select);
        assert_eq!(app.query_inspector_scroll, 0);
        assert!(app.mouse_selection_anchor.is_none());
        assert!(app.mouse_selection_current.is_none());
        assert!(!app.is_selecting_text);
        assert!(app.copied_toast.is_none());
    }

    #[test]
    fn query_log_lifecycle_tracks_success_and_error_entries() {
        let mut app = App::new(sample_config(), false);

        let first_id = app.add_query_log("SELECT 1".to_string());
        let second_id = app.add_query_log("SELECT 2".to_string());

        assert_eq!(first_id, 1);
        assert_eq!(second_id, 2);

        app.complete_query_log_success(first_id, 125, 7);
        app.complete_query_log_error(second_id, "boom".to_string());

        assert_eq!(app.query_logs.len(), 2);

        let first = &app.query_logs[0];
        assert_eq!(first.id, 1);
        assert_eq!(first.sql, "SELECT 1");
        assert_eq!(first.status, QueryStatus::Success);
        assert_eq!(first.duration_ms, Some(125));
        assert_eq!(first.row_count, Some(7));
        assert!(first.error_msg.is_none());

        let second = &app.query_logs[1];
        assert_eq!(second.id, 2);
        assert_eq!(second.sql, "SELECT 2");
        assert_eq!(second.status, QueryStatus::Error);
        assert_eq!(second.duration_ms, None);
        assert_eq!(second.row_count, None);
        assert_eq!(second.error_msg.as_deref(), Some("boom"));
    }

    #[test]
    fn action_build_query_matches_queries_module_for_all_variants() {
        let catalog = "iceberg";
        let schema = "sales";
        let table = "orders";
        let cases = vec![
            (
                Action::TableView,
                queries::page_query(catalog, schema, table, 0, 100),
            ),
            (Action::TableDDL, queries::show_create(catalog, schema, table)),
            (
                Action::InfoSchema,
                queries::info_schema_columns(catalog, schema, table),
            ),
            (
                Action::ShowStats,
                queries::show_stats(catalog, schema, table),
            ),
            (Action::Count, queries::count(catalog, schema, table)),
            (Action::Sample, queries::sample(catalog, schema, table)),
            (
                Action::Partitions,
                queries::partitions(catalog, schema, table),
            ),
            (Action::Schema, queries::describe(catalog, schema, table)),
        ];

        for (action, expected) in cases {
            assert_eq!(action.build_query(catalog, schema, table), expected);
        }
    }

    fn vcol(name: &str, data_type: &str) -> VerticalColumn {
        VerticalColumn {
            index: 0,
            name: name.to_string(),
            data_type: data_type.to_string(),
            key_meta: String::new(),
            description: String::new(),
        }
    }

    #[test]
    fn safe_select_columns_returns_empty_when_no_schema_cached() {
        assert!(safe_select_columns(&[]).is_empty());
    }

    #[test]
    fn safe_select_columns_excludes_map_and_row_typed_columns() {
        let cols = vec![
            vcol("event_type", "varchar"),
            vcol("policy_id", "map(varchar, varchar)"),
            vcol("nested", "row(a varchar, b bigint)"),
            vcol("timestamp", "bigint"),
        ];
        assert_eq!(
            safe_select_columns(&cols),
            vec!["event_type".to_string(), "timestamp".to_string()]
        );
    }

    #[test]
    fn safe_select_columns_falls_back_to_select_star_when_all_columns_unsafe() {
        let cols = vec![vcol("policy_id", "map(varchar, varchar)")];
        assert!(safe_select_columns(&cols).is_empty());
    }
}
