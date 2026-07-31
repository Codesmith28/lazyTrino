use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, List, ListItem, ListState},
    Frame,
};

use crate::app::{App, QueryStatus};

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

            let header_prefix = format!(" {symbol}{meta}");
            let header_len = header_prefix.len();
            let avail_w = (inner.width as usize).saturating_sub(header_len).max(10);
            let wrapped_sql = crate::tui::screens::results::wrap_text(&entry.sql, avail_w);

            let mut lines = Vec::new();
            if let Some(first_line) = wrapped_sql.first() {
                lines.push(Line::from(vec![
                    Span::styled(format!(" {symbol}"), style),
                    Span::styled(meta, Style::default().fg(Color::DarkGray)),
                    Span::styled(first_line.clone(), Style::default().fg(Color::White)),
                ]));
            }
            let indent = " ".repeat(header_len);
            for remaining in wrapped_sql.iter().skip(1) {
                lines.push(Line::from(vec![
                    Span::raw(indent.clone()),
                    Span::styled(remaining.clone(), Style::default().fg(Color::White)),
                ]));
            }

            ListItem::new(lines)
        })
        .collect();

    let mut state = ListState::default();
    let list = List::new(items);
    frame.render_stateful_widget(list, inner, &mut state);
}
