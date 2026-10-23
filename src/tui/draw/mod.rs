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

use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Layout},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Paragraph},
};

use crate::app::{ActivePanel, App, Mode, Screen};
use crate::tui::{screens, theme};

pub mod actions_view;
pub mod header_footer;
pub mod search_query;
pub mod sql_highlight;

pub use actions_view::*;
pub use header_footer::*;
pub use search_query::*;

pub(in crate::tui) fn ui(frame: &mut Frame, app: &App) {
    let effective_screen = if let Screen::Help = &app.screen {
        app.prev_screen.as_deref().unwrap_or(&app.screen)
    } else {
        &app.screen
    };

    let is_in_table = matches!(effective_screen, Screen::Actions(_));

    let outer_chunks =
        Layout::vertical([Constraint::Min(0), Constraint::Length(7)]).split(frame.area());
    let bottom_chunks =
        Layout::vertical([Constraint::Min(0), Constraint::Length(1)]).split(outer_chunks[1]);

    if !is_in_table {
        // Connect / Catalog / Schema / Table: full width. Help lives in the `?` modal.
        if let Screen::Connect(state) = effective_screen {
            screens::connect::render(frame, outer_chunks[0], state, spinner(app));
        } else {
            let search_active =
                matches!(app.mode, Mode::Search) && !matches!(app.screen, Screen::Help);
            let inner_w = outer_chunks[0].width.saturating_sub(2).max(1) as usize;
            let search_height = if search_active {
                let total_chars = 3 + app.search_query.len();
                let lines = total_chars.div_ceil(inner_w);
                (lines as u16 + 2).clamp(3, 8)
            } else {
                3
            };

            let left_chunks = Layout::vertical([
                Constraint::Length(search_height),
                Constraint::Length(3),
                Constraint::Min(0),
            ])
            .split(outer_chunks[0]);

            render_search_bar(frame, left_chunks[0], app);
            render_phase1_count_box(frame, left_chunks[1], effective_screen, &app.search_query);

            // The help screen shows the unfiltered lists behind its modal.
            let search = if matches!(app.screen, Screen::Help) {
                ""
            } else {
                &app.search_query
            };
            match effective_screen {
                Screen::Catalog(state) => {
                    screens::catalog::render(frame, left_chunks[2], state, search, true, app);
                }
                Screen::Schema(state) => {
                    screens::schema::render(frame, left_chunks[2], state, search, true, app);
                }
                Screen::Table(state) => {
                    screens::table::render(frame, left_chunks[2], state, search, true, app);
                }
                _ => {}
            }
        }
    } else {
        // Phase 2: Inside Table View -> Default 18% Menu : 82% Preview (Resizable 10-60%)
        let menu_pct = app.actions_panel_pct.clamp(10, 60);
        let main_chunks = Layout::horizontal([
            Constraint::Percentage(menu_pct),
            Constraint::Percentage(100 - menu_pct),
        ])
        .split(outer_chunks[0]);

        let menu_area = main_chunks[0];
        let preview_column = main_chunks[1];

        let query_active = matches!(app.mode, Mode::QueryInput);
        let inner_w = preview_column.width.saturating_sub(2).max(1) as usize;

        let selected_idx = match effective_screen {
            Screen::Actions(a) => a.selected,
            _ => 0,
        };

        let query_height = if query_active {
            let total_chars = 7 + match effective_screen {
                Screen::Actions(a) => {
                    let is_partitions_or_schema = matches!(
                        crate::app::ACTIONS.get(a.selected).map(|(_, _, act)| act),
                        Some(crate::app::Action::Partitions | crate::app::Action::Schema)
                    );
                    if !is_partitions_or_schema && let Some(ref r) = a.results {
                        r.query_buffer.len()
                    } else {
                        a.query_buffer.len()
                    }
                }
                _ => 0,
            };
            let lines = total_chars.div_ceil(inner_w);
            (lines as u16 + 2).clamp(3, 7)
        } else {
            3
        };

        let preview_chunks = Layout::vertical([
            Constraint::Length(query_height),
            Constraint::Length(3),
            Constraint::Min(0),
        ])
        .split(preview_column);

        render_query_bar(frame, preview_chunks[0], app);
        if let Screen::Actions(state) = effective_screen {
            render_actions_count_box(frame, preview_chunks[1], app, state);
        }
        let preview_pane_area = preview_chunks[2];

        let menu_is_active = app.active_panel == ActivePanel::MenuPane;
        let preview_is_active = app.active_panel == ActivePanel::MainViewer;

        if let Screen::Actions(state) = effective_screen {
            screens::actions::render(
                frame,
                menu_area,
                &state.catalog,
                &state.schema,
                &state.table,
                state.selected,
                menu_is_active,
                app,
            );
        }

        if selected_idx < crate::app::ACTIONS.len() {
            let action = &crate::app::ACTIONS[selected_idx].2;
            let table_name = match effective_screen {
                Screen::Actions(a) => a.table.as_str(),
                _ => "",
            };
            let (is_metadata_loading, is_ddl_loading, has_pending_action) = match effective_screen {
                Screen::Actions(a) => (
                    a.metadata_loading,
                    a.ddl_loading,
                    a.pending_action.is_some(),
                ),
                _ => (false, false, false),
            };

            match action {
                crate::app::Action::Partitions => {
                    if app.loading || (is_metadata_loading && has_pending_action) {
                        let title = format!(" Preview — {table_name} (Partitions) ");
                        let block = Block::default()
                            .title(title)
                            .borders(Borders::ALL)
                            .border_type(BorderType::Rounded)
                            .border_style(theme::border_style(preview_is_active));
                        let inner = block.inner(preview_pane_area);
                        frame.render_widget(block, preview_pane_area);
                        let spin = spinner(app);
                        let spin_text = Paragraph::new(Line::from(vec![
                            Span::styled(format!(" [{spin}] "), theme::warning_bold_style()),
                            Span::styled(
                                "FETCHING PARTITION METADATA...",
                                theme::info_bold_style(),
                            ),
                        ]))
                        .alignment(Alignment::Center);
                        frame.render_widget(spin_text, inner);
                    } else if !app.partition_tree_lines.is_empty() {
                        screens::partition_tree::render(
                            frame,
                            preview_pane_area,
                            &app.partition_tree_lines,
                            table_name,
                            app.partition_scroll,
                            preview_is_active,
                            app,
                        );
                    } else {
                        render_placeholder_preview(
                            frame,
                            preview_pane_area,
                            table_name,
                            selected_idx,
                            preview_is_active,
                        );
                    }
                }
                crate::app::Action::Schema => {
                    if app.loading || (is_metadata_loading && has_pending_action) {
                        let title = format!(" Preview — {table_name} (Schema) ");
                        let block = Block::default()
                            .title(title)
                            .borders(Borders::ALL)
                            .border_type(BorderType::Rounded)
                            .border_style(theme::border_style(preview_is_active));
                        let inner = block.inner(preview_pane_area);
                        frame.render_widget(block, preview_pane_area);
                        let spin = spinner(app);
                        let spin_text = Paragraph::new(Line::from(vec![
                            Span::styled(format!(" [{spin}] "), theme::warning_bold_style()),
                            Span::styled("FETCHING SCHEMA METADATA...", theme::info_bold_style()),
                        ]))
                        .alignment(Alignment::Center);
                        frame.render_widget(spin_text, inner);
                    } else if !app.vertical_schema_cols.is_empty() {
                        screens::vertical_schema::render(
                            frame,
                            preview_pane_area,
                            &app.vertical_schema_cols,
                            table_name,
                            app.schema_scroll,
                            preview_is_active,
                            app,
                        );
                    } else {
                        render_placeholder_preview(
                            frame,
                            preview_pane_area,
                            table_name,
                            selected_idx,
                            preview_is_active,
                        );
                    }
                }
                crate::app::Action::TableView => {
                    let drilldown_browsing = match effective_screen {
                        Screen::Actions(state) => {
                            state.drilldown.as_ref().filter(|d| !d.is_leaf()).cloned()
                        }
                        _ => None,
                    };
                    if let Some(dd) = drilldown_browsing {
                        screens::drilldown::render(
                            frame,
                            preview_pane_area,
                            table_name,
                            &dd,
                            preview_is_active,
                            app,
                        );
                    } else {
                        render_default_results_preview(
                            frame,
                            preview_pane_area,
                            app,
                            app.loading,
                            spinner(app),
                            table_name,
                            selected_idx,
                            preview_is_active,
                        );
                    }
                }
                crate::app::Action::TableDDL => {
                    let ddl_text = match effective_screen {
                        Screen::Actions(a) => a.metadata.as_ref().map(|m| m.ddl_text.clone()),
                        _ => None,
                    };
                    if app.loading || (is_ddl_loading && has_pending_action) {
                        let title = format!(" Preview — {table_name} (Table DDL) ");
                        let block = Block::default()
                            .title(title)
                            .borders(Borders::ALL)
                            .border_type(BorderType::Rounded)
                            .border_style(theme::border_style(preview_is_active));
                        let inner = block.inner(preview_pane_area);
                        frame.render_widget(block, preview_pane_area);
                        let spin = spinner(app);
                        let spin_text = Paragraph::new(Line::from(vec![
                            Span::styled(format!(" [{spin}] "), theme::warning_bold_style()),
                            Span::styled("FETCHING TABLE DDL...", theme::info_bold_style()),
                        ]))
                        .alignment(Alignment::Center);
                        frame.render_widget(spin_text, inner);
                    } else if let Some(ddl_text) = ddl_text {
                        let title = format!(" Preview — {table_name} (Table DDL) ");
                        let block = Block::default()
                            .title(title)
                            .borders(Borders::ALL)
                            .border_type(BorderType::Rounded)
                            .border_style(theme::border_style(preview_is_active));
                        let inner = block.inner(preview_pane_area);
                        frame.render_widget(block, preview_pane_area);
                        let paragraph = Paragraph::new(ddl_text).style(theme::detail_style());
                        frame.render_widget(paragraph, inner);
                    } else {
                        render_default_results_preview(
                            frame,
                            preview_pane_area,
                            app,
                            app.loading,
                            spinner(app),
                            table_name,
                            selected_idx,
                            preview_is_active,
                        );
                    }
                }
                _ => {
                    render_default_results_preview(
                        frame,
                        preview_pane_area,
                        app,
                        app.loading,
                        spinner(app),
                        table_name,
                        selected_idx,
                        preview_is_active,
                    );
                }
            }
        }
    }

    if bottom_chunks[0].height > 0 {
        screens::query_inspector::render(frame, bottom_chunks[0], app);
    }
    render_footer(frame, bottom_chunks[1], app);

    render_copied_toast(frame, frame.area(), app);
    if let Some(ref modal) = app.filter_modal {
        screens::filter_modal::render(frame, frame.area(), modal);
    }
    if let Some(ref modal) = app.column_modal {
        let has_filter = |col: &str| {
            if let Screen::Actions(a) = effective_screen
                && let Some(ref res) = a.results
            {
                res.column_filters.contains_key(col)
            } else {
                false
            }
        };
        screens::column_modal::render(frame, frame.area(), modal, has_filter);
    }
    if let Some(ref modal) = app.help_modal {
        screens::help::render_modal(
            frame,
            frame.area(),
            &modal.search_query,
            modal.is_searching,
            modal.scroll,
        );
    } else if matches!(app.screen, Screen::Help) {
        let is_searching = matches!(app.mode, Mode::Search);
        screens::help::render_modal(
            frame,
            frame.area(),
            &app.search_query,
            is_searching,
            app.help_scroll,
        );
    }
    if let Some(ref modal) = app.saved_views_modal {
        screens::saved_views_modal::render(frame, frame.area(), modal);
    }
    if let Some(ref prompt) = app.save_view_prompt {
        screens::saved_views_modal::render_save_prompt(frame, frame.area(), prompt);
    }
    if let Some(ref modal) = app.applied_filters_modal {
        screens::applied_filters_modal::render(frame, frame.area(), modal);
    }
    paint_screen_text_selection(frame, app);
    if let Some(ref modal) = app.export_modal {
        screens::export_modal::render(frame, frame.area(), modal);
    }
    // Drawn last so copy/export results and failures are visible above any modal.
    render_notice_toast(frame, frame.area(), app);
}

