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

use crossterm::event::{KeyModifiers, MouseButton, MouseEvent, MouseEventKind};

use crate::app::*;

use super::{
    Command,
    navigation::{
        check_trigger_infinite_scroll, extract_list_labels, get_selected, mod_list_selected,
        trigger_action,
    },
    query::{
        active_query_buffer_len, active_query_state_mut, copy_to_clipboard, query_bar_layout,
        query_text_index_from_mouse,
    },
};

pub fn handle_mouse_sync(app: &mut App, mouse: MouseEvent) -> Option<Command> {
    let (term_width, term_height) = crossterm::terminal::size().unwrap_or((80, 24));
    if term_width == 0 || term_height == 0 {
        return None;
    }

    let bottom_y = term_height.saturating_sub(7);
    let border_x = ((term_width as u32 * app.main_panel_pct as u32) / 100) as u16;
    let is_in_table = matches!(app.screen, Screen::Actions(_));

    match mouse.kind {
        MouseEventKind::Down(MouseButton::Left) => {
            if let Some((query_x, query_y, query_width, query_height, inner_w)) =
                query_bar_layout(app, term_width, term_height)
            {
                let inside_query_bar = mouse.column >= query_x
                    && mouse.column < query_x.saturating_add(query_width)
                    && mouse.row >= query_y
                    && mouse.row < query_y.saturating_add(query_height);

                if inside_query_bar {
                    let idx = query_text_index_from_mouse(
                        mouse.column,
                        mouse.row,
                        query_x,
                        query_y,
                        inner_w,
                        active_query_buffer_len(app).unwrap_or(0),
                    );
                    app.mode = Mode::QueryInput;
                    app.active_panel = ActivePanel::MainViewer;
                    app.is_dragging_query_select = true;
                    app.is_selecting_text = false;
                    app.mouse_selection_anchor = None;
                    app.mouse_selection_current = None;
                    if let Some(state) = active_query_state_mut(app) {
                        state.selection_anchor = Some(idx);
                        state.query_cursor = idx;
                        state.invalid_query_error = None;
                    }
                    return None;
                }
            }

            app.mouse_selection_anchor = Some((mouse.column, mouse.row));
            app.mouse_selection_current = Some((mouse.column, mouse.row));
            app.is_selecting_text = true;

            if mouse.row < bottom_y && (mouse.column as i32 - border_x as i32).abs() <= 1 {
                app.is_dragging_resizer = true;
            } else {
                app.is_dragging_resizer = false;

                if mouse.column < border_x && mouse.row < bottom_y {
                    if is_in_table {
                        let clicked_row = mouse.row.saturating_sub(1) as usize;
                        if clicked_row < ACTIONS.len() {
                            return trigger_action(app, clicked_row);
                        } else {
                            app.active_panel = ActivePanel::MenuPane;
                        }
                    } else {
                        app.mode = Mode::Normal;
                        app.active_panel = ActivePanel::MainViewer;
                    }
                } else if mouse.column >= border_x && mouse.row < bottom_y {
                    app.mode = Mode::Normal;
                    app.active_panel = ActivePanel::MainViewer;
                }
            }
        }
        MouseEventKind::Drag(MouseButton::Left) => {
            if app.is_dragging_query_select {
                if let Some((query_x, query_y, _, _, inner_w)) =
                    query_bar_layout(app, term_width, term_height)
                {
                    let idx = query_text_index_from_mouse(
                        mouse.column,
                        mouse.row,
                        query_x,
                        query_y,
                        inner_w,
                        active_query_buffer_len(app).unwrap_or(0),
                    );
                    if let Some(state) = active_query_state_mut(app) {
                        state.query_cursor = idx;
                        if state.selection_anchor == Some(idx) {
                            state.clear_selection();
                        }
                    }
                }
                return None;
            }
            if app.is_selecting_text {
                app.mouse_selection_current = Some((mouse.column, mouse.row));
            }
            if app.is_dragging_resizer {
                let pct = ((mouse.column as u32 * 100) / term_width as u32) as u16;
                app.main_panel_pct = pct.clamp(8, 80);
            }
        }
        MouseEventKind::Up(MouseButton::Left) => {
            app.is_dragging_resizer = false;
            if app.is_dragging_query_select {
                app.is_dragging_query_select = false;
                if let Some(state) = active_query_state_mut(app)
                    && state.selection_anchor == Some(state.query_cursor)
                {
                    state.clear_selection();
                }
                return None;
            }
            if app.is_selecting_text {
                app.is_selecting_text = false;
                if let (Some(anchor), Some(current)) =
                    (app.mouse_selection_anchor, app.mouse_selection_current)
                {
                    let text = extract_selected_text(app, anchor, current);
                    if !text.is_empty() {
                        copy_to_clipboard(&text);
                        app.copied_toast =
                            Some((text.chars().take(30).collect(), std::time::Instant::now()));
                    }
                }
            }
        }
        MouseEventKind::ScrollDown => {
            let shift = mouse.modifiers.contains(KeyModifiers::SHIFT);
            if is_in_table {
                let selected_idx = match &app.screen {
                    Screen::Actions(a) => a.selected,
                    _ => 0,
                };
                if selected_idx == 7 {
                    let max_lines = app.partition_tree_lines.len().saturating_sub(1);
                    app.partition_scroll = (app.partition_scroll + 1).min(max_lines);
                } else if selected_idx == 8 {
                    let max_cols = app.vertical_schema_cols.len().saturating_sub(1);
                    app.schema_scroll = (app.schema_scroll + 1).min(max_cols);
                } else if let Screen::Actions(ref mut a) = app.screen
                    && let Some(ref mut res) = a.results
                {
                    if shift {
                        if !res.columns.is_empty() {
                            res.scroll_h =
                                (res.scroll_h + 1).min(res.columns.len().saturating_sub(1));
                        }
                    } else if !res.rows.is_empty() {
                        res.scroll_v = (res.scroll_v + 1).min(res.rows.len().saturating_sub(1));
                        return check_trigger_infinite_scroll(app);
                    }
                }
            } else if let Some(items) = extract_list_labels(&app.screen)
                && !items.is_empty()
                && let Some(s) = get_selected(&app.screen)
            {
                mod_list_selected(&mut app.screen, (s + 1).min(items.len().saturating_sub(1)));
            }
        }
        MouseEventKind::ScrollUp => {
            let shift = mouse.modifiers.contains(KeyModifiers::SHIFT);
            if is_in_table {
                let selected_idx = match &app.screen {
                    Screen::Actions(a) => a.selected,
                    _ => 0,
                };
                if selected_idx == 7 {
                    app.partition_scroll = app.partition_scroll.saturating_sub(1);
                } else if selected_idx == 8 {
                    app.schema_scroll = app.schema_scroll.saturating_sub(1);
                } else if let Screen::Actions(ref mut a) = app.screen
                    && let Some(ref mut res) = a.results
                {
                    if shift {
                        res.scroll_h = res.scroll_h.saturating_sub(1);
                    } else {
                        res.scroll_v = res.scroll_v.saturating_sub(1);
                    }
                }
            } else if let Some(s) = get_selected(&app.screen) {
                mod_list_selected(&mut app.screen, s.saturating_sub(1));
            }
        }
        _ => {}
    }
    None
}

