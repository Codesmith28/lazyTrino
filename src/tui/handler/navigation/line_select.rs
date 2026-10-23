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

//! Vim-style keyboard selection for the cursor-based list views (catalog /
//! schema / table lists and the partition drill-down): `V`, `v`, `Ctrl+V`
//! and `Shift+↑/↓` start a row-range selection that follows the cursor; `y`
//! copies it. The results grid has its own, richer implementation of the
//! same keys (see `ResultsState`).

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::app::{
    ACTIONS, Action, ActivePanel, App, DrillDownState, LineSelectKind, LineSelection, NoticeKind,
    Screen,
};

use super::list::filter_items;
use super::normalize_key_code;
use crate::tui::handler::query::try_copy_to_clipboard;

/// `(row count, cursor)` of the list view that currently has keyboard
/// focus, or `None` if the focused view isn't a cursor-based list.
pub fn list_cursor(app: &App) -> Option<(usize, usize)> {
    list_context(app).map(|(len, cursor, _)| (len, cursor))
}

/// `(row count, cursor, scope)`; see [`LineSelection::scope`].
fn list_context(app: &App) -> Option<(usize, usize, u64)> {
    let mut h = DefaultHasher::new();
    let (len, selected) = match &app.screen {
        Screen::Catalog(s) => {
            "catalog".hash(&mut h);
            (filter_items(&s.items, &app.search_query).len(), s.selected)
        }
        Screen::Schema(s) => {
            ("schema", &s.catalog).hash(&mut h);
            (filter_items(&s.items, &app.search_query).len(), s.selected)
        }
        Screen::Table(s) => {
            ("table", &s.catalog, &s.schema).hash(&mut h);
            (filter_items(&s.items, &app.search_query).len(), s.selected)
        }
        Screen::Actions(s) if app.active_panel == ActivePanel::MainViewer => {
            let dd = drilldown_list(s)?;
            ("drilldown", &s.catalog, &s.schema, &s.table, &dd.path).hash(&mut h);
            let len = dd.levels_cache.get(dd.depth()).map_or(0, Vec::len);
            (len, dd.selected)
        }
        _ => return None,
    };
    if len == 0 {
        return None;
    }
    (&app.search_query, len).hash(&mut h);
    Some((len, selected.min(len - 1), h.finish()))
}

/// The selection, but only if it still belongs to the list on screen.
pub fn live_selection(app: &App) -> Option<LineSelection> {
    let sel = app.line_selection?;
    let (_, _, scope) = list_context(app)?;
    (sel.scope == scope).then_some(sel)
}

/// Drops a selection that no longer matches the view (async refresh, new
/// search filter, drill-in/out, screen change). Call before anything that
/// treats "a selection exists" as meaningful, like Esc.
pub fn prune(app: &mut App) {
    if app.line_selection.is_some() && live_selection(app).is_none() {
        app.line_selection = None;
    }
}

/// The drill-down, if it is showing a list of partition values (not leaf
/// records and not still loading).
fn drilldown_list(s: &crate::app::ActionState) -> Option<&DrillDownState> {
    let dd = s.drilldown.as_ref()?;
    let is_table_view = matches!(
        ACTIONS.get(s.selected).map(|(_, _, a)| a),
        Some(Action::TableView)
    );
    (is_table_view && !dd.is_leaf() && !dd.loading).then_some(dd)
}

/// Handles the selection keys. Returns `true` if the key was consumed;
/// plain cursor motion (including `Shift+↑/↓`, which also *starts* a
/// selection) returns `false` so the view's own handler still moves the cursor.
pub fn handle_key(app: &mut App, key: KeyEvent) -> bool {
    prune(app);
    let Some((_, cursor, scope)) = list_context(app) else {
        return false;
    };

    let code = normalize_key_code(key.code);
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
    let shift = key.modifiers.contains(KeyModifiers::SHIFT);

    if matches!(code, KeyCode::Up | KeyCode::Down) && shift {
        if app.line_selection.is_none() {
            start(app, cursor, scope, LineSelectKind::Line);
        }
        return false;
    }

    let kind = match code {
        KeyCode::Char('v') | KeyCode::Char('V') if ctrl => LineSelectKind::Block,
        KeyCode::Char('V') if !ctrl => LineSelectKind::Line,
        KeyCode::Char('v') if !ctrl => LineSelectKind::Char,
        KeyCode::Char('y') if app.line_selection.is_some() => {
            copy_range(app);
            return true;
        }
        _ => return false,
    };

    // Like vim: pressing the active visual key again leaves visual mode,
    // pressing a different one switches kind while keeping the anchor.
    match app.line_selection {
        Some(sel) if sel.kind == kind => app.line_selection = None,
        Some(sel) => app.line_selection = Some(LineSelection { kind, ..sel }),
        None => start(app, cursor, scope, kind),
    }
    true
}