/// Highlights a mouse selection whose text is read straight off the screen
/// (partition counts, DDL, banners, …). Widgets with their own selection
/// rendering paint the other cases.
fn paint_screen_text_selection(frame: &mut Frame, app: &App) {
    if !app.mouse_highlight_active() {
        return;
    }
    let (Some(anchor), Some(current)) = (app.mouse_selection_anchor, app.mouse_selection_current)
    else {
        return;
    };
    if anchor == current || !crate::tui::handler::selection_uses_screen_text(app, anchor, current) {
        return;
    }

    let area = frame.area();
    let x0 = anchor.0.min(current.0).max(area.x);
    let x1 = anchor
        .0
        .max(current.0)
        .min(area.x + area.width.saturating_sub(1));
    let y0 = anchor.1.min(current.1).max(area.y);
    let y1 = anchor
        .1
        .max(current.1)
        .min(area.y + area.height.saturating_sub(1));
    let style = theme::text_selection_style();
    let buf = frame.buffer_mut();
    for y in y0..=y1 {
        for x in x0..=x1 {
            if let Some(cell) = buf.cell_mut((x, y)) {
                cell.set_style(style);
            }
        }
    }
}

#[cfg(test)]
pub mod toast_border_regression {
    use super::*;
    use crate::app::{ActionState, App};
    use ratatui::{Terminal, backend::TestBackend, layout::Rect};

