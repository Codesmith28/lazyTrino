use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::app::*;

use super::{Command, export};

pub(super) fn check_trigger_infinite_scroll(app: &mut App) -> Option<Command> {
    if let Screen::Actions(ref mut action_state) = app.screen
        && let Some(state) = action_state.results.as_mut()
        && state.is_paginated
        && !state.is_fetching_next_page
        && state.has_more_rows
        && state.scroll_v + 15 >= state.rows.len()
    {
        state.is_fetching_next_page = true;
        let offset = state.rows.len();
        return Some(Command::FetchNextPage {
            catalog: state.catalog.clone(),
            schema: state.schema.clone(),
            table: state.table.clone(),
            offset,
            limit: state.page_size,
        });
    }
    None
}

pub(super) fn extract_list_labels(screen: &Screen) -> Option<Vec<String>> {
    match screen {
        Screen::Catalog(s) => Some(s.items.iter().map(|x| x.trim().to_string()).collect()),
        Screen::Schema(s) => Some(s.items.iter().map(|x| x.trim().to_string()).collect()),
        Screen::Table(s) => Some(s.items.iter().map(|x| x.trim().to_string()).collect()),
        Screen::Actions(_) => Some(ACTIONS.iter().map(|(_, l, _)| l.to_string()).collect()),
        _ => None,
    }
}

pub(super) fn mod_list_selected(screen: &mut Screen, new_selected: usize) {
    match screen {
        Screen::Catalog(s) => s.selected = new_selected,
        Screen::Schema(s) => s.selected = new_selected,
        Screen::Table(s) => s.selected = new_selected,
        Screen::Actions(s) => s.selected = new_selected,
        _ => {}
    }
}

pub(super) fn get_selected(screen: &Screen) -> Option<usize> {
    match screen {
        Screen::Catalog(s) => Some(s.selected),
        Screen::Schema(s) => Some(s.selected),
        Screen::Table(s) => Some(s.selected),
        Screen::Actions(s) => Some(s.selected),
        _ => None,
    }
}

