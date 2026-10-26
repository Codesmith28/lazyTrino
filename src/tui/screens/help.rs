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

use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Paragraph, ScrollbarOrientation, ScrollbarState},
};

use crate::tui::theme;

struct HelpSection {
    title: &'static str,
    items: &'static [&'static str],
}

const HELP_SECTIONS: &[HelpSection] = &[
    HelpSection {
        title: "Active Pane Concept & Layout",
        items: &[
            "  All Tables View               Catalogs / Schemas / Tables list (full width)",
            "  Inside Table View             Left: Menu Pane (15%) | Right: Query & Main Preview (85%)",
            "  ?                             Open this help (search with /, scroll with j/k)",
            "  Shift+H/L / Shift+←/→ / Tab / Click  Switch focus between Menu Pane (left) and Preview Pane (right)",
            "  { / }  or mouse wheel on it     Scroll the Executed Queries pane (long queries and errors)",
            "  j/k, PgUp/PgDn, wheel in a tall row  Scroll inside a row taller than the pane (e.g. Table DDL)",
            "                                 Long error messages scroll the same way",
            "  h/l, ←/→ or Shift+wheel (Partitions)  Scroll the tree sideways when a line is wider than the pane",
            "  Option Selection              Selecting a menu option shifts focus to Preview Pane while option stays highlighted",
        ],
    },
    HelpSection {
        title: "Navigation & Scrolling",
        items: &[
            "  j / ↓ / Mouse Wheel Down       Move menu selection down / move cell cursor down / scroll pane",
            "  k / ↑ / Mouse Wheel Up         Move menu selection up / move cell cursor up / scroll pane",
            "  PgDn / Ctrl+d / Ctrl+f         Scroll page down",
            "  PgUp / Ctrl+u / Ctrl+b         Scroll page up",
            "  h / ← / [ / ,                  Move cell cursor left / scroll columns left",
            "  l / → / ] / .                  Move cell cursor right / scroll columns right",
            "  H / Ctrl+← / Alt+←             Jump 5 columns left",
            "  L / Ctrl+→ / Alt+→             Jump 5 columns right",
            "  Home / 0                       Jump to first column / beginning of list",
            "  End / $                        Jump to last column / end of list",
            "  Mouse Wheel / Drag             Wheel on scrollbar/header or drag scrollbar to scroll columns",
            "  g / G                          Jump to top / bottom of active preview pane",
            "  C                              Open column visibility modal (filter which columns to see in view)",
            "  F / f                          Open filter modal for the current column",
            "  A / a                          Open applied filters modal (view/delete active partition & column filters)",
            "  S                              Toggle column sorting (desc -> asc -> none)",
            "  y                              Copy selection (or the whole list / loaded grid as CSV when nothing is selected)",
            "  u                              Clear / undo active selection (visual mode or mouse drag)",
            "  Esc                            Clear selection first; with nothing selected: focus Menu Pane / Go back",
            "  Enter                          Select menu option & focus Preview Pane",
        ],
    },
    HelpSection {
        title: "Selection, Copy & Export",
        items: &[
            "  Mouse drag                     Select anything on screen; copied on release and kept highlighted",
            "                                 (in your terminal's own colors) until you click, scroll, type or press Esc",
            "  Shift+↑ / ↓ (/ ← →)            Start / extend a selection (in the grid: cell level, one cell at a time)",
            "  V                              Visual line: select whole rows (lists, partition values, grid)",
            "  v / Ctrl+V                     Visual / block: select a range (grid: cell block)",
            "  y                              Copy the selection (and leave visual mode)",
            "                                 Grid selections (cells, rows, drags) are copied as CSV with the column header",
            "  Esc                            Deselect (only when a selection exists; otherwise its usual meaning)",
            "  Cmd+C / Ctrl+Shift+C           Copy selection, else the focused cell / row (needs a terminal",
            "                                 that forwards the key; otherwise use y or drag-select)",
            "  Shift+drag / Option+drag       Use the terminal's own selection and its native copy shortcut",
            "  Ctrl+E                         Copy or export the ENTIRE view: pick clipboard or CSV file",
            "                                 (~/Downloads/lazytrino/<name>_<timestamp>.csv)",
            "                                 Large views default to CSV export (clipboard copy can freeze the UI)",
            "  Partition rows                 Copied exactly as shown, with the record count beside the value",
            "  Partitions tree / Schema       Drag to select; Ctrl+E for the whole view (no row cursor to use v / V)",
        ],
    },
    HelpSection {
        title: "Table View Options & Hotkeys",
        items: &[
            "  v                              Table View Mode (Infinite Scroll grid)",
            "  c                              Table DDL",
            "  e                              Schema (Vertical Columns Inspector)",
            "  t / m                          Show Stats / Sample (20 rows)",
            "  p                              Partitions Tree",
            "  Ctrl+s                         Save active table view (prompt for view name)",
            "  Ctrl+o / B                     Browse and restore saved table views (Back / Esc / < work from there)",
            "  Space / a / d (in Saved Views) Toggle selection / select all / delete selected views",
        ],
    },
    HelpSection {
        title: "Partitioned Table Drill-Down (cd/ls style)",
        items: &[
            "  (auto)                         Entering a table runs recon (SHOW CREATE TABLE, etc.) up front;",
            "                                 Partitions/Schema/Table View wait for it to finish",
            "  v on a partitioned table       Table View browses partition values one level at a time",
            "                                 instead of querying every row at once (avoids split errors)",
            "  > / Enter / →                  Drill into the highlighted partition value (go one level deeper)",
            "  < / ←                          Go up one partition level (no-op at the top level)",
            "                                 In the leaf record grid, < still goes up a level;",
            "                                 h / j / k / l navigates cells across rows and columns",
            "  Esc                            Clears a selection if there is one, otherwise returns to the Menu Pane",
            "                                 (regardless of drill-down depth)",
        ],
    },
    HelpSection {
        title: "Search & Custom Query",
        items: &[
            "  /                              Search / filter current view or modal",
            "  q or :                         Write custom SQL in Query Bar (in Table View)",
            "  Ctrl+C                         Quit lazyTrino",
        ],
    },
    HelpSection {
        title: "Query Bar (vim-style)",
        items: &[
            "  (opens in INSERT)              Type SQL; Enter runs it; Esc switches to NORMAL mode",
            "  ← → ↑ ↓ / Home / End           Move (↑ ↓ walk wrapped rows) — in every mode",
            "  Opt/Alt+← →, Ctrl+← →          Word hop (also Opt+b / Opt+f); Cmd+← → jump to start / end",
            "  Opt+Backspace, Cmd+Backspace   Delete word / delete to the start (insert mode)",
            "  NORMAL: h j k l w b e W B E    Motions; 0 ^ $ line ends; gg G; f/F/t/T + char, ; ,",
            "  NORMAL: d c y + motion         Delete / change / yank; dd cc yy act on the whole query",
            "  NORMAL: iw aw i\" a\" i' a' i( a(  Text objects, e.g. ci' or di(",
            "  NORMAL: x X D C s S r ~        Quick edits; counts work (3x, d2w)",
            "  NORMAL: i a I A / p P / u Ctrl+R  Enter insert / paste / undo / redo",
            "  NORMAL: v                      Visual select (then motions, y d c, o to swap ends)",
            "  NORMAL: Esc                    Leave the query bar (first cancels a pending d/c/y or count)",
        ],
    },
];