    #[test]
    fn search_and_query_bar_borders_stay_intact_without_active_toast() {
        let mut app = App::new(crate::config::ConnectionConfig::default(), false);
        app.mode = Mode::QueryInput;
        app.screen = Screen::Actions(ActionState {
            catalog: "datalake".into(),
            schema: "s".into(),
            table: "t".into(),
            selected: 4,
            query_buffer: "SELECT 1".into(),
            query_cursor: 8,
            ..Default::default()
        });
        app.main_panel_pct = 15;
        assert!(app.copied_toast.is_none());

        let backend = TestBackend::new(200, 40);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|f| ui(f, &app)).unwrap();
        let buf = terminal.backend().buffer().clone();

        for y in [0u16] {
            let mut corner_x = None;
            for x in (0..200u16).rev() {
                let sym = buf.cell((x, y)).unwrap().symbol().to_string();
                if sym != " " {
                    corner_x = Some(x);
                    break;
                }
            }
            let corner_x = corner_x.expect("row should have a border corner");
            assert!(corner_x > 0, "row {y} corner at unexpected x=0");
            let prev_sym = buf.cell((corner_x - 1, y)).unwrap().symbol().to_string();
            assert_eq!(
                prev_sym,
                "─",
                "row {y}: cell just before the border corner (x={}) is {:?}, expected a dash",
                corner_x - 1,
                prev_sym
            );
        }
    }

    #[test]
    fn active_toast_has_solid_background_with_no_spillover() {
        let mut app = App::new(crate::config::ConnectionConfig::default(), false);
        app.mode = Mode::QueryInput;
        app.screen = Screen::Actions(ActionState {
            catalog: "datalake".into(),
            schema: "s".into(),
            table: "t".into(),
            selected: 4,
            query_buffer: "SELECT 1".into(),
            query_cursor: 8,
            ..Default::default()
        });
        app.main_panel_pct = 15;
        app.copied_toast = Some(("hello".into(), std::time::Instant::now()));

        let width = 200u16;
        let height = 40u16;
        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|f| ui(f, &app)).unwrap();
        let buf = terminal.backend().buffer().clone();

        let area = Rect::new(0, 0, width, height);
        let envelope = toast_envelope(area).expect("envelope should exist at this size");

        let y = envelope.y;
        let mut left_x = None;
        let mut right_x = None;
        for x in envelope.x..envelope.x + envelope.width {
            let sym = buf.cell((x, y)).unwrap().symbol().to_string();
            if sym == "╭" {
                left_x = Some(x);
            }
            if sym == "╮" {
                right_x = Some(x);
                break;
            }
        }
        let left_x = left_x.expect("toast top-left corner not found");
        let right_x = right_x.expect("toast top-right corner not found");
        assert!(right_x > left_x);

        // The toast is a card with an accent-colored border; nothing outside
        // its box may carry the border.
        let accent = theme::toast_style().fg.unwrap();
        for col in [left_x, right_x] {
            for row in [y, y + envelope.height - 1] {
                let cell = buf.cell((col, row)).unwrap();
                assert_eq!(cell.fg, accent, "corner ({col},{row}) lost the accent");
            }
        }
        // inside the box the old content is cleared: no stray accent glyphs
        // other than the border and the title
        let mid = buf.cell((left_x + 1, y + 1)).unwrap();
        assert_eq!(mid.bg, ratatui::style::Color::Reset);

        if right_x + 1 < width {
            let cell = buf.cell((right_x + 1, y)).unwrap();
            assert_ne!(
                cell.symbol(),
                "─",
                "toast border spilled over its right edge"
            );
        }
    }
}

