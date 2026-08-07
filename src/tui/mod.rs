mod draw;
pub mod handler;
pub mod theme;

use anyhow::Result;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyEventKind},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::Terminal;
use std::io::stdout;

use crate::app::App;

mod screens {
    pub mod actions;
    pub mod catalog;
    pub mod connect;
    pub mod drilldown;
    pub mod help;
    pub mod partition_tree;
    pub mod query_inspector;
    pub mod results;
    pub mod schema;
    pub mod table;
    pub mod vertical_schema;
}

pub async fn run(app: &mut App) -> Result<()> {
    enable_raw_mode()?;
    let mut stderr = stdout();
    execute!(stderr, EnterAlternateScreen, EnableMouseCapture)?;

    let backend = ratatui::backend::CrosstermBackend::new(stderr);
    let mut terminal = Terminal::new(backend)?;

    let res = run_loop(&mut terminal, app).await;

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
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
        handler::dispatch_command(
            app,
            handler::Command::Connect {
                url,
                user,
                password,
            },
            &tx,
        );
    }

    loop {
        app.frame_count += 1;

        while let Ok(result) = rx.try_recv() {
            handler::handle_async_result(app, result);
        }

        terminal.draw(|frame| draw::ui(frame, app))?;

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
