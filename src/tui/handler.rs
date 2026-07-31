use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseEvent, MouseEventKind};
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
        is_paginated: bool,
        catalog: String,
        schema: String,
        table: String,
    },
    FetchNextPage {
        catalog: String,
        schema: String,
        table: String,
        offset: usize,
        limit: usize,
    },
}

pub fn extract_from_tables(sql: &str) -> Vec<String> {
    let mut tables = Vec::new();
    let tokens: Vec<&str> = sql.split_whitespace().collect();
    let mut i = 0;
    while i < tokens.len() {
        let upper = tokens[i].to_uppercase();
        if upper == "FROM" || upper == "JOIN" {
            if i + 1 < tokens.len() {
                let next_token = tokens[i + 1];
                if !next_token.starts_with('(') {
                    let clean_table = next_token.trim_matches(|c| c == ',' || c == ';' || c == '(' || c == ')');
                    let clean_upper = clean_table.to_uppercase();
                    let sql_keywords = [
                        "SELECT", "WHERE", "GROUP", "HAVING", "ORDER", "LIMIT", "JOIN", "LEFT",
                        "RIGHT", "INNER", "OUTER", "CROSS", "FULL", "ON", "USING", "AS",
                    ];
                    if !clean_table.is_empty() && !sql_keywords.contains(&clean_upper.as_str()) {
                        tables.push(clean_table.to_string());
                    }
                }
            }
        }
        i += 1;
    }
    tables
}

pub fn validate_and_build_query(
    user_input: &str,
    catalog: &str,
    schema: &str,
    table: &str,
) -> Result<String, String> {
    let trimmed = user_input.trim();
    if trimmed.is_empty() {
        return Ok(crate::trino::queries::page_query(catalog, schema, table, 0, 100));
    }

    let full_target = format!("{catalog}.{schema}.{table}");
    let schema_target = format!("{schema}.{table}");

    let extracted_tables = extract_from_tables(trimmed);

    if !extracted_tables.is_empty() {
        for t in &extracted_tables {
            let normalized = t.trim().trim_matches(|c| c == '"' || c == '`' || c == '\'').to_lowercase();
            let matches_table = normalized == table.to_lowercase()
                || normalized == schema_target.to_lowercase()
                || normalized == full_target.to_lowercase();
            if !matches_table {
                return Err(format!(
                    "Query targets table '{}', but current view scope is '{}.{}.{}'. Queries in this view must operate on table '{}'.",
                    t, catalog, schema, table, table
                ));
            }
        }
        Ok(trimmed.to_string())
    } else {
        let upper_input = trimmed.to_uppercase();
        if upper_input.starts_with("SELECT") {
            let clause_keywords = [" WHERE ", " GROUP BY ", " HAVING ", " ORDER BY ", " LIMIT "];
            let mut insert_idx = None;
            for kw in &clause_keywords {
                if let Some(pos) = upper_input.find(kw) {
                    if insert_idx.map_or(true, |p| pos < p) {
                        insert_idx = Some(pos);
                    }
                }
            }
            if let Some(pos) = insert_idx {
                let (select_part, rest) = trimmed.split_at(pos);
                Ok(format!("{select_part} FROM {full_target}{rest}"))
            } else {
                Ok(format!("{trimmed} FROM {full_target}"))
            }
        } else if upper_input.starts_with("WHERE")
            || upper_input.starts_with("GROUP BY")
            || upper_input.starts_with("HAVING")
            || upper_input.starts_with("ORDER BY")
            || upper_input.starts_with("LIMIT")
        {
            Ok(format!("SELECT * FROM {full_target} {trimmed}"))
        } else if upper_input.contains('=')
            || upper_input.contains('>')
            || upper_input.contains('<')
            || upper_input.contains(" LIKE ")
            || upper_input.contains(" IN ")
            || upper_input.contains(" IS ")
        {
            Ok(format!("SELECT * FROM {full_target} WHERE {trimmed}"))
        } else {
            Ok(format!("SELECT {trimmed} FROM {full_target}"))
        }
    }
}

