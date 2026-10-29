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

//! Vim-style editing for the SQL query panel.
//!
//! The panel holds a single logical line that is only *displayed* wrapped, so
//! `j`/`k` (and `↑`/`↓`) move between wrapped rows, like `gj`/`gk`, and "line"
//! operations (`dd`, `cc`, `yy`) act on the whole query.
//!
//! Supported: counts; motions `h l j k w W b B e E 0 ^ $ gg G f F t T ; ,`;
//! operators `d c y` with motions, doubled (`dd`) and text objects
//! (`iw aw i" a" i' a' i( a( i[ a[ i{ a{`); `x X D C s S r ~ p P u Ctrl+R`;
//! entering insert with `i a I A o O`; and character-wise visual mode (`v`).
//! Arrow keys work in every mode.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::app::{ResultsState, VimMode};

/// Width of the ` SQL > ` prefix on the first wrapped row.
const PREFIX: usize = 7;
const UNDO_LIMIT: usize = 100;

#[derive(Debug, Default, PartialEq, Eq)]
pub struct Outcome {
    /// Text yanked with `y` — the caller also puts it on the system clipboard.
    pub yanked: Option<String>,
    /// `Esc` in normal mode: leave the query panel.
    pub exit: bool,
}

/// Layout facts the editor needs from the renderer.
pub struct Ctx {
    /// Inner width of the query bar (wrapping width).
    pub inner_w: usize,
}

// ---------------------------------------------------------------- helpers --

fn char_idx(buf: &str, byte: usize) -> usize {
    let mut b = byte.min(buf.len());
    while b > 0 && !buf.is_char_boundary(b) {
        b -= 1;
    }
    buf[..b].chars().count()
}

fn byte_idx(chars: &[char], idx: usize) -> usize {
    chars[..idx.min(chars.len())]
        .iter()
        .map(|c| c.len_utf8())
        .sum()
}

fn load(state: &ResultsState) -> (Vec<char>, usize) {
    let chars: Vec<char> = state.query_buffer.chars().collect();
    let idx = char_idx(&state.query_buffer, state.query_cursor).min(chars.len());
    (chars, idx)
}

fn store(state: &mut ResultsState, chars: &[char], idx: usize) {
    state.query_buffer = chars.iter().collect();
    state.query_cursor = byte_idx(chars, idx);
    state.invalid_query_error = None;
}

fn set_cursor(state: &mut ResultsState, chars: &[char], idx: usize) {
    state.query_cursor = byte_idx(chars, idx);
}

fn last_idx(chars: &[char]) -> usize {
    chars.len().saturating_sub(1)
}

fn snapshot(state: &mut ResultsState) {
    state
        .vim
        .undo
        .push((state.query_buffer.clone(), state.query_cursor));
    if state.vim.undo.len() > UNDO_LIMIT {
        state.vim.undo.remove(0);
    }
    state.vim.redo.clear();
}

/// 0 = blank, 1 = word character (or any non-blank for WORD motions), 2 = punctuation.
fn class(c: char, big: bool) -> u8 {
    if c.is_whitespace() {
        0
    } else if big || c.is_alphanumeric() || c == '_' {
        1
    } else {
        2
    }
}

fn first_non_blank(chars: &[char]) -> usize {
    chars
        .iter()
        .position(|c| !c.is_whitespace())
        .unwrap_or(last_idx(chars))
}

// ------------------------------------------------------------ public API --

/// `Esc` from insert mode: back to normal mode with the cursor on the last
/// inserted character, like vim.
pub fn enter_normal(state: &mut ResultsState) {
    state.clear_selection();
    let (chars, idx) = load(state);
    state.vim.mode = VimMode::Normal;
    state.vim.clear_pending();
    state.vim.insert_dirty = false;
    let idx = idx.saturating_sub(1).min(last_idx(&chars));
    set_cursor(state, &chars, idx);
}

/// Call before the first edit of an insert session so `u` undoes the whole
/// session in one step.
pub fn begin_insert_edit(state: &mut ResultsState) {
    if !state.vim.insert_dirty {
        snapshot(state);
        state.vim.insert_dirty = true;
    }
}

/// Moves `delta` wrapped rows up (`-1`) or down (`1`), keeping the visual
/// column. Returns the new byte offset.
pub fn move_rows(buf: &str, cursor: usize, delta: isize, inner_w: usize) -> usize {
    let line0_cap = inner_w.saturating_sub(PREFIX);
    let chunks = crate::tui::draw::chunk_query_buffer(buf, line0_cap, inner_w);
    let cursor = cursor.min(buf.len());
    let cur = chunks
        .iter()
        .position(|(start, end, _)| cursor >= *start && cursor < *end)
        .unwrap_or(chunks.len() - 1);
    let new = (cur as isize + delta).clamp(0, chunks.len() as isize - 1) as usize;
    if new == cur {
        return if delta < 0 && cur == 0 {
            0
        } else if delta > 0 && cur == chunks.len() - 1 {
            buf.len()
        } else {
            cursor
        };
    }
    let (start, _, _) = chunks[cur];
    let col = buf[start..cursor].chars().count() + if cur == 0 { PREFIX } else { 0 };
    let (nstart, nend, nstr) = chunks[new];
    let off = col.saturating_sub(if new == 0 { PREFIX } else { 0 });
    let n_chars = nstr.chars().count();
    let max_off = if new == chunks.len() - 1 {
        n_chars
    } else {
        n_chars.saturating_sub(1)
    };
    let off = off.min(max_off);
    nstr.char_indices()
        .nth(off)
        .map(|(i, _)| nstart + i)
        .unwrap_or(nend)
}

