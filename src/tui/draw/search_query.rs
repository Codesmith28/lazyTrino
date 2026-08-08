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
    layout::Rect,
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Paragraph},
};

use crate::app::{App, Mode, Screen};
use crate::tui::theme;

pub fn chunk_query_buffer(
    buf: &str,
    line0_cap: usize,
    inner_w: usize,
) -> Vec<(usize, usize, &str)> {
    if buf.is_empty() {
        return vec![(0, 0, "")];
    }
    let mut chunks = Vec::new();
    let mut start = 0;
    let mut cap = line0_cap.max(1);

    while start < buf.len() {
        let mut byte_count = 0;
        for (char_count, (idx, ch)) in buf[start..].char_indices().enumerate() {
            if char_count >= cap {
                break;
            }
            byte_count = idx + ch.len_utf8();
        }
        let end = start + byte_count;
        chunks.push((start, end, &buf[start..end]));
        start = end;
        cap = inner_w.max(1);
    }
    chunks
}

pub fn cursor_line_and_col(
    buf: &str,
    cursor: usize,
    line0_cap: usize,
    inner_w: usize,
    prefix_len: usize,
) -> (usize, usize) {
    let cursor = cursor.min(buf.len());
    let chunks = chunk_query_buffer(buf, line0_cap, inner_w);
    for (line_idx, (start, end, _)) in chunks.iter().enumerate() {
        if cursor >= *start && (cursor < *end || line_idx == chunks.len() - 1) {
            let char_offset = buf[*start..cursor.min(*end)].chars().count();
            let col = if line_idx == 0 {
                prefix_len + char_offset
            } else {
                char_offset
            };
            return (line_idx, col);
        }
    }
    (0, prefix_len)
}

pub fn render_search_bar(frame: &mut Frame, area: Rect, app: &App) {
    let is_editing = matches!(app.mode, Mode::Search);
    let title = if is_editing {
        " Centralized Search [EDITING - Press Enter/Esc to finish] "
    } else {
        " Centralized Search [Press / to search] "
    };

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(theme::border_style(is_editing));

    let inner_width = area.width.saturating_sub(2).max(1) as usize;
    let line0_cap = inner_width.saturating_sub(3);

    let (lines, cursor_line, cursor_col) = if app.search_query.is_empty() {
        let span = Span::styled(
            "Type to filter catalogs, schemas, tables, and columns...",
            theme::muted_style(),
        );
        (vec![Line::from(vec![Span::raw(" / "), span])], 0, 3)
    } else {
        let chunks = chunk_query_buffer(&app.search_query, line0_cap, inner_width);
        let mut lines = Vec::new();
        for (line_idx, (_, _, chunk_str)) in chunks.iter().enumerate() {
            if line_idx == 0 {
                lines.push(Line::from(vec![
                    Span::raw(" / "),
                    Span::styled(*chunk_str, theme::bold_text_style()),
                ]));
            } else {
                lines.push(Line::from(vec![Span::styled(
                    *chunk_str,
                    theme::bold_text_style(),
                )]));
            }
        }
        let (cline, ccol) = cursor_line_and_col(
            &app.search_query,
            app.search_query.len(),
            line0_cap,
            inner_width,
            3,
        );
        (lines, cline, ccol)
    };

    let visible_lines = area.height.saturating_sub(2).max(1) as usize;
    let mut scroll_y: u16 = 0;
    if is_editing && cursor_line >= visible_lines {
        scroll_y = (cursor_line - visible_lines + 1) as u16;
    }

    let p = Paragraph::new(lines).block(block).scroll((scroll_y, 0));
    frame.render_widget(p, area);

    if is_editing && cursor_line >= scroll_y as usize {
        let rel_line = cursor_line - scroll_y as usize;
        let cursor_x = area.x + 1 + (cursor_col as u16);
        let cursor_y = area.y + 1 + (rel_line as u16);
        if cursor_y < area.y + area.height - 1 && cursor_x < area.x + area.width - 1 {
            frame.set_cursor_position((cursor_x, cursor_y));
        }
    }
}

