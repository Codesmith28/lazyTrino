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
    widgets::{Block, BorderType, Borders, List, ListItem, ListState},
};

use crate::{
    app::{App, QueryStatus},
    tui::theme,
};

pub fn render(frame: &mut Frame, area: Rect, app: &App) {
    let title = if let Some((ref msg, ref instant)) = app.copied_toast {
        if instant.elapsed().as_secs() < 3 {
            format!(" Executed Queries — Copied: \"{}\" ", msg)
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
        .border_style(theme::style(theme::INACTIVE_BORDER));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    if app.query_logs.is_empty() {
        let empty_item = ListItem::new(Line::from(Span::styled(
            " No queries executed yet",
            theme::muted_style(),
        )));
        let list = List::new(vec![empty_item]);
        frame.render_widget(list, inner);
        return;
    }

    let items: Vec<ListItem> = app
        .query_logs
        .iter()
        .rev()
        .skip(app.query_inspector_scroll)
        .enumerate()
        .map(|(idx, entry)| {
            let item_y = inner.y + idx as u16;
            let is_mouse_sel = app.is_area_mouse_selected(inner.x, inner.width, item_y);

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
                QueryStatus::Error => {
                    let msg = entry.error_msg.as_deref().unwrap_or("error");
                    format!(" [{msg}] ")
                }
                QueryStatus::Running => " [running] ".to_string(),
            };

            let header_prefix = format!(" {symbol}{meta}");
            let header_len = header_prefix.len();
            let avail_w = (inner.width as usize).saturating_sub(header_len).max(10);
            let wrapped_sql = crate::tui::screens::results::wrap_text(&entry.sql, avail_w);

            let mut lines = Vec::new();
            if let Some(first_line) = wrapped_sql.first() {
                let sql_style = if is_mouse_sel {
                    theme::selection_style()
                } else {
                    theme::text_style()
                };
                lines.push(Line::from(vec![
                    Span::styled(format!(" {symbol}"), style),
                    Span::styled(meta, theme::muted_style()),
                    Span::styled(first_line.clone(), sql_style),
                ]));
            }
            let indent = " ".repeat(header_len);
            for remaining in wrapped_sql.iter().skip(1) {
                let sql_style = if is_mouse_sel {
                    theme::selection_style()
                } else {
                    theme::text_style()
                };
                lines.push(Line::from(vec![
                    Span::raw(indent.clone()),
                    Span::styled(remaining.clone(), sql_style),
                ]));
            }

            ListItem::new(lines)
        })
        .collect();

    let mut state = ListState::default();
    let list = List::new(items);
    frame.render_stateful_widget(list, inner, &mut state);
}