/// Handles one key in `Normal` or `Visual` mode. `paste` supplies the system
/// clipboard for `p` when the register is empty.
pub fn handle_key(
    state: &mut ResultsState,
    key: KeyEvent,
    ctx: &Ctx,
    paste: &mut dyn FnMut() -> Option<String>,
) -> Outcome {
    let mut out = Outcome::default();
    let mods = key.modifiers;
    let mut code = key.code;
    // Option/Alt (also Ctrl) + arrows hop words; terminals that send
    // Option+arrow as ESC b / ESC f arrive as Alt+b / Alt+f. Cmd + arrows hop
    // to the ends of the query.
    if mods.contains(KeyModifiers::SUPER) {
        code = match code {
            KeyCode::Left | KeyCode::Up => KeyCode::Char('0'),
            KeyCode::Right | KeyCode::Down => KeyCode::Char('$'),
            _ => return out,
        };
    } else if mods.intersects(KeyModifiers::ALT | KeyModifiers::CONTROL) {
        code = match code {
            KeyCode::Left => KeyCode::Char('b'),
            KeyCode::Right => KeyCode::Char('w'),
            KeyCode::Char('b') | KeyCode::Char('B') if mods.contains(KeyModifiers::ALT) => {
                KeyCode::Char('b')
            }
            KeyCode::Char('f') | KeyCode::Char('F') if mods.contains(KeyModifiers::ALT) => {
                KeyCode::Char('w')
            }
            _ => return out,
        };
    }

    // Normal/visual mode keeps the cursor on a character, never past the end.
    let (chars, idx) = load(state);
    if idx > last_idx(&chars) {
        set_cursor(state, &chars, last_idx(&chars));
    }

    if let Some(prefix) = state.vim.pending.take() {
        resolve_pending(state, prefix, code, ctx, &mut out);
        return out;
    }

    if code == KeyCode::Esc {
        let v = &mut state.vim;
        if v.op.is_some() || v.count.is_some() || v.op_count.is_some() {
            v.clear_pending();
        } else if v.mode == VimMode::Visual {
            leave_visual(state);
        } else {
            state.clear_selection();
            out.exit = true;
        }
        return out;
    }

    let k = match code {
        KeyCode::Char(' ') => 'l',
        KeyCode::Char(c) => c,
        KeyCode::Left | KeyCode::Backspace => 'h',
        KeyCode::Right => 'l',
        KeyCode::Up => 'k',
        KeyCode::Down => 'j',
        KeyCode::Home => '0',
        KeyCode::End => '$',
        KeyCode::Delete => 'x',
        _ => return out,
    };

    if k.is_ascii_digit() {
        let has_count = if state.vim.op.is_some() {
            state.vim.op_count.is_some()
        } else {
            state.vim.count.is_some()
        };
        if k != '0' || has_count {
            let d = k.to_digit(10).unwrap() as usize;
            let slot = if state.vim.op.is_some() {
                &mut state.vim.op_count
            } else {
                &mut state.vim.count
            };
            *slot = Some(slot.unwrap_or(0).saturating_mul(10).saturating_add(d));
            return out;
        }
    }

    let visual = state.vim.mode == VimMode::Visual;
    let n = count(state);

    match k {
        'd' | 'c' | 'y' => {
            if visual {
                apply_visual(state, k, &mut out);
            } else {
                match state.vim.op {
                    None => state.vim.op = Some(k),
                    Some(o) if o == k => {
                        let len = chars.len();
                        apply_op(state, k, 0, len, &mut out);
                    }
                    Some(_) => state.vim.clear_pending(),
                }
            }
        }
        'g' | 'f' | 'F' | 't' | 'T' => state.vim.pending = Some(k),
        'r' => state.vim.pending = Some('r'),
        'i' | 'a' if visual || state.vim.op.is_some() => state.vim.pending = Some(k),
        'h' | 'l' | 'j' | 'k' | 'w' | 'W' | 'b' | 'B' | 'e' | 'E' | '0' | '^' | '$' | 'G' | ';'
        | ',' => motion_key(state, k, ctx, &mut out),
        _ if state.vim.op.is_some() => state.vim.clear_pending(),
        'v' | 'V' => {
            if visual {
                leave_visual(state);
            } else {
                state.vim.mode = VimMode::Visual;
                state.selection_anchor = Some(state.query_cursor);
            }
            state.vim.clear_pending();
        }
        'o' | 'O' if visual => {
            if let Some(anchor) = state.selection_anchor {
                state.selection_anchor = Some(state.query_cursor);
                state.query_cursor = anchor;
            }
        }
        'x' | 'X' | 'D' | 's' | 'S' | 'C' | 'Y' if visual => {
            let op = match k {
                'x' | 'X' | 'D' => 'd',
                'Y' => 'y',
                _ => 'c',
            };
            apply_visual(state, op, &mut out);
        }
        '~' if visual => {
            if let Some((start, end)) = visual_range(state) {
                snapshot(state);
                let (mut chars, _) = load(state);
                toggle_case(&mut chars[start..end]);
                store(state, &chars, start);
                leave_visual(state);
            }
        }
        _ if visual => state.vim.clear_pending(),
        'i' => insert_at(state, idx.min(chars.len())),
        'a' => insert_at(state, (idx + 1).min(chars.len())),
        'I' | 'O' => insert_at(state, first_non_blank(&chars).min(chars.len())),
        'A' | 'o' => insert_at(state, chars.len()),
        'x' => apply_op(state, 'd', idx, (idx + n).min(chars.len()), &mut out),
        'X' => apply_op(state, 'd', idx.saturating_sub(n), idx, &mut out),
        'D' => apply_op(state, 'd', idx, chars.len(), &mut out),
        'C' => apply_op(state, 'c', idx, chars.len(), &mut out),
        's' => apply_op(state, 'c', idx, (idx + n).min(chars.len()), &mut out),
        'S' => apply_op(state, 'c', 0, chars.len(), &mut out),
        'Y' => apply_op(state, 'y', 0, chars.len(), &mut out),
        '~' => {
            if !chars.is_empty() {
                snapshot(state);
                let mut chars = chars;
                let end = (idx + n).min(chars.len());
                toggle_case(&mut chars[idx..end]);
                let new_idx = end.min(last_idx(&chars));
                store(state, &chars, new_idx);
            }
        }
        'p' | 'P' => put(state, k == 'p', n, paste),
        'u' => undo(state),
        _ => {}
    }
    // Counts only survive while an operator or prefix is still waiting.
    if state.vim.op.is_none() && state.vim.pending.is_none() {
        state.vim.count = None;
        state.vim.op_count = None;
    }
    out
}

