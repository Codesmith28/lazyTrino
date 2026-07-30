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
    event::{self, Event, KeyEventKind},
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

use crate::app::{App, Mode, Screen};

mod screens {
    pub mod actions;
    pub mod catalog;
    pub mod connect;
    pub mod help;
    pub mod results;
    pub mod schema;
    pub mod table;
}

const SPINNER_CHARS: &[char] = &['|', '/', '-', '\\'];

fn spinner(app: &App) -> String {
    let idx = (app.frame_count / 3) % SPINNER_CHARS.len() as u64;
    format!(" {} ", SPINNER_CHARS[idx as usize])
}

fn render_sidebar(frame: &mut Frame, area: Rect, app: &App) {
    let block = Block::default()
        .title(" Control Panel ")
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::DarkGray));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let sections = Layout::vertical([
        Constraint::Length(5),
        Constraint::Min(0),
    ])
    .split(inner);

    let conn_lines = vec![
        Line::from(Span::styled(
            format!("URL: {}", app.config.url),
            Style::default().fg(Color::Cyan),
        )),
        Line::from(Span::styled(
            format!("User: {}", app.config.user),
            Style::default().fg(Color::Green),
        )),
    ];
    let conn_widget = Paragraph::new(conn_lines);
    frame.render_widget(conn_widget, sections[0]);

    let mut key_lines: Vec<Line> = vec![
        Line::from(Span::styled(" j/k  nav  l  select", Style::default().fg(Color::Gray))),
        Line::from(Span::styled(" h  back  ?  help", Style::default().fg(Color::Gray))),
    ];

    if app.trino_client.is_some() {
        key_lines.push(Line::from(Span::styled(
            " <space>  leader  q  quit",
            Style::default().fg(Color::Gray),
        )));
    }

    let key_widget = Paragraph::new(key_lines);
    frame.render_widget(key_widget, sections[1]);

    if !app.number_buffer.is_empty() {
        let num_area = Rect::new(
            area.x,
            area.y + area.height.saturating_sub(1),
            area.width,
            1,
        );
        let num_text = Paragraph::new(format!("Jump: {}", app.number_buffer))
            .style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD));
        frame.render_widget(num_text, num_area);
    }

    if app.loading {
        let spin = spinner(app);
        let spin_y = area.y + area.height.saturating_sub(4);
        let spin_area = Rect::new(area.x, spin_y, area.width, 1);
        let spin_text = Paragraph::new(format!("Loading {spin}"))
            .style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD));
        frame.render_widget(spin_text, spin_area);
    }

    if let Mode::Search = app.mode {
        let search_y = area.y + area.height.saturating_sub(3);
        let search_area = Rect::new(area.x, search_y, area.width, 1);
        let search_text = Paragraph::new(format!("/ {}", app.search_query))
            .style(Style::default().fg(Color::Black).bg(Color::White));
        frame.render_widget(search_text, search_area);
    }

    if let Mode::Leader { ref keys } = app.mode {
        let leader_y = area.y + area.height.saturating_sub(2);
        let leader_bg = Rect::new(area.x, leader_y, area.width, 1);
        let leader_text = Paragraph::new(format!("Leader: {keys}"))
            .style(
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            )
            .alignment(Alignment::Center);
        frame.render_widget(leader_text, leader_bg);
    }
}

fn ui(frame: &mut Frame, app: &App) {
    match app.screen {
        Screen::Help => {
            screens::help::render(frame, frame.area());
        }
        _ => {
            let chunks = Layout::horizontal([
                Constraint::Percentage(80),
                Constraint::Percentage(20),
            ])
            .split(frame.area());

            let main = chunks[0];
            render_sidebar(frame, chunks[1], app);

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
        }
    }
}

pub async fn run(app: &mut App) -> Result<()> {
    enable_raw_mode()?;
    let mut stderr = stdout();
    execute!(stderr, EnterAlternateScreen)?;

    let backend = ratatui::backend::CrosstermBackend::new(stderr);
    let mut terminal = Terminal::new(backend)?;

    let res = run_loop(&mut terminal, app).await;

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    res
}

async fn run_loop(
    terminal: &mut Terminal<ratatui::backend::CrosstermBackend<std::io::Stdout>>,
    app: &mut App,
) -> Result<()> {
    loop {
        app.frame_count += 1;
        terminal.draw(|frame| ui(frame, app))?;

        if app.should_quit {
            break Ok(());
        }

        if event::poll(std::time::Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
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
        }
    }
}
