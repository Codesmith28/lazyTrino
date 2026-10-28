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
    layout::{Alignment, Rect},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Paragraph},
};

use crate::app::{ActionState, App, ResultsState, Screen};
use crate::tui::screens;
use crate::tui::theme;

#[allow(dead_code)]
pub fn render_count_box(frame: &mut Frame, area: Rect, res_state: &ResultsState) {
    let block = Block::default()
        .title(" Dataset Records ")
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(theme::style(theme::accent_border_color()));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let mut spans = vec![
        Span::raw("  "),
        Span::styled("● ", theme::warning_bold_style()),
    ];

    render_results_count_spans(&mut spans, res_state, &[]);

    let line = Line::from(spans);
    let p = Paragraph::new(line);
    frame.render_widget(p, inner);
}

fn render_results_count_spans(
    spans: &mut Vec<Span>,
    res_state: &ResultsState,
    schema_cols: &[crate::app::VerticalColumn],
) {
    let loaded_str = crate::app::format_count(res_state.rows.len());

    if !res_state.is_paginated {
        spans.push(Span::styled(loaded_str, theme::number_bold_style()));
        spans.push(Span::styled(" sample rows loaded", theme::text_style()));
        if let Some(tot) = res_state.table_count.or(res_state.total_count) {
            spans.push(Span::styled(" of ", theme::secondary_style()));
            spans.push(Span::styled(
                crate::app::format_count(tot),
                theme::number_bold_style(),
            ));
            spans.push(Span::styled(" table total", theme::text_style()));
        }
    } else {
        if !res_state.has_more_rows {
            spans.push(Span::styled("All ", theme::text_style()));
        }
        spans.push(Span::styled(loaded_str, theme::number_bold_style()));
        spans.push(Span::styled(" rows loaded", theme::text_style()));

        // Increasing order of hierarchy:
        // Level 1: rows loaded
        // Level 2: partition records (if partitioned)
        // Level 3: table total
        if !res_state.filters.is_empty() {
            if let Some(tot) = res_state.total_count {
                spans.push(Span::styled(" of ", theme::secondary_style()));
                spans.push(Span::styled(
                    crate::app::format_count(tot),
                    theme::number_bold_style(),
                ));
                spans.push(Span::styled(" partition records", theme::text_style()));
            } else if res_state.total_count_loading {
                spans.push(Span::styled(" of ", theme::secondary_style()));
                spans.push(Span::styled(
                    "counting partition total...",
                    theme::warning_style(),
                ));
            }

            if let Some(tbl) = res_state.table_count {
                spans.push(Span::styled(" of ", theme::secondary_style()));
                spans.push(Span::styled(
                    crate::app::format_count(tbl),
                    theme::number_bold_style(),
                ));
                spans.push(Span::styled(" table total", theme::text_style()));
            }
        } else {
            // Unpartitioned: rows loaded of table total
            if let Some(tot) = res_state.total_count.or(res_state.table_count) {
                spans.push(Span::styled(" of ", theme::secondary_style()));
                spans.push(Span::styled(
                    crate::app::format_count(tot),
                    theme::number_bold_style(),
                ));
                spans.push(Span::styled(" total", theme::text_style()));
            } else if res_state.total_count_loading {
                spans.push(Span::styled(" of ", theme::secondary_style()));
                spans.push(Span::styled("counting total...", theme::warning_style()));
            }
        }

        if res_state.is_fetching_next_page {
            spans.push(Span::styled(
                " [Fetching page...]",
                theme::warning_bold_style(),
            ));
        }
    }

    let mut all_filter_strs = Vec::new();
    for (c, v) in &res_state.filters {
        all_filter_strs.push(format!("{c}={v}"));
    }
    for (c, f) in &res_state.column_filters {
        match f {
            crate::app::ColumnFilter::Numeric(num) => {
                all_filter_strs.push(format!("{c} {} {}", num.op.as_str(), num.raw_value));
            }
            crate::app::ColumnFilter::Values(vals) => {
                let mut sorted_vals: Vec<_> = vals.iter().cloned().collect();
                sorted_vals.sort();
                if sorted_vals.len() <= 3 {
                    all_filter_strs.push(format!("{c} IN ({})", sorted_vals.join(", ")));
                } else {
                    all_filter_strs.push(format!("{c} IN ({} values)", sorted_vals.len()));
                }
            }
        }
    }
    if !all_filter_strs.is_empty() {
        spans.push(Span::styled("  •  Filters: ", theme::secondary_style()));
        let filter_str = all_filter_strs.join(", ");
        spans.push(Span::styled(filter_str, theme::warning_bold_style()));
    }

    if !res_state.columns.is_empty() && res_state.selected_col < res_state.columns.len() {
        let col_name = &res_state.columns[res_state.selected_col];
        let col_type = schema_cols
            .iter()
            .find(|vc| &vc.name == col_name)
            .map(|vc| vc.data_type.as_str());

        spans.push(Span::styled("  •  Col: ", theme::secondary_style()));
        let col_display = if let Some(t) = col_type {
            format!(
                "{col_name} ({t}) [{}/{}]",
                res_state.selected_col + 1,
                res_state.columns.len()
            )
        } else {
            format!(
                "{col_name} [{}/{}]",
                res_state.selected_col + 1,
                res_state.columns.len()
            )
        };
        spans.push(Span::styled(col_display, theme::info_bold_style()));
    }
}

