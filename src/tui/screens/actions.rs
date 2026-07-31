use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::Line,
    widgets::{Block, BorderType, Borders, List, ListItem, ListState},
    Frame,
};

use crate::app::ACTIONS;

pub fn render(frame: &mut Frame, area: Rect, _catalog: &str, _schema: &str, table: &str, selected: usize, is_active: bool) {
    let border_color = if is_active { Color::Yellow } else { Color::DarkGray };
    let block = Block::default()
        .title(if area.width < 25 { " Menu ".to_string() } else { format!(" Menu — {table} ") })
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(border_color));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let items: Vec<ListItem> = ACTIONS
        .iter()
        .enumerate()
        .map(|(i, (key, label, _))| {
            let is_selected = i == selected;
            let prefix = if is_selected { "▸ " } else { "  " };
            let text = if inner.width < 22 {
                format!("{prefix}[{key}] {label}")
            } else {
                format!("{prefix}[{key}] {label}")
            };
            let line = if is_selected {
                Line::styled(
                    text,
                    Style::default()
                        .fg(Color::Black)
                        .bg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                )
            } else {
                Line::styled(
                    text,
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

    use ratatui::widgets::{Scrollbar, ScrollbarOrientation, ScrollbarState};
    if ACTIONS.len() > 1 {
        let mut scrollbar_state = ScrollbarState::new(ACTIONS.len().saturating_sub(1)).position(selected);
        frame.render_stateful_widget(
            Scrollbar::default()
                .orientation(ScrollbarOrientation::VerticalRight)
                .begin_symbol(Some("▲"))
                .end_symbol(Some("▼")),
            inner,
            &mut scrollbar_state,
        );
    }
}
