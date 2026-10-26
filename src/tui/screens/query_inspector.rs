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

/// Width SQL / error text is wrapped to for a pane of inner width `inner_w`:
/// one column is left for the scrollbar.
pub fn wrap_width(inner_w: u16) -> u16 {
    inner_w.saturating_sub(1)
}

/// Every row of the log (newest entry first), with SQL and error text
/// wrapped to `width`. The pane shows a window of these, starting at
/// `query_inspector_scroll`.
pub fn lines(app: &App, width: u16) -> Vec<InspectorLine> {
    let inner_w = width;
    let mut out = Vec::new();
    for (entry_idx, entry) in app.query_logs.iter().rev().enumerate() {
        let (symbol, style) = match &entry.status {
            QueryStatus::Success => ("✓", theme::success_bold_style()),
            QueryStatus::Error => ("✗", theme::error_bold_style()),
            QueryStatus::Running => ("○", theme::warning_bold_style()),
        };
        let cached = entry.cached && entry.status != QueryStatus::Error;
        let meta = match &entry.status {
            QueryStatus::Success if cached => {
                format!(" [cached, {}r] ", entry.row_count.unwrap_or(0))
            }
            QueryStatus::Success => {
                let dur = entry.duration_ms.unwrap_or(0);
                let rows = entry.row_count.unwrap_or(0);
                format!(" [{dur}ms, {rows}r] ")
            }
            QueryStatus::Error => " [error] ".to_string(),
            QueryStatus::Running if cached => " [cached] ".to_string(),
            QueryStatus::Running => " [running] ".to_string(),
        };
        // A cache hit is called out in the info color so it stands apart
        // from queries that actually went to Trino.
        let meta_style = if cached {
            theme::info_bold_style()
        } else {
            theme::muted_style()
        };

        let header_len = 1 + symbol.chars().count() + meta.chars().count();
        let avail_w = (inner_w as usize).saturating_sub(header_len).max(10);
        let indent = " ".repeat(header_len);

        let wrapped_sql = crate::tui::screens::results::wrap_text(&entry.sql, avail_w);
        for (i, part) in wrapped_sql.into_iter().enumerate() {
            let lead = if i == 0 {
                vec![(format!(" {symbol}"), style), (meta.clone(), meta_style)]
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
pub fn selected_text(app: &App, width: u16, first: usize, last: usize) -> String {
    let rows = lines(app, width);
    let first = first + app.query_inspector_scroll;
    let last = last + app.query_inspector_scroll;
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

/// The largest scroll offset for the pane as last drawn.
pub fn max_scroll(app: &App) -> usize {
    let rect = app.inspector_rect.get();
    let inner_h = rect.height.saturating_sub(2) as usize;
    lines(app, wrap_width(rect.width.saturating_sub(2)))
        .len()
        .saturating_sub(inner_h)
}

/// Scrolls the pane by `delta` lines, clamped to its content.
pub fn scroll_by(app: &mut App, delta: isize) {
    let max = max_scroll(app);
    let cur = app.query_inspector_scroll.min(max) as isize;
    app.query_inspector_scroll = (cur + delta).clamp(0, max as isize) as usize;
}

/// Whether a terminal cell is inside the pane as last drawn.
pub fn contains(app: &App, column: u16, row: u16) -> bool {
    let r = app.inspector_rect.get();
    r.width > 0 && column >= r.x && column < r.right() && row >= r.y && row < r.bottom()
}

pub fn render(frame: &mut Frame, area: Rect, app: &App) {
    app.inspector_rect.set(area);
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

    let all = lines(app, wrap_width(inner.width));
    let total = all.len();
    let height = inner.height as usize;
    let scroll = app.query_inspector_scroll.min(total.saturating_sub(height));
    let rows: Vec<Line> = all
        .into_iter()
        .skip(scroll)
        .take(height)
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
    if total > height {
        let mut bar = ratatui::widgets::ScrollbarState::new(total - height).position(scroll);
        frame.render_stateful_widget(
            theme::scrollbar()
                .orientation(ratatui::widgets::ScrollbarOrientation::VerticalRight)
                .begin_symbol(Some("▲"))
                .end_symbol(Some("▼")),
            inner,
            &mut bar,
        );
    }
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

    fn first_row_text(app: &App) -> String {
        let row = &lines(app, 80)[0];
        let lead: String = row.lead.iter().map(|(t, _)| t.as_str()).collect();
        format!("{lead}{}", row.text)
    }

    #[test]
    fn cache_hits_are_called_out_in_the_log() {
        let mut app = App::new(crate::config::ConnectionConfig::default(), false);
        let id = app.add_query_log_with_cache("SELECT 1".to_string(), true);
        // flagged as soon as it is logged, before the (instant) result arrives
        assert!(first_row_text(&app).contains("[cached] SELECT 1"));
        app.complete_query_log_success(id, 0, 7);
        assert!(app.query_logs[0].cached, "completing keeps the flag");
        assert!(first_row_text(&app).contains("[cached, 7r] SELECT 1"));

        // a real query shows its duration, not "cached"
        let id = app.add_query_log_with_cache("SELECT 2".to_string(), false);
        app.complete_query_log_success(id, 120, 3);
        assert!(first_row_text(&app).contains("[120ms, 3r] SELECT 2"));

        // the badge is styled differently from an ordinary query's
        let rows = lines(&app, 80);
        let cached_row = rows.iter().find(|r| r.text == "SELECT 1").unwrap();
        let real_row = rows.iter().find(|r| r.text == "SELECT 2").unwrap();
        assert_ne!(cached_row.lead[1].1, real_row.lead[1].1);
    }

    #[test]
    fn a_failed_query_is_never_labelled_cached() {
        let mut app = App::new(crate::config::ConnectionConfig::default(), false);
        let id = app.add_query_log_with_cache("SELECT bad".to_string(), true);
        app.complete_query_log_error(id, "boom".to_string());
        let text = first_row_text(&app);
        assert!(
            text.contains("[error]") && !text.contains("cached"),
            "{text}"
        );
    }

    fn busy_app(entries: usize) -> App {
        let mut app = App::new(crate::config::ConnectionConfig::default(), false);
        for i in 0..entries {
            let id = app.add_query_log(format!("SELECT {i}"));
            app.complete_query_log_success(id, 5, 1);
        }
        app
    }

    fn draw_pane(app: &App, w: u16, h: u16) -> Vec<String> {
        use ratatui::{Terminal, backend::TestBackend};
        let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
        term.draw(|f| render(f, Rect::new(0, 0, w, h), app))
            .unwrap();
        let buf = term.backend().buffer().clone();
        (0..h)
            .map(|y| (0..w).map(|x| buf.cell((x, y)).unwrap().symbol()).collect())
            .collect()
    }

    #[test]
    fn the_pane_scrolls_through_a_log_longer_than_it() {
        let mut app = busy_app(30);
        let screen = draw_pane(&app, 60, 7); // 5 visible lines
        assert!(screen[1].contains("SELECT 29"), "newest first: {screen:?}");
        assert!(!screen.join("").contains("SELECT 20"));
        assert!(screen.iter().any(|l| l.contains('▼')), "scrollbar shown");

        scroll_by(&mut app, 10);
        let screen = draw_pane(&app, 60, 7);
        assert!(screen[1].contains("SELECT 19"), "{screen:?}");

        // clamps at both ends: the oldest entry is reachable, nothing past it
        scroll_by(&mut app, 1_000);
        assert_eq!(app.query_inspector_scroll, 30 - 5);
        let screen = draw_pane(&app, 60, 7);
        assert!(screen[5].contains("SELECT 0"), "{screen:?}");
        scroll_by(&mut app, -1_000);
        assert_eq!(app.query_inspector_scroll, 0);
    }

    #[test]
    fn a_short_log_has_nothing_to_scroll() {
        let mut app = busy_app(2);
        let screen = draw_pane(&app, 60, 7);
        assert!(!screen.iter().any(|l| l.contains('▼')));
        scroll_by(&mut app, 5);
        assert_eq!(app.query_inspector_scroll, 0);
    }

    #[test]
    fn one_long_error_can_be_scrolled_through_line_by_line() {
        let mut app = App::new(crate::config::ConnectionConfig::default(), false);
        let id = app.add_query_log("SELECT bad".to_string());
        let error = (0..40)
            .map(|i| format!("detail{i:02}"))
            .collect::<Vec<_>>()
            .join(" ");
        app.complete_query_log_error(id, error);
        let first = draw_pane(&app, 50, 7).join("\n");
        assert!(first.contains("detail00") && !first.contains("detail39"));
        scroll_by(&mut app, 1_000);
        let last = draw_pane(&app, 50, 7).join("\n");
        assert!(
            last.contains("detail39"),
            "end of the error reachable: {last}"
        );
    }

    #[test]
    fn mouse_selection_follows_the_scrolled_window() {
        let mut app = busy_app(30);
        draw_pane(&app, 60, 7);
        scroll_by(&mut app, 10);
        // the first visible row is entry 10 (newest-first), i.e. "SELECT 19"
        assert_eq!(selected_text(&app, wrap_width(58), 0, 0), "SELECT 19");
        assert_eq!(
            selected_text(&app, wrap_width(58), 0, 1),
            "SELECT 19\nSELECT 18"
        );
    }

    #[test]
    fn hit_testing_uses_the_drawn_area() {
        let app = busy_app(3);
        assert!(!contains(&app, 5, 3), "nothing drawn yet");
        draw_pane(&app, 60, 7);
        assert!(contains(&app, 0, 0) && contains(&app, 59, 6));
        assert!(!contains(&app, 60, 0) && !contains(&app, 0, 7));
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
