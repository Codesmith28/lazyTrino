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

//! `r` reloads the view on screen from Trino: it drops every cached query
//! result, then re-runs what the screen shows.

use crate::app::{ACTIONS, Action, ActionState, App, Screen};
use crate::tui::handler::Command;

/// Whether `r` refreshes the current screen.
pub fn applies(app: &App) -> bool {
    matches!(
        app.screen,
        Screen::Catalog(_) | Screen::Schema(_) | Screen::Table(_) | Screen::Actions(_)
    )
}

pub fn refresh_current_view(app: &mut App) -> Option<Command> {
    if let Some(client) = &app.trino_client {
        client.clear_cache();
    }
    match &mut app.screen {
        Screen::Catalog(_) => Some(Command::FetchCatalogs),
        Screen::Schema(s) => Some(Command::FetchSchemas {
            catalog: s.catalog.clone(),
        }),
        Screen::Table(s) => Some(Command::FetchTables {
            catalog: s.catalog.clone(),
            schema: s.schema.clone(),
        }),
        Screen::Actions(s) => Some(refresh_action(s)),
        Screen::Connect(_) | Screen::Help => None,
    }
}

/// Reloads the table's DDL, columns and partition layout, plus whatever the
/// main viewer shows: the current partition level, or the rows of the query
/// behind the grid.
fn refresh_action(s: &mut ActionState) -> Command {
    let action = ACTIONS.get(s.selected).map(|(_, _, a)| *a);
    let (catalog, schema, table) = (s.catalog.clone(), s.schema.clone(), s.table.clone());
    let mut commands = vec![Command::FetchTableMetadata {
        catalog: catalog.clone(),
        schema: schema.clone(),
        table: table.clone(),
    }];

    if let Some(dd) = s.drilldown.as_mut() {
        // Counts are fetched again along with the level.
        dd.counts_cache.clear();
        if let Some(column) = dd.next_column().map(str::to_string) {
            commands.push(Command::FetchPartitionLevel {
                catalog,
                schema,
                table,
                filters: dd.path.clone(),
                column,
            });
            return Command::Batch(commands);
        }
    }

    if action == Some(Action::TableDDL) {
        // Rebuilt from the reloaded DDL.
        s.results = None;
    } else if let Some(res) = s.results.as_mut() {
        res.total_count = None;
        res.table_count = None;
        commands.push(Command::ExecuteQuery {
            query: res.query_buffer.clone(),
            is_paginated: res.is_paginated,
            catalog,
            schema,
            table,
            filters: res.filters.clone(),
            fresh: action.is_some_and(Action::is_metadata),
        });
    }
    Command::Batch(commands)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{ActionState, CatalogState, ResultsState, SchemaState, TableState};
    use crate::config::ConnectionConfig;
    use crate::trino::client::TrinoClient;
    use mockito::{Matcher, Server};
    use serde_json::json;

    fn app() -> App {
        App::new(
            ConnectionConfig {
                url: "http://localhost:8080".to_string(),
                user: "test".to_string(),
                password: String::new(),
                query_cache_limit: crate::config::DEFAULT_QUERY_CACHE_LIMIT,
            },
            false,
        )
    }

    fn batch(cmd: Option<Command>) -> Vec<Command> {
        let Some(Command::Batch(commands)) = cmd else {
            panic!("expected a batch");
        };
        commands
    }

    #[test]
    fn lists_reload_themselves() {
        let mut app = app();
        app.screen = Screen::Catalog(CatalogState {
            items: vec!["datalake".to_string()],
            selected: 0,
        });
        assert!(matches!(
            refresh_current_view(&mut app),
            Some(Command::FetchCatalogs)
        ));

        app.screen = Screen::Schema(SchemaState {
            catalog: "datalake".to_string(),
            items: vec!["tenant".to_string()],
            selected: 0,
        });
        let Some(Command::FetchSchemas { catalog }) = refresh_current_view(&mut app) else {
            panic!("expected FetchSchemas");
        };
        assert_eq!(catalog, "datalake");

        app.screen = Screen::Table(TableState {
            catalog: "datalake".to_string(),
            schema: "tenant".to_string(),
            items: vec!["events".to_string()],
            selected: 0,
        });
        let Some(Command::FetchTables { catalog, schema }) = refresh_current_view(&mut app) else {
            panic!("expected FetchTables");
        };
        assert_eq!((catalog.as_str(), schema.as_str()), ("datalake", "tenant"));
    }

    #[test]
    fn a_table_view_reruns_the_query_behind_the_grid_and_refetches_its_total() {
        let mut app = app();
        app.screen = Screen::Actions(ActionState {
            catalog: "datalake".to_string(),
            schema: "tenant".to_string(),
            table: "events".to_string(),
            selected: 0,
            results: Some(ResultsState {
                query_buffer: "SELECT * FROM events LIMIT 100".to_string(),
                is_paginated: true,
                total_count: Some(42),
                ..Default::default()
            }),
            ..Default::default()
        });

        let commands = batch(refresh_current_view(&mut app));
        assert!(matches!(commands[0], Command::FetchTableMetadata { .. }));
        let Command::ExecuteQuery {
            query,
            is_paginated,
            fresh,
            ..
        } = &commands[1]
        else {
            panic!("expected ExecuteQuery");
        };
        assert_eq!(query, "SELECT * FROM events LIMIT 100");
        assert!(is_paginated);
        assert!(!fresh, "table rows still go through the query cache");

        let Screen::Actions(s) = &app.screen else {
            panic!("still on the action view");
        };
        assert_eq!(s.results.as_ref().unwrap().total_count, None);
    }

    #[test]
    fn a_metadata_view_reruns_its_query_uncached() {
        let mut app = app();
        app.screen = Screen::Actions(ActionState {
            selected: Action::ShowStats.index(),
            results: Some(ResultsState {
                query_buffer: "SHOW STATS FOR events".to_string(),
                ..Default::default()
            }),
            ..Default::default()
        });

        let commands = batch(refresh_current_view(&mut app));
        let Command::ExecuteQuery { fresh, .. } = &commands[1] else {
            panic!("expected ExecuteQuery");
        };
        assert!(fresh);
    }

    #[test]
    fn the_ddl_view_is_rebuilt_from_the_reloaded_ddl() {
        let mut app = app();
        app.screen = Screen::Actions(ActionState {
            selected: Action::TableDDL.index(),
            results: Some(ResultsState::default()),
            ..Default::default()
        });

        let commands = batch(refresh_current_view(&mut app));
        assert_eq!(commands.len(), 1, "only the metadata reload");
        let Screen::Actions(s) = &app.screen else {
            panic!("still on the action view");
        };
        assert!(s.results.is_none());
    }

    #[tokio::test]
    async fn refreshing_drops_the_cached_results() {
        let mut server = Server::new_async().await;
        server
            .mock("POST", "/v1/statement")
            .match_body(Matcher::Exact("SELECT 1".to_string()))
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(json!({"columns": [{"name": "x"}], "data": [[1]]}).to_string())
            .create_async()
            .await;
        let client = TrinoClient::new(&server.url(), "test_user").unwrap();
        client.execute("SELECT 1").await.unwrap();
        assert!(client.is_cached("SELECT 1"));

        let mut app = app();
        app.trino_client = Some(client.clone());
        app.screen = Screen::Catalog(CatalogState {
            items: Vec::new(),
            selected: 0,
        });
        refresh_current_view(&mut app);
        assert!(!client.is_cached("SELECT 1"));
    }
}
