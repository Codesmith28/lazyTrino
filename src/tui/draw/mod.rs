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
        // Phase 1: Default All Tables View (Connect, Catalog, Schema, Table) -> Default 60% list ratio
        let list_pct = if app.main_panel_pct <= 30 {
            60
        } else {
            app.main_panel_pct
        };
        let main_chunks = Layout::horizontal([
            Constraint::Percentage(list_pct),
            Constraint::Percentage(100 - list_pct),
        ])
        .split(outer_chunks[0]);

        screens::help::render_side_panel(frame, main_chunks[1]);

        let is_connect = matches!(effective_screen, Screen::Connect(_));
        let main = if is_connect {
            main_chunks[0]
        } else {
            let search_active =
                matches!(app.mode, Mode::Search) && !matches!(app.screen, Screen::Help);
            let inner_w = main_chunks[0].width.saturating_sub(2).max(1) as usize;
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
            .split(main_chunks[0]);

            render_search_bar(frame, left_chunks[0], app);
            render_phase1_count_box(frame, left_chunks[1], effective_screen, &app.search_query);
            left_chunks[2]
        };

        let main_is_active = true;

        match effective_screen {
            Screen::Connect(state) => {
                screens::connect::render(frame, main, state, spinner(app));
            }
            Screen::Catalog(state) => {
                screens::catalog::render(
                    frame,
                    main,
                    state,
                    if matches!(app.screen, Screen::Help) {
                        ""
                    } else {
                        &app.search_query
                    },
                    main_is_active,
                    app,
                );
            }
            Screen::Schema(state) => {
                screens::schema::render(
                    frame,
                    main,
                    state,
                    if matches!(app.screen, Screen::Help) {
                        ""
                    } else {
                        &app.search_query
                    },
                    main_is_active,
                    app,
                );
            }
            Screen::Table(state) => {
                screens::table::render(
                    frame,
                    main,
                    state,
                    if matches!(app.screen, Screen::Help) {
                        ""
                    } else {
                        &app.search_query
                    },
                    main_is_active,
                    app,
                );
            }
            _ => {}
        }
    } else {
        // Phase 2: Inside Table View -> Default 15% Menu : 85% Preview (Resizable)
        let menu_pct = if app.main_panel_pct > 30 {
            15
        } else {
            app.main_panel_pct.clamp(8, 30)
        };
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

            match action {
                crate::app::Action::Partitions => {
                    if app.loading {
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
                    if app.loading {
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
                    if app.loading {
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

        let expected_bg = theme::toast_style().bg.expect("toast_style must set a bg");
        for row in y..y + envelope.height {
            for col in left_x..=right_x {
                let cell = buf.cell((col, row)).unwrap();
                assert_eq!(
                    cell.bg, expected_bg,
                    "cell ({col},{row}) inside toast box lacks the solid toast background"
                );
            }
        }

        if right_x + 1 < width {
            let cell = buf.cell((right_x + 1, y)).unwrap();
            assert_ne!(
                cell.bg, expected_bg,
                "toast background spilled over its right border"
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