#[cfg(test)]
pub mod query_bar_height_regression {
    use super::*;
    use crate::app::{ActionState, App};
    use ratatui::{Terminal, backend::TestBackend};

    #[test]
    fn long_query_expands_bar_up_to_five_rows() {
        let mut app = App::new(crate::config::ConnectionConfig::default(), false);
        app.mode = Mode::QueryInput;
        let long_query = "SELECT * FROM datalake.sales.orders WHERE ".to_string()
            + &"customer_id = 'abc123' AND ".repeat(14)
            + "1=1";
        let cursor = long_query.len();
        app.screen = Screen::Actions(ActionState {
            catalog: "datalake".into(),
            schema: "sales".into(),
            table: "orders".into(),
            selected: 4,
            query_buffer: long_query,
            query_cursor: cursor,
            ..Default::default()
        });
        app.main_panel_pct = 15;

        let backend = TestBackend::new(120, 40);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|f| ui(f, &app)).unwrap();
        let buf = terminal.backend().buffer().clone();

        let query_bar_top = 0u16;
        let mut bottom_y = query_bar_top;
        for y in query_bar_top..40u16 {
            let has_corner = (0..120u16).any(|x| buf.cell((x, y)).unwrap().symbol() == "╰");
            if has_corner {
                bottom_y = y;
                break;
            }
        }
        let height = bottom_y - query_bar_top + 1;
        assert_eq!(
            height, 7,
            "expected query bar to expand to 7 total rows, got {height}"
        );
    }

    #[test]
    fn short_query_keeps_default_three_row_height() {
        let mut app = App::new(crate::config::ConnectionConfig::default(), false);
        app.mode = Mode::QueryInput;
        let short_query = "SELECT 1".to_string();
        let cursor = short_query.len();
        app.screen = Screen::Actions(ActionState {
            catalog: "datalake".into(),
            schema: "sales".into(),
            table: "orders".into(),
            selected: 4,
            query_buffer: short_query,
            query_cursor: cursor,
            ..Default::default()
        });
        app.main_panel_pct = 15;

        let backend = TestBackend::new(120, 40);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|f| ui(f, &app)).unwrap();
        let buf = terminal.backend().buffer().clone();

        let query_bar_top = 0u16;
        let mut bottom_y = query_bar_top;
        for y in query_bar_top..40u16 {
            let has_corner = (0..120u16).any(|x| buf.cell((x, y)).unwrap().symbol() == "╰");
            if has_corner {
                bottom_y = y;
                break;
            }
        }
        let height = bottom_y - query_bar_top + 1;
        assert_eq!(
            height, 3,
            "short query should keep 3-row height, got {height}"
        );
    }
}

#[cfg(test)]
pub mod help_modal_overlay_regression {
    use super::*;
    use crate::app::{ActionState, App, HelpModalState, Mode, ResultsState};
    use ratatui::{Terminal, backend::TestBackend};

    #[test]
    fn help_modal_renders_on_top_without_hiding_table_view_or_query_bar() {
        let mut app = App::new(crate::config::ConnectionConfig::default(), false);
        app.screen = Screen::Actions(ActionState {
            catalog: "iceberg".into(),
            schema: "analytics".into(),
            table: "customers".into(),
            selected: 0,
            query_buffer: "SELECT id, name FROM customers".into(),
            query_cursor: 30,
            results: Some(ResultsState {
                query_buffer: "SELECT id, name FROM customers".into(),
                query_cursor: 30,
                columns: vec!["id".into(), "name".into()],
                rows: vec![vec!["1".into(), "Alice".into()]],
                is_paginated: true,
                ..Default::default()
            }),
            ..Default::default()
        });
        app.main_panel_pct = 15;
        app.mode = Mode::HelpModal;
        app.help_modal = Some(HelpModalState::default());

        let backend = TestBackend::new(140, 40);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|f| ui(f, &app)).unwrap();
        let buf = terminal.backend().buffer().clone();