pub(super) fn update_number_buffer(app: &mut App, ch: char) {
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

pub(super) fn jump_to_number(app: &mut App) {
    if app.number_buffer.is_empty() {
        return;
    }
    let num: usize = app.number_buffer.parse().unwrap_or(1);
    if let Some(items) = extract_list_labels(&app.screen)
        && num > 0
        && num <= items.len()
    {
        mod_list_selected(&mut app.screen, num - 1);
    }
    app.number_buffer.clear();
}

pub(super) fn go_back(app: &mut App) {
    let logged_in = app.trino_client.is_some();
    app.main_panel_pct = 60;

    let next = match &app.screen {
        Screen::Help => app.prev_screen.take().map(|p| *p),
        Screen::Catalog(_) => {
            if logged_in {
                None
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
        }
        Screen::Schema(s) => {
            let cat = s.catalog.clone();
            let catalogs = app.catalogs.clone();
            let idx = catalogs.iter().position(|c| *c == cat).unwrap_or(0);
            Some(Screen::Catalog(CatalogState {
                items: catalogs,
                selected: idx,
            }))
        }
        Screen::Table(s) => {
            let cat = s.catalog.clone();
            let schema_name = s.schema.clone();
            let schemas = app.schemas.get(&cat).cloned().unwrap_or_default();
            let idx = schemas
                .iter()
                .position(|c| c.trim() == schema_name.trim())
                .unwrap_or(0);
            Some(Screen::Schema(SchemaState {
                catalog: cat,
                items: schemas,
                selected: idx,
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
            let idx = tables
                .iter()
                .position(|t| t.trim() == s.table.trim())
                .unwrap_or(0);
            Some(Screen::Table(TableState {
                catalog: cat,
                schema: schema_name,
                items: tables,
                selected: idx,
            }))
        }
        Screen::Connect(_) => None,
    };

    if let Some(s) = next {
        app.screen = s;
    }
}

/// macOS terminals can emit Option+hjkl/g as special glyphs when Option is not configured as Meta.
/// Remap those characters back to plain vim-style motion keys so Alt/Option navigation still works.
pub(super) fn normalize_key_code(code: KeyCode) -> KeyCode {
    match code {
        KeyCode::Char('∆') => KeyCode::Char('j'),
        KeyCode::Char('˚') => KeyCode::Char('k'),
        KeyCode::Char('˙') => KeyCode::Char('h'),
        KeyCode::Char('¬') => KeyCode::Char('l'),
        KeyCode::Char('©') => KeyCode::Char('g'),
        _ => code,
    }
}

pub fn handle_pane_focus_keys(app: &mut App, key: KeyEvent) -> bool {
    let has_pane_modifier = key.modifiers.contains(KeyModifiers::SHIFT);
    let code = normalize_key_code(key.code);
    let is_h = (code == KeyCode::Char('H')) || (code == KeyCode::Char('h') && has_pane_modifier);
    let is_l = (code == KeyCode::Char('L')) || (code == KeyCode::Char('l') && has_pane_modifier);

    let is_left = key.code == KeyCode::Left && has_pane_modifier;
    let is_right = key.code == KeyCode::Right && has_pane_modifier;

    if !(is_h || is_l || is_left || is_right || code == KeyCode::Tab) {
        return false;
    }

    let is_in_table = matches!(app.screen, Screen::Actions(_));

    if is_in_table {
        if is_h || is_left {
            app.active_panel = ActivePanel::MenuPane;
            return true;
        }
        if is_l || is_right || code == KeyCode::Tab {
            if app.active_panel == ActivePanel::MenuPane {
                app.active_panel = ActivePanel::MainViewer;
            } else {
                app.active_panel = ActivePanel::MenuPane;
            }
            return true;
        }
    }

    false
}

pub fn trigger_action(app: &mut App, action_idx: usize) -> Option<Command> {
    if action_idx >= ACTIONS.len() {
        return None;
    }
    if let Screen::Actions(ref mut s) = app.screen {
        s.selected = action_idx;
        app.active_panel = ActivePanel::MainViewer;
        let action = &ACTIONS[action_idx].2;
        match action {
            Action::Partitions => {
                if app.partition_tree_lines.is_empty() {
                    return Some(Command::FetchTableMetadata {
                        catalog: s.catalog.clone(),
                        schema: s.schema.clone(),
                        table: s.table.clone(),
                    });
                }
                None
            }
            Action::Schema => {
                if app.vertical_schema_cols.is_empty() {
                    return Some(Command::FetchTableMetadata {
                        catalog: s.catalog.clone(),
                        schema: s.schema.clone(),
                        table: s.table.clone(),
                    });
                }
                None
            }
            _ => {
                let is_paginated = matches!(action, Action::TableView);
                let query = action.build_query(&s.catalog, &s.schema, &s.table);
                s.results = None;
                Some(Command::ExecuteQuery {
                    query,
                    is_paginated,
                    catalog: s.catalog.clone(),
                    schema: s.schema.clone(),
                    table: s.table.clone(),
                })
            }
        }
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ConnectionConfig;
    use crate::trino::client::TrinoClient;

    fn sample_config() -> ConnectionConfig {
        ConnectionConfig {
            url: "https://trino.example".to_string(),
            user: "analyst".to_string(),
            password: "secret".to_string(),
        }
    }

    fn sample_app(screen: Screen) -> App {
        let mut app = App::new(sample_config(), false);
        app.screen = screen;
        app
    }

    #[test]
    fn go_back_returns_to_connect_when_logged_out_on_catalog_screen() {
        let config = sample_config();
        let mut app = App::new(config.clone(), false);
        app.main_panel_pct = 25;
        app.screen = Screen::Catalog(CatalogState {
            items: vec!["system".to_string(), "tpch".to_string()],
            selected: 1,
        });

        go_back(&mut app);

        let Screen::Connect(state) = &app.screen else {
            panic!("expected connect screen");
        };
        assert_eq!(state.url, config.url);
        assert_eq!(state.user, config.user);
        assert_eq!(state.password, config.password);
        assert_eq!(app.main_panel_pct, 60);
    }

    #[test]
    fn go_back_is_noop_for_catalog_screen_when_logged_in() {
        let mut app = sample_app(Screen::Catalog(CatalogState {
            items: vec!["system".to_string(), "tpch".to_string()],
            selected: 1,
        }));
        app.trino_client = Some(TrinoClient::new("https://trino.example", "analyst").unwrap());

        go_back(&mut app);

        let Screen::Catalog(state) = &app.screen else {
            panic!("expected catalog screen");
        };
        assert_eq!(state.selected, 1);
        assert_eq!(app.main_panel_pct, 60);
    }

    #[test]
    fn go_back_restores_previous_list_selection() {
        let mut app = sample_app(Screen::Schema(SchemaState {
            catalog: "tpch".to_string(),
            items: vec!["tiny".to_string(), "sf1".to_string()],
            selected: 1,
        }));
        app.catalogs = vec!["system".to_string(), "tpch".to_string(), "hive".to_string()];

        go_back(&mut app);

        let Screen::Catalog(state) = &app.screen else {
            panic!("expected catalog screen");
        };
        assert_eq!(state.items, app.catalogs);
        assert_eq!(state.selected, 1);

        app.screen = Screen::Table(TableState {
            catalog: "tpch".to_string(),
            schema: "sf1".to_string(),
            items: vec!["customer".to_string(), "orders".to_string()],
            selected: 1,
        });
        app.schemas.insert(
            "tpch".to_string(),
            vec!["tiny".to_string(), "sf1".to_string()],
        );

        go_back(&mut app);

        let Screen::Schema(state) = &app.screen else {
            panic!("expected schema screen");
        };
        assert_eq!(state.catalog, "tpch");
        assert_eq!(state.items, vec!["tiny".to_string(), "sf1".to_string()]);
        assert_eq!(state.selected, 1);

        app.screen = Screen::Actions(ActionState {
            catalog: "tpch".to_string(),
            schema: "sf1".to_string(),
            table: "orders".to_string(),
            selected: 0,
            query_buffer: "SELECT * FROM orders".to_string(),
            query_cursor: 20,
            results: None,
        });
        app.tables.insert(
            ("tpch".to_string(), "sf1".to_string()),
            vec!["customer".to_string(), "orders".to_string()],
        );

        go_back(&mut app);

        let Screen::Table(state) = &app.screen else {
            panic!("expected table screen");
        };
        assert_eq!(state.catalog, "tpch");
        assert_eq!(state.schema, "sf1");
        assert_eq!(
            state.items,
            vec!["customer".to_string(), "orders".to_string()]
        );
        assert_eq!(state.selected, 1);
    }

    #[test]
    fn go_back_restores_prev_screen_from_help() {
        let previous_screen = Screen::Table(TableState {
            catalog: "tpch".to_string(),
            schema: "sf1".to_string(),
            items: vec!["customer".to_string(), "orders".to_string()],
            selected: 0,
        });
        let mut app = sample_app(Screen::Help);
        app.prev_screen = Some(Box::new(previous_screen));

        go_back(&mut app);

        let Screen::Table(state) = &app.screen else {
            panic!("expected restored table screen");
        };
        assert_eq!(state.catalog, "tpch");
        assert_eq!(state.schema, "sf1");
        assert_eq!(
            state.items,
            vec!["customer".to_string(), "orders".to_string()]
        );
        assert_eq!(state.selected, 0);
        assert!(app.prev_screen.is_none());
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
                let items = app.schemas[&catalog]
                    .iter()
                    .map(|x| x.trim().to_string())
                    .collect();
                app.screen = Screen::Schema(SchemaState {
                    catalog,
                    items,
                    selected: 0,
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
            app.main_panel_pct = 15;
            app.active_panel = ActivePanel::MenuPane;
            app.partition_tree_lines.clear();
            app.vertical_schema_cols.clear();
            let default_query = ACTIONS[0].2.build_query(&catalog, &schema, &table);
            let query_len = default_query.len();
            app.screen = Screen::Actions(ActionState {
                catalog,
                schema,
                table,
                selected: 0,
                query_buffer: default_query,
                query_cursor: query_len,
                results: None,
            });
            None
        }
        Screen::Actions(s) => {
            let idx = s.selected;
            trigger_action(app, idx)
        }
        _ => None,
    }
}

pub(super) fn connect_keys(app: &mut App, key: KeyEvent) -> Option<Command> {
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
            0 => {
                state.url.pop();
            }
            1 => {
                state.user.pop();
            }
            2 => {
                state.password.pop();
            }
            _ => {}
        },
        KeyCode::Char(c) => match state.focused {
            0 => state.url.push(c),
            1 => state.user.push(c),
            2 => state.password.push(c),
            _ => {}
        },
        KeyCode::Enter if !state.url.is_empty() && !state.user.is_empty() => {
            let url = state.url.clone();
            let user = state.user.clone();
            let password = state.password.clone();
            state.loading = true;
            return Some(Command::Connect {
                url,
                user,
                password,
            });
        }
        _ => {}
    }
    None
}

pub(super) fn copy_active_pane_content(app: &mut App) {
    let mut text_to_copy = String::new();

    if let (Some(anchor), Some(current)) =
        (app.mouse_selection_anchor, app.mouse_selection_current)
    {
        text_to_copy = super::mouse::extract_selected_text(app, anchor, current);
    }

    if text_to_copy.is_empty() {
        match &app.screen {
            Screen::Catalog(s) => {
                text_to_copy = s.items.iter().map(|x| x.trim()).collect::<Vec<_>>().join("\n");
            }
            Screen::Schema(s) => {
                text_to_copy = s.items.iter().map(|x| x.trim()).collect::<Vec<_>>().join("\n");
            }
            Screen::Table(s) => {
                text_to_copy = s.items.iter().map(|x| x.trim()).collect::<Vec<_>>().join("\n");
            }
            Screen::Actions(s) => {
                if app.active_panel == ActivePanel::MenuPane {
                    text_to_copy = ACTIONS
                        .iter()
                        .map(|(_, l, _)| l.to_string())
                        .collect::<Vec<_>>()
                        .join("\n");
                } else if app.active_panel == ActivePanel::MainViewer {
                    if s.selected == 7 {
                        text_to_copy = app.partition_tree_lines.join("\n");
                    } else if s.selected == 8 {
                        text_to_copy = app
                            .vertical_schema_cols
                            .iter()
                            .map(|col| {
                                format!(
                                    "{}\t{}\t{}\t{}",
                                    col.name, col.data_type, col.key_meta, col.description
                                )
                            })
                            .collect::<Vec<_>>()
                            .join("\n");
                    } else {
                        export::copy_results_to_clipboard(app);
                        return;
                    }
                }
            }
            _ => {}
        }
    }

    if !text_to_copy.is_empty() {
        super::query::copy_to_clipboard(&text_to_copy);
        app.copied_toast = Some((
            text_to_copy.chars().take(30).collect(),
            std::time::Instant::now(),
        ));
    }
}

pub(super) fn handle_list_navigation_keys(app: &mut App, key: KeyEvent) -> Option<Command> {
    let code = normalize_key_code(key.code);
    match code {
        KeyCode::Char('j') | KeyCode::Down => {
            if let Some(items) = extract_list_labels(&app.screen)
                && !items.is_empty()
                && let Some(s) = get_selected(&app.screen)
            {
                mod_list_selected(&mut app.screen, (s + 1).min(items.len() - 1));
            }
            None
        }
        KeyCode::Char('k') | KeyCode::Up => {
            if let Some(s) = get_selected(&app.screen) {
                mod_list_selected(&mut app.screen, s.saturating_sub(1));
            }
            None
        }
        KeyCode::Char('l') | KeyCode::Right | KeyCode::Enter => select_current_item(app),
        KeyCode::Char('h') | KeyCode::Left | KeyCode::Esc => {
            app.number_buffer.clear();
            go_back(app);
            None
        }
        KeyCode::Char('g') => {
            mod_list_selected(&mut app.screen, 0);
            None
        }
        KeyCode::Char('G') => {
            if let Some(items) = extract_list_labels(&app.screen)
                && !items.is_empty()
            {
                mod_list_selected(&mut app.screen, items.len() - 1);
            }
            None
        }
        KeyCode::Char('y') | KeyCode::Char('c') => {
            copy_active_pane_content(app);
            None
        }
        _ => None,
    }
}

pub(super) fn catalog_keys(app: &mut App, key: KeyEvent) -> Option<Command> {
    handle_list_navigation_keys(app, key)
}

pub(super) fn schema_keys(app: &mut App, key: KeyEvent) -> Option<Command> {
    handle_list_navigation_keys(app, key)
}

pub(super) fn table_keys(app: &mut App, key: KeyEvent) -> Option<Command> {
    handle_list_navigation_keys(app, key)
}

pub(super) fn actions_keys(app: &mut App, key: KeyEvent) -> Option<Command> {
    let code = normalize_key_code(key.code);

    if let KeyCode::Char(c) = code
        && let Some(pos) = ACTIONS.iter().position(|(k, _, _)| *k == c)
    {
        return trigger_action(app, pos);
    }

    if let Screen::Actions(ref mut s) = app.screen {
        match app.active_panel {
            ActivePanel::MenuPane => match code {
                KeyCode::Char('j') | KeyCode::Down => {
                    s.selected = (s.selected + 1) % ACTIONS.len();
                    None
                }
                KeyCode::Char('k') | KeyCode::Up => {
                    s.selected = if s.selected == 0 {
                        ACTIONS.len() - 1
                    } else {
                        s.selected - 1
                    };
                    None
                }
                KeyCode::Enter | KeyCode::Char('l') | KeyCode::Right => {
                    let idx = s.selected;
                    trigger_action(app, idx)
                }
                KeyCode::Char('h') | KeyCode::Left | KeyCode::Esc => {
                    go_back(app);
                    None
                }
                KeyCode::Char('y') | KeyCode::Char('c') => {
                    copy_active_pane_content(app);
                    None
                }
                _ => None,
            },
            ActivePanel::MainViewer => match code {
                KeyCode::Char('j') | KeyCode::Down => {
                    if s.selected == 7 {
                        let max_lines = app.partition_tree_lines.len().saturating_sub(1);
                        app.partition_scroll = (app.partition_scroll + 1).min(max_lines);
                    } else if s.selected == 8 {
                        let max_cols = app.vertical_schema_cols.len().saturating_sub(1);
                        app.schema_scroll = (app.schema_scroll + 1).min(max_cols);
                    } else if let Some(ref mut res) = s.results {
                        if !res.rows.is_empty() {
                            res.scroll_v = (res.scroll_v + 1).min(res.rows.len().saturating_sub(1));
                        }
                        return check_trigger_infinite_scroll(app);
                    }
                    None
                }
                KeyCode::Char('k') | KeyCode::Up => {
                    if s.selected == 7 {
                        app.partition_scroll = app.partition_scroll.saturating_sub(1);
                    } else if s.selected == 8 {
                        app.schema_scroll = app.schema_scroll.saturating_sub(1);
                    } else if let Some(ref mut res) = s.results {
                        res.scroll_v = res.scroll_v.saturating_sub(1);
                    }
                    None
                }
                KeyCode::Char('l') | KeyCode::Right => {
                    if let Some(ref mut res) = s.results
                        && !res.columns.is_empty()
                    {
                        res.scroll_h = (res.scroll_h + 1).min(res.columns.len().saturating_sub(1));
                    }
                    None
                }
                KeyCode::Char('h') | KeyCode::Left => {
                    if let Some(ref mut res) = s.results {
                        res.scroll_h = res.scroll_h.saturating_sub(1);
                    }
                    None
                }
                KeyCode::Esc => {
                    app.active_panel = ActivePanel::MenuPane;
                    None
                }
                KeyCode::Char('g') => {
                    if s.selected == 7 {
                        app.partition_scroll = 0;
                    } else if s.selected == 8 {
                        app.schema_scroll = 0;
                    } else if let Some(ref mut res) = s.results {
                        res.scroll_v = 0;
                        res.scroll_h = 0;
                    }
                    None
                }
                KeyCode::Char('G') => {
                    if s.selected == 7 {
                        app.partition_scroll = app.partition_tree_lines.len().saturating_sub(1);
                    } else if s.selected == 8 {
                        app.schema_scroll = app.vertical_schema_cols.len().saturating_sub(1);
                    } else if let Some(ref mut res) = s.results {
                        res.scroll_v = res.rows.len().saturating_sub(1);
                        return check_trigger_infinite_scroll(app);
                    }
                    None
                }
                KeyCode::Char('q') | KeyCode::Char(':') => {
                    if s.selected < ACTIONS.len()
                        && matches!(ACTIONS[s.selected].2, Action::TableView)
                    {
                        app.mode = Mode::QueryInput;
                    }
                    None
                }
                KeyCode::Char('y') | KeyCode::Char('c') => {
                    copy_active_pane_content(app);
                    None
                }
                KeyCode::Char('Y')
                    if s.selected < ACTIONS.len()
                        && !matches!(
                            ACTIONS[s.selected].2,
                            Action::Partitions | Action::Schema
                        ) =>
                {
                    export::export_results_to_csv_file(app);
                    None
                }
                _ => None,
            },
        }
    } else {
        None
    }
}
