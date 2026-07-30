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
    layout::{Constraint, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Cell, Paragraph, Row, Table},
    Frame,
};

use crate::app::ResultsState;

pub fn render(frame: &mut Frame, area: Rect, state: &ResultsState, _spinner: String) {
    let block = Block::default()
        .title(" Query Results ")
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::Cyan));
    frame.render_widget(&block, area);

    let inner = block.inner(area);

    let chunks = ratatui::layout::Layout::vertical([
        ratatui::layout::Constraint::Length(1),
        ratatui::layout::Constraint::Min(0),
    ])
    .split(inner);

    let query_preview = if state.query.len() > inner.width.saturating_sub(4) as usize {
        format!("{}...", &state.query[..inner.width.saturating_sub(7) as usize])
    } else {
        state.query.clone()
    };
    let query_line = Paragraph::new(Line::from(vec![
        Span::styled("SQL: ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        Span::styled(query_preview, Style::default().fg(Color::Gray)),
    ]));
    frame.render_widget(query_line, chunks[0]);

    if state.loading {
        return;
    }

    if state.columns.is_empty() && state.rows.is_empty() {
        let empty = Paragraph::new("No results returned")
            .style(Style::default().fg(Color::Gray))
            .alignment(ratatui::layout::Alignment::Center);
        frame.render_widget(empty, chunks[1]);
        return;
    }

    if state.columns.is_empty() && !state.rows.is_empty() && state.rows[0].len() == 1 {
        let err = Paragraph::new(state.rows[0][0].as_str())
            .style(Style::default().fg(Color::Red));
        frame.render_widget(err, chunks[1]);
        return;
    }

    let cell_style = Style::default().fg(Color::White);
    let header_style = Style::default()
        .fg(Color::Yellow)
        .add_modifier(Modifier::BOLD);

    let visible_cols: Vec<(usize, &String)> = state
        .columns
        .iter()
        .enumerate()
        .skip(state.scroll_h)
        .collect();

    let mut col_widths: Vec<Constraint> = visible_cols
        .iter()
        .map(|(col_idx, c)| {
            let max_data = state
                .rows
                .iter()
                .map(|r| r.get(*col_idx).map(|v| v.len()).unwrap_or(0))
                .max()
                .unwrap_or(0);
            Constraint::Length((c.len().max(max_data) as u16).max(10))
        })
        .collect();

    if col_widths.is_empty() {
        col_widths.push(Constraint::Length(15));
    }

    let header_cells: Vec<Cell> = visible_cols
        .iter()
        .map(|(_, c)| Cell::from(c.as_str()).style(header_style))
        .collect();
    let header = Row::new(header_cells).bottom_margin(1);

    let visible_rows: Vec<Row> = state
        .rows
        .iter()
        .skip(state.scroll_v)
        .take((chunks[1].height as usize).saturating_sub(2))
        .map(|row| {
            let cells: Vec<Cell> = visible_cols
                .iter()
                .map(|(col_idx, _)| {
                    let val = row.get(*col_idx).map(|s| s.as_str()).unwrap_or("");
                    Cell::from(val).style(cell_style)
                })
                .collect();
            Row::new(cells)
        })
        .collect();

    let title_text = format!(" Results ({} rows) ", state.rows.len());
    let table = Table::new(visible_rows, col_widths)
        .header(header)
        .column_spacing(2)
        .block(
            Block::default()
                .title(title_text)
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(Color::Cyan)),
        );

    frame.render_widget(table, chunks[1]);
}
