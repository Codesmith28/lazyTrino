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

//! Shared "delete backwards" shortcuts for the single-line text inputs
//! (connect form, `/` search, modal searches, save-view name, numeric filter).
//!
//! Terminals disagree about how a platform's native chord arrives, so every
//! spelling is accepted:
//!
//! | Platform / terminal            | Delete word                   | Delete to line start |
//! |--------------------------------|-------------------------------|----------------------|
//! | macOS                          | Opt+Backspace (`Alt`)         | Cmd+Backspace (`Super`) |
//! | Windows / Linux                | Ctrl+Backspace                | Ctrl+U               |
//! | Terminals that send raw bytes  | Ctrl+W, Ctrl+H (`^W`, `^H`)   | Ctrl+U (`^U`)        |

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// How much text a backspace-style key removes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Deletion {
    Char,
    Word,
    Line,
}

/// Classify `key` as a delete-backwards shortcut, or `None` if it is not one.
/// A plain Backspace is `Deletion::Char`.
pub fn deletion(key: &KeyEvent) -> Option<Deletion> {
    let mods = key.modifiers;
    let ctrl = mods.contains(KeyModifiers::CONTROL);
    match key.code {
        KeyCode::Backspace if mods.contains(KeyModifiers::SUPER) => Some(Deletion::Line),
        KeyCode::Backspace if mods.intersects(KeyModifiers::ALT | KeyModifiers::CONTROL) => {
            Some(Deletion::Word)
        }
        KeyCode::Backspace => Some(Deletion::Char),
        // Ctrl+Backspace arrives as ^H on many Linux terminals.
        KeyCode::Char('w' | 'W' | 'h' | 'H') if ctrl => Some(Deletion::Word),
        KeyCode::Char('u' | 'U') if ctrl => Some(Deletion::Line),
        _ => None,
    }
}

/// Start of the word ending at the end of `s`: trailing whitespace is skipped,
/// then the run of non-whitespace before it.
fn word_start(s: &str) -> usize {
    let trimmed = s.trim_end();
    trimmed
        .char_indices()
        .rev()
        .find(|(_, c)| c.is_whitespace())
        .map(|(i, c)| i + c.len_utf8())
        .unwrap_or(0)
}

/// Remove text from the end of `s` according to `kind`.
pub fn delete_back(s: &mut String, kind: Deletion) {
    match kind {
        Deletion::Char => {
            s.pop();
        }
        Deletion::Word => s.truncate(word_start(s)),
        Deletion::Line => s.clear(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(code: KeyCode, mods: KeyModifiers) -> KeyEvent {
        KeyEvent::new(code, mods)
    }

    #[test]
    fn classifies_every_platform_spelling() {
        let bs = KeyCode::Backspace;
        assert_eq!(deletion(&key(bs, KeyModifiers::NONE)), Some(Deletion::Char));
        assert_eq!(deletion(&key(bs, KeyModifiers::ALT)), Some(Deletion::Word));
        assert_eq!(
            deletion(&key(bs, KeyModifiers::CONTROL)),
            Some(Deletion::Word)
        );
        assert_eq!(
            deletion(&key(bs, KeyModifiers::SUPER)),
            Some(Deletion::Line)
        );
        let ctrl = KeyModifiers::CONTROL;
        assert_eq!(
            deletion(&key(KeyCode::Char('w'), ctrl)),
            Some(Deletion::Word)
        );
        assert_eq!(
            deletion(&key(KeyCode::Char('h'), ctrl)),
            Some(Deletion::Word)
        );
        assert_eq!(
            deletion(&key(KeyCode::Char('u'), ctrl)),
            Some(Deletion::Line)
        );
        assert_eq!(deletion(&key(KeyCode::Char('w'), KeyModifiers::NONE)), None);
        assert_eq!(deletion(&key(KeyCode::Char('x'), ctrl)), None);
    }

    #[test]
    fn deletes_char_word_and_line() {
        let mut s = String::from("select * from  orders ");
        delete_back(&mut s, Deletion::Word);
        assert_eq!(s, "select * from  ");
        delete_back(&mut s, Deletion::Word);
        assert_eq!(s, "select * ");
        delete_back(&mut s, Deletion::Char);
        assert_eq!(s, "select *");
        delete_back(&mut s, Deletion::Line);
        assert_eq!(s, "");
        // empty / whitespace-only input is safe
        delete_back(&mut s, Deletion::Word);
        delete_back(&mut s, Deletion::Char);
        assert_eq!(s, "");
        let mut s = String::from("   ");
        delete_back(&mut s, Deletion::Word);
        assert_eq!(s, "");
    }

    #[test]
    fn word_delete_is_unicode_safe() {
        let mut s = String::from("café münchen");
        delete_back(&mut s, Deletion::Word);
        assert_eq!(s, "café ");
        delete_back(&mut s, Deletion::Word);
        assert_eq!(s, "");
    }
}
