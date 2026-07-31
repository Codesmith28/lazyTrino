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

pub mod handler;

use anyhow::Result;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Paragraph},
    Frame, Terminal,
};
use std::io::stdout;

use crate::app::{ActivePanel, App, Mode, Screen};

mod screens {
    pub mod actions;
    pub mod catalog;
    pub mod connect;
    pub mod help;
    pub mod partition_tree;
    pub mod query_inspector;
    pub mod results;
    pub mod schema;
    pub mod table;
    pub mod vertical_schema;
}

const SPINNER_FRAMES: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

fn spinner(app: &App) -> String {
    let idx = (app.frame_count / 2) as usize % SPINNER_FRAMES.len();
    SPINNER_FRAMES[idx].to_string()
}

fn render_search_bar(frame: &mut Frame, area: Rect, app: &App) {
    let is_editing = matches!(app.mode, Mode::Search);
    let border_color = if is_editing {
        Color::Yellow
    } else {
        Color::DarkGray
    };

    let title = if is_editing {
        " Centralized Search [EDITING - Press Enter/Esc to finish] "
    } else {
        " Centralized Search [Press / to search] "
    };

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(border_color));

    let search_text = if app.search_query.is_empty() {
        Span::styled(
            "Type to filter catalogs, schemas, tables, and columns...",
            Style::default().fg(Color::DarkGray),
        )
    } else {
        Span::styled(&app.search_query, Style::default().fg(Color::White).add_modifier(Modifier::BOLD))
    };

    let p = Paragraph::new(Line::from(vec![Span::raw(" / "), search_text]))
        .block(block)
        .wrap(ratatui::widgets::Wrap { trim: false });
    frame.render_widget(p, area);

    if is_editing {
        let inner_width = area.width.saturating_sub(2).max(1) as usize;
        let cursor_index = 3 + app.search_query.len();
        let line_offset = cursor_index / inner_width;
        let col_offset = cursor_index % inner_width;
        let cursor_x = area.x + 1 + (col_offset as u16);
        let cursor_y = area.y + 1 + (line_offset as u16);
        if cursor_y < area.y + area.height - 1 && cursor_x < area.x + area.width - 1 {
            frame.set_cursor_position((cursor_x, cursor_y));
        }
    }
}