        let mut found_query = false;
        let mut found_table_content = false;
        let mut found_help_modal = false;

        for y in 0..40 {
            let row_str: String = (0..140)
                .map(|x| buf.cell((x, y)).unwrap().symbol())
                .collect();
            if row_str.contains("SELECT id, name FROM customers") {
                found_query = true;
            }
            if row_str.contains("Table Query Bar") {
                assert!(!row_str.contains("Disabled"));
            }
            if row_str.contains("Alice") || row_str.contains("customers") {
                found_table_content = true;
            }
            if row_str.contains("Help & Keybindings") {
                found_help_modal = true;
            }
        }

        assert!(
            found_query,
            "query buffer should remain visible in query bar behind help modal"
        );
        assert!(
            found_table_content,
            "table content should remain visible behind help modal"
        );
        assert!(found_help_modal, "help modal should render floating on top");
    }
}

#[cfg(test)]
pub mod count_box_layout_regression {
    use super::*;
    use crate::app::{ActionState, App, ResultsState};
    use ratatui::{Terminal, backend::TestBackend};

    #[test]
    fn count_box_renders_between_query_bar_and_table_results() {
        let mut app = App::new(crate::config::ConnectionConfig::default(), false);
        app.screen = Screen::Actions(ActionState {
            catalog: "iceberg".into(),
            schema: "analytics".into(),
            table: "customers".into(),
            selected: 0,
            query_buffer: "SELECT id, name FROM customers".into(),
            query_cursor: 30,
            results: Some(ResultsState {
                query_buffer: "SELECT id, name FROM customers".into(),
                query_cursor: 30,
                columns: vec!["id".into(), "name".into()],
                rows: vec![vec!["1".into(), "Alice".into()]],
                is_paginated: true,
                total_count: Some(25000),
                ..Default::default()
            }),
            ..Default::default()
        });
        app.main_panel_pct = 15;

        let backend = TestBackend::new(140, 40);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|f| ui(f, &app)).unwrap();
        let buf = terminal.backend().buffer().clone();

        let mut query_bar_row = None;
        let mut count_box_row = None;
        let mut table_results_row = None;

        for y in 0..40 {
            let row_str: String = (22..140)
                .map(|x| buf.cell((x, y)).unwrap().symbol())
                .collect();
            if row_str.contains("Table Query Bar") && query_bar_row.is_none() {
                query_bar_row = Some(y);
            }
            if row_str.contains("Dataset Records") && count_box_row.is_none() {
                count_box_row = Some(y);
            }
            if (row_str.contains("Query Results") || row_str.contains("Infinite Scroll"))
                && table_results_row.is_none()
            {
                table_results_row = Some(y);
            }
        }

        let qy = query_bar_row.expect("query bar should be rendered");
        let cy = count_box_row.expect("count box should be rendered");
        let ty = table_results_row.expect("table results should be rendered");

        assert!(
            qy < cy,
            "query bar (y={qy}) should be above count box (y={cy})"
        );
        assert!(
            cy < ty,
            "count box (y={cy}) should be above table results (y={ty})"
        );

        // Verify content inside count box line (cy + 1)
        let count_line: String = (0..140)
            .map(|x| buf.cell((x, cy + 1)).unwrap().symbol())
            .collect();
        assert!(
            count_line.contains("1 rows loaded of 25,000 total"),
            "count line must contain formatted numbers: {count_line}"
        );
    }
}

#[cfg(test)]
pub mod table_browse_full_width_regression {
    use super::*;
    use crate::app::{CatalogState, TableState};
    use ratatui::{Terminal, backend::TestBackend};

    #[test]
    fn table_browse_renders_full_width_without_help_side_panel() {
        let mut app = App::new(crate::config::ConnectionConfig::default(), false);
        app.screen = Screen::Table(TableState {
            catalog: "iceberg".into(),
            schema: "analytics".into(),
            items: vec!["customer_events".into(), "user_orders".into()],
            selected: 0,
        });

        let width = 120u16;
        let height = 30u16;
        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|f| ui(f, &app)).unwrap();
        let buf = terminal.backend().buffer().clone();

        // 1. Verify "Help & Keybindings" is NOT rendered on table browse screen
        let mut found_help_panel = false;
        for y in 0..height {
            let row_str: String = (0..width)
                .map(|x| buf.cell((x, y)).unwrap().symbol())
                .collect();
            if row_str.contains("Help & Keybindings") {
                found_help_panel = true;
                break;
            }
        }
        assert!(
            !found_help_panel,
            "Help & Keybindings side panel must not be rendered when browsing tables"
        );

        // 2. Verify table list extends to the right edge of the screen
        let mut found_table_title = false;
        for y in 0..height {
            let row_str: String = (0..width)
                .map(|x| buf.cell((x, y)).unwrap().symbol())
                .collect();
            if row_str.contains("Tables — iceberg.analytics") {
                found_table_title = true;
                // Right border should be at width - 1
                assert_eq!(
                    buf.cell((width - 1, y)).unwrap().symbol(),
                    "╮",
                    "Table box top-right corner should be at the right edge"
                );
                break;
            }
        }
        assert!(found_table_title, "Tables title should be rendered");