fn start(app: &mut App, cursor: usize, scope: u64, kind: LineSelectKind) {
    app.mouse_selection_anchor = None;
    app.mouse_selection_current = None;
    app.is_selecting_text = false;
    app.line_selection = Some(LineSelection {
        anchor: cursor,
        kind,
        scope,
    });
}

/// Text of one row exactly as shown: the value, plus the record count beside
/// it for partition drill-down rows that have been counted.
fn row_text(app: &App, idx: usize) -> Option<String> {
    match &app.screen {
        Screen::Catalog(s) => nth_label(&s.items, &app.search_query, idx),
        Screen::Schema(s) => nth_label(&s.items, &app.search_query, idx),
        Screen::Table(s) => nth_label(&s.items, &app.search_query, idx),
        Screen::Actions(s) => {
            let dd = drilldown_list(s)?;
            let val = dd.levels_cache.get(dd.depth())?.get(idx)?;
            Some(match dd.count_for_value(val) {
                Some(c) => format!("{val}  {} records", crate::app::format_count(c)),
                None => val.clone(),
            })
        }
        _ => None,
    }
}

fn nth_label(items: &[String], search: &str, idx: usize) -> Option<String> {
    filter_items(items, search)
        .get(idx)
        .map(|x| x.trim().to_string())
}

/// Copies the selected row range, then leaves visual mode. Returns whether
/// anything was copied.
pub fn copy_range(app: &mut App) -> bool {
    prune(app);
    let (Some(sel), Some((len, cursor))) = (app.line_selection, list_cursor(app)) else {
        return false;
    };
    let (lo, hi) = sel.range(cursor);
    let rows: Vec<String> = (lo..=hi.min(len - 1))
        .filter_map(|i| row_text(app, i))
        .collect();
    app.line_selection = None;
    copy_text(
        app,
        &rows.join("\n"),
        format!("Copied {} selected item(s)", rows.len()),
    )
}

/// Copies just the row under the cursor (what `Cmd+C` does with no selection).
pub fn copy_focused(app: &mut App) -> bool {
    let Some((_, cursor)) = list_cursor(app) else {
        return false;
    };
    let Some(text) = row_text(app, cursor) else {
        return false;
    };
    copy_text(app, &text, format!("Copied: {text}"))
}