pub fn extract_selected_text(app: &App, anchor: (u16, u16), current: (u16, u16)) -> String {
    let (term_width, term_height) = crossterm::terminal::size().unwrap_or((80, 24));
    if term_width == 0 || term_height == 0 {
        return String::new();
    }

    let bottom_y = term_height.saturating_sub(7);
    let border_x = ((term_width as u32 * app.main_panel_pct as u32) / 100) as u16;
    let height_right = bottom_y;
    let border_y = ((height_right as u32 * app.control_panel_split_pct as u32) / 100) as u16;

    let start_row = anchor.1.min(current.1);
    let end_row = anchor.1.max(current.1);

    if anchor.1 >= bottom_y || current.1 >= bottom_y {
        let mut lines = Vec::new();
        let inner_y = bottom_y + 1;
        let rev_logs: Vec<&QueryLogEntry> = app.query_logs.iter().rev().collect();
        for r in start_row..=end_row {
            if r >= inner_y && r < term_height.saturating_sub(1) {
                let idx = (r - inner_y) as usize + app.query_inspector_scroll;
                if idx < rev_logs.len() {
                    lines.push(rev_logs[idx].sql.clone());
                }
            }
        }
        return lines.join("\n");
    }

    if anchor.0 < border_x && current.0 < border_x {
        match &app.screen {
            Screen::Catalog(s) => {
                let mut lines = Vec::new();
                let inner_y = 4;
                for r in start_row..=end_row {
                    if r >= inner_y {
                        let idx = (r - inner_y) as usize;
                        if idx < s.items.len() {
                            lines.push(s.items[idx].clone());
                        }
                    }
                }
                return lines.join("\n");
            }
            Screen::Schema(s) => {
                let mut lines = Vec::new();
                let inner_y = 4;
                for r in start_row..=end_row {
                    if r >= inner_y {
                        let idx = (r - inner_y) as usize;
                        if idx < s.items.len() {
                            lines.push(s.items[idx].clone());
                        }
                    }
                }
                return lines.join("\n");
            }
            Screen::Table(s) => {
                let mut lines = Vec::new();
                let inner_y = 4;
                for r in start_row..=end_row {
                    if r >= inner_y {
                        let idx = (r - inner_y) as usize;
                        if idx < s.items.len() {
                            lines.push(s.items[idx].clone());
                        }
                    }
                }
                return lines.join("\n");
            }
            Screen::Actions(_) => {
                let mut lines = Vec::new();
                let inner_y = 4;
                for r in start_row..=end_row {
                    if r >= inner_y {
                        let idx = (r - inner_y) as usize;
                        if idx < ACTIONS.len() {
                            lines.push(ACTIONS[idx].1.to_string());
                        }
                    }
                }
                return lines.join("\n");
            }
            _ => {}
        }
    }

    if anchor.0 >= border_x && current.0 >= border_x {
        if start_row < border_y {
            let mut lines = Vec::new();
            let inner_y = 1;
            for r in start_row..=end_row {
                if r >= inner_y {
                    let idx = (r - inner_y) as usize + app.partition_scroll;
                    if idx < app.partition_tree_lines.len() {
                        lines.push(app.partition_tree_lines[idx].clone());
                    }
                }
            }
            return lines.join("\n");
        } else {
            let mut lines = Vec::new();
            let inner_y = border_y + 1;
            for r in start_row..=end_row {
                if r >= inner_y {
                    let idx = (r - inner_y) as usize + app.schema_scroll;
                    if idx < app.vertical_schema_cols.len() {
                        let col = &app.vertical_schema_cols[idx];
                        lines.push(format!("{} {}", col.name, col.data_type));
                    }
                }
            }
            return lines.join("\n");
        }
    }

    String::new()
}