pub fn build_help_lines(search_query: &str) -> Vec<Line<'static>> {
    let query_lower = search_query.trim().to_lowercase();
    let mut help_text: Vec<Line<'static>> = Vec::new();

    if query_lower.is_empty() {
        for (i, section) in HELP_SECTIONS.iter().enumerate() {
            if i > 0 {
                help_text.push(Line::from(""));
            }
            help_text.push(Line::from(Span::styled(
                format!(" {}", section.title),
                theme::header_style(),
            )));
            for item in section.items {
                help_text.push(Line::from(*item));
            }
        }
    } else {
        let mut found_any = false;
        for section in HELP_SECTIONS {
            let section_title_matches = section.title.to_lowercase().contains(&query_lower);
            let matching_items: Vec<&str> = section
                .items
                .iter()
                .copied()
                .filter(|item| section_title_matches || item.to_lowercase().contains(&query_lower))
                .collect();

            if !matching_items.is_empty() {
                if found_any {
                    help_text.push(Line::from(""));
                }
                found_any = true;
                help_text.push(Line::from(Span::styled(
                    format!(" {}", section.title),
                    theme::header_style(),
                )));
                for item in matching_items {
                    help_text.push(Line::from(item));
                }
            }
        }

        if !found_any {
            help_text.push(Line::from(Span::styled(
                format!("  No matching shortcuts or help entries for \"{search_query}\""),
                theme::muted_style(),
            )));
        }
    }

    help_text
}