/// Redo (`Ctrl+R`).
pub fn redo(state: &mut ResultsState) {
    if let Some((buf, cur)) = state.vim.redo.pop() {
        state
            .vim
            .undo
            .push((state.query_buffer.clone(), state.query_cursor));
        state.query_buffer = buf;
        state.query_cursor = cur.min(state.query_buffer.len());
        let (chars, idx) = load(state);
        set_cursor(state, &chars, idx.min(last_idx(&chars)));
        leave_visual(state);
    }
}

fn undo(state: &mut ResultsState) {
    if let Some((buf, cur)) = state.vim.undo.pop() {
        state
            .vim
            .redo
            .push((state.query_buffer.clone(), state.query_cursor));
        state.query_buffer = buf;
        state.query_cursor = cur.min(state.query_buffer.len());
        let (chars, idx) = load(state);
        set_cursor(state, &chars, idx.min(last_idx(&chars)));
    }
}

fn count(state: &ResultsState) -> usize {
    state.vim.count.unwrap_or(1).max(1) * state.vim.op_count.unwrap_or(1).max(1)
}

fn insert_at(state: &mut ResultsState, idx: usize) {
    let (chars, _) = load(state);
    set_cursor(state, &chars, idx);
    state.vim.mode = VimMode::Insert;
    state.vim.insert_dirty = false;
    state.vim.clear_pending();
    state.clear_selection();
}

fn leave_visual(state: &mut ResultsState) {
    state.vim.mode = VimMode::Normal;
    state.vim.clear_pending();
    state.clear_selection();
}

fn toggle_case(chars: &mut [char]) {
    for c in chars {
        *c = if c.is_lowercase() {
            c.to_uppercase().next().unwrap_or(*c)
        } else {
            c.to_lowercase().next().unwrap_or(*c)
        };
    }
}

/// Inclusive visual range as `[start, end)` char indices.
fn visual_range(state: &ResultsState) -> Option<(usize, usize)> {
    let anchor = state.selection_anchor?;
    let (chars, cur) = load(state);
    let a = char_idx(&state.query_buffer, anchor);
    let (lo, hi) = (a.min(cur), a.max(cur));
    let end = (hi + 1).min(chars.len());
    (lo < end).then_some((lo, end))
}

fn apply_visual(state: &mut ResultsState, op: char, out: &mut Outcome) {
    let Some((start, end)) = visual_range(state) else {
        leave_visual(state);
        return;
    };
    apply_op(state, op, start, end, out);
}

/// Applies `op` (`d`, `c` or `y`) to the half-open char range `[start, end)`.
fn apply_op(state: &mut ResultsState, op: char, start: usize, end: usize, out: &mut Outcome) {
    let (mut chars, _) = load(state);
    let end = end.min(chars.len());
    let start = start.min(end);
    let was_visual = state.vim.mode == VimMode::Visual;
    state.vim.clear_pending();
    state.clear_selection();
    if was_visual {
        state.vim.mode = VimMode::Normal;
    }

    let text: String = chars[start..end].iter().collect();
    match op {
        'y' => {
            if !text.is_empty() {
                state.vim.register = text.clone();
                out.yanked = Some(text);
            }
            set_cursor(state, &chars, start.min(last_idx(&chars)));
        }
        'd' | 'c' => {
            if start < end {
                snapshot(state);
                state.vim.register = text;
                chars.drain(start..end);
                store(state, &chars, start);
            }
            if op == 'c' {
                insert_at(state, start.min(chars.len()));
                state.vim.insert_dirty = true;
            } else {
                set_cursor(state, &chars, start.min(last_idx(&chars)));
            }
        }
        _ => {}
    }
}

