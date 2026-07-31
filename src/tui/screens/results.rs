use ratatui::{
    layout::{Constraint, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Cell, Paragraph, Row, Table},
    Frame,
};

use crate::app::ResultsState;

pub fn render(frame: &mut Frame, area: Rect, state: &ResultsState, _spinner: String, is_active: bool) {
    let border_color = if is_active { Color::Yellow } else { Color::DarkGray };
    let block = Block::default()
        .title(" Query Results ")
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(border_color));
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

    let available_width = chunks[1].width.saturating_sub(4) as usize;

    let mut cumulative_width = 0;
    let mut visible_cols: Vec<(usize, &String)> = Vec::new();
    let mut col_widths: Vec<Constraint> = Vec::new();

    for (col_idx, c) in state.columns.iter().enumerate().skip(state.scroll_h) {
        let max_data = state
            .rows
            .iter()
            .map(|r| r.get(col_idx).map(|v| v.len()).unwrap_or(0))
            .max()
            .unwrap_or(0);
        let needed_width = (c.len().max(max_data) as u16).max(8);

        if visible_cols.is_empty() || cumulative_width + (needed_width as usize) + 2 <= available_width {
            cumulative_width += (needed_width as usize) + 2;
            visible_cols.push((col_idx, c));
            col_widths.push(Constraint::Length(needed_width));
        } else {
            break;
        }
    }

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

    let title_text = if state.is_paginated {
        if state.is_fetching_next_page {
            format!(
                " Table View — Infinite Scroll ({} rows loaded, col {}/{}) [Fetching page...] ",
                state.rows.len(),
                state.scroll_h + 1,
                state.columns.len().max(1)
            )
        } else if !state.has_more_rows {
            format!(
                " Table View — Infinite Scroll (All {} rows loaded, col {}/{}) ",
                state.rows.len(),
                state.scroll_h + 1,
                state.columns.len().max(1)
            )
        } else {
            format!(
                " Table View — Infinite Scroll ({} rows loaded, col {}/{}) ",
                state.rows.len(),
                state.scroll_h + 1,
                state.columns.len().max(1)
            )
        }
    } else {
        format!(
            " Sample Mode ({} sample rows, col {}/{}) ",
            state.rows.len(),
            state.scroll_h + 1,
            state.columns.len().max(1)
        )
    };
    let table = Table::new(visible_rows, col_widths)
        .header(header)
        .column_spacing(2)
        .block(
            Block::default()
                .title(title_text)
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(border_color)),
        );

    use ratatui::widgets::{Scrollbar, ScrollbarOrientation, ScrollbarState};

    frame.render_widget(table, chunks[1]);

    if !state.rows.is_empty() {
        let mut v_scroll_state = ScrollbarState::new(state.rows.len().saturating_sub(1)).position(state.scroll_v);
        frame.render_stateful_widget(
            Scrollbar::default()
                .orientation(ScrollbarOrientation::VerticalRight)
                .begin_symbol(Some("▲"))
                .end_symbol(Some("▼")),
            chunks[1],
            &mut v_scroll_state,
        );
    }

    if state.columns.len() > 1 {
        let mut h_scroll_state = ScrollbarState::new(state.columns.len().saturating_sub(1)).position(state.scroll_h);
        frame.render_stateful_widget(
            Scrollbar::default()
                .orientation(ScrollbarOrientation::HorizontalBottom)
                .begin_symbol(Some("◄"))
                .end_symbol(Some("►")),
            chunks[1],
            &mut h_scroll_state,
        );
    }
}