fn copy_text(app: &mut App, text: &str, success: String) -> bool {
    if text.is_empty() {
        return false;
    }
    match try_copy_to_clipboard(text) {
        Ok(()) => app.set_notice(NoticeKind::Success, success),
        Err(err) => app.set_notice(NoticeKind::Error, format!("Clipboard copy failed: {err}")),
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{ActionState, CatalogState};
    use crate::config::ConnectionConfig;
    use std::collections::HashMap;

    fn catalog_app(items: &[&str], selected: usize) -> App {
        let mut app = App::new(ConnectionConfig::default(), false);
        app.screen = Screen::Catalog(CatalogState {
            items: items.iter().map(|s| s.to_string()).collect(),
            selected,
        });
        app
    }

    fn key(code: KeyCode, mods: KeyModifiers) -> KeyEvent {
        KeyEvent::new(code, mods)
    }

    #[test]
    fn test_v_variants_start_and_toggle_selection() {
        let mut app = catalog_app(&["a", "b", "c"], 1);

        assert!(handle_key(
            &mut app,
            key(KeyCode::Char('V'), KeyModifiers::SHIFT)
        ));
        assert_eq!(
            app.line_selection,
            Some(LineSelection {
                anchor: 1,
                kind: LineSelectKind::Line,
                scope: list_context(&app).unwrap().2,
            })
        );

        // different visual key switches kind, keeping the anchor
        assert!(handle_key(
            &mut app,
            key(KeyCode::Char('v'), KeyModifiers::NONE)
        ));
        assert_eq!(app.line_selection.unwrap().kind, LineSelectKind::Char);
        assert_eq!(app.line_selection.unwrap().anchor, 1);

        assert!(handle_key(
            &mut app,
            key(KeyCode::Char('v'), KeyModifiers::CONTROL)
        ));
        assert_eq!(app.line_selection.unwrap().kind, LineSelectKind::Block);

        // same key again leaves visual mode
        assert!(handle_key(
            &mut app,
            key(KeyCode::Char('v'), KeyModifiers::CONTROL)
        ));
        assert!(app.line_selection.is_none());
    }

    #[test]
    fn test_shift_arrows_start_selection_but_still_move() {
        let mut app = catalog_app(&["a", "b", "c"], 0);
        // not consumed: the list handler must still move the cursor
        assert!(!handle_key(
            &mut app,
            key(KeyCode::Down, KeyModifiers::SHIFT)
        ));
        assert_eq!(app.line_selection.unwrap().anchor, 0);

        // a second shift+arrow keeps the original anchor
        if let Screen::Catalog(s) = &mut app.screen {
            s.selected = 1;
        }
        assert!(!handle_key(
            &mut app,
            key(KeyCode::Down, KeyModifiers::SHIFT)
        ));
        assert_eq!(app.line_selection.unwrap().anchor, 0);
    }

    #[test]
    fn test_range_text_spans_anchor_to_cursor_in_either_direction() {
        let mut app = catalog_app(&["a", "b", "c", "d"], 3);
        app.line_selection = Some(LineSelection {
            anchor: 1,
            kind: LineSelectKind::Line,
            scope: list_context(&app).unwrap().2,
        });
        let (lo, hi) = app.line_selection.unwrap().range(3);
        let rows: Vec<_> = (lo..=hi).filter_map(|i| row_text(&app, i)).collect();
        assert_eq!(rows, vec!["b", "c", "d"]);
    }

    #[test]
    fn test_selection_dropped_when_not_in_a_list() {
        let mut app = App::new(ConnectionConfig::default(), false);
        app.line_selection = Some(LineSelection {
            anchor: 0,
            kind: LineSelectKind::Line,
            scope: 0,
        });
        assert!(!handle_key(
            &mut app,
            key(KeyCode::Char('j'), KeyModifiers::NONE)
        ));
        assert!(app.line_selection.is_none());
    }

    #[test]
    fn test_selection_is_discarded_when_the_list_changes_underneath() {
        // async refresh: the list grows while a selection is anchored
        let mut app = catalog_app(&["a", "b", "c"], 1);
        handle_key(&mut app, key(KeyCode::Char('V'), KeyModifiers::SHIFT));
        assert!(live_selection(&app).is_some());
        if let Screen::Catalog(s) = &mut app.screen {
            s.items.push("d".into());
        }
        assert!(live_selection(&app).is_none());
        prune(&mut app);
        assert!(app.line_selection.is_none());

        // changing the search filter also invalidates it
        let mut app = catalog_app(&["a1", "a2", "b1"], 0);
        handle_key(&mut app, key(KeyCode::Char('V'), KeyModifiers::SHIFT));
        app.search_query = "a".into();
        assert!(live_selection(&app).is_none());
    }

    #[test]
    fn test_selection_is_discarded_when_drilling_to_another_level() {
        let dd = DrillDownState {
            partition_cols: vec!["date".into(), "svc".into()],
            levels_cache: vec![vec!["d1".into(), "d2".into()], vec!["x".into(), "y".into()]],
            ..Default::default()
        };
        let mut app = App::new(ConnectionConfig::default(), false);
        app.active_panel = ActivePanel::MainViewer;
        app.screen = Screen::Actions(ActionState {
            selected: 0,
            drilldown: Some(dd),
            ..Default::default()
        });
        handle_key(&mut app, key(KeyCode::Char('V'), KeyModifiers::SHIFT));
        assert!(live_selection(&app).is_some());
        if let Screen::Actions(s) = &mut app.screen {
            s.drilldown
                .as_mut()
                .unwrap()
                .path
                .push(("date".into(), "d1".into()));
        }
        assert!(live_selection(&app).is_none());
    }

    #[test]
    fn test_drilldown_rows_carry_counts_beside_values() {
        let mut counts = HashMap::new();
        counts.insert(vec![("date".to_string(), "d1".to_string())], 1500);
        let dd = DrillDownState {
            partition_cols: vec!["date".into()],
            levels_cache: vec![vec!["d1".into(), "d2".into()]],
            counts_cache: counts,
            ..Default::default()
        };
        let mut app = App::new(ConnectionConfig::default(), false);
        app.active_panel = ActivePanel::MainViewer;
        app.screen = Screen::Actions(ActionState {
            selected: 0,
            drilldown: Some(dd),
            ..Default::default()
        });
        assert_eq!(list_cursor(&app), Some((2, 0)));
        assert_eq!(row_text(&app, 0).as_deref(), Some("d1  1,500 records"));
        assert_eq!(row_text(&app, 1).as_deref(), Some("d2"));
    }
}