fn put(state: &mut ResultsState, after: bool, n: usize, paste: &mut dyn FnMut() -> Option<String>) {
    let text = if state.vim.register.is_empty() {
        paste().unwrap_or_default()
    } else {
        state.vim.register.clone()
    };
    if text.is_empty() {
        return;
    }
    let text = text.replace(['\n', '\r'], " ").repeat(n);
    let new: Vec<char> = text.chars().collect();
    let (mut chars, idx) = load(state);
    snapshot(state);
    let at = if after && !chars.is_empty() {
        (idx + 1).min(chars.len())
    } else {
        idx
    };
    let count = new.len();
    chars.splice(at..at, new);
    store(state, &chars, at + count - 1);
}

// ---------------------------------------------------------------- motions --

fn motion_key(state: &mut ResultsState, k: char, ctx: &Ctx, out: &mut Outcome) {
    let (chars, idx) = load(state);
    let n = count(state);
    let find = state.vim.last_find;
    let op = state.vim.op;

    let target = match k {
        ';' | ',' => find.and_then(|(kind, ch)| {
            let kind = if k == ',' { flip(kind) } else { kind };
            find_char(&chars, idx, kind, ch, n)
        }),
        'j' | 'k' if op.is_some() => None,
        _ => compute_motion(state, &chars, idx, k, n, ctx),
    };
    finish_motion(state, &chars, idx, target, out);
}

fn flip(kind: char) -> char {
    match kind {
        'f' => 'F',
        'F' => 'f',
        't' => 'T',
        _ => 't',
    }
}

/// Returns `(target char index, inclusive)`.
fn compute_motion(
    state: &ResultsState,
    chars: &[char],
    idx: usize,
    k: char,
    n: usize,
    ctx: &Ctx,
) -> Option<(usize, bool)> {
    let len = chars.len();
    // `cw` behaves like `ce` on a non-blank, as in vim.
    let k = if state.vim.op == Some('c')
        && matches!(k, 'w' | 'W')
        && chars.get(idx).is_some_and(|c| !c.is_whitespace())
    {
        if k == 'w' { 'e' } else { 'E' }
    } else {
        k
    };
    Some(match k {
        'h' => (idx.saturating_sub(n), false),
        'l' => ((idx + n).min(len), false),
        '0' => (0, false),
        '^' => (first_non_blank(chars), false),
        '$' => (last_idx(chars), true),
        'G' => (last_idx(chars), true),
        'w' | 'W' => {
            let mut i = idx;
            for _ in 0..n {
                i = next_word(chars, i, k == 'W');
            }
            (i, false)
        }
        'e' | 'E' => {
            let mut i = idx;
            for _ in 0..n {
                i = word_end(chars, i, k == 'E');
            }
            (i, true)
        }
        'b' | 'B' => {
            let mut i = idx;
            for _ in 0..n {
                i = prev_word(chars, i, k == 'B');
            }
            (i, false)
        }
        'j' | 'k' => {
            let delta = if k == 'j' { 1 } else { -1 } * n as isize;
            let byte = move_rows(
                &state.query_buffer,
                state.query_cursor,
                delta,
                ctx.inner_w.max(PREFIX + 1),
            );
            (char_idx(&state.query_buffer, byte), false)
        }
        _ => return None,
    })
}

/// Moves the cursor (or applies the pending operator) for a computed motion.
fn finish_motion(
    state: &mut ResultsState,
    chars: &[char],
    idx: usize,
    target: Option<(usize, bool)>,
    out: &mut Outcome,
) {
    let Some((to, inclusive)) = target else {
        state.vim.clear_pending();
        return;
    };
    if let Some(op) = state.vim.op {
        let (start, end) = if to >= idx {
            (idx, if inclusive { to + 1 } else { to })
        } else {
            (to, idx)
        };
        apply_op(state, op, start, end, out);
    } else {
        set_cursor(state, chars, to.min(last_idx(chars)));
        state.vim.count = None;
        state.vim.op_count = None;
    }
}

fn next_word(chars: &[char], idx: usize, big: bool) -> usize {
    let len = chars.len();
    let mut i = idx;
    if i >= len {
        return len;
    }
    let cls = class(chars[i], big);
    if cls != 0 {
        while i < len && class(chars[i], big) == cls {
            i += 1;
        }
    }
    while i < len && class(chars[i], big) == 0 {
        i += 1;
    }
    i
}

fn word_end(chars: &[char], idx: usize, big: bool) -> usize {
    let len = chars.len();
    if len == 0 {
        return 0;
    }
    let mut i = idx + 1;
    while i < len && class(chars[i], big) == 0 {
        i += 1;
    }
    if i >= len {
        return last_idx(chars);
    }
    let cls = class(chars[i], big);
    while i + 1 < len && class(chars[i + 1], big) == cls {
        i += 1;
    }
    i
}