fn check_trigger_infinite_scroll(app: &mut App) -> Option<Command> {
    if let Screen::Results(ref mut state) = app.screen {
        if state.is_paginated && !state.is_fetching_next_page && state.has_more_rows {
            if state.scroll_v + 15 >= state.rows.len() {
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
        }
    }
    None
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

#[allow(dead_code)]
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
                let is_paginated = matches!(action, Action::TableView);
                let query = action.build_query(&cat, &schema, &table);
                info!(%query, "Leader action triggered");
                app.mode = Mode::Normal;
                return Some(Command::ExecuteQuery {
                    query,
                    is_paginated,
                    catalog: cat,
                    schema,
                    table,
                });
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

#[allow(dead_code)]
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
    let logged_in = app.trino_client.is_some();

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
        Screen::Results(_) => {
            let prev = app.prev_screen.take().map(|p| *p);
            match prev {
                Some(Screen::Connect(_)) if logged_in => {
                    Some(Screen::Catalog(CatalogState {
                        items: app.catalogs.clone(),
                        selected: 0,
                        scroll: 0,
                    }))
                }
                Some(p) => Some(p),
                None => {
                    if !app.catalogs.is_empty() {
                        Some(Screen::Catalog(CatalogState {
                            items: app.catalogs.clone(),
                            selected: 0,
                            scroll: 0,
                        }))
                    } else if !logged_in {
                        let c = app.config.clone();
                        Some(Screen::Connect(ConnectState {
                            url: c.url,
                            user: c.user,
                            password: c.password,
                            focused: 0,
                            loading: false,
                            error: None,
                        }))
                    } else {
                        None
                    }
                }
            }
        }
    };

    if let Some(s) = next {
        app.screen = s;
    }
}

use crossterm::event::MouseButton;

fn normalize_key_code(code: KeyCode) -> KeyCode {
    match code {
        KeyCode::Char('∆') => KeyCode::Char('j'),
        KeyCode::Char('˚') => KeyCode::Char('k'),
        KeyCode::Char('˙') => KeyCode::Char('h'),
        KeyCode::Char('¬') => KeyCode::Char('l'),
        KeyCode::Char('©') => KeyCode::Char('g'),
        _ => code,
    }
}

fn is_mac_option_code(code: KeyCode) -> bool {
    matches!(code, KeyCode::Char('∆') | KeyCode::Char('˚') | KeyCode::Char('˙') | KeyCode::Char('¬') | KeyCode::Char('©'))
}

fn handle_pane_activation(app: &mut App, key: KeyEvent) -> bool {
    let shift = key.modifiers.contains(KeyModifiers::SHIFT);
    let alt = key.modifiers.contains(KeyModifiers::ALT);
    let mac_opt = is_mac_option_code(key.code);
    let has_pane_modifier = shift || alt || mac_opt;

    let code = normalize_key_code(key.code);
    let is_h = (code == KeyCode::Char('H')) || (code == KeyCode::Char('h') && has_pane_modifier);
    let is_j = (code == KeyCode::Char('J')) || (code == KeyCode::Char('j') && has_pane_modifier);
    let is_k = (code == KeyCode::Char('K')) || (code == KeyCode::Char('k') && has_pane_modifier);
    let is_l = (code == KeyCode::Char('L')) || (code == KeyCode::Char('l') && has_pane_modifier);

    let is_left = key.code == KeyCode::Left && has_pane_modifier;
    let is_right = key.code == KeyCode::Right && has_pane_modifier;
    let is_up = key.code == KeyCode::Up && has_pane_modifier;
    let is_down = key.code == KeyCode::Down && has_pane_modifier;

    if !(is_h || is_j || is_k || is_l || is_left || is_right || is_up || is_down) {
        return false;
    }

    let active_table = match &app.screen {
        Screen::Actions(_) => true,
        Screen::Table(t) => !t.items.is_empty(),
        _ => false,
    };

    if is_k || is_up {
        match app.active_panel {
            ActivePanel::SchemaInspector => {
                if active_table {
                    app.active_panel = ActivePanel::PartitionTree;
                } else {
                    app.active_panel = ActivePanel::MainViewer;
                }
            }
            ActivePanel::PartitionTree => {
                app.active_panel = ActivePanel::MainViewer;
            }
            ActivePanel::MainViewer => {}
        }
        return true;
    }

    if is_j || is_down {
        match app.active_panel {
            ActivePanel::MainViewer => {
                if active_table {
                    app.active_panel = ActivePanel::PartitionTree;
                }
            }
            ActivePanel::PartitionTree => {
                if active_table {
                    app.active_panel = ActivePanel::SchemaInspector;
                }
            }
            ActivePanel::SchemaInspector => {}
        }
        return true;
    }

    if is_h || is_left {
        match app.active_panel {
            ActivePanel::PartitionTree | ActivePanel::SchemaInspector => {
                app.active_panel = ActivePanel::MainViewer;
            }
            ActivePanel::MainViewer => {}
        }
        return true;
    }

    if is_l || is_right {
        match app.active_panel {
            ActivePanel::MainViewer => {
                if active_table {
                    app.active_panel = ActivePanel::PartitionTree;
                }
            }
            ActivePanel::PartitionTree | ActivePanel::SchemaInspector => {}
        }
        return true;
    }

    false
}

