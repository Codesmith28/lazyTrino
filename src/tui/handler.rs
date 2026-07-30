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

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseEvent, MouseEventKind};
use tracing::{error, info, warn};

use crate::app::*;
use crate::trino::client::TrinoClient;
use crate::trino::queries;

pub enum Command {
    Connect {
        url: String,
        user: String,
        password: String,
    },
    FetchSchemas {
        catalog: String,
    },
    FetchTables {
        catalog: String,
        schema: String,
    },
    FetchTableMetadata {
        catalog: String,
        schema: String,
        table: String,
    },
    ExecuteQuery {
        query: String,
    },
}

fn extract_list_labels(screen: &Screen) -> Option<Vec<String>> {
    match screen {
        Screen::Catalog(s) => Some(s.items.iter().map(|x| x.trim().to_string()).collect()),
        Screen::Schema(s) => Some(s.items.iter().map(|x| x.trim().to_string()).collect()),
        Screen::Table(s) => Some(s.items.iter().map(|x| x.trim().to_string()).collect()),
        Screen::Actions(_) => Some(ACTIONS.iter().map(|(_, l, _)| l.to_string()).collect()),
        _ => None,
    }
}

fn mod_list_selected(screen: &mut Screen, new_selected: usize) {
    match screen {
        Screen::Catalog(s) => s.selected = new_selected,
        Screen::Schema(s) => s.selected = new_selected,
        Screen::Table(s) => s.selected = new_selected,
        Screen::Actions(s) => s.selected = new_selected,
        _ => {}
    }
}

fn get_selected(screen: &Screen) -> Option<usize> {
    match screen {
        Screen::Catalog(s) => Some(s.selected),
        Screen::Schema(s) => Some(s.selected),
        Screen::Table(s) => Some(s.selected),
        Screen::Actions(s) => Some(s.selected),
        _ => None,
    }
}

fn update_number_buffer(app: &mut App, ch: char) {
    if ch.is_ascii_digit() {
        let mut buf = app.number_buffer.clone();
        buf.push(ch);
        let num: usize = buf.parse().unwrap_or(0);
        if let Some(items) = extract_list_labels(&app.screen) {
            if num <= items.len() && num > 0 {
                app.number_buffer = buf;
            } else {
                app.number_buffer = ch.to_string();
            }
        }
    }
}

fn jump_to_number(app: &mut App) {
    if app.number_buffer.is_empty() {
        return;
    }
    let num: usize = app.number_buffer.parse().unwrap_or(1);
    if let Some(items) = extract_list_labels(&app.screen) {
        if num > 0 && num <= items.len() {
            mod_list_selected(&mut app.screen, num - 1);
        }
    }
    app.number_buffer.clear();
}

fn handle_leader_mode(app: &mut App, key: KeyEvent) -> Option<Command> {
    match key.code {
        KeyCode::Char(c) => {
            let (cat, schema, table) = match &app.screen {
                Screen::Table(t) => {
                    if t.items.is_empty() {
                        return None;
                    }
                    (t.catalog.clone(), t.schema.clone(), t.items[t.selected].clone())
                }
                Screen::Actions(a) => (a.catalog.clone(), a.schema.clone(), a.table.clone()),
                _ => return None,
            };
            if let Some(action) = app.action_for(c) {
                let query = action.build_query(&cat, &schema, &table);
                info!(%query, "Leader action triggered");
                app.mode = Mode::Normal;
                return Some(Command::ExecuteQuery { query });
            }
            warn!(char = %c, "Unknown leader key");
            app.mode = Mode::Normal;
            None
        }
        KeyCode::Esc => {
            app.mode = Mode::Normal;
            None
        }
        _ => None,
    }
}

fn handle_search_mode(app: &mut App, key: KeyEvent) -> Option<Command> {
    match key.code {
        KeyCode::Char(c) if !c.is_ascii_control() => {
            app.search_query.push(c);
        }
        KeyCode::Backspace => {
            app.search_query.pop();
        }
        KeyCode::Enter | KeyCode::Esc => {
            if key.code == KeyCode::Esc {
                app.search_query.clear();
            }
            app.mode = Mode::Normal;
            app.active_panel = ActivePanel::MainViewer;
        }
        _ => {}
    }
    None
}

