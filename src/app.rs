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
    #[allow(dead_code)]
    pub index: usize,
    pub name: String,
    pub data_type: String,
    pub key_meta: String,
    pub description: String,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ActivePanel {
    MainViewer,
    PartitionTree,
    SchemaInspector,
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
        Self {
            url: "http://localhost:57574".to_string(),
            user: "sarthak".to_string(),
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
    #[allow(dead_code)]
    pub scroll: usize,
}

#[derive(Clone)]
pub struct SchemaState {
    pub catalog: String,
    pub items: Vec<String>,
    pub selected: usize,
    #[allow(dead_code)]
    pub scroll: usize,
}

#[derive(Clone)]
pub struct TableState {
    pub catalog: String,
    pub schema: String,
    pub items: Vec<String>,
    pub selected: usize,
    #[allow(dead_code)]
    pub scroll: usize,
}

#[derive(Clone)]
pub struct ActionState {
    pub catalog: String,
    pub schema: String,
    pub table: String,
    pub selected: usize,
}

#[derive(Clone)]
pub struct ResultsState {
    #[allow(dead_code)]
    pub query: String,
    pub query_buffer: String,
    pub query_cursor: usize,
    pub columns: Vec<String>,
    pub rows: Vec<Vec<String>>,
    pub scroll_v: usize,
    pub scroll_h: usize,
    pub loading: bool,
    #[allow(dead_code)]
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
        if let Some(anchor) = self.selection_anchor {
            if anchor != self.query_cursor {
                let start = anchor.min(self.query_cursor);
                let end = anchor.max(self.query_cursor);
                return Some((start, end));
            }
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

#[derive(Clone)]
pub enum Screen {
    Connect(ConnectState),
    Catalog(CatalogState),
    Schema(SchemaState),
    Table(TableState),
    Actions(ActionState),
    Results(ResultsState),
    Help,
}

pub enum Mode {
    Normal,
    Leader {
        #[allow(dead_code)]
        keys: String,
    },
    Search,
    QueryInput,
}

pub enum Action {
    Describe,
    TableDDL,
    InfoSchema,
    ShowStats,
    Count,
    Sample,
    TableView,
    Partitions,
}

pub const ACTIONS: &[(char, &str, Action)] = &[
    ('d', "Describe", Action::Describe),
    ('c', "Table DDL", Action::TableDDL),
    ('i', "Info Schema", Action::InfoSchema),
    ('s', "Show Stats", Action::ShowStats),
    ('n', "Count", Action::Count),
    ('p', "Sample Mode (20 rows)", Action::Sample),
    ('v', "Table View Mode (Infinite Scroll)", Action::TableView),
    ('P', "Partitions", Action::Partitions),
];

impl Action {
    pub fn build_query(&self, catalog: &str, schema: &str, table: &str) -> String {
        match self {
            Action::Describe => crate::trino::queries::describe(catalog, schema, table),
            Action::TableDDL => crate::trino::queries::show_create(catalog, schema, table),
            Action::InfoSchema => crate::trino::queries::info_schema_columns(catalog, schema, table),
            Action::ShowStats => crate::trino::queries::show_stats(catalog, schema, table),
            Action::Count => crate::trino::queries::count(catalog, schema, table),
            Action::Sample => crate::trino::queries::sample(catalog, schema, table),
            Action::TableView => crate::trino::queries::page_query(catalog, schema, table, 0, 100),
            Action::Partitions => crate::trino::queries::partitions(catalog, schema, table),
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
    #[allow(dead_code)]
    pub active_table_name: Option<String>,
    pub auto_connect: bool,
    pub main_panel_pct: u16,
    pub control_panel_split_pct: u16,
    pub is_dragging_resizer: bool,
    pub is_dragging_v_resizer: bool,
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
            active_table_name: None,
            auto_connect,
            main_panel_pct: 60,
            control_panel_split_pct: 40,
            is_dragging_resizer: false,
            is_dragging_v_resizer: false,
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

    pub fn action_for(&self, key: char) -> Option<&'static Action> {
        ACTIONS
            .iter()
            .find(|(k, _, _)| *k == key)
            .map(|(_, _, a)| a)
    }
}

