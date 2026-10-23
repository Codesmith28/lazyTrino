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

//! "What you see is what you copy": text of a screen rectangle taken from the
//! last rendered frame. This is the fallback for any view that has no
//! dedicated selection extractor (partition counts, DDL, banners, modals…).

use ratatui::buffer::Buffer;
use ratatui::text::Span;

/// Frame furniture that is never part of the content: box borders,
/// scrollbar track/thumb/arrows and the `▸` row cursor.
const CHROME: &[char] = &[
    '│', '┃', '║', '╭', '╮', '╰', '╯', '─', '━', '┌', '┐', '└', '┘', '▲', '▼', '█', '▸',
];

/// Text inside the rectangle spanned by two screen cells (inclusive).
pub fn extract(buf: &Buffer, anchor: (u16, u16), current: (u16, u16)) -> String {
    let area = buf.area;
    if area.width == 0 || area.height == 0 {
        return String::new();
    }
    let max_x = area.x + area.width - 1;
    let max_y = area.y + area.height - 1;
    let (x0, x1) = (
        anchor.0.min(current.0).max(area.x),
        anchor.0.max(current.0).min(max_x),
    );
    let (y0, y1) = (
        anchor.1.min(current.1).max(area.y),
        anchor.1.max(current.1).min(max_y),
    );
    if x0 > x1 || y0 > y1 {
        return String::new();
    }

    let lines: Vec<String> = (y0..=y1)
        .map(|y| clean_line(&row_text(buf, y, x0, x1)))
        .collect();

    let first = lines.iter().position(|l| !l.is_empty());
    let last = lines.iter().rposition(|l| !l.is_empty());
    match (first, last) {
        (Some(a), Some(b)) => lines[a..=b].join("\n"),
        _ => String::new(),
    }
}

fn row_text(buf: &Buffer, y: u16, x0: u16, x1: u16) -> String {
    let mut out = String::new();
    let mut x = x0;
    while x <= x1 {
        let Some(cell) = buf.cell((x, y)) else { break };
        let sym = cell.symbol();
        out.push_str(sym);
        // A wide glyph owns the following cell(s); don't emit their filler.
        let width = Span::raw(sym).width().max(1) as u16;
        x = x.saturating_add(width);
    }
    out
}

fn clean_line(raw: &str) -> String {
    let trimmed_end = raw.trim_end_matches(|c: char| c.is_whitespace() || CHROME.contains(&c));
    // Strip leading chrome and the padding after it, but keep real indentation.
    let mut start = 0;
    let mut stripped_chrome = false;
    for (i, c) in trimmed_end.char_indices() {
        if CHROME.contains(&c) {
            stripped_chrome = true;
            start = i + c.len_utf8();
        } else if c.is_whitespace() && stripped_chrome {
            start = i + c.len_utf8();
        } else {
            break;
        }
    }
    trimmed_end[start..].to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::layout::Rect;

    fn buf(lines: &[&str]) -> Buffer {
        Buffer::with_lines(lines.iter().copied())
    }

    #[test]
    fn test_strips_borders_cursor_and_keeps_count_beside_value() {
        let b = buf(&[
            "╭ Table View ────────────╮",
            "│▸ 2026-08-06  45,120 records│",
            "│  2026-08-07  counting...   │",
            "╰────────────────────────╯",
        ]);
        let text = extract(&b, (0, 0), (40, 3));
        assert_eq!(
            text,
            "Table View\n2026-08-06  45,120 records\n2026-08-07  counting..."
        );
    }

    #[test]
    fn test_keeps_leading_indentation_without_borders() {
        let b = buf(&["   CREATE TABLE t (", "      id bigint"]);
        assert_eq!(
            extract(&b, (0, 0), (30, 1)),
            "   CREATE TABLE t (\n      id bigint"
        );
    }

    #[test]
    fn test_selection_is_a_rectangle_and_order_independent() {
        let b = buf(&["abcdef", "ghijkl", "mnopqr"]);
        let a = extract(&b, (1, 0), (3, 1));
        assert_eq!(a, "bcd\nhij");
        assert_eq!(extract(&b, (3, 1), (1, 0)), a);
    }

    #[test]
    fn test_out_of_bounds_is_clamped_and_blank_is_empty() {
        let b = buf(&["ab", "  "]);
        assert_eq!(extract(&b, (0, 0), (500, 500)), "ab");
        assert_eq!(
            extract(&Buffer::empty(Rect::new(0, 0, 0, 0)), (0, 0), (1, 1)),
            ""
        );
        assert_eq!(extract(&b, (0, 1), (1, 1)), "");
    }

    #[test]
    fn test_wide_glyphs_do_not_gain_filler_spaces() {
        let b = buf(&["日本語 ok"]);
        assert_eq!(extract(&b, (0, 0), (20, 0)), "日本語 ok");
    }
}
