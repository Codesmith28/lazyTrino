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
    pub query: String,
    pub columns: Vec<String>,
    pub rows: Vec<Vec<String>>,
    pub scroll_v: usize,
    pub scroll_h: usize,
    pub loading: bool,
    #[allow(dead_code)]
    pub error: Option<String>,
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
}

pub enum Action {
    Describe,
    TableDDL,
    InfoSchema,
    ShowStats,
    Count,
    Preview,
    Partitions,
}

pub const ACTIONS: &[(char, &str, Action)] = &[
    ('d', "Describe", Action::Describe),
    ('c', "Table DDL", Action::TableDDL),
    ('i', "Info Schema", Action::InfoSchema),
    ('s', "Show Stats", Action::ShowStats),
    ('n', "Count", Action::Count),
    ('p', "Preview", Action::Preview),
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
            Action::Preview => crate::trino::queries::preview(catalog, schema, table),
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
    pub query_inspector_scroll: usize,
}

impl App {
    pub fn new(config: ConnectionConfig, auto_connect: bool) -> Self {
        Self {
            screen: Screen::Connect(ConnectState {
                url: config.url.clone(),
                user: config.user.clone(),
                password: config.password.clone(),
                ..Default::default()
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
            main_panel_pct: 65,
            control_panel_split_pct: 50,
            is_dragging_resizer: false,
            is_dragging_v_resizer: false,
            query_inspector_scroll: 0,
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