pub fn render_actions_count_box(frame: &mut Frame, area: Rect, app: &App, state: &ActionState) {
    let title = if !state.table.is_empty() {
        let sep = || Span::styled(" › ", theme::muted_style());
        let mut t = vec![
            Span::raw(" Dataset Records — "),
            Span::styled(state.catalog.clone(), theme::catalog_bold_style()),
            sep(),
            Span::styled(state.schema.clone(), theme::schema_bold_style()),
            sep(),
            Span::styled(state.table.clone(), theme::table_bold_style()),
        ];
        if let Some(ref dd) = state.drilldown
            && !dd.path.is_empty()
        {
            let p_str = dd
                .path
                .iter()
                .map(|(c, v)| format!("{c}={v}"))
                .collect::<Vec<_>>()
                .join(" › ");
            t.push(sep());
            t.push(Span::styled(p_str, theme::warning_bold_style()));
        }
        t.push(Span::raw(" "));
        Line::from(t)
    } else {
        Line::from(" Dataset Records ")
    };

    // Same border as the menu pane, so the two boxes read as one frame.
    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(theme::border_style(
            app.active_panel == crate::app::ActivePanel::MenuPane,
        ));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let mut spans = vec![
        Span::raw("  "),
        Span::styled("● ", theme::warning_bold_style()),
    ];

    let action = crate::app::ACTIONS.get(state.selected).map(|(_, _, a)| a);

    match action {
        Some(crate::app::Action::TableView) => {
            if let Some(ref dd) = state.drilldown
                && !dd.is_leaf()
            {
                // In Drilldown: counts in increasing order of hierarchy:
                // selected partition records -> parent partition records -> table total
                let depth = dd.depth();
                let next_col = dd.next_column().unwrap_or("");
                let selected_val = dd
                    .levels_cache
                    .get(depth)
                    .and_then(|items| items.get(dd.selected))
                    .map(|s| s.as_str());

                if let Some(val) = selected_val {
                    let mut candidate_filter = dd.path.clone();
                    candidate_filter.push((next_col.to_string(), val.to_string()));
                    if let Some(cnt) = dd.counts_cache.get(&candidate_filter) {
                        spans.push(Span::styled(
                            crate::app::format_count(*cnt),
                            theme::number_bold_style(),
                        ));
                        spans.push(Span::styled(
                            format!(" records in {next_col}={val}"),
                            theme::text_style(),
                        ));
                    } else {
                        spans.push(Span::styled(
                            format!("counting records in {next_col}={val}..."),
                            theme::warning_style(),
                        ));
                    }
                } else {
                    spans.push(Span::styled("Partition hierarchy", theme::text_style()));
                }

                if depth > 0
                    && let Some(parent_cnt) = dd.counts_cache.get(&dd.path)
                {
                    spans.push(Span::styled(" of ", theme::secondary_style()));
                    spans.push(Span::styled(
                        crate::app::format_count(*parent_cnt),
                        theme::number_bold_style(),
                    ));
                    spans.push(Span::styled(
                        " parent partition records",
                        theme::text_style(),
                    ));
                }

                if let Some(tot) = dd.table_count {
                    spans.push(Span::styled(" of ", theme::secondary_style()));
                    spans.push(Span::styled(
                        crate::app::format_count(tot),
                        theme::number_bold_style(),
                    ));
                    spans.push(Span::styled(" table total", theme::text_style()));
                } else if dd.table_count_loading {
                    spans.push(Span::styled(" of ", theme::secondary_style()));
                    spans.push(Span::styled(
                        "counting table total...",
                        theme::warning_style(),
                    ));
                }
            } else if let Some(ref res_state) = state.results {
                render_results_count_spans(&mut spans, res_state, &app.vertical_schema_cols);
            } else {
                spans.push(Span::styled("Table View", theme::bold_text_style()));
                let table_total = state
                    .drilldown
                    .as_ref()
                    .and_then(|d| d.table_count)
                    .or_else(|| {
                        state
                            .results
                            .as_ref()
                            .and_then(|r| r.table_count.or(r.total_count))
                    });
                if let Some(tot) = table_total {
                    spans.push(Span::styled(" of ", theme::secondary_style()));
                    spans.push(Span::styled(
                        crate::app::format_count(tot),
                        theme::number_bold_style(),
                    ));
                    spans.push(Span::styled(" table total", theme::text_style()));
                }
            }
        }
        Some(crate::app::Action::Partitions) => {
            let levels = app.partition_tree_lines.len().saturating_sub(1);
            let table_total = state
                .drilldown
                .as_ref()
                .and_then(|d| d.table_count)
                .or_else(|| {
                    state
                        .results
                        .as_ref()
                        .and_then(|r| r.table_count.or(r.total_count))
                });
            spans.push(Span::styled(
                crate::app::format_count(levels),
                theme::number_bold_style(),
            ));
            spans.push(Span::styled(" partition levels", theme::text_style()));
            if let Some(tot) = table_total {
                spans.push(Span::styled(" of ", theme::secondary_style()));
                spans.push(Span::styled(
                    crate::app::format_count(tot),
                    theme::number_bold_style(),
                ));
                spans.push(Span::styled(" table records", theme::text_style()));
            }
            spans.push(Span::styled(" in table ", theme::secondary_style()));
            spans.push(Span::styled(&state.table, theme::table_bold_style()));
        }
        Some(crate::app::Action::Schema) => {
            let cols = app.vertical_schema_cols.len();
            let table_total = state
                .drilldown
                .as_ref()
                .and_then(|d| d.table_count)
                .or_else(|| {
                    state
                        .results
                        .as_ref()
                        .and_then(|r| r.table_count.or(r.total_count))
                });
            spans.push(Span::styled(
                crate::app::format_count(cols),
                theme::number_bold_style(),
            ));
            spans.push(Span::styled(" columns", theme::text_style()));
            if let Some(tot) = table_total {
                spans.push(Span::styled(" of ", theme::secondary_style()));
                spans.push(Span::styled(
                    crate::app::format_count(tot),
                    theme::number_bold_style(),
                ));
                spans.push(Span::styled(" table records", theme::text_style()));
            }
            spans.push(Span::styled(" in table ", theme::secondary_style()));
            spans.push(Span::styled(&state.table, theme::table_bold_style()));
        }
        Some(crate::app::Action::TableDDL) => {
            let lines = state
                .metadata
                .as_ref()
                .map(|m| m.ddl_text.lines().count())
                .unwrap_or(0);
            spans.push(Span::styled(
                crate::app::format_count(lines),
                theme::number_bold_style(),
            ));
            spans.push(Span::styled(
                " lines of DDL definition for table ",
                theme::text_style(),
            ));
            spans.push(Span::styled(&state.table, theme::table_bold_style()));
        }
        _ => {
            if let Some(ref res_state) = state.results {
                render_results_count_spans(&mut spans, res_state, &app.vertical_schema_cols);
            } else {
                let name = crate::app::ACTIONS
                    .get(state.selected)
                    .map(|(_, n, _)| *n)
                    .unwrap_or("");
                spans.push(Span::styled(name, theme::bold_text_style()));
                let table_total = state
                    .drilldown
                    .as_ref()
                    .and_then(|d| d.table_count)
                    .or_else(|| {
                        state
                            .results
                            .as_ref()
                            .and_then(|r| r.table_count.or(r.total_count))
                    });
                if let Some(tot) = table_total {
                    spans.push(Span::styled(" of ", theme::secondary_style()));
                    spans.push(Span::styled(
                        crate::app::format_count(tot),
                        theme::number_bold_style(),
                    ));
                    spans.push(Span::styled(" table records", theme::text_style()));
                }
            }
        }
    }

    let line = Line::from(spans);
    let p = Paragraph::new(line);
    frame.render_widget(p, inner);
}