/// Width the help text is wrapped to inside the help modal for a terminal of
/// size `area`: the modal's inner width minus the scrollbar column.
pub fn content_width(area: Rect) -> usize {
    wrap_width(
        crate::tui::screens::modal::modal_large(area)
            .width
            .saturating_sub(2),
    )
}

fn wrap_width(inner_width: u16) -> usize {
    (inner_width as usize).saturating_sub(1).max(10)
}

/// Help lines already wrapped for the modal at terminal size `area`, so
/// scrolling math (which counts rows) matches what is drawn.
pub fn wrapped_help_lines(search_query: &str, area: Rect) -> Vec<Line<'static>> {
    wrap_help_lines(build_help_lines(search_query), content_width(area))
}

/// Word-wraps help lines to `width` columns. Continuation rows are indented
/// to the item's description column ("  key      description" keeps its
/// hanging indent), so wrapped entries stay readable.
pub fn wrap_help_lines(lines: Vec<Line<'static>>, width: usize) -> Vec<Line<'static>> {
    let mut out = Vec::with_capacity(lines.len());
    for line in lines {
        if line.width() <= width {
            out.push(line);
            continue;
        }
        let style = line.spans.first().map(|s| s.style).unwrap_or_default();
        let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
        for row in wrap_text(&text, width) {
            out.push(Line::from(Span::styled(row, style)));
        }
    }
    out
}

fn text_width(s: &str) -> usize {
    Span::raw(s).width()
}

/// Column where an item's description starts: after the key column's run of
/// 2+ spaces, or the leading indent for continuation / plain lines.
fn hanging_indent(text: &str, width: usize) -> usize {
    let chars: Vec<char> = text.chars().collect();
    let lead = chars.iter().take_while(|c| **c == ' ').count();
    let mut indent = lead;
    if lead < 8 {
        let mut i = lead;
        while i + 1 < chars.len() {
            if chars[i] == ' ' && chars[i + 1] == ' ' {
                let mut j = i;
                while j < chars.len() && chars[j] == ' ' {
                    j += 1;
                }
                if j < chars.len() {
                    indent = j;
                }
                break;
            }
            i += 1;
        }
    }
    // Never let the indent starve the text of room.
    if indent + 12 > width {
        lead.min(width / 4)
    } else {
        indent
    }
}

fn wrap_text(text: &str, width: usize) -> Vec<String> {
    let indent = hanging_indent(text, width);
    let chars: Vec<char> = text.chars().collect();
    let head: String = chars.iter().take(indent).collect();
    let body: String = chars.iter().skip(indent).collect();
    let room = width.saturating_sub(indent).max(1);

    let mut rows: Vec<String> = Vec::new();
    let mut cur = String::new();
    for word in body.split_whitespace() {
        let mut word = word.to_string();
        // hard-split words wider than a whole row
        while text_width(&word) > room {
            if !cur.is_empty() {
                rows.push(std::mem::take(&mut cur));
            }
            let mut piece = String::new();
            let mut rest = String::new();
            for ch in word.chars() {
                if rest.is_empty() && text_width(&piece) + text_width(&ch.to_string()) <= room {
                    piece.push(ch);
                } else {
                    rest.push(ch);
                }
            }
            if piece.is_empty() {
                break;
            }
            rows.push(piece);
            word = rest;
        }
        if cur.is_empty() {
            cur = word;
        } else if text_width(&cur) + 1 + text_width(&word) <= room {
            cur.push(' ');
            cur.push_str(&word);
        } else {
            rows.push(std::mem::take(&mut cur));
            cur = word;
        }
    }
    if !cur.is_empty() || rows.is_empty() {
        rows.push(cur);
    }

    rows.into_iter()
        .enumerate()
        .map(|(i, row)| {
            if i == 0 {
                format!("{head}{row}")
            } else {
                format!("{}{row}", " ".repeat(indent))
            }
        })
        .collect()
}

pub fn render_modal(
    frame: &mut Frame,
    area: Rect,
    search_query: &str,
    is_searching: bool,
    scroll: usize,
) {
    let popup_area = crate::tui::screens::modal::modal_large(area);
    let inner = crate::tui::screens::modal::render_modal_frame(
        frame,
        popup_area,
        " Help & Keybindings ",
        true,
    );

    if inner.height < 4 {
        return;
    }

    let chunks = Layout::vertical([
        Constraint::Length(3), // Dedicated search box
        Constraint::Min(0),    // Help content
        Constraint::Length(1), // Hint footer
    ])
    .split(inner);

    let search_area = chunks[0];
    let content_area = chunks[1];
    let hint_area = chunks[2];

    let search_title = if is_searching {
        " Search [/] [Typing — Enter/Esc to finish] "
    } else {
        " Search [/] "
    };
    let search_block = Block::default()
        .title(search_title)
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(theme::border_style(is_searching));
    let s_inner = search_block.inner(search_area);
    frame.render_widget(search_block, search_area);

    let search_line = if search_query.is_empty() {
        if is_searching {
            Line::from(vec![
                Span::styled("/ ", theme::warning_bold_style()),
                Span::styled("█", theme::warning_bold_style()),
            ])
        } else {
            Line::from(vec![
                Span::styled("/ ", theme::muted_style()),
                Span::styled("Type / to search shortcuts...", theme::muted_style()),
            ])
        }
    } else {
        let mut spans = vec![
            Span::styled("/ ", theme::warning_bold_style()),
            Span::styled(search_query, theme::text_style()),
        ];
        if is_searching {
            spans.push(Span::styled("█", theme::warning_bold_style()));
        }
        Line::from(spans)
    };
    frame.render_widget(Paragraph::new(search_line), s_inner);

    let help_lines = wrap_help_lines(
        build_help_lines(search_query),
        wrap_width(content_area.width),
    );
    let total_lines = help_lines.len();
    let visible_height = content_area.height as usize;
    let max_scroll = total_lines.saturating_sub(visible_height);
    let effective_scroll = scroll.min(max_scroll);

    let paragraph = Paragraph::new(help_lines).scroll((effective_scroll as u16, 0));
    frame.render_widget(paragraph, content_area);

    if total_lines > visible_height && visible_height > 0 {
        let mut scrollbar_state = ScrollbarState::new(max_scroll).position(effective_scroll);
        frame.render_stateful_widget(
            crate::tui::theme::scrollbar()
                .orientation(ScrollbarOrientation::VerticalRight)
                .begin_symbol(Some("▲"))
                .end_symbol(Some("▼")),
            content_area,
            &mut scrollbar_state,
        );
    }

    let hint_line = if is_searching {
        Line::from(vec![
            Span::styled("Type", theme::selection_style()),
            Span::styled(":filter  ", theme::muted_style()),
            Span::styled("Enter/Esc", theme::selection_style()),
            Span::styled(":done searching", theme::muted_style()),
        ])
    } else {
        Line::from(vec![
            Span::styled("/", theme::selection_style()),
            Span::styled(":search  ", theme::muted_style()),
            Span::styled("j/k", theme::selection_style()),
            Span::styled(":scroll  ", theme::muted_style()),
            Span::styled("Esc/?/q", theme::selection_style()),
            Span::styled(":close help", theme::muted_style()),
        ])
    };
    frame.render_widget(Paragraph::new(hint_line), hint_area);
}

#[allow(dead_code)]
pub fn render(frame: &mut Frame, area: Rect, search_query: &str, is_searching: bool) {
    render_modal(frame, area, search_query, is_searching, 0);
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{Terminal, backend::TestBackend};

    #[test]
    fn test_help_render_empty_search() {
        let backend = TestBackend::new(100, 40);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| {
                render(f, f.area(), "", false);
            })
            .unwrap();
        let buf = terminal.backend().buffer().clone();
        let mut title_found = false;
        for y in 0..40 {
            for x in 0..80 {
                if buf.cell((x, y)).unwrap().symbol() == "H"
                    && buf.cell((x + 1, y)).unwrap().symbol() == "e"
                    && buf.cell((x + 2, y)).unwrap().symbol() == "l"
                    && buf.cell((x + 3, y)).unwrap().symbol() == "p"
                {
                    title_found = true;
                    break;
                }
            }
        }
        assert!(title_found);
    }

    #[test]
    fn test_help_render_with_filter() {
        let backend = TestBackend::new(100, 40);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| {
                render(f, f.area(), "filter", true);
            })
            .unwrap();
    }

    fn plain(lines: &[Line]) -> Vec<String> {
        lines
            .iter()
            .map(|l| l.spans.iter().map(|s| s.content.as_ref()).collect())
            .collect()
    }

    #[test]
    fn wrapped_rows_never_exceed_width_and_lose_no_characters() {
        for width in [20usize, 30, 45, 60, 100] {
            let original = build_help_lines("");
            let wrapped = wrap_help_lines(original.clone(), width);
            for row in plain(&wrapped) {
                assert!(
                    Span::raw(row.as_str()).width() <= width,
                    "width {width}: row too wide: {row:?}"
                );
            }
            // Overlong tokens may be hard-split, so compare characters, not words.
            let chars = |ls: &[Line]| -> String {
                plain(ls)
                    .concat()
                    .chars()
                    .filter(|c| !c.is_whitespace())
                    .collect()
            };
            assert_eq!(
                chars(&original),
                chars(&wrapped),
                "width {width}: text changed"
            );
        }
    }

    #[test]
    fn wrapped_description_hangs_under_the_description_column() {
        let line = Line::from(
            "  Ctrl+E                         Copy or export the ENTIRE view: pick clipboard or CSV file",
        );
        let rows = plain(&wrap_help_lines(vec![line], 60));
        assert!(rows.len() >= 2, "{rows:?}");
        assert!(rows[0].starts_with("  Ctrl+E"));
        let indent = rows[0].find("Copy").unwrap();
        for cont in &rows[1..] {
            assert!(cont.starts_with(&" ".repeat(indent)), "{cont:?}");
            assert!(!cont[indent..].starts_with(' '), "{cont:?}");
        }
    }

    #[test]
    fn short_lines_and_blank_lines_are_untouched() {
        let lines = vec![Line::from(""), Line::from("  j / ↓   Move down")];
        assert_eq!(plain(&wrap_help_lines(lines.clone(), 80)), plain(&lines));
    }

    #[test]
    fn overlong_words_are_hard_split_instead_of_dropped() {
        let long = format!("  key   {}", "x".repeat(50));
        let rows = plain(&wrap_help_lines(vec![Line::from(long)], 24));
        assert!(
            rows.iter().all(|r| Span::raw(r.as_str()).width() <= 24),
            "{rows:?}"
        );
        assert_eq!(rows.join("").matches('x').count(), 50);
    }

    #[test]
    fn modal_on_a_narrow_terminal_shows_wrapped_text_not_cut_off_text() {
        let backend = TestBackend::new(60, 40);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| render_modal(f, f.area(), "", false, 0))
            .unwrap();
        let buf = terminal.backend().buffer().clone();
        let out: String = (0..40)
            .map(|y| {
                (0..60)
                    .map(|x| buf.cell((x, y)).unwrap().symbol())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n");
        // This long entry only fits on one row when truncated; wrapped, its tail
        // ("pane") must still be visible somewhere in the first screenful.
        assert!(out.contains("Switch focus"), "{out}");
        assert!(out.contains("Menu Pane"), "{out}");
    }

    #[test]
    fn pagedown_and_mouse_math_agree_with_what_is_drawn() {
        let area = Rect::new(0, 0, 70, 30);
        let drawn = wrap_help_lines(build_help_lines(""), content_width(area));
        assert_eq!(drawn.len(), wrapped_help_lines("", area).len());
        assert!(
            drawn.len() > build_help_lines("").len(),
            "narrow area should wrap"
        );
    }
}
