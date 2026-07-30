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
    let border_color = if matches!(app.mode, Mode::Search) {
        Color::Yellow
    } else {
        Color::DarkGray
    };

    let title = if matches!(app.mode, Mode::Search) {
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

    let p = Paragraph::new(Line::from(vec![Span::raw(" / "), search_text])).block(block);
    frame.render_widget(p, area);
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
        let info_widget = Paragraph::new(conn_lines);
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

            let left_chunks = Layout::vertical([
                Constraint::Length(3),
                Constraint::Min(0),
            ])
            .split(main_chunks[0]);

            render_search_bar(frame, left_chunks[0], app);
            render_control_panel(frame, main_chunks[1], app);

            let main = left_chunks[1];

            match &app.screen {
                Screen::Connect(state) => {
                    screens::connect::render(frame, main, state, spinner(app));
                }
                Screen::Catalog(state) => {
                    screens::catalog::render(frame, main, state, &app.search_query);
                }
                Screen::Schema(state) => {
                    screens::schema::render(frame, main, state, &app.search_query);
                }
                Screen::Table(state) => {
                    screens::table::render(frame, main, state, &app.search_query);
                }
                Screen::Actions(state) => {
                    screens::actions::render(
                        frame,
                        main,
                        &state.catalog,
                        &state.schema,
                        &state.table,
                        state.selected,
                    );
                }
                Screen::Results(state) => {
                    screens::results::render(frame, main, state, spinner(app));
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
    if app.auto_connect {
        let url = app.config.url.clone();
        let user = app.config.user.clone();
        let password = app.config.password.clone();
        handler::execute_command(app, handler::Command::Connect { url, user, password }).await;
    }

    loop {
        app.frame_count += 1;
        terminal.draw(|frame| ui(frame, app))?;

        if app.should_quit {
            break Ok(());
        }

        if event::poll(std::time::Duration::from_millis(100))? {
            match event::read()? {
                Event::Key(key) => {
                    if key.kind != KeyEventKind::Press {
                        continue;
                    }

                    let cmd = handler::handle_key_sync(app, key);

                    if let Some(cmd) = cmd {
                        app.loading = true;
                        app.frame_count += 1;
                        terminal.draw(|frame| ui(frame, app))?;
                        handler::execute_command(app, cmd).await;
                    }
                }
                Event::Mouse(mouse) => {
                    handler::handle_mouse_sync(app, mouse);
                }
                _ => {}
            }
        }
    }
}