fn render_query_bar(frame: &mut Frame, area: Rect, app: &App) {
    let is_editing = matches!(app.mode, Mode::QueryInput);
    let is_table_view = matches!(app.screen, Screen::Results(_));

    let border_color = if is_editing {
        Color::Yellow
    } else if is_table_view {
        Color::Cyan
    } else {
        Color::DarkGray
    };

    let title = if is_editing {
        " Table Query Bar [EDITING - Press Enter to run, Esc to cancel] "
    } else if is_table_view {
        " Table Query Bar [Press 'q' or ':' to write query] "
    } else {
        " Table Query Bar [Disabled - Active only in full data table view] "
    };

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(border_color));

    let (spans, cursor_pos) = match &app.screen {
        Screen::Results(state) => {
            if state.query_buffer.is_empty() {
                (
                    vec![Span::styled(
                        "Write full query (e.g. SELECT * FROM table WHERE age > 10)...",
                        Style::default().fg(Color::DarkGray),
                    )],
                    if is_editing { Some(0) } else { None },
                )
            } else if is_editing {
                if let Some((sel_start, sel_end)) = state.selection_range() {
                    let sel_start = sel_start.min(state.query_buffer.len());
                    let sel_end = sel_end.min(state.query_buffer.len());
                    let before = &state.query_buffer[..sel_start];
                    let selected = &state.query_buffer[sel_start..sel_end];
                    let after = &state.query_buffer[sel_end..];
                    (
                        vec![
                            Span::styled(before, Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
                            Span::styled(selected, Style::default().bg(Color::LightYellow).fg(Color::Black).add_modifier(Modifier::BOLD)),
                            Span::styled(after, Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
                        ],
                        Some(state.query_cursor),
                    )
                } else {
                    (
                        vec![Span::styled(
                            &state.query_buffer,
                            Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
                        )],
                        Some(state.query_cursor),
                    )
                }
            } else {
                (
                    vec![Span::styled(
                        &state.query_buffer,
                        Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
                    )],
                    None,
                )
            }
        }
        _ => (
            vec![Span::styled(
                "Query bar works only when viewing full data table view",
                Style::default().fg(Color::DarkGray),
            )],
            None,
        ),
    };

    let inner_w = area.width.saturating_sub(2).max(1) as usize;
    let visible_lines = area.height.saturating_sub(2).max(1) as usize;

    let mut scroll_y: u16 = 0;
    if is_editing {
        if let Some(pos) = cursor_pos {
            let cursor_index = 7 + pos;
            let line_offset = cursor_index / inner_w;
            if line_offset >= visible_lines {
                scroll_y = (line_offset - visible_lines + 1) as u16;
            }
        }
    }

    let mut line_spans = vec![Span::styled(" SQL > ", Style::default().fg(Color::Yellow))];
    line_spans.extend(spans);

    let p = Paragraph::new(Line::from(line_spans))
    .block(block)
    .wrap(ratatui::widgets::Wrap { trim: false })
    .scroll((scroll_y, 0));
    frame.render_widget(p, area);

    if is_editing {
        if let Some(pos) = cursor_pos {
            let cursor_index = 7 + pos;
            let line_offset = cursor_index / inner_w;
            let col_offset = cursor_index % inner_w;

            let rel_line = line_offset.saturating_sub(scroll_y as usize);
            let cursor_x = area.x + 1 + (col_offset as u16);
            let cursor_y = area.y + 1 + (rel_line as u16);
            if cursor_y < area.y + area.height - 1 && cursor_x < area.x + area.width - 1 {
                frame.set_cursor_position((cursor_x, cursor_y));
            }
        }
    }
}

fn render_control_panel(frame: &mut Frame, area: Rect, app: &App) {
    let border_color = Color::DarkGray;

    let (title, active_table_name) = match &app.screen {
        Screen::Actions(a) => (format!(" Table: {} ", a.table), Some(a.table.clone())),
        Screen::Table(t) => {
            if !t.items.is_empty() && t.selected < t.items.len() {
                let table_name = t.items[t.selected].trim().to_string();
                (format!(" Table: {table_name} "), Some(table_name))
            } else {
                (" Control Panel ".to_string(), None)
            }
        }
        _ => (" Control Panel ".to_string(), None),
    };

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(border_color));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if inner.height == 0 || inner.width == 0 {
        return;
    }

    let loader_height = if app.loading { 2 } else { 0 };
    let main_height = inner.height.saturating_sub(loader_height);

    let sections = Layout::vertical([
        Constraint::Length(main_height),
        Constraint::Length(loader_height),
    ])
    .split(inner);

    if let Some(t_name) = &active_table_name {
        let sub_chunks = Layout::vertical([
            Constraint::Percentage(app.control_panel_split_pct),
            Constraint::Percentage(100 - app.control_panel_split_pct),
        ])
        .split(sections[0]);

        screens::partition_tree::render(
            frame,
            sub_chunks[0],
            &app.partition_tree_lines,
            t_name,
            app.partition_scroll,
            app.active_panel == ActivePanel::PartitionTree,
        );
        screens::vertical_schema::render(
            frame,
            sub_chunks[1],
            &app.vertical_schema_cols,
            t_name,
            app.schema_scroll,
            app.active_panel == ActivePanel::SchemaInspector,
        );
    } else {
        let conn_lines = vec![
            Line::from(Span::styled(
                format!("URL: {}", app.config.url),
                Style::default().fg(Color::Cyan),
            )),
            Line::from(Span::styled(
                format!("User: {}", app.config.user),
                Style::default().fg(Color::Green),
            )),
            Line::from(Span::raw("")),
            Line::from(Span::styled("Active Pane Controls:", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))),
            Line::from(Span::styled(" Shift+H/J/K/L or Shift+Arrows : Switch active pane focus", Style::default().fg(Color::Gray))),
            Line::from(Span::styled(" Left Click inside pane         : Focus pane", Style::default().fg(Color::Gray))),
            Line::from(Span::styled(" j/k or ↓/↑ or Mouse Wheel      : Scroll active pane", Style::default().fg(Color::Gray))),
            Line::from(Span::styled(" Left Drag on panel border      : Resize panel width", Style::default().fg(Color::Gray))),
            Line::from(Span::raw("")),
            Line::from(Span::styled("General:", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))),
            Line::from(Span::styled(" /                            : Central Search", Style::default().fg(Color::Gray))),
            Line::from(Span::styled(" <space>                      : Action Leader", Style::default().fg(Color::Gray))),
            Line::from(Span::styled(" Ctrl+C                       : Quit", Style::default().fg(Color::Gray))),
        ];
        let info_widget = Paragraph::new(conn_lines).wrap(ratatui::widgets::Wrap { trim: false });
        frame.render_widget(info_widget, sections[0]);
    }

    if app.loading {
        let spin = spinner(app);
        let spin_text = Paragraph::new(Line::from(vec![
            Span::styled(format!(" [{spin}] "), Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            Span::styled("EXECUTING TRINO QUERY...", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        ]))
        .alignment(Alignment::Center);
        frame.render_widget(spin_text, sections[1]);
    }
}

fn ui(frame: &mut Frame, app: &App) {
    match app.screen {
        Screen::Help => {
            screens::help::render(frame, frame.area());
        }
        _ => {
            let outer_chunks = Layout::vertical([
                Constraint::Min(0),
                Constraint::Length(7),
            ])
            .split(frame.area());

            let main_chunks = Layout::horizontal([
                Constraint::Percentage(app.main_panel_pct),
                Constraint::Percentage(100 - app.main_panel_pct),
            ])
            .split(outer_chunks[0]);

            let is_table_view = matches!(app.screen, Screen::Results(_));
            let search_active = matches!(app.mode, Mode::Search);
            let query_active = matches!(app.mode, Mode::QueryInput);

            let inner_w = main_chunks[0].width.saturating_sub(2).max(1) as usize;

            let search_height = if search_active {
                let total_chars = 3 + app.search_query.len();
                let lines = (total_chars + inner_w - 1) / inner_w;
                (lines as u16 + 2).clamp(3, 8)
            } else {
                3
            };

            let query_height = if is_table_view {
                if query_active {
                    if let Screen::Results(ref s) = app.screen {
                        let total_chars = 7 + s.query_buffer.len();
                        let lines = (total_chars + inner_w - 1) / inner_w;
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

            let left_chunks = if is_table_view {
                Layout::vertical([
                    Constraint::Length(search_height),
                    Constraint::Length(query_height),
                    Constraint::Min(0),
                ])
                .split(main_chunks[0])
            } else {
                Layout::vertical([
                    Constraint::Length(search_height),
                    Constraint::Min(0),
                ])
                .split(main_chunks[0])
            };

            render_search_bar(frame, left_chunks[0], app);
            render_control_panel(frame, main_chunks[1], app);

            let main = if is_table_view {
                render_query_bar(frame, left_chunks[1], app);
                left_chunks[2]
            } else {
                left_chunks[1]
            };

            let main_is_active = app.active_panel == ActivePanel::MainViewer;

            match &app.screen {
                Screen::Connect(state) => {
                    screens::connect::render(frame, main, state, spinner(app));
                }
                Screen::Catalog(state) => {
                    screens::catalog::render(frame, main, state, &app.search_query, main_is_active);
                }
                Screen::Schema(state) => {
                    screens::schema::render(frame, main, state, &app.search_query, main_is_active);
                }
                Screen::Table(state) => {
                    screens::table::render(frame, main, state, &app.search_query, main_is_active);
                }
                Screen::Actions(state) => {
                    screens::actions::render(
                        frame,
                        main,
                        &state.catalog,
                        &state.schema,
                        &state.table,
                        state.selected,
                        main_is_active,
                    );
                }
                Screen::Results(state) => {
                    screens::results::render(frame, main, state, spinner(app), main_is_active);
                }
                Screen::Help => unreachable!(),
            }

            screens::query_inspector::render(frame, outer_chunks[1], app);
        }
    }
}

pub async fn run(app: &mut App) -> Result<()> {
    enable_raw_mode()?;
    let mut stderr = stdout();
    execute!(stderr, EnterAlternateScreen, EnableMouseCapture)?;

    let backend = ratatui::backend::CrosstermBackend::new(stderr);
    let mut terminal = Terminal::new(backend)?;

    let res = run_loop(&mut terminal, app).await;

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen, DisableMouseCapture)?;
    terminal.show_cursor()?;

    res
}

async fn run_loop(
    terminal: &mut Terminal<ratatui::backend::CrosstermBackend<std::io::Stdout>>,
    app: &mut App,
) -> Result<()> {
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<handler::AsyncResult>();

    if app.auto_connect {
        let url = app.config.url.clone();
        let user = app.config.user.clone();
        let password = app.config.password.clone();
        handler::dispatch_command(app, handler::Command::Connect { url, user, password }, &tx);
    }

    loop {
        app.frame_count += 1;

        while let Ok(result) = rx.try_recv() {
            handler::handle_async_result(app, result);
        }

        terminal.draw(|frame| ui(frame, app))?;

        if app.should_quit {
            break Ok(());
        }

        if event::poll(std::time::Duration::from_millis(33))? {
            match event::read()? {
                Event::Key(key) => {
                    if key.kind != KeyEventKind::Press {
                        continue;
                    }

                    let cmd = handler::handle_key_sync(app, key);
                    if let Some(cmd) = cmd {
                        handler::dispatch_command(app, cmd, &tx);
                    }
                }
                Event::Mouse(mouse) => {
                    let cmd = handler::handle_mouse_sync(app, mouse);
                    if let Some(cmd) = cmd {
                        handler::dispatch_command(app, cmd, &tx);
                    }
                }
                _ => {}
            }
        }
    }
}

