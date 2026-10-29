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

//! The bottom help bar scrolls sideways when its hints are wider than the
//! terminal. Clicking it gives it focus. While it is focused it alone takes
//! the scroll keys, so the pane behind it never moves at the same time, and
//! the mouse wheel over it scrolls it without reaching any pane.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};

use crate::app::{App, Mode};

const KEY_STEP: isize = 8;
const WHEEL_STEP: isize = 4;

/// Whether the hints are wider than the bar, so there is something to scroll.
pub fn is_scrollable(app: &App) -> bool {
    app.footer_max_scroll.get() > 0
}

pub fn contains(app: &App, column: u16, row: u16) -> bool {
    let r = app.footer_rect.get();
    r.width > 0
        && r.height > 0
        && column >= r.x
        && column < r.x + r.width
        && row >= r.y
        && row < r.y + r.height
}

pub fn scroll_by(app: &App, delta: isize) {
    let max = app.footer_max_scroll.get() as isize;
    let next = (app.footer_scroll.get() as isize + delta).clamp(0, max);
    app.footer_scroll.set(next as usize);
}

pub fn scroll_to(app: &App, offset: usize) {
    app.footer_scroll
        .set(offset.min(app.footer_max_scroll.get()));
}

/// Scroll keys while the bar is focused. Returns true when the key was
/// consumed. Any other key hands focus back and goes on to the pane.
pub fn handle_key(app: &mut App, key: KeyEvent) -> bool {
    if !app.footer_focused {
        return false;
    }
    if app.mode != Mode::Normal || !is_scrollable(app) {
        app.footer_focused = false;
        return false;
    }
    let plain = !key
        .modifiers
        .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT);
    match key.code {
        KeyCode::Left => scroll_by(app, -KEY_STEP),
        KeyCode::Right => scroll_by(app, KEY_STEP),
        KeyCode::Home => scroll_to(app, 0),
        KeyCode::End => scroll_to(app, usize::MAX),
        KeyCode::Char('h') if plain => scroll_by(app, -KEY_STEP),
        KeyCode::Char('l') if plain => scroll_by(app, KEY_STEP),
        KeyCode::Char('g') if plain => scroll_to(app, 0),
        KeyCode::Char('G') if plain => scroll_to(app, usize::MAX),
        KeyCode::Esc => app.footer_focused = false,
        _ => {
            app.footer_focused = false;
            return false;
        }
    }
    true
}