pub fn render_phase1_count_box(frame: &mut Frame, area: Rect, screen: &Screen, search: &str) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(theme::style(theme::accent_border_color()));

    let mut spans = vec![
        Span::raw("  "),
        Span::styled("● ", theme::warning_bold_style()),
    ];

    match screen {
        Screen::Catalog(state) => {
            let total = state.items.len();
            let filtered = if search.is_empty() {
                total
            } else {
                state
                    .items
                    .iter()
                    .filter(|i| i.to_lowercase().contains(&search.to_lowercase()))
                    .count()
            };
            if filtered == total {
                spans.push(Span::styled(
                    crate::app::format_count(total),
                    theme::number_bold_style(),
                ));
                spans.push(Span::styled(" catalogs total", theme::text_style()));
            } else {
                spans.push(Span::styled(
                    crate::app::format_count(filtered),
                    theme::number_bold_style(),
                ));
                spans.push(Span::styled(" of ", theme::secondary_style()));
                spans.push(Span::styled(
                    crate::app::format_count(total),
                    theme::number_bold_style(),
                ));
                spans.push(Span::styled(" catalogs total", theme::text_style()));
            }
        }
        Screen::Schema(state) => {
            let total = state.items.len();
            let filtered = if search.is_empty() {
                total
            } else {
                state
                    .items
                    .iter()
                    .filter(|i| i.to_lowercase().contains(&search.to_lowercase()))
                    .count()
            };
            if filtered == total {
                spans.push(Span::styled(
                    crate::app::format_count(total),
                    theme::number_bold_style(),
                ));
            } else {
                spans.push(Span::styled(
                    format!(
                        "{}/{}",
                        crate::app::format_count(filtered),
                        crate::app::format_count(total)
                    ),
                    theme::number_bold_style(),
                ));
            }
            spans.push(Span::styled(" schemas of catalog ", theme::text_style()));
            spans.push(Span::styled(&state.catalog, theme::catalog_bold_style()));
        }
        Screen::Table(state) => {
            let total = state.items.len();
            let filtered = if search.is_empty() {
                total
            } else {
                state
                    .items
                    .iter()
                    .filter(|i| i.to_lowercase().contains(&search.to_lowercase()))
                    .count()
            };
            if filtered == total {
                spans.push(Span::styled(
                    crate::app::format_count(total),
                    theme::number_bold_style(),
                ));
            } else {
                spans.push(Span::styled(
                    format!(
                        "{}/{}",
                        crate::app::format_count(filtered),
                        crate::app::format_count(total)
                    ),
                    theme::number_bold_style(),
                ));
            }
            spans.push(Span::styled(" tables of schema ", theme::text_style()));
            spans.push(Span::styled(&state.schema, theme::schema_bold_style()));
            spans.push(Span::styled(" of catalog ", theme::secondary_style()));
            spans.push(Span::styled(&state.catalog, theme::catalog_bold_style()));
        }
        _ => return,
    }

    let inner = block.inner(area);
    frame.render_widget(block, area);
    frame.render_widget(Paragraph::new(Line::from(spans)), inner);
}

