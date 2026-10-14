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
use ratatui::layout::Rect;

use crate::app::*;

use super::{
    Command,
    navigation::{
        check_trigger_infinite_scroll, clipboard::format_csv_row, extract_list_labels,
        get_selected, mod_list_selected, trigger_action,
    },
    query::{
        active_query_buffer, active_query_state_mut, copy_to_clipboard, query_bar_layout,
        query_text_index_from_mouse,
    },
};

pub fn handle_mouse_sync(app: &mut App, mouse: MouseEvent) -> Option<Command> {
    let (term_width, term_height) = crossterm::terminal::size().unwrap_or((80, 24));
    if term_width == 0 || term_height == 0 {
        return None;
    }

    if app.filter_modal.is_some() {
        return handle_filter_modal_mouse(app, mouse, term_width, term_height);
    }
    if app.column_modal.is_some() {
        return handle_column_modal_mouse(app, mouse, term_width, term_height);
    }
    if app.help_modal.is_some() {
        return handle_help_modal_mouse(app, mouse, term_width, term_height);
    }
    if app.saved_views_modal.is_some() {
        return crate::tui::handler::saved_views_modal::handle_saved_views_modal_mouse(
            app,
            mouse,
            term_width,
            term_height,
        );
    }
    if app.save_view_prompt.is_some() {
        return crate::tui::handler::saved_views_modal::handle_save_view_prompt_mouse(
            app,
            mouse,
            term_width,
            term_height,
        );
    }

    let bottom_y = term_height.saturating_sub(7);
    let is_in_table = matches!(app.screen, Screen::Actions(_));
    let border_pct = if is_in_table {
        if app.main_panel_pct > 30 {
            15
        } else {
            app.main_panel_pct.clamp(8, 30)
        }
    } else if app.main_panel_pct <= 30 {
        60
    } else {
        app.main_panel_pct
    };
    let border_x = ((term_width as u32 * border_pct as u32) / 100) as u16;

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
                    let buf = active_query_buffer(app).unwrap_or_default();
                    let idx = query_text_index_from_mouse(
                        mouse.column,
                        mouse.row,
                        query_x,
                        query_y,
                        inner_w,
                        &buf,
                    );
                    let is_table_view = match &app.screen {
                        Screen::Actions(a) => a.selected == 0,
                        _ => false,
                    };
                    if is_table_view {
                        app.mode = Mode::QueryInput;
                    }
                    app.set_active_panel(ActivePanel::MainViewer);
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
            app.is_selecting_text = false;

            if mouse.row < bottom_y && mouse.column == border_x {
                app.is_dragging_resizer = true;
            } else {
                app.is_dragging_resizer = false;

                if mouse.column < border_x && mouse.row < bottom_y {
                    if is_in_table {
                        let clicked_row = mouse.row.saturating_sub(1) as usize;
                        let line_counts = crate::tui::screens::actions::item_line_counts(
                            border_x.saturating_sub(2),
                        );
                        let mut acc = 0;
                        let mut clicked_idx = None;
                        for (idx, count) in line_counts.iter().enumerate() {
                            if clicked_row >= acc && clicked_row < acc + count {
                                clicked_idx = Some(idx);
                                break;
                            }
                            acc += count;
                        }
                        if let Some(idx) = clicked_idx {
                            return trigger_action(app, idx);
                        } else {
                            app.set_active_panel(ActivePanel::MenuPane);
                        }
                    } else {
                        app.mode = Mode::Normal;
                        app.set_active_panel(ActivePanel::MainViewer);
                    }
                } else if mouse.column >= border_x && mouse.row < bottom_y {
                    app.mode = Mode::Normal;
                    app.set_active_panel(ActivePanel::MainViewer);

                    if let Some(inner) = results_table_inner_rect(app, term_width, term_height)
                        && let Screen::Actions(ref mut a) = app.screen
                        && let Some(ref mut res) = a.results
                    {
                        let has_h_scrollbar = res.visible_columns().len() > 1;
                        let is_on_h_scrollbar = has_h_scrollbar
                            && mouse.row >= inner.y + inner.height.saturating_sub(1)
                            && mouse.row <= inner.y + inner.height
                            && mouse.column >= inner.x
                            && mouse.column < inner.x + inner.width;

                        if is_on_h_scrollbar {
                            let avail_width = (inner.width as usize).saturating_sub(4).max(1);
                            let max_scroll_h = crate::tui::screens::results::compute_max_scroll_h(
                                res,
                                avail_width,
                            );

                            if mouse.column <= inner.x + 1 {
                                crate::tui::screens::results::scroll_table_horizontal_left(
                                    res,
                                    avail_width,
                                );
                            } else if mouse.column >= inner.x + inner.width.saturating_sub(2) {
                                crate::tui::screens::results::scroll_table_horizontal_right(
                                    res,
                                    avail_width,
                                );
                            } else {
                                let track_start = inner.x + 1;
                                let track_width = (inner.width as usize).saturating_sub(2).max(1);
                                let offset = (mouse.column.saturating_sub(track_start) as usize)
                                    .min(track_width.saturating_sub(1));
                                let target = (offset * (max_scroll_h + 1)) / track_width;
                                crate::tui::screens::results::scroll_table_horizontal_to(
                                    res,
                                    avail_width,
                                    target.min(max_scroll_h),
                                );
                            }
                            app.is_dragging_h_scrollbar = true;
                            app.is_selecting_text = false;
                            app.mouse_selection_anchor = None;
                            app.mouse_selection_current = None;
                        } else if let Some((row_idx, col_idx)) =
                            crate::tui::screens::results::hit_test_cell(
                                res,
                                inner,
                                mouse.column,
                                mouse.row,
                            )
                        {
                            let is_shift = mouse.modifiers.contains(KeyModifiers::SHIFT);
                            if is_shift {
                                if res.visual_mode == VisualMode::None {
                                    res.visual_mode = VisualMode::Block {
                                        anchor_row: res.selected_row,
                                        anchor_col: res.selected_col,
                                    };
                                }
                                res.selected_row = row_idx;
                                res.selected_col = col_idx;
                            } else {
                                res.exit_visual_mode();
                                res.selected_row = row_idx;
                                res.selected_col = col_idx;
                            }
                            res.last_selected_row.set(row_idx);
                            res.last_selected_col.set(col_idx);
                            app.clear_mouse_selection();
                        }
                    } else if let Screen::Actions(ref mut a) = app.screen
                        && let Some(ref mut dd) = a.drilldown
                        && a.results.is_none()
                    {
                        let preview_inner_y = 3 + 3 + 1;
                        let list_start_y = if dd.truncated {
                            preview_inner_y + 1
                        } else {
                            preview_inner_y
                        };
                        if mouse.row >= list_start_y {
                            let idx = (mouse.row - list_start_y) as usize;
                            let depth = dd.depth();
                            if let Some(items) = dd.levels_cache.get(depth)
                                && idx < items.len()
                            {
                                dd.selected = idx;
                                return crate::tui::handler::navigation::actions::maybe_fetch_selected_partition_count(a);
                            }
                        }
                    }
                }
            }
        }
        MouseEventKind::Drag(MouseButton::Left) => {
            if app.is_dragging_h_scrollbar {
                if let Some(inner) = results_table_inner_rect(app, term_width, term_height)
                    && let Screen::Actions(ref mut a) = app.screen
                    && let Some(ref mut res) = a.results
                {
                    let all_vis = res.visible_columns();
                    if all_vis.len() > 1 {
                        let avail_width = (inner.width as usize).saturating_sub(4).max(1);
                        let max_scroll_h =
                            crate::tui::screens::results::compute_max_scroll_h(res, avail_width);
                        let track_start = inner.x + 1;
                        let track_width = (inner.width as usize).saturating_sub(2).max(1);
                        let offset = (mouse.column.saturating_sub(track_start) as usize)
                            .min(track_width.saturating_sub(1));
                        let target = (offset * (max_scroll_h + 1)) / track_width;
                        crate::tui::screens::results::scroll_table_horizontal_to(
                            res,
                            avail_width,
                            target.min(max_scroll_h),
                        );
                    }
                }
                return None;
            }
            if app.is_dragging_query_select {
                if let Some((query_x, query_y, _, _, inner_w)) =
                    query_bar_layout(app, term_width, term_height)
                {
                    let buf = active_query_buffer(app).unwrap_or_default();
                    let idx = query_text_index_from_mouse(
                        mouse.column,
                        mouse.row,
                        query_x,
                        query_y,
                        inner_w,
                        &buf,
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
            if let Some(inner) = results_table_inner_rect(app, term_width, term_height)
                && let Screen::Actions(ref mut a) = app.screen
                && let Some(ref mut res) = a.results
                && !app.is_dragging_h_scrollbar
                && !app.is_dragging_resizer
                && !app.is_dragging_query_select
                && let Some((row_idx, col_idx)) =
                    crate::tui::screens::results::hit_test_cell(res, inner, mouse.column, mouse.row)
            {
                if res.visual_mode == VisualMode::None {
                    res.visual_mode = VisualMode::Block {
                        anchor_row: res.selected_row,
                        anchor_col: res.selected_col,
                    };
                }
                res.selected_row = row_idx;
                res.selected_col = col_idx;
                res.last_selected_row.set(row_idx);
                res.last_selected_col.set(col_idx);
                app.clear_mouse_selection();
                return None;
            }
            if let Some((a_x, a_y)) = app.mouse_selection_anchor
                && (mouse.column != a_x || mouse.row != a_y)
            {
                app.is_selecting_text = true;
                app.mouse_selection_current = Some((mouse.column, mouse.row));
            }
            if app.is_dragging_resizer {
                let pct = ((mouse.column as u32 * 100) / term_width as u32) as u16;
                app.main_panel_pct = pct.clamp(8, 80);
            }
        }
        MouseEventKind::Down(MouseButton::Right) => {
            if let Some(inner) = results_table_inner_rect(app, term_width, term_height)
                && let Screen::Actions(ref mut a) = app.screen
                && let Some(ref mut res) = a.results
                && let Some((row_idx, col_idx)) =
                    crate::tui::screens::results::hit_test_cell(res, inner, mouse.column, mouse.row)
            {
                if res.visual_mode == VisualMode::None {
                    res.selected_row = row_idx;
                    res.selected_col = col_idx;
                }
                crate::tui::handler::navigation::clipboard::copy_results_selection_or_cell(app);
                return None;
            }
        }
        MouseEventKind::Up(MouseButton::Left) => {
            app.is_dragging_resizer = false;
            app.is_dragging_h_scrollbar = false;
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
                    && anchor != current
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
            if app.help_modal.is_some() || matches!(app.screen, Screen::Help) {
                if let Some(ref mut m) = app.help_modal {
                    m.scroll = m.scroll.saturating_add(1);
                    app.help_scroll = m.scroll;
                } else {
                    app.help_scroll = app.help_scroll.saturating_add(1);
                }
                return None;
            }
            let shift = mouse.modifiers.contains(KeyModifiers::SHIFT);
            let alt = mouse.modifiers.contains(KeyModifiers::ALT);
            let ctrl = mouse.modifiers.contains(KeyModifiers::CONTROL);
            let table_rect = results_table_inner_rect(app, term_width, term_height);
            let avail_width = table_rect
                .map(|r| (r.width as usize).saturating_sub(4).max(1))
                .unwrap_or(80);

            let is_on_h_scrollbar = table_rect
                .map(|inner| {
                    mouse.row >= inner.y + inner.height.saturating_sub(2)
                        && mouse.row <= inner.y + inner.height
                        && mouse.column >= inner.x
                        && mouse.column < inner.x + inner.width
                })
                .unwrap_or(false);

            let is_on_header = table_rect
                .map(|inner| {
                    mouse.row <= inner.y + 1
                        && mouse.column >= inner.x
                        && mouse.column < inner.x + inner.width
                })
                .unwrap_or(false);

            let is_horizontal = shift || alt || ctrl || is_on_h_scrollbar || is_on_header;

            if is_in_table {
                let selected_idx = match &app.screen {
                    Screen::Actions(a) => a.selected,
                    _ => 0,
                };
                let action = ACTIONS.get(selected_idx).map(|(_, _, a)| a);
                if matches!(action, Some(Action::Partitions)) {
                    let max_lines = app.partition_tree_lines.len().saturating_sub(1);
                    app.partition_scroll = (app.partition_scroll + 3).min(max_lines);
                } else if matches!(action, Some(Action::Schema)) {
                    let max_cols = app.vertical_schema_cols.len().saturating_sub(1);
                    app.schema_scroll = (app.schema_scroll + 3).min(max_cols);
                } else if let Screen::Actions(ref mut a) = app.screen
                    && let Some(ref mut dd) = a.drilldown
                    && a.results.is_none()
                {
                    let depth = dd.depth();
                    if let Some(items) = dd.levels_cache.get(depth)
                        && !items.is_empty()
                    {
                        dd.selected = (dd.selected + 3).min(items.len().saturating_sub(1));
                    }
                    return crate::tui::handler::navigation::actions::maybe_fetch_selected_partition_count(a);
                } else if let Screen::Actions(ref mut a) = app.screen
                    && let Some(ref mut res) = a.results
                {
                    if is_horizontal {
                        crate::tui::screens::results::scroll_table_horizontal_right(
                            res,
                            avail_width,
                        );
                    } else if !res.rows.is_empty() {
                        let delta = 3;
                        let new_scroll_v =
                            (res.scroll_v.get() + delta).min(res.rows.len().saturating_sub(1));
                        res.scroll_v.set(new_scroll_v);
                        if res.selected_row < new_scroll_v {
                            res.selected_row = new_scroll_v;
                            res.last_selected_row.set(new_scroll_v);
                        }
                        return check_trigger_infinite_scroll(app);
                    }
                }
            } else if let Some(items) = extract_list_labels(app)
                && !items.is_empty()
                && let Some(s) = get_selected(&app.screen)
            {
                mod_list_selected(&mut app.screen, (s + 1).min(items.len().saturating_sub(1)));
            }
        }
        MouseEventKind::ScrollUp => {
            if app.help_modal.is_some() || matches!(app.screen, Screen::Help) {
                if let Some(ref mut m) = app.help_modal {
                    m.scroll = m.scroll.saturating_sub(1);
                    app.help_scroll = m.scroll;
                } else {
                    app.help_scroll = app.help_scroll.saturating_sub(1);
                }
                return None;
            }
            let shift = mouse.modifiers.contains(KeyModifiers::SHIFT);
            let alt = mouse.modifiers.contains(KeyModifiers::ALT);
            let ctrl = mouse.modifiers.contains(KeyModifiers::CONTROL);
            let table_rect = results_table_inner_rect(app, term_width, term_height);
            let view_rows = table_rect
                .map(|r| (r.height as usize).saturating_sub(3).max(1))
                .unwrap_or(15);
            let avail_width = table_rect
                .map(|r| (r.width as usize).saturating_sub(4).max(1))
                .unwrap_or(80);

            let is_on_h_scrollbar = table_rect
                .map(|inner| {
                    mouse.row >= inner.y + inner.height.saturating_sub(2)
                        && mouse.row <= inner.y + inner.height
                        && mouse.column >= inner.x
                        && mouse.column < inner.x + inner.width
                })
                .unwrap_or(false);

            let is_on_header = table_rect
                .map(|inner| {
                    mouse.row <= inner.y + 1
                        && mouse.column >= inner.x
                        && mouse.column < inner.x + inner.width
                })
                .unwrap_or(false);

            let is_horizontal = shift || alt || ctrl || is_on_h_scrollbar || is_on_header;

            if is_in_table {
                let selected_idx = match &app.screen {
                    Screen::Actions(a) => a.selected,
                    _ => 0,
                };
                let action = ACTIONS.get(selected_idx).map(|(_, _, a)| a);
                if matches!(action, Some(Action::Partitions)) {
                    app.partition_scroll = app.partition_scroll.saturating_sub(3);
                } else if matches!(action, Some(Action::Schema)) {
                    app.schema_scroll = app.schema_scroll.saturating_sub(3);
                } else if let Screen::Actions(ref mut a) = app.screen
                    && let Some(ref mut dd) = a.drilldown
                    && a.results.is_none()
                {
                    dd.selected = dd.selected.saturating_sub(3);
                    return crate::tui::handler::navigation::actions::maybe_fetch_selected_partition_count(a);
                } else if let Screen::Actions(ref mut a) = app.screen
                    && let Some(ref mut res) = a.results
                {
                    if is_horizontal {
                        crate::tui::screens::results::scroll_table_horizontal_left(
                            res,
                            avail_width,
                        );
                    } else {
                        let delta = 3;
                        let new_scroll_v = res.scroll_v.get().saturating_sub(delta);
                        res.scroll_v.set(new_scroll_v);
                        let max_visible_row = new_scroll_v + view_rows.saturating_sub(1);
                        if res.selected_row > max_visible_row {
                            res.selected_row =
                                max_visible_row.min(res.rows.len().saturating_sub(1));
                            res.last_selected_row.set(res.selected_row);
                        }
                    }
                }
            } else if let Some(s) = get_selected(&app.screen) {
                mod_list_selected(&mut app.screen, s.saturating_sub(1));
            }
        }
        MouseEventKind::ScrollRight => {
            let avail_width = results_table_inner_rect(app, term_width, term_height)
                .map(|r| (r.width as usize).saturating_sub(4).max(1))
                .unwrap_or(80);
            if is_in_table
                && let Screen::Actions(ref mut a) = app.screen
                && let Some(ref mut res) = a.results
            {
                crate::tui::screens::results::scroll_table_horizontal_right(res, avail_width);
            }
        }
        MouseEventKind::ScrollLeft => {
            let avail_width = results_table_inner_rect(app, term_width, term_height)
                .map(|r| (r.width as usize).saturating_sub(4).max(1))
                .unwrap_or(80);
            if is_in_table
                && let Screen::Actions(ref mut a) = app.screen
                && let Some(ref mut res) = a.results
            {
                crate::tui::screens::results::scroll_table_horizontal_left(res, avail_width);
            }
        }
        _ => {}
    }
    None
}

pub fn results_table_inner_rect(app: &App, term_width: u16, term_height: u16) -> Option<Rect> {
    let bottom_y = term_height.saturating_sub(7);
    if bottom_y == 0 {
        return None;
    }
    let Screen::Actions(a) = &app.screen else {
        return None;
    };
    a.results.as_ref()?;

    let menu_pct = if app.main_panel_pct > 30 {
        15
    } else {
        app.main_panel_pct.clamp(8, 30)
    };
    let border_x = ((term_width as u32 * menu_pct as u32) / 100) as u16;
    let preview_width = term_width.saturating_sub(border_x);
    let inner_w = preview_width.saturating_sub(2).max(1) as usize;

    let query_height = if matches!(app.mode, Mode::QueryInput) {
        let total_chars = 7 + a
            .results
            .as_ref()
            .map(|r| r.query_buffer.len())
            .unwrap_or_else(|| a.query_buffer.len());
        let lines = total_chars.div_ceil(inner_w);
        (lines as u16 + 2).clamp(3, 7)
    } else {
        3
    };

    let top_offset = query_height + 3;
    if top_offset >= bottom_y {
        return None;
    }

    let preview_height = bottom_y.saturating_sub(top_offset);
    if preview_width < 2 || preview_height < 2 {
        return None;
    }

    Some(Rect {
        x: border_x + 1,
        y: top_offset + 1,
        width: preview_width.saturating_sub(2),
        height: preview_height.saturating_sub(2),
    })
}

pub fn extract_selected_text(app: &App, anchor: (u16, u16), current: (u16, u16)) -> String {
    let (term_width, term_height) = crossterm::terminal::size().unwrap_or((80, 24));
    if term_width == 0 || term_height == 0 {
        return String::new();
    }

    let bottom_y = term_height.saturating_sub(7);

    let start_row = anchor.1.min(current.1);
    let end_row = anchor.1.max(current.1);
    let min_col = anchor.0.min(current.0);
    let max_col = anchor.0.max(current.0);

    // 1. Selection in Bottom Query Inspector Pane
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

    let is_in_table = matches!(app.screen, Screen::Actions(_));

    if !is_in_table {
        // Phase 1 screens: Catalog, Schema, Table
        let list_pct = if app.main_panel_pct <= 30 {
            60
        } else {
            app.main_panel_pct
        };
        let border_x = ((term_width as u32 * list_pct as u32) / 100) as u16;

        let inner_w = border_x.saturating_sub(2).max(1) as usize;
        let search_active = matches!(app.mode, Mode::Search);
        let search_height = if search_active {
            let total_chars = 3 + app.search_query.len();
            let lines = total_chars.div_ceil(inner_w);
            (lines as u16 + 2).clamp(3, 8)
        } else {
            3
        };

        if min_col < border_x {
            let inner_y = search_height + 3 + 1;
            match &app.screen {
                Screen::Catalog(s) => {
                    let mut lines = Vec::new();
                    for r in start_row..=end_row {
                        if r >= inner_y {
                            let idx = (r - inner_y) as usize;
                            if idx < s.items.len() {
                                lines.push(s.items[idx].trim().to_string());
                            }
                        }
                    }
                    return lines.join("\n");
                }
                Screen::Schema(s) => {
                    let mut lines = Vec::new();
                    for r in start_row..=end_row {
                        if r >= inner_y {
                            let idx = (r - inner_y) as usize;
                            if idx < s.items.len() {
                                lines.push(s.items[idx].trim().to_string());
                            }
                        }
                    }
                    return lines.join("\n");
                }
                Screen::Table(s) => {
                    let mut lines = Vec::new();
                    for r in start_row..=end_row {
                        if r >= inner_y {
                            let idx = (r - inner_y) as usize;
                            if idx < s.items.len() {
                                lines.push(s.items[idx].trim().to_string());
                            }
                        }
                    }
                    return lines.join("\n");
                }
                _ => {}
            }
        }
    } else if let Screen::Actions(state) = &app.screen {
        // Phase 2 screen: Screen::Actions
        let menu_pct = if app.main_panel_pct > 30 {
            15
        } else {
            app.main_panel_pct.clamp(8, 30)
        };
        let border_x = ((term_width as u32 * menu_pct as u32) / 100) as u16;

        let preview_w = term_width.saturating_sub(border_x);
        let inner_w = preview_w.saturating_sub(2).max(1) as usize;

        let query_active = matches!(app.mode, Mode::QueryInput);
        let query_height = if query_active {
            let total_chars = 7 + state
                .results
                .as_ref()
                .map(|r| r.query_buffer.len())
                .unwrap_or_else(|| state.query_buffer.len());
            let lines = total_chars.div_ceil(inner_w);
            (lines as u16 + 2).clamp(3, 7)
        } else {
            3
        };

        // Left Menu Pane (Actions list)
        if max_col < border_x || (app.active_panel == ActivePanel::MenuPane && min_col < border_x) {
            let inner_y = 1;
            let mut lines = Vec::new();
            for r in start_row..=end_row {
                if r >= inner_y {
                    let idx = (r - inner_y) as usize;
                    if idx < ACTIONS.len() {
                        lines.push(ACTIONS[idx].1.to_string());
                    }
                }
            }
            if !lines.is_empty() {
                return lines.join("\n");
            }
        }

        // Right Main Viewer Pane
        let preview_inner_y = query_height + 3 + 1;

        if matches!(
            ACTIONS.get(state.selected).map(|(_, _, a)| a),
            Some(crate::app::Action::Partitions)
        ) {
            // Action::Partitions
            let mut lines = Vec::new();
            for r in start_row..=end_row {
                if r >= preview_inner_y {
                    let idx = (r - preview_inner_y) as usize + app.partition_scroll;
                    if idx < app.partition_tree_lines.len() {
                        lines.push(app.partition_tree_lines[idx].clone());
                    }
                }
            }
            return lines.join("\n");
        } else if matches!(
            ACTIONS.get(state.selected).map(|(_, _, a)| a),
            Some(crate::app::Action::Schema)
        ) {
            // Action::Schema
            let mut lines = Vec::new();
            for r in start_row..=end_row {
                if r >= preview_inner_y + 2 {
                    let idx = (r - (preview_inner_y + 2)) as usize + app.schema_scroll;
                    if idx < app.vertical_schema_cols.len() {
                        let col = &app.vertical_schema_cols[idx];
                        lines.push(format_csv_row([
                            col.name.as_str(),
                            col.data_type.as_str(),
                            col.is_nullable.as_str(),
                            col.key_meta.as_str(),
                            col.description.as_str(),
                        ]));
                    }
                }
            }
            return lines.join("\n");
        } else if let Some(ref res) = state.results {
            // Results table view
            let mut lines = Vec::new();
            if let Some(inner) = results_table_inner_rect(app, term_width, term_height) {
                let available_width = inner.width.saturating_sub(4) as usize;
                let (visible_cols, col_width_sizes, _) =
                    crate::tui::screens::results::visible_columns_layout(res, available_width);

                let mut sel_cols = Vec::new();
                let mut cur_col_x = inner.x;
                for (v_idx, (orig_col_idx, _)) in visible_cols.iter().enumerate() {
                    let w = col_width_sizes.get(v_idx).copied().unwrap_or(15) as u16;
                    let next_x = cur_col_x + w + 2;
                    if cur_col_x <= max_col && next_x > min_col {
                        sel_cols.push(*orig_col_idx);
                    }
                    cur_col_x = next_x;
                }
                if sel_cols.is_empty() && !visible_cols.is_empty() {
                    sel_cols = visible_cols.iter().map(|(orig_idx, _)| *orig_idx).collect();
                }

                let is_stats = crate::tui::screens::results::is_stats_table(&res.columns);
                let (stats_total_row_count, stats_total_data_size) = if is_stats {
                    crate::tui::screens::results::compute_stats_aggregates(&res.columns, &res.rows)
                } else {
                    (None, None)
                };
                let data_size_col_idx = res
                    .columns
                    .iter()
                    .position(|c| c.eq_ignore_ascii_case("data_size"));
                let row_count_col_idx = res
                    .columns
                    .iter()
                    .position(|c| c.eq_ignore_ascii_case("row_count"));

                let row_visual_heights: Vec<u16> = res
                    .rows
                    .iter()
                    .map(|row| {
                        let is_summary = is_stats
                            && crate::tui::screens::results::is_stats_summary_row(
                                &res.columns,
                                row,
                            );
                        crate::tui::screens::results::compute_row_height(
                            row,
                            &visible_cols,
                            &col_width_sizes,
                            is_summary,
                            data_size_col_idx,
                            stats_total_data_size,
                            row_count_col_idx,
                            stats_total_row_count.as_ref(),
                        )
                    })
                    .collect();

                let has_h_scrollbar = res.columns.len() > 1;
                let table_reserved_rows = if has_h_scrollbar { 3 } else { 2 };
                let viewport_lines = (inner.height as usize)
                    .saturating_sub(table_reserved_rows)
                    .max(1);

                let effective_scroll_v = crate::tui::screens::results::calculate_effective_scroll_v(
                    res.scroll_v.get(),
                    res.selected_row,
                    res.last_selected_row.get(),
                    viewport_lines,
                    &row_visual_heights,
                    res.rows.len(),
                );

                if start_row <= inner.y + 1 && end_row >= inner.y {
                    let header = format_csv_row(
                        sel_cols
                            .iter()
                            .map(|&i| res.columns.get(i).map(|s| s.as_str()).unwrap_or("")),
                    );
                    lines.push(header);
                }

                let mut cur_y = inner.y + 2;
                for (row_idx, row) in res.rows.iter().enumerate().skip(effective_scroll_v) {
                    let h = row_visual_heights.get(row_idx).copied().unwrap_or(1);
                    if cur_y <= end_row && cur_y + h > start_row {
                        let row_line = format_csv_row(
                            sel_cols
                                .iter()
                                .map(|&i| row.get(i).map(|s| s.as_str()).unwrap_or("")),
                        );
                        lines.push(row_line);
                    }
                    cur_y += h;
                    if cur_y >= inner.y + inner.height {
                        break;
                    }
                }
                return lines.join("\n");
            }
        }
    }

    String::new()
}

fn handle_filter_modal_mouse(
    app: &mut App,
    mouse: MouseEvent,
    term_width: u16,
    term_height: u16,
) -> Option<Command> {
    let _ = app.filter_modal.as_ref()?;
    let popup_area =
        crate::tui::screens::modal::modal_medium(Rect::new(0, 0, term_width, term_height));

    match mouse.kind {
        MouseEventKind::Down(MouseButton::Left) => {
            if mouse.column < popup_area.x
                || mouse.column >= popup_area.x + popup_area.width
                || mouse.row < popup_area.y
                || mouse.row >= popup_area.y + popup_area.height
            {
                // Click outside modal dismisses it cleanly
                app.filter_modal = None;
                app.mode = Mode::Normal;
                return None;
            }

            // Click inside modal: guarantee mode remains FilterModal
            app.mode = Mode::FilterModal;
            let rel_x = mouse.column.saturating_sub(popup_area.x);
            let rel_y = mouse.row.saturating_sub(popup_area.y);

            let modal = app.filter_modal.as_mut()?;
            // Tab row: row 1
            if rel_y == 1 && modal.is_numeric_col {
                if rel_x < popup_area.width / 2 {
                    modal.mode = FilterMode::Values;
                } else {
                    modal.mode = FilterMode::Numeric;
                }
                return None;
            }

            // Footer row: apply or cancel
            if rel_y >= popup_area.height.saturating_sub(2) {
                if rel_x < 15 {
                    return crate::tui::handler::filter_modal::apply_filter_modal(app);
                } else if rel_x >= popup_area.width.saturating_sub(15) {
                    app.filter_modal = None;
                    app.mode = Mode::Normal;
                    return None;
                }
            }

            if modal.mode == FilterMode::Numeric {
                // Operator row is at rel_y == 3
                if rel_y == 3 {
                    let ops = NumericOp::ALL;
                    if (12..18).contains(&rel_x) {
                        modal.numeric_op = ops[0];
                    } else if (18..26).contains(&rel_x) {
                        modal.numeric_op = ops[1];
                    } else if (26..34).contains(&rel_x) {
                        modal.numeric_op = ops[2];
                    } else if (34..42).contains(&rel_x) {
                        modal.numeric_op = ops[3];
                    } else if (42..50).contains(&rel_x) {
                        modal.numeric_op = ops[4];
                    }
                    modal.numeric_focus = NumericFocus::Value;
                } else if (4..=6).contains(&rel_y) {
                    modal.numeric_focus = NumericFocus::Value;
                }
            } else if modal.mode == FilterMode::Values {
                let search_box_y = if modal.is_numeric_col { 2 } else { 1 };
                if rel_y >= search_box_y && rel_y <= search_box_y + 2 {
                    modal.is_searching = true;
                } else {
                    let list_start_y = search_box_y + 3;
                    if rel_y >= list_start_y && rel_y < popup_area.height.saturating_sub(2) {
                        let item_offset = (rel_y - list_start_y) as usize;
                        let matched = modal.filtered_indices();
                        let target_idx = modal.scroll + item_offset;
                        if target_idx < matched.len() {
                            modal.cursor = target_idx;
                            let orig_idx = matched[target_idx];
                            if let Some(val) = modal.values.get(orig_idx).cloned() {
                                if modal.selected_values.contains(&val) {
                                    modal.selected_values.remove(&val);
                                } else {
                                    modal.selected_values.insert(val);
                                }
                            }
                        }
                    }
                }
            }
            None
        }
        MouseEventKind::ScrollUp => {
            if let Some(ref mut m) = app.filter_modal
                && m.mode == FilterMode::Values
            {
                m.cursor = m.cursor.saturating_sub(1);
                if m.cursor < m.scroll {
                    m.scroll = m.cursor;
                }
            }
            None
        }
        MouseEventKind::ScrollDown => {
            if let Some(ref mut m) = app.filter_modal
                && m.mode == FilterMode::Values
            {
                let matched_len = m.filtered_indices().len();
                if matched_len > 0 {
                    m.cursor = (m.cursor + 1).min(matched_len - 1);
                    if m.cursor >= m.scroll + 10 {
                        m.scroll = m.cursor.saturating_sub(9);
                    }
                }
            }
            None
        }
        _ => None,
    }
}

fn handle_column_modal_mouse(
    app: &mut App,
    mouse: MouseEvent,
    term_width: u16,
    term_height: u16,
) -> Option<Command> {
    let _ = app.column_modal.as_ref()?;
    let popup_area =
        crate::tui::screens::modal::modal_medium(Rect::new(0, 0, term_width, term_height));
    match mouse.kind {
        MouseEventKind::Down(MouseButton::Left) => {
            if mouse.column < popup_area.x
                || mouse.column >= popup_area.x + popup_area.width
                || mouse.row < popup_area.y
                || mouse.row >= popup_area.y + popup_area.height
            {
                app.column_modal = None;
                app.mode = Mode::Normal;
                return None;
            }
            app.mode = Mode::ColumnModal;
            let _rel_x = mouse.column.saturating_sub(popup_area.x);
            let rel_y = mouse.row.saturating_sub(popup_area.y);
            let modal = app.column_modal.as_mut()?;

            // Check if clicking in search box area (rel_y 1..=3)
            if (1..=3).contains(&rel_y) {
                modal.is_searching = true;
                return None;
            }

            // List area starts at rel_y 4
            let list_start_y = 4;
            let list_end_y = popup_area.height.saturating_sub(2);
            if rel_y >= list_start_y && rel_y < list_end_y {
                let item_offset = (rel_y - list_start_y) as usize;
                let matched = modal.filtered_indices();
                let target_idx = modal.scroll + item_offset;
                if target_idx < matched.len() {
                    modal.cursor = target_idx;
                    let orig_idx = matched[target_idx];
                    if !modal.tree_nodes.is_empty() {
                        let prev_selected = modal.selected_columns.clone();
                        crate::trino::flatten::toggle_tree_node_selection(
                            &mut modal.selected_columns,
                            &modal.tree_nodes,
                            orig_idx,
                        );
                        if modal.selected_columns.is_empty() {
                            modal.selected_columns = prev_selected;
                        }
                    } else if let Some(col) = modal.columns.get(orig_idx).cloned() {
                        if modal.selected_columns.contains(&col) {
                            if modal.selected_columns.len() > 1 {
                                modal.selected_columns.remove(&col);
                            }
                        } else {
                            modal.selected_columns.insert(col);
                        }
                    }
                }
            }
            None
        }
        MouseEventKind::ScrollUp => {
            if let Some(ref mut m) = app.column_modal {
                m.cursor = m.cursor.saturating_sub(2);
                if m.cursor < m.scroll {
                    m.scroll = m.cursor;
                }
            }
            None
        }
        MouseEventKind::ScrollDown => {
            if let Some(ref mut m) = app.column_modal {
                let matched_len = m.filtered_indices().len();
                if matched_len > 0 {
                    let vis_h = (popup_area.height.saturating_sub(6) as usize).max(1);
                    m.cursor = (m.cursor + 2).min(matched_len - 1);
                    if m.cursor >= m.scroll + vis_h {
                        m.scroll = m.cursor.saturating_sub(vis_h.saturating_sub(1));
                    }
                }
            }
            None
        }
        _ => None,
    }
}

fn handle_help_modal_mouse(
    app: &mut App,
    mouse: MouseEvent,
    term_width: u16,
    term_height: u16,
) -> Option<Command> {
    let _ = app.help_modal.as_ref()?;
    let popup_area =
        crate::tui::screens::modal::modal_large(Rect::new(0, 0, term_width, term_height));

    match mouse.kind {
        MouseEventKind::Down(MouseButton::Left) => {
            if mouse.column < popup_area.x
                || mouse.column >= popup_area.x + popup_area.width
                || mouse.row < popup_area.y
                || mouse.row >= popup_area.y + popup_area.height
            {
                crate::tui::handler::help_modal::close_help_modal(app);
            }
            None
        }
        MouseEventKind::ScrollUp => {
            if let Some(ref mut m) = app.help_modal {
                m.scroll = m.scroll.saturating_sub(3);
                app.help_scroll = m.scroll;
            }
            None
        }
        MouseEventKind::ScrollDown => {
            if let Some(ref mut m) = app.help_modal {
                let total_lines =
                    crate::tui::screens::help::build_help_lines(&m.search_query).len();
                let content_h = (popup_area.height.saturating_sub(6) as usize).max(1);
                let max_scroll = total_lines.saturating_sub(content_h);
                m.scroll = (m.scroll + 3).min(max_scroll);
                app.help_scroll = m.scroll;
            }
            None
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ConnectionConfig;

    fn sample_app() -> App {
        App::new(
            ConnectionConfig {
                url: "http://localhost:8080".to_string(),
                user: "admin".to_string(),
                password: "".to_string(),
            },
            false,
        )
    }

    #[test]
    fn test_extract_selected_text_catalog_screen() {
        let mut app = sample_app();
        app.clear_mouse_selection();
        app.screen = Screen::Catalog(CatalogState {
            items: vec![
                "iceberg".to_string(),
                "tpch".to_string(),
                "system".to_string(),
            ],
            selected: 0,
        });

        let text = extract_selected_text(&app, (5, 7), (5, 8));
        assert_eq!(text, "iceberg\ntpch");
    }

    #[test]
    fn test_extract_selected_text_query_inspector() {
        let mut app = sample_app();
        let term_height = crossterm::terminal::size().unwrap_or((80, 24)).1;
        let bottom_y = term_height.saturating_sub(7);

        app.add_query_log("SELECT 1".to_string());
        app.add_query_log("SELECT 2".to_string());

        let text = extract_selected_text(&app, (10, bottom_y + 1), (10, bottom_y + 1));
        assert_eq!(text, "SELECT 2");
    }

    #[test]
    fn test_extract_selected_text_actions_results() {
        let mut app = sample_app();
        let res_state = ResultsState {
            query_buffer: "SELECT * FROM orders".to_string(),
            query_cursor: 20,
            columns: vec!["id".to_string(), "status".to_string()],
            rows: vec![
                vec!["1".to_string(), "OK".to_string()],
                vec!["2".to_string(), "PENDING".to_string()],
            ],
            scroll_v: std::cell::Cell::new(0),
            scroll_h: std::cell::Cell::new(0),
            loading: false,
            error: None,
            is_paginated: true,
            catalog: "iceberg".to_string(),
            schema: "sales".to_string(),
            table: "orders".to_string(),
            offset: 0,
            page_size: 100,
            is_fetching_next_page: false,
            has_more_rows: false,
            invalid_query_error: None,
            selection_anchor: None,
            filters: Vec::new(),
            total_count: None,
            total_count_loading: false,
            table_count: None,
            ..Default::default()
        };

        app.clear_mouse_selection();
        app.screen = Screen::Actions(ActionState {
            catalog: "iceberg".to_string(),
            schema: "sales".to_string(),
            table: "orders".to_string(),
            selected: 0,
            query_buffer: "SELECT * FROM orders".to_string(),
            query_cursor: 20,
            results: Some(res_state),
            ..Default::default()
        });

        let term_width = crossterm::terminal::size().unwrap_or((80, 24)).0;
        let menu_x = term_width / 2; // Right side

        // Header line (inner.y = 7) and first row (inner.y + 2 = 9)
        let text = extract_selected_text(&app, (menu_x, 7), (menu_x, 9));
        assert_eq!(text, "id,status\n1,OK");
    }

    #[test]
    fn test_extract_selected_text_partitions() {
        let mut app = sample_app();
        app.partition_tree_lines = vec![
            " s3a://local-minio-bucket/lakehouse/".to_string(),
            " ├── date=2024-01-01/".to_string(),
            " └── date=2024-01-02/".to_string(),
        ];
        app.clear_mouse_selection();
        app.screen = Screen::Actions(ActionState {
            catalog: "iceberg".to_string(),
            schema: "sales".to_string(),
            table: "orders".to_string(),
            selected: ACTIONS
                .iter()
                .position(|(_, _, a)| *a == crate::app::Action::Partitions)
                .unwrap(),
            query_buffer: "".to_string(),
            query_cursor: 0,
            results: None,
            ..Default::default()
        });

        let term_width = crossterm::terminal::size().unwrap_or((80, 24)).0;
        let right_x = term_width / 2;

        let text = extract_selected_text(&app, (right_x, 7), (right_x, 9));
        assert_eq!(
            text,
            " s3a://local-minio-bucket/lakehouse/\n ├── date=2024-01-01/\n └── date=2024-01-02/"
        );
    }

    #[test]
    fn test_mouse_click_selects_cell_not_entire_line() {
        let mut app = sample_app();
        let res_state = ResultsState {
            query_buffer: "SELECT * FROM orders".to_string(),
            query_cursor: 20,
            columns: vec!["id".to_string(), "status".to_string()],
            rows: vec![
                vec!["1".to_string(), "OK".to_string()],
                vec!["2".to_string(), "PENDING".to_string()],
                vec!["3".to_string(), "FAILED".to_string()],
            ],
            scroll_v: std::cell::Cell::new(0),
            scroll_h: std::cell::Cell::new(0),
            selected_row: 0,
            selected_col: 0,
            loading: false,
            error: None,
            is_paginated: true,
            catalog: "iceberg".to_string(),
            schema: "sales".to_string(),
            table: "orders".to_string(),
            offset: 0,
            page_size: 100,
            is_fetching_next_page: false,
            has_more_rows: false,
            invalid_query_error: None,
            selection_anchor: None,
            ..Default::default()
        };

        app.clear_mouse_selection();
        app.screen = Screen::Actions(ActionState {
            catalog: "iceberg".to_string(),
            schema: "sales".to_string(),
            table: "orders".to_string(),
            selected: 0,
            query_buffer: "SELECT * FROM orders".to_string(),
            query_cursor: 20,
            results: Some(res_state),
            ..Default::default()
        });

        let (term_width, term_height) = crossterm::terminal::size().unwrap_or((80, 24));
        let inner = results_table_inner_rect(&app, term_width, term_height).expect("inner rect");

        // Row 1 click at (inner.x + 2, inner.y + 3)
        // Header is at inner.y, bottom margin is inner.y + 1, row 0 is inner.y + 2, row 1 is inner.y + 3
        let click_event = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: inner.x + 2,
            row: inner.y + 3,
            modifiers: KeyModifiers::NONE,
        };

        handle_mouse_sync(&mut app, click_event);

        if let Screen::Actions(a) = &app.screen {
            let res = a.results.as_ref().unwrap();
            assert_eq!(
                res.selected_row, 1,
                "Clicking row 1 must set selected_row to 1"
            );
            assert_eq!(
                res.selected_col, 0,
                "Clicking column 0 must set selected_col to 0"
            );
        } else {
            panic!("Expected Screen::Actions");
        }

        // Must NOT set is_selecting_text (no blue line highlight)
        assert!(
            !app.is_selecting_text,
            "A single click must not start text selection"
        );

        // MouseUp event without drag
        let up_event = MouseEvent {
            kind: MouseEventKind::Up(MouseButton::Left),
            column: inner.x + 2,
            row: inner.y + 3,
            modifiers: KeyModifiers::NONE,
        };
        handle_mouse_sync(&mut app, up_event);
        assert!(
            app.copied_toast.is_none(),
            "A single click must not copy row to clipboard"
        );
    }

    #[test]
    fn test_mouse_scroll_down_and_up_in_table_view() {
        let mut app = App::new(crate::config::ConnectionConfig::default(), false);
        let rows: Vec<Vec<String>> = (0..50)
            .map(|i| vec![format!("id_{i}"), format!("val_{i}")])
            .collect();
        let res_state = ResultsState {
            columns: vec!["id".to_string(), "val".to_string()],
            rows,
            scroll_v: std::cell::Cell::new(0),
            scroll_h: std::cell::Cell::new(0),
            selected_row: 0,
            selected_col: 0,
            is_paginated: true,
            ..Default::default()
        };
        app.screen = Screen::Actions(ActionState {
            catalog: "iceberg".to_string(),
            schema: "sales".to_string(),
            table: "orders".to_string(),
            selected: 0,
            results: Some(res_state),
            ..Default::default()
        });

        let scroll_down = MouseEvent {
            kind: MouseEventKind::ScrollDown,
            column: 10,
            row: 10,
            modifiers: KeyModifiers::NONE,
        };

        // Scroll down 4 times
        for _ in 0..4 {
            handle_mouse_sync(&mut app, scroll_down);
        }

        if let Screen::Actions(a) = &app.screen {
            let res = a.results.as_ref().unwrap();
            assert_eq!(
                res.scroll_v.get(),
                12,
                "4 scroll down events must advance scroll_v by 12"
            );
            assert!(
                res.selected_row >= 12,
                "selected_row must stay within visible viewport"
            );
        } else {
            panic!("Expected Screen::Actions");
        }

        let scroll_up = MouseEvent {
            kind: MouseEventKind::ScrollUp,
            column: 10,
            row: 10,
            modifiers: KeyModifiers::NONE,
        };

        // Scroll up 2 times
        for _ in 0..2 {
            handle_mouse_sync(&mut app, scroll_up);
        }

        if let Screen::Actions(a) = &app.screen {
            let res = a.results.as_ref().unwrap();
            assert_eq!(
                res.scroll_v.get(),
                6,
                "2 scroll up events must retreat scroll_v by 6"
            );
        } else {
            panic!("Expected Screen::Actions");
        }
    }

    #[test]
    fn test_mouse_scroll_in_drilldown() {
        let mut app = App::new(crate::config::ConnectionConfig::default(), false);
        let items: Vec<String> = (0..20).map(|i| format!("partition_{i}")).collect();
        let dd = DrillDownState {
            partition_cols: vec!["dt".to_string()],
            path: Vec::new(),
            levels_cache: vec![items],
            selected: 0,
            ..Default::default()
        };
        app.screen = Screen::Actions(ActionState {
            catalog: "iceberg".to_string(),
            schema: "sales".to_string(),
            table: "orders".to_string(),
            selected: 0,
            drilldown: Some(dd),
            results: None,
            ..Default::default()
        });

        let scroll_down = MouseEvent {
            kind: MouseEventKind::ScrollDown,
            column: 10,
            row: 10,
            modifiers: KeyModifiers::NONE,
        };
        handle_mouse_sync(&mut app, scroll_down);

        if let Screen::Actions(a) = &app.screen {
            let dd = a.drilldown.as_ref().unwrap();
            assert_eq!(
                dd.selected, 3,
                "ScrollDown in drilldown must advance selected by 3"
            );
        }

        let scroll_up = MouseEvent {
            kind: MouseEventKind::ScrollUp,
            column: 10,
            row: 10,
            modifiers: KeyModifiers::NONE,
        };
        handle_mouse_sync(&mut app, scroll_up);

        if let Screen::Actions(a) = &app.screen {
            let dd = a.drilldown.as_ref().unwrap();
            assert_eq!(
                dd.selected, 0,
                "ScrollUp in drilldown must retreat selected by 3"
            );
        }
    }

    #[test]
    fn test_mouse_horizontal_scroll_in_table_view() {
        let mut app = App::new(crate::config::ConnectionConfig::default(), false);
        let cols: Vec<String> = (0..20).map(|i| format!("column_{i}")).collect();
        let rows = vec![(0..20).map(|i| format!("val_{i}")).collect()];

        app.screen = Screen::Actions(ActionState {
            catalog: "system".to_string(),
            schema: "runtime".to_string(),
            table: "nodes".to_string(),
            selected: 0,
            drilldown: None,
            results: Some(ResultsState {
                columns: cols,
                rows,
                scroll_v: std::cell::Cell::new(0),
                scroll_h: std::cell::Cell::new(0),
                selected_col: 0,
                selected_row: 0,
                ..Default::default()
            }),
            ..Default::default()
        });

        // 1. Shift + ScrollDown moves right
        let shift_scroll_down = MouseEvent {
            kind: MouseEventKind::ScrollDown,
            column: 10,
            row: 10,
            modifiers: KeyModifiers::SHIFT,
        };
        handle_mouse_sync(&mut app, shift_scroll_down);

        if let Screen::Actions(a) = &app.screen {
            let res = a.results.as_ref().unwrap();
            assert_eq!(
                res.scroll_h.get(),
                1,
                "Shift+ScrollDown should advance scroll_h to 1"
            );
            assert_eq!(res.selected_col, 1);
        }

        // 2. ScrollRight moves right
        let scroll_right = MouseEvent {
            kind: MouseEventKind::ScrollRight,
            column: 10,
            row: 10,
            modifiers: KeyModifiers::NONE,
        };
        handle_mouse_sync(&mut app, scroll_right);

        if let Screen::Actions(a) = &app.screen {
            let res = a.results.as_ref().unwrap();
            assert_eq!(
                res.scroll_h.get(),
                2,
                "ScrollRight should advance scroll_h to 2"
            );
            assert_eq!(res.selected_col, 2);
        }

        // 3. Shift + ScrollUp moves left
        let shift_scroll_up = MouseEvent {
            kind: MouseEventKind::ScrollUp,
            column: 10,
            row: 10,
            modifiers: KeyModifiers::SHIFT,
        };
        handle_mouse_sync(&mut app, shift_scroll_up);

        if let Screen::Actions(a) = &app.screen {
            let res = a.results.as_ref().unwrap();
            assert_eq!(
                res.scroll_h.get(),
                1,
                "Shift+ScrollUp should retreat scroll_h to 1"
            );
        }

        // 4. ScrollLeft moves left
        let scroll_left = MouseEvent {
            kind: MouseEventKind::ScrollLeft,
            column: 10,
            row: 10,
            modifiers: KeyModifiers::NONE,
        };
        handle_mouse_sync(&mut app, scroll_left);

        if let Screen::Actions(a) = &app.screen {
            let res = a.results.as_ref().unwrap();
            assert_eq!(
                res.scroll_h.get(),
                0,
                "ScrollLeft should retreat scroll_h to 0"
            );
        }

        let (term_w, term_h) = crossterm::terminal::size().unwrap_or((80, 24));
        if let Some(inner) = results_table_inner_rect(&app, term_w, term_h) {
            let scrollbar_row = inner.y + inner.height - 1;

            // 5. Mouse wheel scroll over bottom scrollbar row (without Shift modifier) scrolls horizontally
            let wheel_on_scrollbar = MouseEvent {
                kind: MouseEventKind::ScrollDown,
                column: inner.x + 5,
                row: scrollbar_row,
                modifiers: KeyModifiers::NONE,
            };
            handle_mouse_sync(&mut app, wheel_on_scrollbar);
            if let Screen::Actions(a) = &app.screen {
                let res = a.results.as_ref().unwrap();
                assert_eq!(
                    res.scroll_h.get(),
                    1,
                    "Wheel over scrollbar row must scroll right"
                );
            }

            // 6. Mouse wheel scroll over header row (without Shift modifier) scrolls horizontally
            let wheel_on_header = MouseEvent {
                kind: MouseEventKind::ScrollDown,
                column: inner.x + 5,
                row: inner.y,
                modifiers: KeyModifiers::NONE,
            };
            handle_mouse_sync(&mut app, wheel_on_header);
            if let Screen::Actions(a) = &app.screen {
                let res = a.results.as_ref().unwrap();
                assert_eq!(
                    res.scroll_h.get(),
                    2,
                    "Wheel over header row must scroll right"
                );
            }

            // 7. Clicking right arrow ► on horizontal scrollbar
            let click_right_arrow = MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                column: inner.x + inner.width - 1,
                row: scrollbar_row,
                modifiers: KeyModifiers::NONE,
            };
            handle_mouse_sync(&mut app, click_right_arrow);
            assert!(app.is_dragging_h_scrollbar);
            if let Screen::Actions(a) = &app.screen {
                let res = a.results.as_ref().unwrap();
                assert_eq!(res.scroll_h.get(), 3, "Clicking ► must advance scroll_h");
            }

            // 8. Clicking left arrow ◄ on horizontal scrollbar
            let click_left_arrow = MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                column: inner.x,
                row: scrollbar_row,
                modifiers: KeyModifiers::NONE,
            };
            handle_mouse_sync(&mut app, click_left_arrow);
            assert!(app.is_dragging_h_scrollbar);
            if let Screen::Actions(a) = &app.screen {
                let res = a.results.as_ref().unwrap();
                assert_eq!(res.scroll_h.get(), 2, "Clicking ◄ must retreat scroll_h");
            }

            // 9. Mouse up releases scrollbar dragging
            let mouse_up = MouseEvent {
                kind: MouseEventKind::Up(MouseButton::Left),
                column: inner.x,
                row: scrollbar_row,
                modifiers: KeyModifiers::NONE,
            };
            handle_mouse_sync(&mut app, mouse_up);
            assert!(
                !app.is_dragging_h_scrollbar,
                "Mouse up must reset is_dragging_h_scrollbar"
            );
        }
    }

    #[test]
    fn test_mouse_scroll_and_click_in_help_modal() {
        let mut app = sample_app();
        crate::tui::handler::help_modal::open_help_modal(&mut app);
        assert_eq!(app.help_modal.as_ref().unwrap().scroll, 0);

        // ScrollDown
        let scroll_down = MouseEvent {
            kind: MouseEventKind::ScrollDown,
            column: 30,
            row: 10,
            modifiers: KeyModifiers::NONE,
        };
        handle_mouse_sync(&mut app, scroll_down);
        assert!(
            app.help_modal.as_ref().unwrap().scroll > 0,
            "Mouse ScrollDown must scroll help modal"
        );

        // ScrollUp
        let scroll_up = MouseEvent {
            kind: MouseEventKind::ScrollUp,
            column: 30,
            row: 10,
            modifiers: KeyModifiers::NONE,
        };
        handle_mouse_sync(&mut app, scroll_up);
        assert_eq!(
            app.help_modal.as_ref().unwrap().scroll,
            0,
            "Mouse ScrollUp must scroll back up"
        );

        // Click outside closes help modal (e.g. at (0, 0))
        let click_outside = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 0,
            row: 0,
            modifiers: KeyModifiers::NONE,
        };
        handle_mouse_sync(&mut app, click_outside);
        assert!(
            app.help_modal.is_none(),
            "Click outside help modal must close it"
        );
    }

    #[test]
    fn test_mouse_scroll_and_click_in_column_modal() {
        let mut app = sample_app();
        app.column_modal = Some(ColumnModalState {
            columns: vec!["id".to_string(), "name".to_string(), "status".to_string()],
            selected_columns: ["id".to_string(), "name".to_string(), "status".to_string()]
                .into_iter()
                .collect(),
            cursor: 0,
            scroll: 0,
            search_query: String::new(),
            is_searching: false,
            tree_nodes: Vec::new(),
        });
        app.mode = Mode::ColumnModal;

        // Mouse scroll down
        let scroll_down = MouseEvent {
            kind: MouseEventKind::ScrollDown,
            column: 30,
            row: 10,
            modifiers: KeyModifiers::NONE,
        };
        handle_mouse_sync(&mut app, scroll_down);
        assert!(
            app.column_modal.as_ref().unwrap().cursor > 0,
            "Mouse ScrollDown must move cursor"
        );

        // Mouse scroll up
        let scroll_up = MouseEvent {
            kind: MouseEventKind::ScrollUp,
            column: 30,
            row: 10,
            modifiers: KeyModifiers::NONE,
        };
        handle_mouse_sync(&mut app, scroll_up);
        assert_eq!(
            app.column_modal.as_ref().unwrap().cursor,
            0,
            "Mouse ScrollUp must return to 0"
        );

        // Click outside closes column modal
        let click_outside = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 0,
            row: 0,
            modifiers: KeyModifiers::NONE,
        };
        handle_mouse_sync(&mut app, click_outside);
        assert!(
            app.column_modal.is_none(),
            "Click outside column modal must close it"
        );
    }

    #[test]
    fn test_mouse_shift_click_and_drag_selection() {
        let mut app = sample_app();
        let res_state = ResultsState {
            query_buffer: "SELECT * FROM orders".to_string(),
            query_cursor: 20,
            columns: vec!["id".to_string(), "status".to_string()],
            rows: vec![
                vec!["1".to_string(), "OK".to_string()],
                vec!["2".to_string(), "PENDING".to_string()],
                vec!["3".to_string(), "FAILED".to_string()],
            ],
            scroll_v: std::cell::Cell::new(0),
            scroll_h: std::cell::Cell::new(0),
            selected_row: 0,
            selected_col: 0,
            catalog: "iceberg".to_string(),
            schema: "sales".to_string(),
            table: "orders".to_string(),
            ..Default::default()
        };

        app.clear_mouse_selection();
        app.screen = Screen::Actions(ActionState {
            catalog: "iceberg".to_string(),
            schema: "sales".to_string(),
            table: "orders".to_string(),
            selected: 0,
            query_buffer: "SELECT * FROM orders".to_string(),
            query_cursor: 20,
            results: Some(res_state),
            ..Default::default()
        });

        let (term_width, term_height) = crossterm::terminal::size().unwrap_or((80, 24));
        let inner = results_table_inner_rect(&app, term_width, term_height).expect("inner rect");

        // 1. Click row 0, col 0
        let click_r0c0 = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: inner.x + 2,
            row: inner.y + 2,
            modifiers: KeyModifiers::NONE,
        };
        handle_mouse_sync(&mut app, click_r0c0);
        let Screen::Actions(a) = &app.screen else {
            panic!()
        };
        let res = a.results.as_ref().unwrap();
        assert_eq!(res.visual_mode, VisualMode::None);
        assert_eq!(res.selected_row, 0);
        assert_eq!(res.selected_col, 0);

        // 2. Drag to row 1, col 1
        let drag_r1c1 = MouseEvent {
            kind: MouseEventKind::Drag(MouseButton::Left),
            column: inner.x + 18,
            row: inner.y + 3,
            modifiers: KeyModifiers::NONE,
        };
        handle_mouse_sync(&mut app, drag_r1c1);
        let Screen::Actions(a) = &app.screen else {
            panic!()
        };
        let res = a.results.as_ref().unwrap();
        assert_eq!(
            res.visual_mode,
            VisualMode::Block {
                anchor_row: 0,
                anchor_col: 0
            }
        );
        assert_eq!(res.selected_row, 1);
        assert_eq!(res.selected_col, 1);

        // 3. Right-click copies the block selection
        let right_click = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Right),
            column: inner.x + 18,
            row: inner.y + 3,
            modifiers: KeyModifiers::NONE,
        };
        handle_mouse_sync(&mut app, right_click);
        let toast = app.copied_toast.as_ref().unwrap().0.clone();
        assert!(toast.contains("cell block (CSV)"));
    }
}
