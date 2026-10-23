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

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::app::{App, HelpModalState, Mode};
use crate::tui::handler::Command;
use crate::tui::handler::navigation::normalize_key_code;

pub fn open_help_modal(app: &mut App) {
    app.clear_mouse_selection();
    app.help_modal = Some(HelpModalState::default());
    app.help_scroll = 0;
    app.mode = Mode::HelpModal;
}

pub fn close_help_modal(app: &mut App) {
    app.help_modal = None;
    app.mode = Mode::Normal;
}

pub fn handle_help_modal_key(app: &mut App, key: KeyEvent) -> Option<Command> {
    let is_searching = app
        .help_modal
        .as_ref()
        .map(|m| m.is_searching)
        .unwrap_or(false);

    if is_searching {
        match key.code {
            KeyCode::Esc | KeyCode::Enter => {
                if let Some(ref mut m) = app.help_modal {
                    m.is_searching = false;
                }
                None
            }
            KeyCode::Backspace => {
                if let Some(ref mut m) = app.help_modal {
                    m.search_query.pop();
                    m.scroll = 0;
                    app.help_scroll = 0;
                }
                None
            }
            KeyCode::Char(c) => {
                if let Some(ref mut m) = app.help_modal {
                    m.search_query.push(c);
                    m.scroll = 0;
                    app.help_scroll = 0;
                }
                None
            }
            _ => None,
        }
    } else {
        let code = normalize_key_code(key.code);
        if code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
            app.should_quit = true;
            return None;
        }

        match code {
            KeyCode::Char('?') | KeyCode::Char('q') | KeyCode::Esc => {
                close_help_modal(app);
                None
            }
            KeyCode::Char('/') => {
                if let Some(ref mut m) = app.help_modal {
                    m.is_searching = true;
                }
                None
            }
            KeyCode::Char('j') | KeyCode::Down => {
                if let Some(ref mut m) = app.help_modal {
                    m.scroll = m.scroll.saturating_add(1);
                    app.help_scroll = m.scroll;
                }
                None
            }
            KeyCode::Char('k') | KeyCode::Up => {
                if let Some(ref mut m) = app.help_modal {
                    m.scroll = m.scroll.saturating_sub(1);
                    app.help_scroll = m.scroll;
                }
                None
            }
            KeyCode::PageDown | KeyCode::Char('d') | KeyCode::Char('f')
                if code == KeyCode::PageDown || key.modifiers.contains(KeyModifiers::CONTROL) =>
            {
                if let Some(ref mut m) = app.help_modal {
                    let (w, h) = crossterm::terminal::size().unwrap_or((80, 24));
                    let total_lines = crate::tui::screens::help::wrapped_help_lines(
                        &m.search_query,
                        ratatui::layout::Rect::new(0, 0, w, h),
                    )
                    .len();
                    m.scroll = (m.scroll + 15).min(total_lines.saturating_sub(1));
                    app.help_scroll = m.scroll;
                }
                None
            }
            KeyCode::PageUp | KeyCode::Char('u') | KeyCode::Char('b')
                if code == KeyCode::PageUp || key.modifiers.contains(KeyModifiers::CONTROL) =>
            {
                if let Some(ref mut m) = app.help_modal {
                    m.scroll = m.scroll.saturating_sub(15);
                    app.help_scroll = m.scroll;
                }
                None
            }
            KeyCode::Char('g') | KeyCode::Home => {
                if let Some(ref mut m) = app.help_modal {
                    m.scroll = 0;
                    app.help_scroll = 0;
                }
                None
            }
            KeyCode::Char('G') | KeyCode::End => {
                if let Some(ref mut m) = app.help_modal {
                    m.scroll = 100;
                    app.help_scroll = 100;
                }
                None
            }
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ConnectionConfig;

    #[test]
    fn test_open_and_close_help_modal() {
        let mut app = App::new(ConnectionConfig::default(), false);
        assert_eq!(app.mode, Mode::Normal);
        assert!(app.help_modal.is_none());

        open_help_modal(&mut app);
        assert_eq!(app.mode, Mode::HelpModal);
        assert!(app.help_modal.is_some());

        // Press '?' to close
        handle_help_modal_key(&mut app, KeyEvent::from(KeyCode::Char('?')));
        assert_eq!(app.mode, Mode::Normal);
        assert!(app.help_modal.is_none());

        // Open again, press 'q' to close
        open_help_modal(&mut app);
        handle_help_modal_key(&mut app, KeyEvent::from(KeyCode::Char('q')));
        assert_eq!(app.mode, Mode::Normal);
        assert!(app.help_modal.is_none());

        // Open again, press Esc to close
        open_help_modal(&mut app);
        handle_help_modal_key(&mut app, KeyEvent::from(KeyCode::Esc));
        assert_eq!(app.mode, Mode::Normal);
        assert!(app.help_modal.is_none());
    }

    #[test]
    fn test_help_modal_scroll() {
        let mut app = App::new(ConnectionConfig::default(), false);
        open_help_modal(&mut app);

        handle_help_modal_key(&mut app, KeyEvent::from(KeyCode::Char('j')));
        assert_eq!(app.help_modal.as_ref().unwrap().scroll, 1);
        assert_eq!(app.help_scroll, 1);

        handle_help_modal_key(&mut app, KeyEvent::from(KeyCode::Char('k')));
        assert_eq!(app.help_modal.as_ref().unwrap().scroll, 0);
        assert_eq!(app.help_scroll, 0);

        handle_help_modal_key(&mut app, KeyEvent::from(KeyCode::Char('G')));
        assert_eq!(app.help_modal.as_ref().unwrap().scroll, 100);

        handle_help_modal_key(&mut app, KeyEvent::from(KeyCode::Char('g')));
        assert_eq!(app.help_modal.as_ref().unwrap().scroll, 0);
    }

    #[test]
    fn test_help_modal_search_flow() {
        let mut app = App::new(ConnectionConfig::default(), false);
        open_help_modal(&mut app);

        // Press '/' to start searching
        handle_help_modal_key(&mut app, KeyEvent::from(KeyCode::Char('/')));
        assert!(app.help_modal.as_ref().unwrap().is_searching);

        // Type "filter"
        for c in "filter".chars() {
            handle_help_modal_key(&mut app, KeyEvent::from(KeyCode::Char(c)));
        }
        assert_eq!(app.help_modal.as_ref().unwrap().search_query, "filter");

        // Backspace
        handle_help_modal_key(&mut app, KeyEvent::from(KeyCode::Backspace));
        assert_eq!(app.help_modal.as_ref().unwrap().search_query, "filte");

        // Press Esc to exit typing mode (modal stays open)
        handle_help_modal_key(&mut app, KeyEvent::from(KeyCode::Esc));
        assert!(!app.help_modal.as_ref().unwrap().is_searching);
        assert_eq!(app.mode, Mode::HelpModal);

        // Press Esc again to close modal
        handle_help_modal_key(&mut app, KeyEvent::from(KeyCode::Esc));
        assert_eq!(app.mode, Mode::Normal);
        assert!(app.help_modal.is_none());
    }
}