fn go_back(app: &mut App) {
    let next = match &app.screen {
        Screen::Help => app.prev_screen.take().map(|p| *p),
        Screen::Catalog(_) => {
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
        Screen::Schema(s) => {
            let cat = s.catalog.clone();
            let catalogs = app.catalogs.clone();
            let idx = catalogs.iter().position(|c| *c == cat).unwrap_or(0);
            Some(Screen::Catalog(CatalogState {
                items: catalogs,
                selected: idx,
                scroll: idx.saturating_sub(5),
            }))
        }
        Screen::Table(s) => {
            let cat = s.catalog.clone();
            let schema_name = s.schema.clone();
            let schemas = app.schemas.get(&cat).cloned().unwrap_or_default();
            let idx = schemas.iter().position(|c| c.trim() == schema_name.trim()).unwrap_or(0);
            Some(Screen::Schema(SchemaState {
                catalog: cat,
                items: schemas,
                selected: idx,
                scroll: idx.saturating_sub(5),
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
            let idx = tables.iter().position(|t| t.trim() == s.table.trim()).unwrap_or(0);
            Some(Screen::Table(TableState {
                catalog: cat,
                schema: schema_name,
                items: tables,
                selected: idx,
                scroll: idx.saturating_sub(5),
            }))
        }
        Screen::Connect(_) => None,
        Screen::Results(_) => app.prev_screen.take().map(|p| *p).or_else(|| {
            if let Some(_cat) = app.catalogs.first() {
                Some(Screen::Catalog(CatalogState {
                    items: app.catalogs.clone(),
                    selected: 0,
                    scroll: 0,
                }))
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
        }),
    };
    if let Some(next) = next {
        info!("Navigating back");
        app.screen = next;
    }
}

pub fn handle_mouse_sync(app: &mut App, mouse: MouseEvent) {
    match mouse.kind {
        MouseEventKind::ScrollDown => {
            if let Screen::Results(ref mut state) = app.screen {
                state.scroll_v = (state.scroll_v + 1).min(state.rows.len().saturating_sub(1));
            } else if matches!(app.screen, Screen::Table(_) | Screen::Actions(_)) {
                app.partition_scroll += 1;
                app.schema_scroll += 1;
            } else if let Some(items) = extract_list_labels(&app.screen) {
                if let Some(s) = get_selected(&app.screen) {
                    mod_list_selected(&mut app.screen, (s + 1).min(items.len().saturating_sub(1)));
                }
            }
        }
        MouseEventKind::ScrollUp => {
            if let Screen::Results(ref mut state) = app.screen {
                state.scroll_v = state.scroll_v.saturating_sub(1);
            } else if matches!(app.screen, Screen::Table(_) | Screen::Actions(_)) {
                app.partition_scroll = app.partition_scroll.saturating_sub(1);
                app.schema_scroll = app.schema_scroll.saturating_sub(1);
            } else if let Some(s) = get_selected(&app.screen) {
                mod_list_selected(&mut app.screen, s.saturating_sub(1));
            }
        }
        _ => {}
    }
}

pub fn handle_key_sync(app: &mut App, key: KeyEvent) -> Option<Command> {
    if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
        info!("User pressed Ctrl+C, quitting...");
        app.should_quit = true;
        return None;
    }

    match app.mode {
        Mode::Search => return handle_search_mode(app, key),
        Mode::Leader { .. } => return handle_leader_mode(app, key),
        Mode::Normal => {}
    }

    let is_alt = key.modifiers.contains(KeyModifiers::ALT);

    if is_alt {
        if let Screen::Results(state) = &mut app.screen {
            match key.code {
                KeyCode::Char('j') | KeyCode::Down => {
                    if !state.rows.is_empty() {
                        state.scroll_v = (state.scroll_v + 1).min(state.rows.len().saturating_sub(1));
                    }
                    return None;
                }
                KeyCode::Char('k') | KeyCode::Up => {
                    state.scroll_v = state.scroll_v.saturating_sub(1);
                    return None;
                }
                KeyCode::Char('h') | KeyCode::Left => {
                    state.scroll_h = state.scroll_h.saturating_sub(1);
                    return None;
                }
                KeyCode::Char('l') | KeyCode::Right => {
                    state.scroll_h = (state.scroll_h + 1).min(state.columns.len().saturating_sub(1));
                    return None;
                }
                _ => {}
            }
        } else if matches!(app.screen, Screen::Table(_) | Screen::Actions(_)) {
            let is_shift = key.modifiers.contains(KeyModifiers::SHIFT);
            match key.code {
                KeyCode::Char('j') | KeyCode::Char('J') | KeyCode::Down => {
                    if is_shift {
                        app.partition_scroll += 1;
                    } else {
                        app.schema_scroll += 1;
                    }
                    return None;
                }
                KeyCode::Char('k') | KeyCode::Char('K') | KeyCode::Up => {
                    if is_shift {
                        app.partition_scroll = app.partition_scroll.saturating_sub(1);
                    } else {
                        app.schema_scroll = app.schema_scroll.saturating_sub(1);
                    }
                    return None;
                }
                _ => {}
            }
        }
    }

    match key.code {
        KeyCode::Char('?') => {
            info!("Showing help");
            app.prev_screen = Some(Box::new(app.screen.clone()));
            app.screen = Screen::Help;
            return None;
        }
        KeyCode::Char('/') => {
            info!("Search mode entered");
            app.search_query.clear();
            app.mode = Mode::Search;
            app.active_panel = ActivePanel::SearchBar;
            return None;
        }
        KeyCode::Esc => {
            app.number_buffer.clear();
            go_back(app);
            return None;
        }
        _ => {}
    }

    if key.code == KeyCode::Enter && !app.number_buffer.is_empty() {
        jump_to_number(app);
        return None;
    }

    if let KeyCode::Char(c) = key.code {
        if c.is_ascii_digit() {
            update_number_buffer(app, c);
            return None;
        }
    }

    match key.code {
        KeyCode::Char('j') | KeyCode::Down => {
            if let Screen::Results(state) = &mut app.screen {
                if !state.rows.is_empty() {
                    state.scroll_v = (state.scroll_v + 1).min(state.rows.len().saturating_sub(1));
                }
            } else if let Some(items) = extract_list_labels(&app.screen) {
                if !items.is_empty() {
                    if let Some(s) = get_selected(&app.screen) {
                        mod_list_selected(&mut app.screen, (s + 1).min(items.len() - 1));
                    }
                }
            }
            return None;
        }
        KeyCode::Char('k') | KeyCode::Up => {
            if let Screen::Results(state) = &mut app.screen {
                state.scroll_v = state.scroll_v.saturating_sub(1);
            } else if let Some(s) = get_selected(&app.screen) {
                mod_list_selected(&mut app.screen, s.saturating_sub(1));
            }
            return None;
        }
        KeyCode::Char('h') | KeyCode::Left => {
            app.number_buffer.clear();
            go_back(app);
            return None;
        }
        KeyCode::Char('l') | KeyCode::Right => {
            if let Screen::Results(state) = &mut app.screen {
                if !state.columns.is_empty() {
                    state.scroll_h = (state.scroll_h + 1).min(state.columns.len().saturating_sub(1));
                }
                return None;
            } else {
                return select_current_item(app);
            }
        }
        KeyCode::Enter => {
            if matches!(app.screen, Screen::Results(_)) {
                return None;
            } else {
                return select_current_item(app);
            }
        }
        KeyCode::Char('g') => {
            if let Screen::Results(state) = &mut app.screen {
                state.scroll_v = 0;
            } else {
                mod_list_selected(&mut app.screen, 0);
            }
            return None;
        }
        KeyCode::Char('G') => {
            if let Screen::Results(state) = &mut app.screen {
                state.scroll_v = state.rows.len().saturating_sub(1);
            } else if let Some(items) = extract_list_labels(&app.screen) {
                if !items.is_empty() {
                    mod_list_selected(&mut app.screen, items.len() - 1);
                }
            }
            return None;
        }
        KeyCode::Char(' ') => {
            if matches!(app.screen, Screen::Table(_) | Screen::Actions(_)) {
                info!("Leader mode entered");
                app.mode = Mode::Leader { keys: String::new() };
            }
            return None;
        }
        _ => {}
    }

    match &app.screen {
        Screen::Connect(_) => connect_keys(app, key),
        Screen::Catalog(_) => catalog_keys(app, key),
        Screen::Schema(_) => schema_keys(app, key),
        Screen::Table(_) => table_keys(app, key),
        Screen::Actions(_) => actions_keys(app, key),
        Screen::Results(_) => results_keys(app, key),
        Screen::Help => None,
    }
}

fn select_current_item(app: &mut App) -> Option<Command> {
    match &app.screen {
        Screen::Catalog(s) => {
            if s.items.is_empty() {
                return None;
            }
            let catalog = s.items[s.selected].trim().to_string();
            if app.schemas.contains_key(&catalog) {
                let items = app.schemas[&catalog].iter().map(|x| x.trim().to_string()).collect();
                app.screen = Screen::Schema(SchemaState {
                    catalog,
                    items,
                    selected: 0,
                    scroll: 0,
                });
                None
            } else {
                Some(Command::FetchSchemas { catalog })
            }
        }
        Screen::Schema(s) => {
            if s.items.is_empty() {
                return None;
            }
            let schema = s.items[s.selected].trim().to_string();
            let catalog = s.catalog.trim().to_string();
            if app.tables.contains_key(&(catalog.clone(), schema.clone())) {
                let items = app.tables[&(catalog.clone(), schema.clone())]
                    .iter()
                    .map(|x| x.trim().to_string())
                    .collect();
                app.screen = Screen::Table(TableState {
                    catalog,
                    schema,
                    items,
                    selected: 0,
                    scroll: 0,
                });
                None
            } else {
                Some(Command::FetchTables { catalog, schema })
            }
        }
        Screen::Table(s) => {
            if s.items.is_empty() {
                return None;
            }
            let catalog = s.catalog.clone();
            let schema = s.schema.clone();
            let table = s.items[s.selected].trim().to_string();
            app.screen = Screen::Actions(ActionState {
                catalog: catalog.clone(),
                schema: schema.clone(),
                table: table.clone(),
                selected: 0,
            });
            Some(Command::FetchTableMetadata {
                catalog,
                schema,
                table,
            })
        }
        Screen::Actions(s) => {
            if ACTIONS.is_empty() {
                return None;
            }
            let (_, _, action) = &ACTIONS[s.selected];
            let query = action.build_query(&s.catalog, &s.schema, &s.table);
            Some(Command::ExecuteQuery { query })
        }
        _ => None,
    }
}

fn connect_keys(app: &mut App, key: KeyEvent) -> Option<Command> {
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
            0 => { state.url.pop(); }
            1 => { state.user.pop(); }
            2 => { state.password.pop(); }
            _ => {},
        },
        KeyCode::Char(c) => match state.focused {
            0 => state.url.push(c),
            1 => state.user.push(c),
            2 => state.password.push(c),
            _ => {},
        },
        KeyCode::Enter => {
            if !state.url.is_empty() && !state.user.is_empty() {
                let url = state.url.clone();
                let user = state.user.clone();
                let password = state.password.clone();
                state.loading = true;
                return Some(Command::Connect { url, user, password });
            }
        }
        _ => {}
    }
    None
}

fn catalog_keys(app: &mut App, key: KeyEvent) -> Option<Command> {
    if matches!(key.code, KeyCode::Enter | KeyCode::Char('l')) {
        select_current_item(app)
    } else {
        None
    }
}

fn schema_keys(app: &mut App, key: KeyEvent) -> Option<Command> {
    if matches!(key.code, KeyCode::Enter | KeyCode::Char('l')) {
        select_current_item(app)
    } else {
        None
    }
}

fn table_keys(app: &mut App, key: KeyEvent) -> Option<Command> {
    if matches!(key.code, KeyCode::Enter | KeyCode::Char('l')) {
        select_current_item(app)
    } else {
        None
    }
}

fn actions_keys(app: &mut App, key: KeyEvent) -> Option<Command> {
    match key.code {
        KeyCode::Enter | KeyCode::Char('l') => select_current_item(app),
        KeyCode::Char(c) => {
            if let Screen::Actions(s) = &app.screen {
                if let Some(action) = app.action_for(c) {
                    let query = action.build_query(&s.catalog, &s.schema, &s.table);
                    return Some(Command::ExecuteQuery { query });
                }
            }
            None
        }
        _ => None,
    }
}

fn results_keys(app: &mut App, key: KeyEvent) -> Option<Command> {
    if let Screen::Results(state) = &mut app.screen {
        match key.code {
            KeyCode::Char('j') | KeyCode::Down => {
                if !state.rows.is_empty() {
                    state.scroll_v = (state.scroll_v + 1).min(state.rows.len().saturating_sub(1));
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                state.scroll_v = state.scroll_v.saturating_sub(1);
            }
            KeyCode::Char('g') => state.scroll_v = 0,
            KeyCode::Char('G') => state.scroll_v = state.rows.len().saturating_sub(1),
            KeyCode::Char('l') | KeyCode::Right => {
                state.scroll_h = state.scroll_h.saturating_add(3);
            }
            KeyCode::Char('h') | KeyCode::Left => {
                state.scroll_h = state.scroll_h.saturating_sub(3);
            }
            _ => {}
        }
    }
    None
}

pub async fn execute_command(app: &mut App, cmd: Command) {
    match cmd {
        Command::Connect { url, user, password } => {
            let sql = queries::show_catalogs();
            let log_id = app.add_query_log(sql.clone());
            let client = TrinoClient::new(&url, &user);
            app.loading = true;
            info!("Fetching catalogs...");
            match client.fetch_catalogs().await {
                Ok(catalogs) => {
                    info!(count = catalogs.len(), "Catalogs fetched");
                    app.complete_query_log_success(log_id, 15, catalogs.len());
                    app.config.url = url;
                    app.config.user = user;
                    app.config.password = password;
                    app.trino_client = Some(client);
                    app.catalogs = catalogs.iter().map(|c| c.trim().to_string()).collect();
                    app.screen = Screen::Catalog(CatalogState {
                        items: app.catalogs.clone(),
                        selected: 0,
                        scroll: 0,
                    });
                }
                Err(e) => {
                    error!(error = %e, "Failed to connect");
                    app.complete_query_log_error(log_id, e.to_string());
                    if let Screen::Connect(s) = &mut app.screen {
                        s.loading = false;
                        s.error = Some(format!("Connection failed: {e}"));
                    }
                }
            }
            app.loading = false;
        }
        Command::FetchSchemas { catalog } => {
            let client = app.trino_client.clone().expect("TrinoClient not initialized");
            let sql = queries::show_schemas(&catalog);
            let log_id = app.add_query_log(sql.clone());
            app.loading = true;
            info!(%catalog, "Fetching schemas");
            match client.fetch_schemas(&catalog).await {
                Ok(schemas) => {
                    let trimmed: Vec<String> = schemas.iter().map(|s| s.trim().to_string()).collect();
                    app.complete_query_log_success(log_id, 25, trimmed.len());
                    app.schemas.insert(catalog.clone(), trimmed.clone());
                    app.screen = Screen::Schema(SchemaState {
                        catalog: catalog.clone(),
                        items: trimmed,
                        selected: 0,
                        scroll: 0,
                    });
                }
                Err(e) => {
                    error!(%catalog, error = %e, "Failed to fetch schemas");
                    app.complete_query_log_error(log_id, e.to_string());
                }
            }
            app.loading = false;
        }
        Command::FetchTables { catalog, schema } => {
            let client = app.trino_client.clone().expect("TrinoClient not initialized");
            let sql = queries::show_tables(&catalog, &schema);
            let log_id = app.add_query_log(sql.clone());
            app.loading = true;
            info!(%catalog, %schema, "Fetching tables");
            match client.fetch_tables(&catalog, &schema).await {
                Ok(tables) => {
                    let trimmed: Vec<String> = tables.iter().map(|t| t.trim().to_string()).collect();
                    app.complete_query_log_success(log_id, 35, trimmed.len());
                    app.tables.insert((catalog.clone(), schema.clone()), trimmed.clone());
                    app.screen = Screen::Table(TableState {
                        catalog: catalog.clone(),
                        schema: schema.clone(),
                        items: trimmed,
                        selected: 0,
                        scroll: 0,
                    });
                }
                Err(e) => {
                    error!(%catalog, %schema, error = %e, "Failed to fetch tables");
                    app.complete_query_log_error(log_id, e.to_string());
                }
            }
            app.loading = false;
        }
        Command::FetchTableMetadata { catalog, schema, table } => {
            let client = match app.trino_client.clone() {
                Some(c) => c,
                None => return,
            };

            let part_query = queries::partitions(&catalog, &schema, &table);
            let log_id = app.add_query_log(part_query.clone());
            match client.execute(&part_query).await {
                Ok(res) => {
                    app.complete_query_log_success(log_id, res.duration_ms, res.data.len());
                    let raw_lines: Vec<String> = res.data.into_iter().map(|r| r.join("/")).collect();
                    app.partition_tree_lines = crate::tui::screens::partition_tree::build_tree_lines(&raw_lines);
                }
                Err(_) => {
                    let show_part_query = queries::show_partitions(&catalog, &schema, &table);
                    let log_id2 = app.add_query_log(show_part_query.clone());
                    match client.execute(&show_part_query).await {
                        Ok(res2) => {
                            app.complete_query_log_success(log_id2, res2.duration_ms, res2.data.len());
                            let raw_lines: Vec<String> = res2.data.into_iter().map(|r| r.join("/")).collect();
                            app.partition_tree_lines = crate::tui::screens::partition_tree::build_tree_lines(&raw_lines);
                        }
                        Err(e) => {
                            app.complete_query_log_error(log_id2, e.to_string());
                            app.partition_tree_lines = crate::tui::screens::partition_tree::build_tree_lines(&[]);
                        }
                    }
                }
            }

            let desc_query = queries::info_schema_columns(&catalog, &schema, &table);
            let log_id3 = app.add_query_log(desc_query.clone());
            match client.execute(&desc_query).await {
                Ok(res) => {
                    app.complete_query_log_success(log_id3, res.duration_ms, res.data.len());
                    let cols: Vec<VerticalColumn> = res
                        .data
                        .iter()
                        .enumerate()
                        .map(|(idx, r)| {
                            let name = r.get(0).cloned().unwrap_or_default();
                            let dtype = r.get(1).cloned().unwrap_or_default();
                            let is_nullable = r.get(2).cloned().unwrap_or_default();
                            let comment = r.get(3).cloned().unwrap_or_default();

                            let key_meta = if name.starts_with("_hoodie") {
                                "Hudi Metadata".to_string()
                            } else if name.starts_with("$") || name.contains("iceberg") {
                                "Iceberg Meta".to_string()
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
                    app.vertical_schema_cols = cols;
                }
                Err(e) => {
                    app.complete_query_log_error(log_id3, e.to_string());
                }
            }
        }
        Command::ExecuteQuery { query } => {
            let client = app.trino_client.clone().expect("TrinoClient not initialized");
            info!(%query, "Executing query");
            let log_id = app.add_query_log(query.clone());
            app.loading = true;
            app.prev_screen = Some(Box::new(app.screen.clone()));
            app.screen = Screen::Results(ResultsState {
                query: query.clone(),
                columns: Vec::new(),
                rows: vec![vec!["Loading...".to_string()]],
                scroll_v: 0,
                scroll_h: 0,
                loading: true,
                error: None,
            });
            match client.execute(&query).await {
                Ok(results) => {
                    app.complete_query_log_success(log_id, results.duration_ms, results.data.len());
                    let cols: Vec<String> = results.columns.iter().map(|c| c.name.clone()).collect();
                    let rows = results.data;
                    info!(row_count = rows.len(), "Query completed");
                    app.screen = Screen::Results(ResultsState {
                        query,
                        columns: cols,
                        rows,
                        scroll_v: 0,
                        scroll_h: 0,
                        loading: false,
                        error: None,
                    });
                }
                Err(e) => {
                    error!(error = %e, "Query failed");
                    app.complete_query_log_error(log_id, e.to_string());
                    app.screen = Screen::Results(ResultsState {
                        query,
                        columns: Vec::new(),
                        rows: vec![vec![format!("Error: {e}")]],
                        scroll_v: 0,
                        scroll_h: 0,
                        loading: false,
                        error: Some(e.to_string()),
                    });
                }
            }
            app.loading = false;
        }
    }
}