pub fn handle_mouse_sync(app: &mut App, mouse: MouseEvent) -> Option<Command> {
    let (term_width, term_height) = crossterm::terminal::size().unwrap_or((80, 24));
    if term_width == 0 || term_height == 0 {
        return None;
    }

    let bottom_y = term_height.saturating_sub(7);
    let border_x = ((term_width as u32 * app.main_panel_pct as u32) / 100) as u16;

    let height_right = bottom_y;
    let border_y = ((height_right as u32 * app.control_panel_split_pct as u32) / 100) as u16;

    let active_table = match &app.screen {
        Screen::Actions(_) => true,
        Screen::Table(t) => !t.items.is_empty(),
        _ => false,
    };

    match mouse.kind {
        MouseEventKind::Down(MouseButton::Left) => {
            if mouse.row < bottom_y && (mouse.column as i32 - border_x as i32).abs() <= 1 {
                app.is_dragging_resizer = true;
                app.is_dragging_v_resizer = false;
            } else if mouse.column >= border_x && active_table && (mouse.row as i32 - border_y as i32).abs() <= 1 {
                app.is_dragging_v_resizer = true;
                app.is_dragging_resizer = false;
            } else {
                app.is_dragging_resizer = false;
                app.is_dragging_v_resizer = false;

                if mouse.column < border_x && mouse.row < bottom_y {
                    let is_table_view = matches!(app.screen, Screen::Results(_));
                    let search_active = matches!(app.mode, Mode::Search);
                    let query_active = matches!(app.mode, Mode::QueryInput);
                    let inner_w = border_x.saturating_sub(2).max(1) as usize;

                    let search_h: u16 = if search_active {
                        let total = 3 + app.search_query.len();
                        let lines = (total + inner_w - 1) / inner_w;
                        (lines as u16 + 2).clamp(3, 8)
                    } else {
                        3
                    };

                    let query_h: u16 = if is_table_view {
                        if query_active {
                            if let Screen::Results(ref s) = app.screen {
                                let total = 7 + s.query_buffer.len();
                                let lines = (total + inner_w - 1) / inner_w;
                                (lines as u16 + 2).clamp(3, 4)
                            } else {
                                3
                            }
                        } else {
                            3
                        }
                    } else {
                        0
                    };

                    if mouse.row < search_h {
                        app.mode = Mode::Search;
                    } else if is_table_view && mouse.row < search_h + query_h {
                        app.mode = Mode::QueryInput;
                        if let Screen::Results(ref mut state) = app.screen {
                            state.query_cursor = state.query_buffer.len();
                        }
                    } else {
                        app.mode = Mode::Normal;
                        app.active_panel = ActivePanel::MainViewer;
                    }
                } else if mouse.column >= border_x && mouse.row < bottom_y && active_table {
                    if mouse.row < border_y {
                        app.active_panel = ActivePanel::PartitionTree;
                    } else {
                        app.active_panel = ActivePanel::SchemaInspector;
                    }
                }
            }
        }
        MouseEventKind::Drag(MouseButton::Left) => {
            if app.is_dragging_resizer {
                let pct = ((mouse.column as u32 * 100) / term_width as u32) as u16;
                app.main_panel_pct = pct.clamp(20, 80);
            } else if app.is_dragging_v_resizer {
                let rel_y = mouse.row as u32;
                if height_right > 0 {
                    let pct = ((rel_y * 100) / height_right as u32) as u16;
                    app.control_panel_split_pct = pct.clamp(20, 80);
                }
            }
        }
        MouseEventKind::Up(MouseButton::Left) => {
            app.is_dragging_resizer = false;
            app.is_dragging_v_resizer = false;
        }
        MouseEventKind::ScrollDown => {
            let shift = mouse.modifiers.contains(KeyModifiers::SHIFT);
            match app.active_panel {
                ActivePanel::MainViewer => {
                    if let Screen::Results(ref mut state) = app.screen {
                        if shift {
                            if !state.columns.is_empty() {
                                state.scroll_h = (state.scroll_h + 1).min(state.columns.len().saturating_sub(1));
                            }
                        } else if !state.rows.is_empty() {
                            state.scroll_v = (state.scroll_v + 1).min(state.rows.len().saturating_sub(1));
                            return check_trigger_infinite_scroll(app);
                        }
                    } else if let Some(items) = extract_list_labels(&app.screen) {
                        if !items.is_empty() {
                            if let Some(s) = get_selected(&app.screen) {
                                mod_list_selected(&mut app.screen, (s + 1).min(items.len().saturating_sub(1)));
                            }
                        }
                    }
                }
                ActivePanel::PartitionTree => {
                    let max_lines = app.partition_tree_lines.len().saturating_sub(1);
                    app.partition_scroll = (app.partition_scroll + 1).min(max_lines);
                }
                ActivePanel::SchemaInspector => {
                    let max_cols = app.vertical_schema_cols.len().saturating_sub(1);
                    app.schema_scroll = (app.schema_scroll + 1).min(max_cols);
                }
            }
        }
        MouseEventKind::ScrollUp => {
            let shift = mouse.modifiers.contains(KeyModifiers::SHIFT);
            match app.active_panel {
                ActivePanel::MainViewer => {
                    if let Screen::Results(ref mut state) = app.screen {
                        if shift {
                            state.scroll_h = state.scroll_h.saturating_sub(1);
                        } else {
                            state.scroll_v = state.scroll_v.saturating_sub(1);
                        }
                    } else if let Some(s) = get_selected(&app.screen) {
                        mod_list_selected(&mut app.screen, s.saturating_sub(1));
                    }
                }
                ActivePanel::PartitionTree => {
                    app.partition_scroll = app.partition_scroll.saturating_sub(1);
                }
                ActivePanel::SchemaInspector => {
                    app.schema_scroll = app.schema_scroll.saturating_sub(1);
                }
            }
        }
        MouseEventKind::ScrollRight => {
            if app.active_panel == ActivePanel::MainViewer {
                if let Screen::Results(ref mut state) = app.screen {
                    if !state.columns.is_empty() {
                        state.scroll_h = (state.scroll_h + 1).min(state.columns.len().saturating_sub(1));
                    }
                }
            }
        }
        MouseEventKind::ScrollLeft => {
            if app.active_panel == ActivePanel::MainViewer {
                if let Screen::Results(ref mut state) = app.screen {
                    state.scroll_h = state.scroll_h.saturating_sub(1);
                }
            }
        }
        _ => {}
    }
    None
}