        // 3. Catalog screen is full width too: no side panel anywhere
        app.screen = Screen::Catalog(CatalogState {
            items: vec!["iceberg".into()],
            selected: 0,
        });
        terminal.draw(|f| ui(f, &app)).unwrap();
        let buf2 = terminal.backend().buffer().clone();
        let mut catalog_title_right_edge = false;
        for y in 0..height {
            let row_str: String = (0..width)
                .map(|x| buf2.cell((x, y)).unwrap().symbol())
                .collect();
            assert!(
                !row_str.contains("Help & Keybindings"),
                "Help & Keybindings side panel must be gone from the catalog screen"
            );
            if row_str.contains("Catalogs") && buf2.cell((width - 1, y)).unwrap().symbol() == "╮"
            {
                catalog_title_right_edge = true;
            }
        }
        assert!(
            catalog_title_right_edge,
            "catalog list should extend to the right edge of the screen"
        );
    }
}

#[cfg(test)]
mod selection_overlay_tests {
    use super::*;
    use crate::app::{ActionState, DrillDownState};
    use ratatui::{Terminal, backend::TestBackend, style::Modifier};

    #[test]
    fn selected_screen_text_without_a_widget_highlight_is_painted_in_terminal_colors() {
        let dd = DrillDownState {
            partition_cols: vec!["date".into()],
            levels_cache: vec![vec!["2026-08-06".into()]],
            table_count: Some(1_234_567),
            ..Default::default()
        };
        let mut app = App::new(crate::config::ConnectionConfig::default(), false);
        app.active_panel = ActivePanel::MainViewer;
        app.screen = Screen::Actions(ActionState {
            catalog: "c".into(),
            schema: "s".into(),
            table: "t".into(),
            selected: 0,
            drilldown: Some(dd),
            ..Default::default()
        });

        let mut term = Terminal::new(TestBackend::new(80, 24)).unwrap();
        term.draw(|f| ui(f, &app)).unwrap();
        let buf = term.backend().buffer().clone();

        // find the "Total Table: ..." banner and select it with the mouse
        let (y, x) = (0..24u16)
            .find_map(|y| {
                let row: String = (0..80)
                    .map(|x| buf.cell((x, y)).unwrap().symbol())
                    .collect();
                row.find("Total Table")
                    .map(|byte| (y, row[..byte].chars().count() as u16))
            })
            .expect("banner row");
        app.mouse_selection_anchor = Some((x, y));
        app.mouse_selection_current = Some((x + 20, y));
        app.mouse_selection_committed = true;

        term.draw(|f| ui(f, &app)).unwrap();
        let buf = term.backend().buffer().clone();
        for dx in 0..=20 {
            let c = buf.cell((x + dx, y)).unwrap();
            assert!(c.modifier.contains(Modifier::REVERSED), "cell +{dx}");
            assert_eq!(c.fg, ratatui::style::Color::Reset);
            assert_eq!(c.bg, ratatui::style::Color::Reset);
        }
        assert!(
            !buf.cell((x + 21, y))
                .unwrap()
                .modifier
                .contains(Modifier::REVERSED)
        );
        assert!(
            !buf.cell((x, y + 1))
                .unwrap()
                .modifier
                .contains(Modifier::REVERSED)
        );

        // and it copies exactly what is highlighted
        app.last_frame = buf;
        let text = crate::tui::handler::extract_selected_text(&app, (x, y), (x + 20, y));
        assert!(text.starts_with("Total Table"), "{text}");

        // gone once invalidated
        app.mouse_selection_committed = false;
        term.draw(|f| ui(f, &app)).unwrap();
        assert!(
            !term
                .backend()
                .buffer()
                .cell((x, y))
                .unwrap()
                .modifier
                .contains(Modifier::REVERSED)
        );
    }
}

#[cfg(test)]
mod theme_contrast_tests {
    //! Renders real screens and checks every visible glyph against both a
    //! light and a dark terminal background (the terminal's own default
    //! colors stand in for `Color::Reset`).
    use super::*;
    use crate::app::{
        ActionState, CatalogState, DrillDownState, ExportModalState, HelpModalState, NoticeKind,
        ResultsState, ViewExport, VisualMode,
    };
    use ratatui::{
        Terminal,
        backend::TestBackend,
        buffer::Buffer,
        style::{Color, Modifier},
    };

    type Rgb = (u8, u8, u8);

    /// A terminal profile: default colors plus its 16 ANSI colors, in
    /// `Color` order (black, red, green, yellow, blue, magenta, cyan, gray,
    /// dark gray, light red, ... white).
    struct Profile {
        name: &'static str,
        fg: Rgb,
        bg: Rgb,
        ansi: [Rgb; 16],
    }

    // macOS Terminal.app "Basic" (light) and "Pro" (dark) ship the same ANSI
    // table (except bright blue, taken from xterm's #5c5cff: the stock
    // #0000ff is unreadable on black); real themes tune it per background.
    const ANSI: [Rgb; 16] = [
        (0, 0, 0),
        (153, 0, 0),
        (0, 166, 0),
        (153, 153, 0),
        (0, 0, 178),
        (178, 0, 178),
        (0, 166, 178),
        (191, 191, 191),
        (102, 102, 102),
        (229, 0, 0),
        (0, 217, 0),
        (229, 229, 0),
        (92, 92, 255),
        (229, 0, 229),
        (0, 229, 229),
        (229, 229, 229),
    ];

