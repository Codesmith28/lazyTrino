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

use crossterm::event::{KeyCode, KeyEvent, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;

use crate::app::{
    App, AppliedFilterItem, AppliedFilterKind, AppliedFiltersModalState, ColumnFilter, Mode, Screen,
};
use crate::tui::handler::Command;
use crate::tui::handler::filter_modal::reapply_in_memory_filters;

pub fn open_applied_filters_modal(app: &mut App) {
    let mut items = Vec::new();

    if let Screen::Actions(ref state) = app.screen {
        // 1. Collect Partition filters
        if let Some(ref dd) = state.drilldown {
            for (idx, (col, val)) in dd.path.iter().enumerate() {
                items.push(AppliedFilterItem {
                    kind: AppliedFilterKind::Partition,
                    name: col.clone(),
                    display_value: val.clone(),
                    column_filter: None,
                    partition_index: Some(idx),
                });
            }
        } else if let Some(ref res) = state.results {
            for (idx, (col, val)) in res.filters.iter().enumerate() {
                items.push(AppliedFilterItem {
                    kind: AppliedFilterKind::Partition,
                    name: col.clone(),
                    display_value: val.clone(),
                    column_filter: None,
                    partition_index: Some(idx),
                });
            }
        }

        // 2. Collect Column filters
        if let Some(ref res) = state.results {
            let mut col_filters: Vec<_> = res.column_filters.iter().collect();
            col_filters.sort_by(|a, b| a.0.cmp(b.0));

            for (col_name, filter) in col_filters {
                let display_val = match filter {
                    ColumnFilter::Numeric(num) => {
                        format!("{} {}", num.op.as_str(), num.raw_value)
                    }
                    ColumnFilter::Values(vals) => {
                        let mut sorted_vals: Vec<_> = vals.iter().cloned().collect();
                        sorted_vals.sort();
                        if sorted_vals.len() <= 3 {
                            format!("IN ({})", sorted_vals.join(", "))
                        } else {
                            format!("{} values selected", sorted_vals.len())
                        }
                    }
                };
                items.push(AppliedFilterItem {
                    kind: AppliedFilterKind::Column,
                    name: col_name.clone(),
                    display_value: display_val,
                    column_filter: Some(filter.clone()),
                    partition_index: None,
                });
            }
        }
    }

    app.applied_filters_modal = Some(AppliedFiltersModalState { items, selected: 0 });
    app.mode = Mode::AppliedFiltersModal;
}

pub fn close_applied_filters_modal(app: &mut App) {
    app.applied_filters_modal = None;
    app.mode = Mode::Normal;
}

pub fn handle_applied_filters_modal_key(app: &mut App, key: KeyEvent) -> Option<Command> {
    let code = key.code;

    match code {
        KeyCode::Esc
        | KeyCode::Enter
        | KeyCode::Char('q')
        | KeyCode::Char('a')
        | KeyCode::Char('A') => {
            close_applied_filters_modal(app);
            None
        }
        KeyCode::Char('j') | KeyCode::Down => {
            if let Some(ref mut modal) = app.applied_filters_modal
                && !modal.items.is_empty()
            {
                modal.selected = (modal.selected + 1).min(modal.items.len().saturating_sub(1));
            }
            None
        }
        KeyCode::Char('k') | KeyCode::Up => {
            if let Some(ref mut modal) = app.applied_filters_modal {
                modal.selected = modal.selected.saturating_sub(1);
            }
            None
        }
        KeyCode::Char('d') | KeyCode::Char('x') | KeyCode::Delete => remove_selected_filter(app),
        KeyCode::Char('c') | KeyCode::Char('C') => clear_all_filters(app),
        _ => None,
    }
}

pub fn handle_applied_filters_modal_mouse(
    app: &mut App,
    mouse: MouseEvent,
    term_width: u16,
    term_height: u16,
) -> Option<Command> {
    let area = Rect::new(0, 0, term_width, term_height);
    let popup_area = crate::tui::screens::modal::modal_medium(area);

    let is_inside = mouse.column >= popup_area.x
        && mouse.column < popup_area.x.saturating_add(popup_area.width)
        && mouse.row >= popup_area.y
        && mouse.row < popup_area.y.saturating_add(popup_area.height);

    match mouse.kind {
        MouseEventKind::Down(MouseButton::Left) => {
            if !is_inside {
                close_applied_filters_modal(app);
                return None;
            }

            // Clicked inside modal: determine item
            let inner_y = popup_area.y + 1;
            if mouse.row >= inner_y {
                let clicked_row = (mouse.row - inner_y) as usize;
                if let Some(ref mut modal) = app.applied_filters_modal
                    && clicked_row < modal.items.len()
                {
                    modal.selected = clicked_row;
                }
            }
            None
        }
        MouseEventKind::ScrollDown => {
            if let Some(ref mut modal) = app.applied_filters_modal
                && !modal.items.is_empty()
            {
                modal.selected = (modal.selected + 1).min(modal.items.len().saturating_sub(1));
            }
            None
        }
        MouseEventKind::ScrollUp => {
            if let Some(ref mut modal) = app.applied_filters_modal {
                modal.selected = modal.selected.saturating_sub(1);
            }
            None
        }
        _ => None,
    }
}

fn remove_selected_filter(app: &mut App) -> Option<Command> {
    let (item_to_remove, _new_selected) = {
        let modal = app.applied_filters_modal.as_mut()?;
        if modal.items.is_empty() {
            return None;
        }
        let sel = modal.selected.min(modal.items.len() - 1);
        let item = modal.items.remove(sel);
        let next_sel = if modal.items.is_empty() {
            0
        } else {
            sel.min(modal.items.len() - 1)
        };
        modal.selected = next_sel;
        (item, next_sel)
    };

    match item_to_remove.kind {
        AppliedFilterKind::Column => {
            if let Screen::Actions(ref mut state) = app.screen
                && let Some(ref mut res) = state.results
            {
                res.column_filters.remove(&item_to_remove.name);
            }
            reapply_in_memory_filters(app)
        }
        AppliedFilterKind::Partition => {
            if let Screen::Actions(ref mut state) = app.screen {
                crate::tui::handler::navigation::drilldown::drilldown_go_up(state)
            } else {
                None
            }
        }
    }
}

fn clear_all_filters(app: &mut App) -> Option<Command> {
    if let Some(ref mut modal) = app.applied_filters_modal {
        modal
            .items
            .retain(|i| i.kind == AppliedFilterKind::Partition);
        modal.selected = 0;
    }

    if let Screen::Actions(ref mut state) = app.screen
        && let Some(ref mut res) = state.results
    {
        res.column_filters.clear();
    }
    reapply_in_memory_filters(app)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{ActionState, DrillDownState, ResultsState};
    use std::collections::{HashMap, HashSet};

    fn make_test_app() -> App {
        let config = crate::config::ConnectionConfig {
            url: "https://trino.example".to_string(),
            user: "analyst".to_string(),
            password: "secret".to_string(),
            query_cache_limit: crate::config::DEFAULT_QUERY_CACHE_LIMIT,
        };
        let mut app = App::new(config, false);
        let mut column_filters = HashMap::new();
        column_filters.insert(
            "status".to_string(),
            ColumnFilter::Values(HashSet::from(["active".to_string()])),
        );
        let res = ResultsState {
            filters: vec![("dt".to_string(), "2026-10-14".to_string())],
            column_filters,
            columns: vec!["dt".to_string(), "status".to_string()],
            rows: vec![
                vec!["2026-10-14".to_string(), "active".to_string()],
                vec!["2026-10-14".to_string(), "inactive".to_string()],
            ],
            ..Default::default()
        };

        app.screen = Screen::Actions(ActionState {
            catalog: "cat".to_string(),
            schema: "sch".to_string(),
            table: "tbl".to_string(),
            results: Some(res),
            drilldown: Some(DrillDownState {
                partition_cols: vec!["dt".to_string()],
                path: vec![("dt".to_string(), "2026-10-14".to_string())],
                ..Default::default()
            }),
            ..Default::default()
        });
        app
    }

    #[test]
    fn test_open_applied_filters_modal_collects_partition_and_column_filters() {
        let mut app = make_test_app();
        open_applied_filters_modal(&mut app);
        assert!(app.applied_filters_modal.is_some());
        let modal = app.applied_filters_modal.as_ref().unwrap();
        assert_eq!(modal.items.len(), 2);
        assert_eq!(modal.items[0].kind, AppliedFilterKind::Partition);
        assert_eq!(modal.items[0].name, "dt");
        assert_eq!(modal.items[1].kind, AppliedFilterKind::Column);
        assert_eq!(modal.items[1].name, "status");
    }

    #[test]
    fn test_remove_filter_with_d() {
        let mut app = make_test_app();
        open_applied_filters_modal(&mut app);
        // Move selection to item 1 (status column filter)
        handle_applied_filters_modal_key(&mut app, KeyEvent::from(KeyCode::Char('j')));
        assert_eq!(app.applied_filters_modal.as_ref().unwrap().selected, 1);

        // Delete selected filter with 'd'
        handle_applied_filters_modal_key(&mut app, KeyEvent::from(KeyCode::Char('d')));
        let modal = app.applied_filters_modal.as_ref().unwrap();
        assert_eq!(modal.items.len(), 1);
        assert_eq!(modal.items[0].kind, AppliedFilterKind::Partition);

        // Verify column filter was removed from results state
        if let Screen::Actions(a) = &app.screen {
            let res = a.results.as_ref().unwrap();
            assert!(!res.column_filters.contains_key("status"));
        } else {
            panic!();
        }
    }

    #[test]
    fn test_clear_all_filters_with_c() {
        let mut app = make_test_app();
        open_applied_filters_modal(&mut app);
        handle_applied_filters_modal_key(&mut app, KeyEvent::from(KeyCode::Char('c')));

        let modal = app.applied_filters_modal.as_ref().unwrap();
        // Column filters cleared, only partition filters retained
        assert_eq!(modal.items.len(), 1);
        assert_eq!(modal.items[0].kind, AppliedFilterKind::Partition);
    }

    #[test]
    fn test_close_applied_filters_modal_with_esc() {
        let mut app = make_test_app();
        open_applied_filters_modal(&mut app);
        assert!(app.applied_filters_modal.is_some());
        handle_applied_filters_modal_key(&mut app, KeyEvent::from(KeyCode::Esc));
        assert!(app.applied_filters_modal.is_none());
    }
}
