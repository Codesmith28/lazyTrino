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
    style::Style,
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Paragraph},
};

use crate::{
    app::{App, QueryStatus},
    tui::theme,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LineKind {
    Sql,
    Error,
}

/// One rendered row of the inspector. The renderer and the mouse-selection
/// extractor both work from these, so what is highlighted is what is copied.
pub struct InspectorLine {
    /// Index into the newest-first log list.
    pub entry: usize,
    pub kind: LineKind,
    lead: Vec<(String, Style)>,
    text: String,
    text_style: Style,
}

/// All rows for the visible log entries (newest first, starting at the scroll
/// offset), with SQL and error text wrapped to `inner_w`.
pub fn lines(app: &App, inner_w: u16) -> Vec<InspectorLine> {
    let mut out = Vec::new();
    for (entry_idx, entry) in app
        .query_logs
        .iter()
        .rev()
        .enumerate()
        .skip(app.query_inspector_scroll)
    {
        let (symbol, style) = match &entry.status {
            QueryStatus::Success => ("✓", theme::success_bold_style()),
            QueryStatus::Error => ("✗", theme::error_bold_style()),
            QueryStatus::Running => ("○", theme::warning_bold_style()),
        };
        let meta = match &entry.status {
            QueryStatus::Success => {
                let dur = entry.duration_ms.unwrap_or(0);
                let rows = entry.row_count.unwrap_or(0);
                format!(" [{dur}ms, {rows}r] ")
            }
            QueryStatus::Error => " [error] ".to_string(),
            QueryStatus::Running => " [running] ".to_string(),
        };

        let header_len = 1 + symbol.chars().count() + meta.chars().count();
        let avail_w = (inner_w as usize).saturating_sub(header_len).max(10);
        let indent = " ".repeat(header_len);

        let wrapped_sql = crate::tui::screens::results::wrap_text(&entry.sql, avail_w);
        for (i, part) in wrapped_sql.into_iter().enumerate() {
            let lead = if i == 0 {
                vec![
                    (format!(" {symbol}"), style),
                    (meta.clone(), theme::muted_style()),
                ]
            } else {
                vec![(indent.clone(), Style::default())]
            };
            out.push(InspectorLine {
                entry: entry_idx,
                kind: LineKind::Sql,
                lead,
                text: part,
                text_style: theme::text_style(),
            });
        }

        if entry.status == QueryStatus::Error
            && let Some(msg) = entry.error_msg.as_deref().filter(|m| !m.trim().is_empty())
        {
            // The full error, wrapped under the query instead of squeezed
            // (and clipped) into the header.
            for part in crate::tui::screens::results::wrap_text(msg.trim(), avail_w) {
                out.push(InspectorLine {
                    entry: entry_idx,
                    kind: LineKind::Error,
                    lead: vec![(indent.clone(), Style::default())],
                    text: part,
                    text_style: theme::error_style(),
                });
            }
        }
    }
    out
}

/// Text for the inspector rows `first..=last` (relative to the first visible
/// row): the full SQL and/or full error message of every entry touched, each
/// once, regardless of how they wrap on screen.
pub fn selected_text(app: &App, inner_w: u16, first: usize, last: usize) -> String {
    let rows = lines(app, inner_w);
    let rev: Vec<_> = app.query_logs.iter().rev().collect();
    let mut seen: Vec<(usize, LineKind)> = Vec::new();
    for line in rows.iter().skip(first).take(last.saturating_sub(first) + 1) {
        if !seen.contains(&(line.entry, line.kind)) {
            seen.push((line.entry, line.kind));
        }
    }
    seen.iter()
        .filter_map(|(entry, kind)| {
            let e = rev.get(*entry)?;
            match kind {
                LineKind::Sql => Some(e.sql.clone()),
                LineKind::Error => e.error_msg.clone(),
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn render(frame: &mut Frame, area: Rect, app: &App) {
    let title = if let Some((ref msg, ref instant)) = app.copied_toast {
        if instant.elapsed().as_secs() < 2 {
            format!(
                " Executed Queries — Copied: \"{}\" ",
                crate::tui::draw::sanitize_toast_text(msg)
            )
        } else {
            " Executed Queries ".to_string()
        }
    } else {
        " Executed Queries ".to_string()
    };

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(theme::style(theme::inactive_border_color()));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    if app.query_logs.is_empty() {
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(
                " No queries executed yet",
                theme::muted_style(),
            ))),
            inner,
        );
        return;
    }

    let rows: Vec<Line> = lines(app, inner.width)
        .into_iter()
        .take(inner.height as usize)
        .enumerate()
        .map(|(i, line)| {
            let y = inner.y + i as u16;
            let selected = app.is_area_mouse_selected(inner.x, inner.width, y);
            let mut spans: Vec<Span> = line
                .lead
                .into_iter()
                .map(|(t, st)| Span::styled(t, st))
                .collect();
            spans.push(Span::styled(
                line.text,
                if selected {
                    theme::text_selection_style()
                } else {
                    line.text_style
                },
            ));
            Line::from(spans)
        })
        .collect();
    frame.render_widget(Paragraph::new(rows), inner);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::App;

    fn app_with_error() -> App {
        let mut app = App::new(crate::config::ConnectionConfig::default(), false);
        app.add_query_log("SELECT 1".to_string());
        let id = app.add_query_log("SELECT * FROM missing_table WHERE a = 1 AND b = 2".to_string());
        app.complete_query_log_error(
            id,
            "line 1:15: Table 'hive.default.missing_table' does not exist, check the name and retry"
                .to_string(),
        );
        app
    }

    #[test]
    fn errors_are_full_wrapped_rows_under_their_query() {
        let app = app_with_error();
        let rows = lines(&app, 40);
        // newest entry first: its SQL rows, then its error rows
        assert_eq!(rows[0].entry, 0);
        assert_eq!(rows[0].kind, LineKind::Sql);
        let error_rows: Vec<_> = rows.iter().filter(|r| r.kind == LineKind::Error).collect();
        assert!(error_rows.len() >= 2, "long error should wrap");
        assert!(error_rows.iter().all(|r| r.entry == 0));
        let text: String = error_rows
            .iter()
            .map(|r| r.text.as_str())
            .collect::<Vec<_>>()
            .join(" ");
        assert!(text.contains("does not exist"));
        // every row fits the width
        for r in &rows {
            let lead: usize = r.lead.iter().map(|(t, _)| t.chars().count()).sum();
            assert!(lead + r.text.chars().count() <= 40, "{:?}", r.text);
        }
    }

    #[test]
    fn selecting_error_rows_copies_the_whole_error_message_only() {
        let app = app_with_error();
        let rows = lines(&app, 40);
        let first_err = rows.iter().position(|r| r.kind == LineKind::Error).unwrap();
        let last_err = rows
            .iter()
            .rposition(|r| r.kind == LineKind::Error)
            .unwrap();
        assert_eq!(
            selected_text(&app, 40, first_err, last_err),
            "line 1:15: Table 'hive.default.missing_table' does not exist, check the name and retry"
        );
        // one wrapped row of the error is enough to get all of it
        assert!(selected_text(&app, 40, first_err, first_err).ends_with("retry"));
    }

    #[test]
    fn selecting_sql_and_error_together_copies_both_once_in_order() {
        let app = app_with_error();
        let all = lines(&app, 40).len();
        let text = selected_text(&app, 40, 0, all - 1);
        let parts: Vec<_> = text.split('\n').collect();
        assert_eq!(
            parts[0],
            "SELECT * FROM missing_table WHERE a = 1 AND b = 2"
        );
        assert!(parts[1].starts_with("line 1:15"));
        assert_eq!(parts[2], "SELECT 1");
        assert_eq!(parts.len(), 3);
    }

    #[test]
    fn rows_map_to_entries_even_when_sql_wraps() {
        // the old code assumed one screen row per entry
        let app = app_with_error();
        let rows = lines(&app, 20);
        let last = rows.len() - 1;
        assert_eq!(rows[last].entry, 1);
        assert_eq!(selected_text(&app, 20, last, last), "SELECT 1");
    }
}