    fn profiles() -> [Profile; 2] {
        [
            Profile {
                name: "light",
                fg: (0, 0, 0),
                bg: (255, 255, 255),
                ansi: ANSI,
            },
            Profile {
                name: "dark",
                fg: (229, 229, 229),
                bg: (0, 0, 0),
                ansi: ANSI,
            },
        ]
    }

    fn resolve(c: Color, term_default: Rgb, p: &Profile) -> Rgb {
        let ansi = |i: usize| p.ansi[i];
        match c {
            Color::Reset => term_default,
            Color::Black => ansi(0),
            Color::Red => ansi(1),
            Color::Green => ansi(2),
            Color::Yellow => ansi(3),
            Color::Blue => ansi(4),
            Color::Magenta => ansi(5),
            Color::Cyan => ansi(6),
            Color::Gray => ansi(7),
            Color::DarkGray => ansi(8),
            Color::LightRed => ansi(9),
            Color::LightGreen => ansi(10),
            Color::LightYellow => ansi(11),
            Color::LightBlue => ansi(12),
            Color::LightMagenta => ansi(13),
            Color::LightCyan => ansi(14),
            Color::White => ansi(15),
            Color::Indexed(i) if i < 16 => ansi(i as usize),
            Color::Indexed(_) | Color::Rgb(..) => {
                panic!("{c:?} is not a terminal-native color; profiles cannot remap it")
            }
        }
    }

