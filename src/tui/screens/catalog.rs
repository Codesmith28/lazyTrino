use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::Line,
    widgets::{Block, BorderType, Borders, List, ListItem, ListState},
    Frame,
};

use crate::app::CatalogState;

pub fn render(frame: &mut Frame, area: Rect, state: &CatalogState, search: &str) {
    let block = Block::default()
        .title(" Catalogs ")
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::Cyan));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let filtered: Vec<(usize, &String)> = state
        .items
        .iter()
        .enumerate()
        .filter(|(_, name)| search.is_empty() || name.to_lowercase().contains(&search.to_lowercase()))
        .collect();

    if filtered.is_empty() {
        return;
    }

    let items: Vec<ListItem> = filtered
        .iter()
        .map(|(orig_idx, name)| {
            let prefix = format!("{:>3} ", orig_idx + 1);
            let line = if *orig_idx == state.selected {
                Line::styled(
                    format!("{prefix}{name}"),
                    Style::default()
                        .fg(Color::Black)
                        .bg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                )
            } else {
                Line::styled(
                    format!("{prefix}{name}"),
                    Style::default().fg(Color::White),
                )
            };
            ListItem::new(line)
        })
        .collect();

    let mut list_state = ListState::default().with_selected(Some(state.selected));
    let list = List::new(items)
        .highlight_style(
            Style::default()
                .fg(Color::Black)
                .bg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        );

    frame.render_stateful_widget(list, inner, &mut list_state);
}