#[allow(clippy::too_many_arguments)]
pub fn render_default_results_preview(
    frame: &mut Frame,
    preview_pane_area: Rect,
    app: &App,
    is_loading: bool,
    spin: String,
    table_name: &str,
    selected_idx: usize,
    preview_is_active: bool,
) {
    let screen = if let Screen::Help = &app.screen {
        app.prev_screen.as_deref().unwrap_or(&app.screen)
    } else {
        &app.screen
    };
    if let Screen::Actions(state) = screen {
        if let Some(ref res_state) = state.results {
            screens::results::render(
                frame,
                preview_pane_area,
                res_state,
                spin,
                preview_is_active,
                app,
            );
        } else if is_loading
            || (state.ddl_loading && state.pending_action.is_some())
            || (state.metadata_loading && state.pending_action.is_some())
        {
            let title = format!(
                " Preview — {} ({}) ",
                table_name,
                crate::app::ACTIONS[selected_idx].1
            );
            let block = Block::default()
                .title(title)
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(theme::border_style(preview_is_active));
            let inner = block.inner(preview_pane_area);
            frame.render_widget(block, preview_pane_area);
            let msg = if state.ddl_loading {
                "ANALYZING TABLE STRUCTURE & PARTITIONS..."
            } else if state.metadata_loading {
                "FETCHING TABLE METADATA..."
            } else {
                "EXECUTING TRINO QUERY..."
            };
            let spin_text = Paragraph::new(Line::from(vec![
                Span::styled(format!(" [{spin}] "), theme::warning_bold_style()),
                Span::styled(msg, theme::info_bold_style()),
            ]))
            .alignment(Alignment::Center);
            frame.render_widget(spin_text, inner);
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
}

pub fn render_placeholder_preview(
    frame: &mut Frame,
    area: Rect,
    table_name: &str,
    selected_idx: usize,
    preview_is_active: bool,
) {
    let action_name = if selected_idx < crate::app::ACTIONS.len() {
        crate::app::ACTIONS[selected_idx].1
    } else {
        ""
    };
    let title = format!(" Preview — {table_name} ({action_name}) ");
    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(theme::border_style(preview_is_active));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let info_lines = vec![
        Line::from(Span::styled(
            " Table Preview Area",
            theme::info_bold_style(),
        )),
        Line::from(""),
        Line::from(Span::styled(
            format!(
                " Active Selection: [{}] {action_name}",
                crate::app::ACTIONS[selected_idx].0
            ),
            theme::warning_bold_style(),
        )),
        Line::from(""),
        Line::from(Span::styled(
            " Press Enter (or hit hotkey) to load and display preview output.",
            theme::text_style(),
        )),
        Line::from(""),
        Line::from(Span::styled(" Menu Shortcuts:", theme::muted_style())),
        Line::from("   [v] Table View Mode       [c] Table DDL            [i] Info Schema"),
        Line::from("   [s] Show Stats            [p] Sample (20 rows)     [P] Partition Tree"),
        Line::from("   [S] Vertical Schema"),
    ];
    let info_p = Paragraph::new(info_lines).alignment(Alignment::Center);
    frame.render_widget(info_p, inner);
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{Terminal, backend::TestBackend, style::Modifier};

    #[test]
    fn test_render_count_box_paginated_with_total_and_filters() {
        let state = ResultsState {
            rows: vec![vec!["1".to_string()], vec!["2".to_string()]],
            is_paginated: true,
            has_more_rows: true,
            total_count: Some(5000),
            filters: vec![("status".to_string(), "active".to_string())],
            ..Default::default()
        };

        let backend = TestBackend::new(80, 3);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| {
                render_count_box(f, Rect::new(0, 0, 80, 3), &state);
            })
            .unwrap();

        let buffer = terminal.backend().buffer().clone();
        let content: String = (0..3)
            .map(|y| {
                (0..80)
                    .map(|x| buffer.cell((x, y)).unwrap().symbol())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n");

        assert!(content.contains("Dataset Records"), "Must have box title");
        assert!(
            content.contains("2 rows loaded of 5,000 partition records"),
            "Must format loaded of total: {content}"
        );
        assert!(
            content.contains("Filters: status=active"),
            "Must show filters: {content}"
        );

        // Verify that '2' and '5' cells have BOLD modifier
        let mut found_bold_loaded = false;
        let mut found_bold_total = false;
        for x in 0..80 {
            let cell = buffer.cell((x, 1)).unwrap();
            if cell.symbol() == "2" && cell.style().add_modifier.contains(Modifier::BOLD) {
                found_bold_loaded = true;
            }
            if cell.symbol() == "5" && cell.style().add_modifier.contains(Modifier::BOLD) {
                found_bold_total = true;
            }
        }
        assert!(found_bold_loaded, "x must be bold");
        assert!(found_bold_total, "y must be bold");
    }

    #[test]
    fn test_render_count_box_counting_total() {
        let state = ResultsState {
            rows: vec![vec!["1".to_string()]],
            is_paginated: true,
            has_more_rows: true,
            total_count: None,
            total_count_loading: true,
            ..Default::default()
        };

        let backend = TestBackend::new(80, 3);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| {
                render_count_box(f, Rect::new(0, 0, 80, 3), &state);
            })
            .unwrap();

        let buffer = terminal.backend().buffer().clone();
        let content: String = (0..3)
            .map(|y| {
                (0..80)
                    .map(|x| buffer.cell((x, y)).unwrap().symbol())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n");

        assert!(
            content.contains("1 rows loaded of counting total..."),
            "Must show counting total: {content}"
        );
    }

    #[test]
    fn test_render_count_box_sample_mode() {
        let state = ResultsState {
            rows: vec![vec!["1".to_string()], vec!["2".to_string()]],
            is_paginated: false,
            ..Default::default()
        };

        let backend = TestBackend::new(80, 3);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| {
                render_count_box(f, Rect::new(0, 0, 80, 3), &state);
            })
            .unwrap();

        let buffer = terminal.backend().buffer().clone();
        let content: String = (0..3)
            .map(|y| {
                (0..80)
                    .map(|x| buffer.cell((x, y)).unwrap().symbol())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n");

        assert!(
            content.contains("2 sample rows loaded"),
            "Must show sample mode rows: {content}"
        );
    }

    #[test]
    fn test_render_count_box_shows_active_column_and_warning_filter_color() {
        let state = ResultsState {
            rows: vec![vec!["1".to_string(), "active".to_string()]],
            columns: vec!["id".to_string(), "status".to_string()],
            selected_col: 1,
            is_paginated: true,
            has_more_rows: false,
            filters: vec![("dt".to_string(), "2026-10-14".to_string())],
            ..Default::default()
        };

        let backend = TestBackend::new(100, 3);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| {
                render_count_box(f, Rect::new(0, 0, 100, 3), &state);
            })
            .unwrap();

        let buffer = terminal.backend().buffer().clone();
        let content: String = (0..3)
            .map(|y| {
                (0..100)
                    .map(|x| buffer.cell((x, y)).unwrap().symbol())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n");

        assert!(content.contains("Filters: dt=2026-10-14"));
        assert!(content.contains("Col: status [2/2]"));

        // Verify filter text cell color is Yellow, NOT Cyan
        let mut found_yellow_filter = false;
        for x in 0..100 {
            let cell = buffer.cell((x, 1)).unwrap();
            if cell.symbol() == "d" && cell.style().fg == theme::warning_style().fg {
                found_yellow_filter = true;
                break;
            }
        }
        assert!(
            found_yellow_filter,
            "Filter text must be rendered in Yellow (warning style)"
        );
    }
}
