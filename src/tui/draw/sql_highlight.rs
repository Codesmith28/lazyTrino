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

//! Lightweight SQL syntax highlighting for the query bar. It only decides
//! *colors*; the text itself is never altered, so cursor math is unaffected.

use ratatui::{style::Style, text::Span};

use crate::tui::theme;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Class {
    Plain,
    Keyword,
    Str,
    Number,
    Comment,
    QuotedIdent,
}

impl Class {
    fn style(self) -> Style {
        match self {
            Class::Plain => theme::bold_text_style(),
            Class::Keyword => theme::sql_keyword_style(),
            Class::Str => theme::sql_string_style(),
            Class::Number => theme::number_bold_style(),
            Class::Comment => theme::sql_comment_style(),
            Class::QuotedIdent => theme::sql_ident_style(),
        }
    }
}

const KEYWORDS: &[&str] = &[
    "SELECT",
    "FROM",
    "WHERE",
    "AND",
    "OR",
    "NOT",
    "IN",
    "IS",
    "NULL",
    "LIKE",
    "BETWEEN",
    "AS",
    "ON",
    "JOIN",
    "LEFT",
    "RIGHT",
    "INNER",
    "OUTER",
    "FULL",
    "CROSS",
    "USING",
    "GROUP",
    "BY",
    "ORDER",
    "HAVING",
    "LIMIT",
    "OFFSET",
    "DISTINCT",
    "UNION",
    "ALL",
    "EXCEPT",
    "INTERSECT",
    "WITH",
    "CASE",
    "WHEN",
    "THEN",
    "ELSE",
    "END",
    "ASC",
    "DESC",
    "CAST",
    "TRUE",
    "FALSE",
    "EXISTS",
    "OVER",
    "PARTITION",
    "SHOW",
    "DESCRIBE",
    "EXPLAIN",
    "TABLE",
    "TABLES",
    "SCHEMAS",
    "CATALOGS",
    "COLUMNS",
    "VALUES",
    "INSERT",
    "INTO",
    "CREATE",
    "DROP",
    "ALTER",
    "DELETE",
    "UPDATE",
    "SET",
    "TIMESTAMP",
    "DATE",
    "INTERVAL",
    "ANY",
    "SOME",
    "UNNEST",
    "TABLESAMPLE",
    "FETCH",
    "FIRST",
    "NEXT",
    "ROWS",
    "ONLY",
];

fn is_keyword(word: &str) -> bool {
    KEYWORDS.iter().any(|k| k.eq_ignore_ascii_case(word))
}

/// Classifies every byte of `sql`; `out[i]` is the class of byte `i`.
fn classify(sql: &str) -> Vec<Class> {
    let bytes = sql.as_bytes();
    let mut out = vec![Class::Plain; bytes.len()];
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        let start = i;
        let class = if b == b'-' && bytes.get(i + 1) == Some(&b'-') {
            while i < bytes.len() && bytes[i] != b'\n' {
                i += 1;
            }
            Class::Comment
        } else if b == b'/' && bytes.get(i + 1) == Some(&b'*') {
            i += 2;
            while i < bytes.len() && !(bytes[i] == b'*' && bytes.get(i + 1) == Some(&b'/')) {
                i += 1;
            }
            i = (i + 2).min(bytes.len());
            Class::Comment
        } else if b == b'\'' || b == b'"' {
            i += 1;
            while i < bytes.len() {
                if bytes[i] == b {
                    // a doubled quote is an escaped quote
                    if bytes.get(i + 1) == Some(&b) {
                        i += 2;
                        continue;
                    }
                    i += 1;
                    break;
                }
                i += 1;
            }
            if b == b'\'' {
                Class::Str
            } else {
                Class::QuotedIdent
            }
        } else if b.is_ascii_digit() {
            while i < bytes.len() && (bytes[i].is_ascii_digit() || bytes[i] == b'.') {
                i += 1;
            }
            Class::Number
        } else if b.is_ascii_alphabetic() || b == b'_' {
            while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
                i += 1;
            }
            if is_keyword(&sql[start..i]) {
                Class::Keyword
            } else {
                Class::Plain
            }
        } else {
            i += 1;
            Class::Plain
        };
        for slot in &mut out[start..i] {
            *slot = class;
        }
    }
    out
}

/// Spans for `buf[start..end]`, colored by SQL token. `selection` (byte range
/// in `buf`) is painted with the text-selection style instead.
pub fn spans<'a>(
    buf: &'a str,
    start: usize,
    end: usize,
    selection: Option<(usize, usize)>,
) -> Vec<Span<'a>> {
    let classes = classify(buf);
    let (sel_start, sel_end) = selection.unwrap_or((0, 0));
    let sel_of = |i: usize| selection.is_some() && i >= sel_start && i < sel_end;

    let mut spans = Vec::new();
    let mut seg_start = start;
    let mut cur = (classes.get(start).copied(), sel_of(start));
    for i in start..end {
        let key = (classes.get(i).copied(), sel_of(i));
        if key != cur && buf.is_char_boundary(i) {
            push(&mut spans, buf, seg_start, i, cur);
            seg_start = i;
            cur = key;
        }
    }
    push(&mut spans, buf, seg_start, end, cur);
    spans
}

fn push<'a>(
    spans: &mut Vec<Span<'a>>,
    buf: &'a str,
    from: usize,
    to: usize,
    (class, selected): (Option<Class>, bool),
) {
    if from >= to {
        return;
    }
    let style = if selected {
        theme::query_selection_style()
    } else {
        class.unwrap_or(Class::Plain).style()
    };
    spans.push(Span::styled(&buf[from..to], style));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(spans: &[Span]) -> String {
        spans.iter().map(|s| s.content.as_ref()).collect()
    }

    #[test]
    fn colors_tokens_without_changing_the_text() {
        let sql = "SELECT id, 'a''b', 42 FROM t -- tail\nWHERE x = \"c\"";
        let s = spans(sql, 0, sql.len(), None);
        assert_eq!(text(&s), sql);
        let style_of = |needle: &str| {
            s.iter()
                .find(|sp| sp.content.contains(needle))
                .unwrap_or_else(|| panic!("no span with {needle}"))
                .style
        };
        assert_eq!(style_of("SELECT"), Class::Keyword.style());
        assert_eq!(style_of("'a''b'"), Class::Str.style());
        assert_eq!(style_of("42"), Class::Number.style());
        assert_eq!(style_of("-- tail"), Class::Comment.style());
        assert_eq!(style_of("\"c\""), Class::QuotedIdent.style());
        assert_eq!(style_of(" id"), Class::Plain.style());
    }

    #[test]
    fn identifiers_containing_keywords_are_not_keywords() {
        let sql = "selected_at from_x";
        let s = spans(sql, 0, sql.len(), None);
        assert!(s.iter().all(|sp| sp.style == Class::Plain.style()));
    }

    #[test]
    fn chunks_and_selection_are_respected() {
        let sql = "SELECT * FROM orders";
        // second chunk only, with "FROM" selected
        let s = spans(sql, 7, sql.len(), Some((9, 13)));
        assert_eq!(text(&s), "* FROM orders");
        assert!(
            s.iter()
                .any(|sp| sp.content == "FROM" && sp.style == theme::query_selection_style())
        );
        // multibyte text never splits mid-character
        let sql = "SELECT 'héllo' FROM t";
        let s = spans(sql, 0, sql.len(), Some((9, 10)));
        assert_eq!(text(&s), sql);
    }
}