    fn lum((r, g, b): Rgb) -> f64 {
        let f = |v: u8| {
            let v = f64::from(v) / 255.0;
            if v <= 0.04045 {
                v / 12.92
            } else {
                ((v + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * f(r) + 0.7152 * f(g) + 0.0722 * f(b)
    }

    fn contrast(a: Rgb, b: Rgb) -> f64 {
        let (la, lb) = (lum(a), lum(b));
        (la.max(lb) + 0.05) / (la.min(lb) + 0.05)
    }

    /// Returns "x,y 'glyph' ratio" for every glyph below `min`.
    fn low_contrast(buf: &Buffer, p: &Profile, min: f64) -> Vec<String> {
        let mut bad = Vec::new();
        for y in 0..buf.area.height {
            for x in 0..buf.area.width {
                let cell = buf.cell((x, y)).unwrap();
                if cell.symbol().trim().is_empty() {
                    continue;
                }
                let (mut fg, mut bg) = (resolve(cell.fg, p.fg, p), resolve(cell.bg, p.bg, p));
                if cell.modifier.contains(Modifier::REVERSED) {
                    std::mem::swap(&mut fg, &mut bg);
                }
                let ratio = contrast(fg, bg);
                if ratio < min {
                    bad.push(format!("({x},{y}) {:?} {ratio:.2}", cell.symbol()));
                }
            }
        }
        bad
    }

    fn app_on(screen: Screen) -> App {
        let mut app = App::new(crate::config::ConnectionConfig::default(), false);
        app.screen = screen;
        app
    }

    fn grid_app() -> App {
        let mut res = ResultsState {
            columns: vec!["id".into(), "status".into()],
            rows: vec![
                vec!["1".into(), "OK".into()],
                vec!["2".into(), "PENDING".into()],
                vec!["3".into(), "FAILED".into()],
            ],
            ..Default::default()
        };
        res.visual_mode = VisualMode::Block {
            anchor_row: 0,
            anchor_col: 0,
        };
        res.selected_row = 1;
        res.selected_col = 1;
        let mut app = app_on(Screen::Actions(ActionState {
            catalog: "c".into(),
            schema: "s".into(),
            table: "t".into(),
            selected: 0,
            results: Some(res),
            ..Default::default()
        }));
        app.active_panel = ActivePanel::MainViewer;
        app
    }

    fn drilldown_app() -> App {
        let dd = DrillDownState {
            partition_cols: vec!["date".into(), "svc".into()],
            levels_cache: vec![vec!["2026-08-06".into(), "2026-08-07".into()]],
            table_count: Some(1_234_567),
            ..Default::default()
        };
        let mut app = app_on(Screen::Actions(ActionState {
            catalog: "c".into(),
            schema: "s".into(),
            table: "t".into(),
            selected: 0,
            drilldown: Some(dd),
            ..Default::default()
        }));
        app.active_panel = ActivePanel::MainViewer;
        app
    }

    fn catalog_app() -> App {
        let mut app = app_on(Screen::Catalog(CatalogState {
            items: vec!["hive".into(), "iceberg".into(), "delta".into()],
            selected: 1,
        }));
        app.line_selection = None;
        app
    }

    #[test]
    fn theme_is_legible_on_light_and_dark_terminals() {
        let mut scenarios: Vec<(&str, App)> = vec![
            (
                "connect",
                App::new(crate::config::ConnectionConfig::default(), false),
            ),
            ("catalog", catalog_app()),
            ("grid + visual block", grid_app()),
            ("partition drill-down", drilldown_app()),
        ];

        let mut help = catalog_app();
        help.help_modal = Some(HelpModalState::default());
        help.mode = Mode::HelpModal;
        scenarios.push(("help modal", help));

        let mut export = catalog_app();
        export.export_modal = Some(ExportModalState {
            view: ViewExport {
                title: "Catalogs".into(),
                file_stem: "catalogs".into(),
                columns: vec!["catalog".into()],
                rows: vec![vec!["hive".into()]],
                note: Some("Filtered by search".into()),
            },
            selected: 1,
            size_bytes: 10 * 1024 * 1024,
        });
        export.mode = Mode::ExportModal;
        scenarios.push(("export modal", export));

        for (kind, label) in [
            (NoticeKind::Success, "ok toast"),
            (NoticeKind::Error, "error toast"),
        ] {
            let mut a = catalog_app();
            a.set_notice(kind, "Exported 12 rows to ~/Downloads/lazytrino/x.csv");
            scenarios.push((label, a));
        }

        let mut selected = grid_app();
        selected.mouse_selection_anchor = Some((30, 6));
        selected.mouse_selection_current = Some((50, 8));
        selected.mouse_selection_committed = true;
        scenarios.push(("grid + mouse selection", selected));

        let mut failures = Vec::new();
        for (name, app) in &scenarios {
            let mut term = Terminal::new(TestBackend::new(120, 36)).unwrap();
            term.draw(|f| ui(f, app)).unwrap();
            let buf = term.backend().buffer().clone();
            for p in profiles() {
                // ANSI accents on a stock palette: readable, not necessarily AA.
                let bad = low_contrast(&buf, &p, 2.5);
                if !bad.is_empty() {
                    failures.push(format!(
                        "{name} on {} terminal: {} low-contrast glyphs, e.g. {:?}",
                        p.name,
                        bad.len(),
                        &bad[..bad.len().min(4)]
                    ));
                }
            }
        }
        assert!(failures.is_empty(), "{}", failures.join("\n"));
    }

    /// Vertical scrollbars (▲ … ▼) must look the same on every row: they used
    /// to inherit the highlighted row's color / weight / reverse video.
    fn assert_scrollbars_are_untouched(name: &str, app: &App) {
        use ratatui::style::Color;
        let mut term = Terminal::new(TestBackend::new(120, 36)).unwrap();
        term.draw(|f| ui(f, app)).unwrap();
        let buf = term.backend().buffer().clone();
        let mut found = 0;
        for y in 0..buf.area.height {
            for x in 0..buf.area.width {
                if buf.cell((x, y)).unwrap().symbol() != "▲" {
                    continue;
                }
                found += 1;
                let mut row = y;
                loop {
                    let cell = buf.cell((x, row)).unwrap();
                    assert_eq!(cell.fg, Color::Reset, "{name}: scrollbar ({x},{row}) fg");
                    assert_eq!(cell.bg, Color::Reset, "{name}: scrollbar ({x},{row}) bg");
                    assert!(
                        cell.modifier.is_empty(),
                        "{name}: scrollbar ({x},{row}) has {:?}",
                        cell.modifier
                    );
                    // The bar runs ▲ (thumb ║ █) ▼; it can be cut short where a
                    // horizontal bar or the border meets it.
                    let next = buf.cell((x, row + 1)).map(|c| c.symbol().to_string());
                    if cell.symbol() == "▼" || !matches!(next.as_deref(), Some("║" | "█" | "▼"))
                    {
                        break;
                    }
                    row += 1;
                }
            }
        }
        assert!(found > 0, "{name}: no scrollbar rendered");
    }

    #[test]
    fn scrollbars_keep_their_own_style_under_highlights_and_tints() {
        // list with a highlighted cursor row (selection fills the whole row)
        assert_scrollbars_are_untouched("catalog", &catalog_app());
        // grid: selected cell, visual block, typed-value colors
        assert_scrollbars_are_untouched("grid", &grid_app());
        // drill-down list with its cursor row
        assert_scrollbars_are_untouched("drill-down", &drilldown_app());
        // a mouse selection (reverse video) across the scrollbar column
        let mut selected = catalog_app();
        selected.mouse_selection_anchor = Some((2, 8));
        selected.mouse_selection_current = Some((118, 11));
        selected.mouse_selection_committed = true;
        assert_scrollbars_are_untouched("catalog + mouse selection", &selected);
    }

    #[test]
    fn the_checker_itself_catches_unreadable_pairs() {
        // Guard against a vacuous test: white-on-white and dark-blue-on-black
        // must be flagged, terminal-default text must not.
        let [light, dark] = profiles();
        let mut buf = Buffer::empty(ratatui::layout::Rect::new(0, 0, 4, 1));
        buf.set_string(
            0,
            0,
            "text",
            ratatui::style::Style::default().fg(Color::White),
        );
        assert_eq!(low_contrast(&buf, &light, 2.5).len(), 4);
        let mut buf = Buffer::empty(ratatui::layout::Rect::new(0, 0, 4, 1));
        buf.set_string(
            0,
            0,
            "text",
            ratatui::style::Style::default().fg(Color::Blue),
        );
        assert_eq!(low_contrast(&buf, &dark, 2.5).len(), 4);
        let mut buf = Buffer::empty(ratatui::layout::Rect::new(0, 0, 4, 1));
        buf.set_string(0, 0, "text", ratatui::style::Style::default());
        assert!(low_contrast(&buf, &light, 2.5).is_empty());
        assert!(low_contrast(&buf, &dark, 2.5).is_empty());
    }
}
