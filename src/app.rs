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
    Leader { keys: String },
    Search,
}

pub enum Action {
    Describe,
    ShowCreate,
    InfoSchema,
    ShowStats,
    Count,
    Preview,
    Partitions,
    Files,
    Properties,
    Snapshots,
    History,
    MetadataLog,
}

pub const ACTIONS: &[(char, &str, Action)] = &[
    ('d', "Describe", Action::Describe),
    ('c', "Show Create", Action::ShowCreate),
    ('i', "Info Schema", Action::InfoSchema),
    ('s', "Show Stats", Action::ShowStats),
    ('n', "Count", Action::Count),
    ('p', "Preview", Action::Preview),
    ('P', "Partitions", Action::Partitions),
    ('f', "Files", Action::Files),
    ('r', "Properties", Action::Properties),
    ('S', "Snapshots", Action::Snapshots),
    ('h', "History", Action::History),
    ('m', "Metadata Log", Action::MetadataLog),
];

impl Action {
    pub fn build_query(&self, catalog: &str, schema: &str, table: &str) -> String {
        match self {
            Action::Describe => crate::trino::queries::describe(catalog, schema, table),
            Action::ShowCreate => crate::trino::queries::show_create(catalog, schema, table),
            Action::InfoSchema => crate::trino::queries::info_schema_columns(catalog, schema, table),
            Action::ShowStats => crate::trino::queries::show_stats(catalog, schema, table),
            Action::Count => crate::trino::queries::count(catalog, schema, table),
            Action::Preview => crate::trino::queries::preview(catalog, schema, table),
            Action::Partitions => crate::trino::queries::partitions(catalog, schema, table),
            Action::Files => crate::trino::queries::files(catalog, schema, table),
            Action::Properties => crate::trino::queries::properties(catalog, schema, table),
            Action::Snapshots => crate::trino::queries::snapshots(catalog, schema, table),
            Action::History => crate::trino::queries::history(catalog, schema, table),
            Action::MetadataLog => crate::trino::queries::metadata_log(catalog, schema, table),
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
}

impl App {
    pub fn new(config: ConnectionConfig) -> Self {
        Self {
            screen: Screen::Connect(ConnectState::default()),
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
        }
    }

    pub fn action_for(&self, key: char) -> Option<&'static Action> {
        ACTIONS
            .iter()
            .find(|(k, _, _)| *k == key)
            .map(|(_, _, a)| a)
    }
}
