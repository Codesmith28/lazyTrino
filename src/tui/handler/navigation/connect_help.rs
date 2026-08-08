use crossterm::event::{KeyCode, KeyEvent};

use crate::app::{App, Screen};
use crate::tui::handler::Command;

pub fn connect_keys(app: &mut App, key: KeyEvent) -> Option<Command> {
    let state_loading = match &app.screen {
        Screen::Connect(s) => s.loading,
        _ => return None,
    };
    if state_loading {
        return None;
    }
    let state = match &mut app.screen {
        Screen::Connect(s) => s,
        _ => return None,
    };
    match key.code {
        KeyCode::Tab | KeyCode::Char('\t') => {
            state.focused = (state.focused + 1) % 3;
        }
        KeyCode::Backspace => match state.focused {
            0 => {
                state.url.pop();
            }
            1 => {
                state.user.pop();
            }
            2 => {
                state.password.pop();
            }
            _ => {}
        },
        KeyCode::Char(c) => match state.focused {
            0 => state.url.push(c),
            1 => state.user.push(c),
            2 => state.password.push(c),
            _ => {}
        },
        KeyCode::Enter if !state.url.is_empty() && !state.user.is_empty() => {
            let url = state.url.clone();
            let user = state.user.clone();
            let password = state.password.clone();
            state.loading = true;
            return Some(Command::Connect {
                url,
                user,
                password,
            });
        }
        _ => {}
    }
    None
}

#[allow(dead_code)]
pub fn help_keys(app: &mut App, key: KeyEvent) -> Option<Command> {
    match key.code {
        KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('?') => {
            if let Some(prev) = app.prev_screen.take() {
                app.screen = *prev;
            }
        }
        _ => {}
    }
    None
}

#[allow(dead_code)]
pub fn query_inspector_keys(app: &mut App, key: KeyEvent) -> Option<Command> {
    match key.code {
        KeyCode::Char('j') | KeyCode::Down => {
            app.query_inspector_scroll = app.query_inspector_scroll.saturating_add(1);
        }
        KeyCode::Char('k') | KeyCode::Up => {
            app.query_inspector_scroll = app.query_inspector_scroll.saturating_sub(1);
        }
        _ => {}
    }
    None
}