pub fn handle_key_sync(app: &mut App, key: KeyEvent) -> Option<Command> {
    let code = normalize_key_code(key.code);

    if code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
        info!("User pressed Ctrl+C, quitting...");
        app.should_quit = true;
        return None;
    }

    if matches!(app.mode, Mode::Search) {
        match code {
            KeyCode::Esc | KeyCode::Enter => {
                app.mode = Mode::Normal;
            }
            KeyCode::Backspace => {
                app.search_query.pop();
            }
            KeyCode::Char(c) => {
                app.search_query.push(c);
            }
            _ => {}
        }
        return None;
    }

    if matches!(app.mode, Mode::QueryInput) {
        if let Screen::Results(ref mut state) = app.screen {
            match code {
                KeyCode::Esc => {
                    app.mode = Mode::Normal;
                    return None;
                }
                KeyCode::Enter => {
                    app.mode = Mode::Normal;
                    let input = state.query_buffer.clone();
                    let cat = state.catalog.clone();
                    let sch = state.schema.clone();
                    let tbl = state.table.clone();
                    let is_paginated = state.is_paginated;

                    match validate_and_build_query(&input, &cat, &sch, &tbl) {
                        Ok(full_sql) => {
                            state.invalid_query_error = None;
                            return Some(Command::ExecuteQuery {
                                query: full_sql,
                                is_paginated,
                                catalog: cat,
                                schema: sch,
                                table: tbl,
                            });
                        }
                        Err(err_msg) => {
                            state.invalid_query_error = Some(err_msg);
                            return None;
                        }
                    }
                }
                KeyCode::Backspace => {
                    if state.query_cursor > 0 && !state.query_buffer.is_empty() {
                        let idx = state.query_cursor - 1;
                        state.query_buffer.remove(idx);
                        state.query_cursor -= 1;
                    }
                }
                KeyCode::Delete => {
                    if state.query_cursor < state.query_buffer.len() {
                        state.query_buffer.remove(state.query_cursor);
                    }
                }
                KeyCode::Left => {
                    state.query_cursor = state.query_cursor.saturating_sub(1);
                }
                KeyCode::Right => {
                    state.query_cursor = (state.query_cursor + 1).min(state.query_buffer.len());
                }
                KeyCode::Home => {
                    state.query_cursor = 0;
                }
                KeyCode::End => {
                    state.query_cursor = state.query_buffer.len();
                }
                KeyCode::Char(c) => {
                    let idx = state.query_cursor.min(state.query_buffer.len());
                    state.query_buffer.insert(idx, c);
                    state.query_cursor += 1;
                }
                _ => {}
            }
        } else {
            app.mode = Mode::Normal;
        }
        return None;
    }

    if handle_pane_activation(app, key) {
        return None;
    }

    if code == KeyCode::Char('/') {
        app.mode = Mode::Search;
        return None;
    }

    if matches!(app.mode, Mode::Leader { .. }) {
        if let KeyCode::Char(c) = code {
            if let Screen::Actions(s) = &app.screen {
                if let Some(action) = app.action_for(c) {
                    let is_table_view = matches!(action, Action::TableView);
                    let query = action.build_query(&s.catalog, &s.schema, &s.table);
                    app.mode = Mode::Normal;
                    return Some(Command::ExecuteQuery {
                        query,
                        is_paginated: is_table_view,
                        catalog: s.catalog.clone(),
                        schema: s.schema.clone(),
                        table: s.table.clone(),
                    });
                }
            }
        }
        app.mode = Mode::Normal;
        return None;
    }

    if code == KeyCode::Char('?') {
        app.prev_screen = Some(Box::new(app.screen.clone()));
        app.screen = Screen::Help;
        return None;
    }

    if key.kind != KeyEventKind::Press {
        return None;
    }

    if code == KeyCode::Esc {
        app.number_buffer.clear();
        go_back(app);
        return None;
    }

    if code == KeyCode::Enter && !app.number_buffer.is_empty() {
        jump_to_number(app);
        return None;
    }

    if let KeyCode::Char(c) = code {
        if c.is_ascii_digit() && matches!(app.active_panel, ActivePanel::MainViewer) {
            update_number_buffer(app, c);
            return None;
        }
    }

    match app.active_panel {
        ActivePanel::MainViewer => {
            match code {
                KeyCode::Char('j') | KeyCode::Down => {
                    if let Screen::Results(state) = &mut app.screen {
                        if !state.rows.is_empty() {
                            state.scroll_v = (state.scroll_v + 1).min(state.rows.len().saturating_sub(1));
                        }
                        return check_trigger_infinite_scroll(app);
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
                    if let Screen::Results(state) = &mut app.screen {
                        state.scroll_h = state.scroll_h.saturating_sub(1);
                    } else {
                        app.number_buffer.clear();
                        go_back(app);
                    }
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
                        return check_trigger_infinite_scroll(app);
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
        }
        ActivePanel::PartitionTree => {
            let max_lines = app.partition_tree_lines.len().saturating_sub(1);
            match code {
                KeyCode::Char('j') | KeyCode::Down => {
                    app.partition_scroll = (app.partition_scroll + 1).min(max_lines);
                    return None;
                }
                KeyCode::Char('k') | KeyCode::Up => {
                    app.partition_scroll = app.partition_scroll.saturating_sub(1);
                    return None;
                }
                KeyCode::Char('g') => {
                    app.partition_scroll = 0;
                    return None;
                }
                KeyCode::Char('G') => {
                    app.partition_scroll = max_lines;
                    return None;
                }
                _ => {}
            }
        }
        ActivePanel::SchemaInspector => {
            let max_cols = app.vertical_schema_cols.len().saturating_sub(1);
            match code {
                KeyCode::Char('j') | KeyCode::Down => {
                    app.schema_scroll = (app.schema_scroll + 1).min(max_cols);
                    return None;
                }
                KeyCode::Char('k') | KeyCode::Up => {
                    app.schema_scroll = app.schema_scroll.saturating_sub(1);
                    return None;
                }
                KeyCode::Char('g') => {
                    app.schema_scroll = 0;
                    return None;
                }
                KeyCode::Char('G') => {
                    app.schema_scroll = max_cols;
                    return None;
                }
                _ => {}
            }
        }
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
            let is_paginated = matches!(action, Action::TableView);
            let query = action.build_query(&s.catalog, &s.schema, &s.table);
            Some(Command::ExecuteQuery {
                query,
                is_paginated,
                catalog: s.catalog.clone(),
                schema: s.schema.clone(),
                table: s.table.clone(),
            })
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
    if key.code == KeyCode::Enter {
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
                    let is_table_view = matches!(action, Action::TableView);
                    let query = action.build_query(&s.catalog, &s.schema, &s.table);
                    return Some(Command::ExecuteQuery {
                        query,
                        is_paginated: is_table_view,
                        catalog: s.catalog.clone(),
                        schema: s.schema.clone(),
                        table: s.table.clone(),
                    });
                }
            }
            None
        }
        _ => None,
    }
}