/// Clicks and the wheel over the bar. Returns true when the event belonged to
/// it, so no pane sees it.
pub fn handle_mouse(app: &mut App, mouse: MouseEvent) -> bool {
    if app.mode != Mode::Normal {
        return false;
    }
    let over = contains(app, mouse.column, mouse.row);
    match mouse.kind {
        MouseEventKind::Down(_) => {
            app.footer_focused = over && is_scrollable(app);
            over
        }
        MouseEventKind::Up(MouseButton::Left) | MouseEventKind::Drag(MouseButton::Left) => over,
        MouseEventKind::ScrollDown | MouseEventKind::ScrollRight if over => {
            scroll_by(app, WHEEL_STEP);
            true
        }
        MouseEventKind::ScrollUp | MouseEventKind::ScrollLeft if over => {
            scroll_by(app, -WHEEL_STEP);
            true
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ConnectionConfig;
    use ratatui::layout::Rect;

    /// An app whose bar sits on row 20 and can scroll 30 columns.
    fn app() -> App {
        let app = App::new(
            ConnectionConfig {
                url: "http://localhost:8080".to_string(),
                user: "test".to_string(),
                password: String::new(),
                query_cache_limit: crate::config::DEFAULT_QUERY_CACHE_LIMIT,
            },
            false,
        );
        app.footer_rect.set(Rect::new(0, 20, 80, 1));
        app.footer_max_scroll.set(30);
        app
    }

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::from(code)
    }

    fn mouse(kind: MouseEventKind, column: u16, row: u16) -> MouseEvent {
        MouseEvent {
            kind,
            column,
            row,
            modifiers: KeyModifiers::NONE,
        }
    }

    #[test]
    fn scrolling_stays_within_the_hints() {
        let app = app();
        scroll_by(&app, -5);
        assert_eq!(app.footer_scroll.get(), 0);
        scroll_by(&app, 100);
        assert_eq!(app.footer_scroll.get(), 30);
        scroll_to(&app, 7);
        assert_eq!(app.footer_scroll.get(), 7);
    }

    #[test]
    fn a_focused_bar_takes_the_scroll_keys() {
        let mut app = app();
        app.footer_focused = true;
        for code in [KeyCode::Right, KeyCode::Char('l')] {
            assert!(handle_key(&mut app, key(code)));
        }
        assert_eq!(app.footer_scroll.get(), 16);
        assert!(handle_key(&mut app, key(KeyCode::Left)));
        assert!(handle_key(&mut app, key(KeyCode::Char('h'))));
        assert_eq!(app.footer_scroll.get(), 0);
        assert!(handle_key(&mut app, key(KeyCode::End)));
        assert_eq!(app.footer_scroll.get(), 30);
        assert!(handle_key(&mut app, key(KeyCode::Char('g'))));
        assert_eq!(app.footer_scroll.get(), 0);
        assert!(app.footer_focused, "scrolling keeps the focus");
    }

    #[test]
    fn an_unfocused_bar_leaves_every_key_to_the_pane() {
        let mut app = app();
        assert!(!handle_key(&mut app, key(KeyCode::Right)));
        assert!(!handle_key(&mut app, key(KeyCode::Char('l'))));
        assert_eq!(app.footer_scroll.get(), 0);
    }

    #[test]
    fn esc_leaves_the_bar_and_other_keys_hand_focus_back_to_the_pane() {
        let mut app = app();
        app.footer_focused = true;
        assert!(handle_key(&mut app, key(KeyCode::Esc)), "Esc is consumed");
        assert!(!app.footer_focused);

        app.footer_focused = true;
        assert!(
            !handle_key(&mut app, key(KeyCode::Char('j'))),
            "other keys go on to the pane"
        );
        assert!(!app.footer_focused);
    }

    #[test]
    fn a_bar_with_nothing_to_scroll_gives_focus_back() {
        let mut app = app();
        app.footer_max_scroll.set(0);
        app.footer_focused = true;
        assert!(!handle_key(&mut app, key(KeyCode::Right)));
        assert!(!app.footer_focused);
    }

    #[test]
    fn clicking_the_bar_focuses_it_and_clicking_elsewhere_does_not() {
        let mut app = app();
        assert!(handle_mouse(
            &mut app,
            mouse(MouseEventKind::Down(MouseButton::Left), 10, 20)
        ));
        assert!(app.footer_focused);

        assert!(
            !handle_mouse(
                &mut app,
                mouse(MouseEventKind::Down(MouseButton::Left), 10, 5)
            ),
            "a click on a pane is the pane's"
        );
        assert!(!app.footer_focused, "and it takes the focus away");
    }

    #[test]
    fn a_click_on_a_bar_that_fits_is_swallowed_without_taking_focus() {
        let mut app = app();
        app.footer_max_scroll.set(0);
        assert!(handle_mouse(
            &mut app,
            mouse(MouseEventKind::Down(MouseButton::Left), 10, 20)
        ));
        assert!(!app.footer_focused);
        assert!(
            handle_mouse(
                &mut app,
                mouse(MouseEventKind::Up(MouseButton::Left), 10, 20)
            ),
            "the release must not reach a pane either"
        );
    }

    #[test]
    fn a_focused_bar_and_the_active_pane_never_scroll_together() {
        use crate::app::{ActionState, ActivePanel, Screen};

        let mut app = app();
        app.screen = Screen::Actions(ActionState::default());
        app.active_panel = ActivePanel::MenuPane;
        app.footer_focused = true;

        // `l` / → would normally move focus to the main viewer
        crate::tui::handler::handle_key_sync(&mut app, key(KeyCode::Right));
        assert_eq!(app.footer_scroll.get(), 8, "the bar took the key");
        assert_eq!(app.active_panel, ActivePanel::MenuPane, "the pane did not");

        // once the bar lets go, the same key is the pane's again
        crate::tui::handler::handle_key_sync(&mut app, key(KeyCode::Esc));
        assert!(!app.footer_focused);
        crate::tui::handler::handle_key_sync(&mut app, key(KeyCode::Right));
        assert_eq!(app.footer_scroll.get(), 8, "the bar no longer moves");
        assert_eq!(app.active_panel, ActivePanel::MainViewer);
    }

    #[test]
    fn the_wheel_over_the_bar_scrolls_it_and_never_reaches_a_pane() {
        let mut app = app();
        assert!(handle_mouse(
            &mut app,
            mouse(MouseEventKind::ScrollDown, 10, 20)
        ));
        assert_eq!(app.footer_scroll.get(), 4);
        assert!(handle_mouse(
            &mut app,
            mouse(MouseEventKind::ScrollUp, 10, 20)
        ));
        assert_eq!(app.footer_scroll.get(), 0);

        assert!(
            !handle_mouse(&mut app, mouse(MouseEventKind::ScrollDown, 10, 5)),
            "the wheel over a pane is the pane's"
        );
        assert_eq!(app.footer_scroll.get(), 0);
    }
}