fn prev_word(chars: &[char], idx: usize, big: bool) -> usize {
    let mut i = idx;
    while i > 0 && class(chars[i - 1], big) == 0 {
        i -= 1;
    }
    if i == 0 {
        return 0;
    }
    let cls = class(chars[i - 1], big);
    while i > 0 && class(chars[i - 1], big) == cls {
        i -= 1;
    }
    i
}

fn find_char(chars: &[char], idx: usize, kind: char, ch: char, n: usize) -> Option<(usize, bool)> {
    let len = chars.len();
    match kind {
        'f' | 't' => {
            let mut seen = 0;
            for (i, &c) in chars.iter().enumerate().take(len).skip(idx + 1) {
                if c == ch {
                    seen += 1;
                    if seen == n {
                        return if kind == 'f' {
                            Some((i, true))
                        } else if i > idx + 1 {
                            Some((i - 1, true))
                        } else {
                            None
                        };
                    }
                }
            }
            None
        }
        _ => {
            let mut seen = 0;
            for i in (0..idx).rev() {
                if chars[i] == ch {
                    seen += 1;
                    if seen == n {
                        return if kind == 'F' {
                            Some((i, false))
                        } else if i + 1 < idx {
                            Some((i + 1, false))
                        } else {
                            None
                        };
                    }
                }
            }
            None
        }
    }
}

fn resolve_pending(
    state: &mut ResultsState,
    prefix: char,
    code: KeyCode,
    ctx: &Ctx,
    out: &mut Outcome,
) {
    let KeyCode::Char(ch) = code else {
        state.vim.clear_pending();
        return;
    };
    let (chars, idx) = load(state);
    let n = count(state);
    match prefix {
        'g' => match ch {
            'g' => finish_motion(state, &chars, idx, Some((0, false)), out),
            'j' | 'k' => {
                let target = compute_motion(state, &chars, idx, ch, n, ctx);
                finish_motion(state, &chars, idx, target, out);
            }
            _ => state.vim.clear_pending(),
        },
        'f' | 'F' | 't' | 'T' => {
            state.vim.last_find = Some((prefix, ch));
            let target = find_char(&chars, idx, prefix, ch, n);
            finish_motion(state, &chars, idx, target, out);
        }
        'r' => {
            let (start, end) = if state.vim.mode == VimMode::Visual {
                visual_range(state).unwrap_or((idx, idx))
            } else {
                (idx, idx + n)
            };
            if end <= chars.len() && start < end {
                snapshot(state);
                let mut chars = chars;
                chars[start..end].fill(ch);
                let cursor = if state.vim.mode == VimMode::Visual {
                    start
                } else {
                    end - 1
                };
                store(state, &chars, cursor);
            }
            if state.vim.mode == VimMode::Visual {
                leave_visual(state);
            }
            state.vim.clear_pending();
        }
        'i' | 'a' => {
            let Some((start, end)) = text_object(&chars, idx, prefix, ch) else {
                state.vim.clear_pending();
                return;
            };
            if state.vim.mode == VimMode::Visual {
                state.selection_anchor = Some(byte_idx(&chars, start));
                set_cursor(state, &chars, end.saturating_sub(1).max(start));
                state.vim.clear_pending();
            } else if let Some(op) = state.vim.op {
                apply_op(state, op, start, end, out);
            }
        }
        _ => state.vim.clear_pending(),
    }
}

// ----------------------------------------------------------- text objects --

