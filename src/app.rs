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

#[derive(Clone)]
pub struct ActionState {
    pub catalog: String,
    pub schema: String,
    pub table: String,
    pub selected: usize,
    pub query_buffer: String,
    pub query_cursor: usize,
    pub results: Option<ResultsState>,
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
    Describe,
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
    ('d', "Describe", Action::Describe),
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
            Action::Describe => crate::trino::queries::describe(catalog, schema, table),
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
            (Action::Describe, queries::describe(catalog, schema, table)),
            (
                Action::TableDDL,
                queries::show_create(catalog, schema, table),
            ),
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
}
