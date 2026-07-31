use ratatui::{
    layout::{Constraint, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Cell, Paragraph, Row, Table},
    Frame,
};

use crate::app::ResultsState;

pub fn wrap_text(text: &str, max_width: usize) -> Vec<String> {
    if max_width == 0 {
        return vec![text.to_string()];
    }
    let mut lines = Vec::new();
    for raw_line in text.lines() {
        if raw_line.is_empty() {
            lines.push(String::new());
            continue;
        }
        let words: Vec<&str> = raw_line.split_whitespace().collect();
        if words.is_empty() {
            lines.push(String::new());
            continue;
        }
        let mut current_line = String::new();
        for word in words {
            if word.len() > max_width {
                if !current_line.is_empty() {
                    lines.push(current_line);
                    current_line = String::new();
                }
                let mut start = 0;
                while start < word.len() {
                    let end = (start + max_width).min(word.len());
                    lines.push(word[start..end].to_string());
                    start = end;
                }
            } else {
                if current_line.is_empty() {
                    current_line.push_str(word);
                } else if current_line.len() + 1 + word.len() <= max_width {
                    current_line.push(' ');
                    current_line.push_str(word);
                } else {
                    lines.push(current_line);
                    current_line = word.to_string();
                }
            }
        }
        if !current_line.is_empty() {
            lines.push(current_line);
        }
    }
    if lines.is_empty() {
        vec![text.to_string()]
    } else {
        lines
    }
}

pub fn render(frame: &mut Frame, area: Rect, state: &ResultsState, _spinner: String, is_active: bool) {
    let border_color = if is_active { Color::Yellow } else { Color::DarkGray };

    let title_prefix = " Query Results : ";
    let main_title = if state.invalid_query_error.is_some() {
        format!("{title_prefix}Invalid Query ")
    } else if state.is_paginated {
        if state.is_fetching_next_page {
            format!(
                "{title_prefix}Table View — Infinite Scroll ({} rows loaded, col {}/{}) [Fetching page...] ",
                state.rows.len(),
                state.scroll_h + 1,
                state.columns.len().max(1)
            )
        } else if !state.has_more_rows {
            format!(
                "{title_prefix}Table View — Infinite Scroll (All {} rows loaded, col {}/{}) ",
                state.rows.len(),
                state.scroll_h + 1,
                state.columns.len().max(1)
            )
        } else {
            format!(
                "{title_prefix}Table View — Infinite Scroll ({} rows loaded, col {}/{}) ",
                state.rows.len(),
                state.scroll_h + 1,
                state.columns.len().max(1)
            )
        }
    } else {
        format!(
            "{title_prefix}Sample Mode ({} sample rows, col {}/{}) ",
            state.rows.len(),
            state.scroll_h + 1,
            state.columns.len().max(1)
        )
    };

    let block = Block::default()
        .title(main_title)
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(border_color));

    let inner = block.inner(area);
    frame.render_widget(&block, area);

    if state.loading {
        return;
    }

    if let Some(invalid_msg) = &state.invalid_query_error {
        let err_lines = vec![
            Line::from(Span::styled("✖ Invalid query for this view", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD))),
            Line::from(""),
            Line::from(Span::styled(invalid_msg.as_str(), Style::default().fg(Color::Red))),
            Line::from(""),
            Line::from(Span::styled(
                format!("Current View Scope: {}.{}.{}", state.catalog, state.schema, state.table),
                Style::default().fg(Color::Cyan),
            )),
            Line::from(""),
            Line::from(Span::styled("Note: Queries in this view can only target data inside the current table.", Style::default().fg(Color::DarkGray))),
            Line::from(Span::styled("Enter full SQL queries operating on this table (e.g. 'SELECT * FROM table WHERE col = val').", Style::default().fg(Color::DarkGray))),
        ];
        let p = Paragraph::new(err_lines).wrap(ratatui::widgets::Wrap { trim: false });
        frame.render_widget(p, inner);
        return;
    }

    if let Some(err) = &state.error {
        let err = Paragraph::new(err.as_str())
            .style(Style::default().fg(Color::Red))
            .wrap(ratatui::widgets::Wrap { trim: false });
        frame.render_widget(err, inner);
        return;
    }

    if state.columns.is_empty() && state.rows.is_empty() {
        let empty = Paragraph::new("No results returned")
            .style(Style::default().fg(Color::Gray))
            .alignment(ratatui::layout::Alignment::Center)
            .wrap(ratatui::widgets::Wrap { trim: false });
        frame.render_widget(empty, inner);
        return;
    }

    if state.columns.is_empty() && !state.rows.is_empty() && state.rows[0].len() == 1 {
        let err = Paragraph::new(state.rows[0][0].as_str())
            .style(Style::default().fg(Color::Red))
            .wrap(ratatui::widgets::Wrap { trim: false });
        frame.render_widget(err, inner);
        return;
    }

    let cell_style = Style::default().fg(Color::White);
    let header_style = Style::default()
        .fg(Color::Yellow)
        .add_modifier(Modifier::BOLD);

    let available_width = inner.width.saturating_sub(4) as usize;

    let mut cumulative_width = 0;
    let mut visible_cols: Vec<(usize, &String)> = Vec::new();
    let mut col_widths: Vec<Constraint> = Vec::new();
    let mut col_width_sizes: Vec<usize> = Vec::new();

    for (col_idx, c) in state.columns.iter().enumerate().skip(state.scroll_h) {
        let max_data = state
            .rows
            .iter()
            .map(|r| r.get(col_idx).map(|v| v.len()).unwrap_or(0))
            .max()
            .unwrap_or(0);
        let raw_width = (c.len().max(max_data) as u16).max(8);
        let max_allowed = (available_width / 2).max(15) as u16;
        let needed_width = raw_width.min(max_allowed);

        if visible_cols.is_empty() || cumulative_width + (needed_width as usize) + 2 <= available_width {
            cumulative_width += (needed_width as usize) + 2;
            visible_cols.push((col_idx, c));
            col_widths.push(Constraint::Length(needed_width));
            col_width_sizes.push(needed_width as usize);
        } else {
            break;
        }
    }

    if col_widths.is_empty() {
        col_widths.push(Constraint::Length(15));
        col_width_sizes.push(15);
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
        .take((inner.height as usize).saturating_sub(2))
        .map(|row| {
            let mut max_row_height: u16 = 1;
            let cells: Vec<Cell> = visible_cols
                .iter()
                .enumerate()
                .map(|(v_idx, (col_idx, _))| {
                    let val = row.get(*col_idx).map(|s| s.as_str()).unwrap_or("");
                    let col_w = col_width_sizes.get(v_idx).copied().unwrap_or(15);
                    let wrapped_lines = wrap_text(val, col_w);
                    max_row_height = max_row_height.max(wrapped_lines.len() as u16);
                    Cell::from(wrapped_lines.join("\n")).style(cell_style)
                })
                .collect();
            Row::new(cells).height(max_row_height)
        })
        .collect();

    let table = Table::new(visible_rows, col_widths)
        .header(header)
        .column_spacing(2);

    use ratatui::widgets::{Scrollbar, ScrollbarOrientation, ScrollbarState};

    frame.render_widget(table, inner);

    if !state.rows.is_empty() {
        let mut v_scroll_state = ScrollbarState::new(state.rows.len().saturating_sub(1)).position(state.scroll_v);
        frame.render_stateful_widget(
            Scrollbar::default()
                .orientation(ScrollbarOrientation::VerticalRight)
                .begin_symbol(Some("▲"))
                .end_symbol(Some("▼")),
            inner,
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
            inner,
            &mut h_scroll_state,
        );
    }
}