fn text_object(chars: &[char], idx: usize, kind: char, ch: char) -> Option<(usize, usize)> {
    let len = chars.len();
    if idx >= len {
        return None;
    }
    let inner = kind == 'i';
    match ch {
        'w' | 'W' => {
            let big = ch == 'W';
            let cls = class(chars[idx], big);
            let mut s = idx;
            while s > 0 && class(chars[s - 1], big) == cls {
                s -= 1;
            }
            let mut e = idx + 1;
            while e < len && class(chars[e], big) == cls {
                e += 1;
            }
            if inner {
                return Some((s, e));
            }
            if cls != 0 {
                let mut e2 = e;
                while e2 < len && class(chars[e2], big) == 0 {
                    e2 += 1;
                }
                if e2 == e {
                    while s > 0 && class(chars[s - 1], big) == 0 {
                        s -= 1;
                    }
                }
                Some((s, e2))
            } else {
                Some((s, e))
            }
        }
        '"' | '\'' | '`' => {
            let quotes: Vec<usize> = (0..len).filter(|&i| chars[i] == ch).collect();
            let pair = quotes
                .as_chunks::<2>()
                .0
                .iter()
                .map(|p| (p[0], p[1]))
                .find(|(a, b)| *a <= idx && idx <= *b)
                .or_else(|| {
                    quotes
                        .as_chunks::<2>()
                        .0
                        .iter()
                        .map(|p| (p[0], p[1]))
                        .find(|(a, _)| *a > idx)
                })?;
            Some(if inner {
                (pair.0 + 1, pair.1)
            } else {
                (pair.0, pair.1 + 1)
            })
        }
        '(' | ')' | 'b' | '[' | ']' | '{' | '}' | 'B' | '<' | '>' => {
            let (open, close) = match ch {
                '(' | ')' | 'b' => ('(', ')'),
                '[' | ']' => ('[', ']'),
                '{' | '}' | 'B' => ('{', '}'),
                _ => ('<', '>'),
            };
            let mut depth = 0usize;
            let mut o = None;
            for i in (0..=idx).rev() {
                let c = chars[i];
                if c == close && i != idx {
                    depth += 1;
                } else if c == open {
                    if depth == 0 {
                        o = Some(i);
                        break;
                    }
                    depth -= 1;
                }
            }
            let o = o?;
            let mut depth = 0usize;
            let mut c_idx = None;
            for (j, &c) in chars.iter().enumerate().skip(o + 1) {
                if c == open {
                    depth += 1;
                } else if c == close {
                    if depth == 0 {
                        c_idx = Some(j);
                        break;
                    }
                    depth -= 1;
                }
            }
            let c_idx = c_idx?;
            Some(if inner {
                (o + 1, c_idx)
            } else {
                (o, c_idx + 1)
            })
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn st(buf: &str, cursor_char: usize) -> ResultsState {
        let mut s = ResultsState {
            query_buffer: buf.to_string(),
            ..Default::default()
        };
        let chars: Vec<char> = buf.chars().collect();
        s.query_cursor = byte_idx(&chars, cursor_char);
        s.vim.mode = VimMode::Normal;
        s
    }

    fn ctx() -> Ctx {
        Ctx { inner_w: 40 }
    }

    /// Feeds characters as key presses; `<Esc>`-style names via the helpers below.
    fn feed(s: &mut ResultsState, keys: &str) -> Outcome {
        let mut last = Outcome::default();
        for c in keys.chars() {
            last = press(s, KeyCode::Char(c));
        }
        last
    }

    /// Simulates typing in insert mode (what the panel's insert handler does).
    fn type_text(s: &mut ResultsState, text: &str) {
        assert_eq!(s.vim.mode, VimMode::Insert);
        for c in text.chars() {
            begin_insert_edit(s);
            s.query_buffer.insert(s.query_cursor, c);
            s.query_cursor += c.len_utf8();
        }
    }

    fn press(s: &mut ResultsState, code: KeyCode) -> Outcome {
        press_mod(s, code, KeyModifiers::NONE)
    }

    fn press_mod(s: &mut ResultsState, code: KeyCode, m: KeyModifiers) -> Outcome {
        handle_key(s, KeyEvent::new(code, m), &ctx(), &mut || None)
    }

    fn cursor(s: &ResultsState) -> usize {
        char_idx(&s.query_buffer, s.query_cursor)
    }

    #[test]
    fn word_motions_follow_vim() {
        let mut s = st("SELECT a.b, c FROM t", 0);
        feed(&mut s, "w");
        assert_eq!(cursor(&s), 7); // a
        feed(&mut s, "w");
        assert_eq!(cursor(&s), 8); // .
        feed(&mut s, "e");
        assert_eq!(cursor(&s), 9); // b ... (end of "b"? `.` run is 1 char, e jumps to b)
        feed(&mut s, "b");
        assert_eq!(cursor(&s), 8);
        feed(&mut s, "W");
        assert_eq!(cursor(&s), 12); // c
        feed(&mut s, "$");
        assert_eq!(cursor(&s), 19);
        feed(&mut s, "0");
        assert_eq!(cursor(&s), 0);
        feed(&mut s, "3w");
        assert_eq!(cursor(&s), 9);
    }

    #[test]
    fn arrows_and_vim_keys_agree_in_normal_mode() {
        let mut a = st("hello world", 4);
        let mut b = st("hello world", 4);
        press(&mut a, KeyCode::Left);
        feed(&mut b, "h");
        assert_eq!(a.query_cursor, b.query_cursor);
        press(&mut a, KeyCode::Right);
        feed(&mut b, "l");
        assert_eq!(a.query_cursor, b.query_cursor);
        press(&mut a, KeyCode::End);
        assert_eq!(cursor(&a), 10);
        press(&mut a, KeyCode::Home);
        assert_eq!(cursor(&a), 0);
        // never past the last character
        press(&mut a, KeyCode::End);
        press(&mut a, KeyCode::Right);
        assert_eq!(cursor(&a), 10);
        // alt+arrows are word jumps
        press(&mut a, KeyCode::Home);
        press_mod(&mut a, KeyCode::Right, KeyModifiers::ALT);
        assert_eq!(cursor(&a), 6);
        press_mod(&mut a, KeyCode::Left, KeyModifiers::ALT);
        assert_eq!(cursor(&a), 0);
        // Option+arrow sent as ESC f / ESC b by Terminal.app / iTerm2
        press_mod(&mut a, KeyCode::Char('f'), KeyModifiers::ALT);
        assert_eq!(cursor(&a), 6);
        press_mod(&mut a, KeyCode::Char('b'), KeyModifiers::ALT);
        assert_eq!(cursor(&a), 0);
        // Ctrl+arrows too; Cmd+arrows jump to the ends of the query
        press_mod(&mut a, KeyCode::Right, KeyModifiers::CONTROL);
        assert_eq!(cursor(&a), 6);
        press_mod(&mut a, KeyCode::Right, KeyModifiers::SUPER);
        assert_eq!(cursor(&a), 10);
        press_mod(&mut a, KeyCode::Left, KeyModifiers::SUPER);
        assert_eq!(cursor(&a), 0);
    }

    #[test]
    fn delete_change_yank_with_motions() {
        let mut s = st("SELECT * FROM orders", 0);
        feed(&mut s, "dw");
        assert_eq!(s.query_buffer, "* FROM orders");
        feed(&mut s, "wd$");
        assert_eq!(s.query_buffer, "* ");
        assert_eq!(s.vim.register, "FROM orders");

        let mut s = st("SELECT * FROM orders", 0);
        feed(&mut s, "cw");
        assert_eq!(s.vim.mode, VimMode::Insert);
        type_text(&mut s, "WITH");
        assert_eq!(s.query_buffer, "WITH * FROM orders");

        let mut s = st("abc def", 0);
        let out = feed(&mut s, "yw");
        assert_eq!(out.yanked.as_deref(), Some("abc "));
        assert_eq!(s.query_buffer, "abc def");
        feed(&mut s, "wP");
        assert_eq!(s.query_buffer, "abc abc def");
    }

    #[test]
    fn doubled_operators_and_counts_act_on_the_whole_query() {
        let mut s = st("SELECT 1", 3);
        feed(&mut s, "dd");
        assert_eq!(s.query_buffer, "");
        feed(&mut s, "u");
        assert_eq!(s.query_buffer, "SELECT 1");

        let mut s = st("a b c d e", 0);
        feed(&mut s, "d2w");
        assert_eq!(s.query_buffer, "c d e");
        let mut s = st("a b c d e", 0);
        feed(&mut s, "2dw");
        assert_eq!(s.query_buffer, "c d e");
        let mut s = st("abcdef", 1);
        feed(&mut s, "3x");
        assert_eq!(s.query_buffer, "aef");
    }

    #[test]
    fn single_key_edits() {
        let mut s = st("abc", 1);
        feed(&mut s, "x");
        assert_eq!(s.query_buffer, "ac");
        feed(&mut s, "rZ");
        assert_eq!(s.query_buffer, "aZ");
        feed(&mut s, "~");
        assert_eq!(s.query_buffer, "az");

        let mut s = st("abc def", 5);
        feed(&mut s, "D");
        assert_eq!(s.query_buffer, "abc d");
        feed(&mut s, "X");
        assert_eq!(s.query_buffer, "abcd");

        let mut s = st("abc", 0);
        feed(&mut s, "C");
        assert_eq!(s.vim.mode, VimMode::Insert);
        type_text(&mut s, "xyz");
        assert_eq!(s.query_buffer, "xyz");
    }

    #[test]
    fn insert_commands_place_the_cursor_like_vim() {
        let mut s = st("  abc", 3);
        feed(&mut s, "I");
        assert_eq!((cursor(&s), s.vim.mode), (2, VimMode::Insert));
        let mut s = st("abc", 0);
        feed(&mut s, "a");
        assert_eq!((cursor(&s), s.vim.mode), (1, VimMode::Insert));
        let mut s = st("abc", 0);
        feed(&mut s, "A");
        assert_eq!((cursor(&s), s.vim.mode), (3, VimMode::Insert));
        // Esc from insert lands on the last typed character
        enter_normal(&mut s);
        assert_eq!((cursor(&s), s.vim.mode), (2, VimMode::Normal));
    }

    #[test]
    fn find_and_repeat() {
        let mut s = st("a,b,c,d", 0);
        feed(&mut s, "f,");
        assert_eq!(cursor(&s), 1);
        feed(&mut s, ";");
        assert_eq!(cursor(&s), 3);
        feed(&mut s, ",");
        assert_eq!(cursor(&s), 1);
        let mut s = st("a,b,c,d", 0);
        feed(&mut s, "df,");
        assert_eq!(s.query_buffer, "b,c,d");
        feed(&mut s, "$F,");
        assert_eq!(cursor(&s), 3);
    }

    #[test]
    fn text_objects() {
        let mut s = st("SELECT name FROM t WHERE x = 'a b'", 8);
        feed(&mut s, "diw");
        assert_eq!(s.query_buffer, "SELECT  FROM t WHERE x = 'a b'");

        let mut s = st("WHERE x = 'a b'", 12);
        feed(&mut s, "ci'");
        type_text(&mut s, "z");
        assert_eq!(s.query_buffer, "WHERE x = 'z'");

        let mut s = st("count(a, (b))", 7);
        feed(&mut s, "di(");
        assert_eq!(s.query_buffer, "count()");

        let mut s = st("foo bar baz", 4);
        feed(&mut s, "daw");
        assert_eq!(s.query_buffer, "foo baz");
    }

    #[test]
    fn visual_mode_is_inclusive_and_operates_on_the_selection() {
        let mut s = st("SELECT id FROM t", 0);
        feed(&mut s, "v");
        assert_eq!(s.vim.mode, VimMode::Visual);
        feed(&mut s, "e");
        assert_eq!(s.selection_range(), Some((0, 6)));
        let out = feed(&mut s, "y");
        assert_eq!(out.yanked.as_deref(), Some("SELECT"));
        assert_eq!(s.vim.mode, VimMode::Normal);
        assert!(s.selection_range().is_none());

        let mut s = st("SELECT id FROM t", 7);
        feed(&mut s, "ved");
        assert_eq!(s.query_buffer, "SELECT  FROM t");

        // a lone v selects the character under the cursor
        let mut s = st("abc", 1);
        feed(&mut s, "v");
        assert_eq!(s.selection_range(), Some((1, 2)));
        // o swaps ends, viw selects the word, Esc leaves
        let mut s = st("foo bar", 5);
        feed(&mut s, "viw");
        assert_eq!(s.selection_range(), Some((4, 7)));
        let out = press(&mut s, KeyCode::Esc);
        assert!(!out.exit);
        assert_eq!(s.vim.mode, VimMode::Normal);
        assert!(s.selection_range().is_none());
    }

    #[test]
    fn undo_and_redo_restore_the_query() {
        let mut s = st("SELECT 1", 0);
        feed(&mut s, "dw");
        feed(&mut s, "x");
        assert_eq!(s.query_buffer, "");
        feed(&mut s, "u");
        assert_eq!(s.query_buffer, "1");
        feed(&mut s, "u");
        assert_eq!(s.query_buffer, "SELECT 1");
        redo(&mut s);
        assert_eq!(s.query_buffer, "1");
        redo(&mut s);
        assert_eq!(s.query_buffer, "");
    }

    #[test]
    fn insert_session_is_one_undo_step() {
        let mut s = st("abc", 0);
        feed(&mut s, "A");
        for c in "xyz".chars() {
            begin_insert_edit(&mut s);
            s.query_buffer.push(c);
            s.query_cursor += 1;
        }
        enter_normal(&mut s);
        feed(&mut s, "u");
        assert_eq!(s.query_buffer, "abc");
    }

    #[test]
    fn esc_cancels_pending_then_leaves_the_panel() {
        let mut s = st("abc", 0);
        feed(&mut s, "d");
        assert!(!press(&mut s, KeyCode::Esc).exit);
        assert!(s.vim.op.is_none());
        feed(&mut s, "2");
        assert!(!press(&mut s, KeyCode::Esc).exit);
        assert!(press(&mut s, KeyCode::Esc).exit);
    }

    #[test]
    fn put_uses_register_and_clamps() {
        let mut s = st("ab", 1);
        s.vim.register = "XY".into();
        feed(&mut s, "p");
        assert_eq!(s.query_buffer, "abXY");
        assert_eq!(cursor(&s), 3);
        // empty register falls back to the clipboard supplier
        let mut s = st("ab", 0);
        let mut supplier = || Some("Q".to_string());
        handle_key(
            &mut s,
            KeyEvent::from(KeyCode::Char('p')),
            &ctx(),
            &mut supplier,
        );
        assert_eq!(s.query_buffer, "aQb");
    }

    #[test]
    fn row_motions_walk_the_wrapped_display_and_keep_the_column() {
        // inner_w 20 -> first row holds 13 chars, later rows 20
        let buf = "SELECT * FROM iceberg.demo.lineitem_partitioned WHERE x = 1";
        let down = move_rows(buf, 2, 1, 20);
        // visual column 9 (offset 2 + 7 prefix) on row 0 is offset 9 on row 1, which starts at 13
        assert_eq!(down, 13 + 9);
        let up = move_rows(buf, down, -1, 20);
        assert_eq!(up, 2);
        // moving past the ends stays put / clamps
        assert_eq!(move_rows(buf, 0, -1, 20), 0);
        assert_eq!(move_rows(buf, buf.len(), 1, 20), buf.len());
    }

    #[test]
    fn j_and_k_in_normal_mode_use_rows() {
        let buf = "SELECT * FROM iceberg.demo.lineitem_partitioned WHERE x = 1";
        let mut s = st(buf, 0);
        press(&mut s, KeyCode::Down);
        assert!(cursor(&s) >= 13);
        feed(&mut s, "k");
        assert_eq!(cursor(&s), 0);
        // operators ignore j/k instead of mangling the query
        feed(&mut s, "dj");
        assert_eq!(s.query_buffer, buf);
    }

    #[test]
    fn multibyte_text_is_edited_by_character() {
        let mut s = st("héllo wörld", 0);
        feed(&mut s, "wx");
        assert_eq!(s.query_buffer, "héllo örld");
        feed(&mut s, "0lx");
        assert_eq!(s.query_buffer, "hllo örld");
        feed(&mut s, "$");
        assert_eq!(cursor(&s), 8);
    }

    #[test]
    fn unknown_keys_and_modified_keys_do_nothing() {
        let mut s = st("abc", 1);
        feed(&mut s, "Z");
        assert_eq!(s.query_buffer, "abc");
        press_mod(&mut s, KeyCode::Char('x'), KeyModifiers::CONTROL);
        assert_eq!(s.query_buffer, "abc");
        // an unknown key after an operator cancels it
        feed(&mut s, "dZ");
        assert!(s.vim.op.is_none());
        assert_eq!(s.query_buffer, "abc");
    }
}