fn results_keys(app: &mut App, key: KeyEvent) -> Option<Command> {
    if let Screen::Results(state) = &mut app.screen {
        let code = normalize_key_code(key.code);
        match code {
            KeyCode::Char('q') | KeyCode::Char(':') => {
                app.mode = Mode::QueryInput;
                state.query_cursor = state.query_buffer.len();
                return None;
            }
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
                Ok(res) if !res.data.is_empty() => {
                    app.complete_query_log_success(log_id, res.duration_ms, res.data.len());
                    let raw_lines: Vec<String> = res.data.into_iter().map(|r| r.join("/")).collect();
                    app.partition_tree_lines = crate::tui::screens::partition_tree::build_tree_lines(&raw_lines);
                }
                _ => {
                    let show_create_query = queries::show_create(&catalog, &schema, &table);
                    let log_id2 = app.add_query_log(show_create_query.clone());
                    match client.execute(&show_create_query).await {
                        Ok(res2) => {
                            app.complete_query_log_success(log_id2, res2.duration_ms, res2.data.len());
                            let ddl_str = res2.data.get(0).and_then(|r| r.get(0)).cloned().unwrap_or_default();
                            app.partition_tree_lines = crate::tui::screens::partition_tree::build_tree_lines(&[ddl_str]);
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
        Command::ExecuteQuery { query, is_paginated, catalog, schema, table } => {
            let client = app.trino_client.clone().expect("TrinoClient not initialized");
            info!(%query, "Executing query");
            let log_id = app.add_query_log(query.clone());
            app.loading = true;
            app.prev_screen = Some(Box::new(app.screen.clone()));

            let (query_buffer, query_cursor) = match &app.screen {
                Screen::Results(s) => (s.query_buffer.clone(), s.query_cursor),
                _ => (query.clone(), query.len()),
            };

            app.screen = Screen::Results(ResultsState {
                query: query.clone(),
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
            });
            match client.execute(&query).await {
                Ok(results) => {
                    app.complete_query_log_success(log_id, results.duration_ms, results.data.len());
                    let cols: Vec<String> = results.columns.iter().map(|c| c.name.clone()).collect();
                    let rows = results.data;
                    let has_more = if is_paginated { rows.len() >= 100 } else { false };
                    info!(row_count = rows.len(), "Query completed");
                    app.screen = Screen::Results(ResultsState {
                        query,
                        query_buffer,
                        query_cursor,
                        columns: cols,
                        rows,
                        scroll_v: 0,
                        scroll_h: 0,
                        loading: false,
                        error: None,
                        is_paginated,
                        catalog,
                        schema,
                        table,
                        offset: 0,
                        page_size: 100,
                        is_fetching_next_page: false,
                        has_more_rows: has_more,
                        invalid_query_error: None,
                    });
                }
                Err(e) => {
                    error!(error = %e, "Query failed");
                    app.complete_query_log_error(log_id, e.to_string());
                    app.screen = Screen::Results(ResultsState {
                        query,
                        query_buffer,
                        query_cursor,
                        columns: Vec::new(),
                        rows: vec![vec![format!("Error: {e}")]],
                        scroll_v: 0,
                        scroll_h: 0,
                        loading: false,
                        error: Some(e.to_string()),
                        is_paginated: false,
                        catalog,
                        schema,
                        table,
                        offset: 0,
                        page_size: 100,
                        is_fetching_next_page: false,
                        has_more_rows: false,
                        invalid_query_error: None,
                    });
                }
            }
            app.loading = false;
        }
        Command::FetchNextPage { catalog, schema, table, offset, limit } => {
            let client = match app.trino_client.clone() {
                Some(c) => c,
                None => return,
            };
            let query = queries::page_query(&catalog, &schema, &table, offset, limit);
            info!(%query, %offset, %limit, "Fetching next page for infinite scroll");
            let log_id = app.add_query_log(query.clone());

            match client.execute(&query).await {
                Ok(results) => {
                    app.complete_query_log_success(log_id, results.duration_ms, results.data.len());
                    let new_rows = results.data;
                    let fetched_count = new_rows.len();
                    if let Screen::Results(ref mut state) = app.screen {
                        state.rows.extend(new_rows);
                        state.offset = offset;
                        state.is_fetching_next_page = false;
                        if fetched_count < limit {
                            state.has_more_rows = false;
                        }
                    }
                }
                Err(e) => {
                    error!(error = %e, "Failed to fetch next page");
                    app.complete_query_log_error(log_id, e.to_string());
                    if let Screen::Results(ref mut state) = app.screen {
                        state.is_fetching_next_page = false;
                        state.has_more_rows = false;
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_from_tables() {
        assert_eq!(
            extract_from_tables("SELECT * FROM datalake.some_db.some_table WHERE id > 5"),
            vec!["datalake.some_db.some_table"]
        );
        assert_eq!(
            extract_from_tables("SELECT * FROM some_table"),
            vec!["some_table"]
        );
        assert_eq!(
            extract_from_tables("WHERE age > 20 ORDER BY id"),
            Vec::<String>::new()
        );
    }

    #[test]
    fn test_partial_query_where_clause() {
        let res = validate_and_build_query(
            "WHERE status = 'ACTIVE'",
            "datalake",
            "some_db",
            "some_table",
        );
        assert_eq!(
            res.unwrap(),
            "SELECT * FROM datalake.some_db.some_table WHERE status = 'ACTIVE'"
        );
    }

    #[test]
    fn test_partial_query_select_where() {
        let res = validate_and_build_query(
            "SELECT name, age WHERE age > 25",
            "datalake",
            "some_db",
            "some_table",
        );
        assert_eq!(
            res.unwrap(),
            "SELECT name, age FROM datalake.some_db.some_table WHERE age > 25"
        );
    }

    #[test]
    fn test_partial_query_order_by() {
        let res = validate_and_build_query(
            "ORDER BY created_at DESC LIMIT 10",
            "datalake",
            "some_db",
            "some_table",
        );
        assert_eq!(
            res.unwrap(),
            "SELECT * FROM datalake.some_db.some_table ORDER BY created_at DESC LIMIT 10"
        );
    }

    #[test]
    fn test_valid_full_query_same_table() {
        let res = validate_and_build_query(
            "SELECT * FROM datalake.some_db.some_table WHERE age > 10",
            "datalake",
            "some_db",
            "some_table",
        );
        assert!(res.is_ok());
        assert_eq!(
            res.unwrap(),
            "SELECT * FROM datalake.some_db.some_table WHERE age > 10"
        );
    }

    #[test]
    fn test_invalid_query_different_table() {
        let res = validate_and_build_query(
            "SELECT * FROM table_a WHERE age > 10",
            "datalake",
            "some_db",
            "some_table",
        );
        assert!(res.is_err());
        assert!(res.unwrap_err().contains("Query targets table 'table_a'"));
    }

    #[test]
    fn test_wrap_text() {
        use crate::tui::screens::results::wrap_text;
        let wrapped = wrap_text("hello world this is a test of long text wrapping", 10);
        assert!(wrapped.len() > 1);
        for line in &wrapped {
            assert!(line.len() <= 10);
        }
    }
}

