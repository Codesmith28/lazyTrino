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
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, List, ListItem, ListState},
    Frame,
};

use crate::app::{App, QueryStatus};

pub fn render(frame: &mut Frame, area: Rect, app: &App) {
    let block = Block::default()
        .title(" Executed Queries ")
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::DarkGray));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    if app.query_logs.is_empty() {
        let empty_item = ListItem::new(Line::from(Span::styled(
            " No queries executed yet",
            Style::default().fg(Color::DarkGray),
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
        .take(inner.height as usize)
        .map(|entry| {
            let (symbol, style) = match &entry.status {
                QueryStatus::Success => (
                    "✓",
                    Style::default().fg(Color::Green).add_modifier(Modifier::BOLD),
                ),
                QueryStatus::Error => (
                    "✗",
                    Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                ),
                QueryStatus::Running => (
                    "○",
                    Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
                ),
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

            let line = Line::from(vec![
                Span::styled(format!(" {symbol}"), style),
                Span::styled(meta, Style::default().fg(Color::DarkGray)),
                Span::styled(
                    entry.sql.clone(),
                    Style::default().fg(Color::White),
                ),
            ]);

            ListItem::new(line)
        })
        .collect();

    let mut state = ListState::default();
    let list = List::new(items);
    frame.render_stateful_widget(list, inner, &mut state);
}