pub fn render_query_bar(frame: &mut Frame, area: Rect, app: &App) {
    let is_editing = matches!(app.mode, Mode::QueryInput);
    let is_table_view = matches!(
        &app.screen,
        Screen::Actions(state) if state.results.as_ref().is_some_and(|results| results.is_paginated)
    );

    let title = if is_editing {
        " Table Query Bar [EDITING - Press Enter to run, Esc to cancel] "
    } else if is_table_view {
        " Table Query Bar [Press 'q' or ':' to write query] "
    } else {
        " Table Query Bar [Disabled - Active only in full data table view] "
    };

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(theme::style(theme::query_bar_border_color(
            is_editing,
            is_table_view,
        )));

    let (buf, cursor, sel_range) = match &app.screen {
        Screen::Actions(state) => {
            if let Some(ref res) = state.results {
                (
                    res.query_buffer.as_str(),
                    res.query_cursor,
                    res.selection_range(),
                )
            } else {
                (state.query_buffer.as_str(), state.query_cursor, None)
            }
        }
        _ => ("", 0, None),
    };

    let inner_w = area.width.saturating_sub(2).max(1) as usize;
    let line0_cap = inner_w.saturating_sub(7);

    let (lines, cursor_line, cursor_col) = if buf.is_empty() {
        let span = Span::styled(
            "Write query (e.g. SELECT * FROM table)...",
            theme::muted_style(),
        );
        (
            vec![Line::from(vec![
                Span::styled(" SQL > ", theme::warning_style()),
                span,
            ])],
            0,
            7,
        )
    } else {
        let chunks = chunk_query_buffer(buf, line0_cap, inner_w);
        let mut lines = Vec::new();
        for (line_idx, (start, end, chunk_str)) in chunks.iter().enumerate() {
            let mut spans = Vec::new();
            if line_idx == 0 {
                spans.push(Span::styled(" SQL > ", theme::warning_style()));
            }
            if is_editing && let Some((sel_start, sel_end)) = sel_range {
                let s_start = sel_start.clamp(*start, *end);
                let s_end = sel_end.clamp(*start, *end);
                if s_start < s_end {
                    let before = &buf[*start..s_start];
                    let selected = &buf[s_start..s_end];
                    let after = &buf[s_end..*end];
                    if !before.is_empty() {
                        spans.push(Span::styled(before, theme::bold_text_style()));
                    }
                    spans.push(Span::styled(selected, theme::query_selection_style()));
                    if !after.is_empty() {
                        spans.push(Span::styled(after, theme::bold_text_style()));
                    }
                } else {
                    spans.push(Span::styled(*chunk_str, theme::bold_text_style()));
                }
            } else {
                spans.push(Span::styled(*chunk_str, theme::bold_text_style()));
            }
            lines.push(Line::from(spans));
        }
        let (cline, ccol) = cursor_line_and_col(buf, cursor, line0_cap, inner_w, 7);
        (lines, cline, ccol)
    };

    let visible_lines = area.height.saturating_sub(2).max(1) as usize;
    let mut scroll_y: u16 = 0;
    if is_editing && cursor_line >= visible_lines {
        scroll_y = (cursor_line - visible_lines + 1) as u16;
    }

    let p = Paragraph::new(lines).block(block).scroll((scroll_y, 0));
    frame.render_widget(p, area);

    if is_editing && cursor_line >= scroll_y as usize {
        let rel_line = cursor_line - scroll_y as usize;
        let cursor_x = area.x + 1 + (cursor_col as u16);
        let cursor_y = area.y + 1 + (rel_line as u16);
        if cursor_y < area.y + area.height - 1 && cursor_x < area.x + area.width - 1 {
            frame.set_cursor_position((cursor_x, cursor_y));
        }
    }
}
