use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::Line,
    widgets::{Block, BorderType, Borders, List, ListItem, ListState},
    Frame,
};

use crate::app::ACTIONS;

pub fn render(frame: &mut Frame, area: Rect, _catalog: &str, _schema: &str, table: &str, selected: usize) {
    let block = Block::default()
        .title(format!(" Actions — {table} "))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::Cyan));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let items: Vec<ListItem> = ACTIONS
        .iter()
        .enumerate()
        .map(|(i, (key, label, _))| {
            let line = if i == selected {
                Line::styled(
                    format!(" {key}  {label}"),
                    Style::default()
                        .fg(Color::Black)
                        .bg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                )
            } else {
                Line::styled(
                    format!(" {key}  {label}"),
                    Style::default().fg(Color::White),
                )
            };
            ListItem::new(line)
        })
        .collect();

    let mut list_state = ListState::default().with_selected(Some(selected));
    let list = List::new(items)
        .highlight_style(
            Style::default()
                .fg(Color::Black)
                .bg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        );

    frame.render_stateful_widget(list, inner, &mut list_state);
}
