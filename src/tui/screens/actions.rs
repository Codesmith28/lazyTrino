use ratatui::{
    Frame,
    layout::Rect,
    text::Line,
    widgets::{Block, BorderType, Borders, List, ListItem, ListState},
};

use crate::{
    app::{ACTIONS, App},
    tui::theme,
};

pub fn render(
    frame: &mut Frame,
    area: Rect,
    _catalog: &str,
    _schema: &str,
    table: &str,
    selected: usize,
    is_active: bool,
    app: &App,
) {
    let block = Block::default()
        .title(if area.width < 25 {
            " Menu ".to_string()
        } else {
            format!(" Menu — {table} ")
        })
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(theme::border_style(is_active));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let items: Vec<ListItem> = ACTIONS
        .iter()
        .enumerate()
        .map(|(i, (key, label, _))| {
            let item_y = inner.y + i as u16;
            let is_mouse_sel = app.is_area_mouse_selected(inner.x, inner.width, item_y);

            let is_selected = i == selected;
            let prefix = if is_selected { "▸ " } else { "  " };
            let text = format!("{prefix}[{key}] {label}");
            let line = if is_mouse_sel || is_selected {
                Line::styled(text, theme::selection_style())
            } else {
                Line::styled(text, theme::text_style())
            };
            ListItem::new(line)
        })
        .collect();

    let mut list_state = ListState::default().with_selected(Some(selected));
    let list = List::new(items).highlight_style(theme::selection_style());

    frame.render_stateful_widget(list, inner, &mut list_state);

    use ratatui::widgets::{Scrollbar, ScrollbarOrientation, ScrollbarState};
    if ACTIONS.len() > 1 {
        let mut scrollbar_state =
            ScrollbarState::new(ACTIONS.len().saturating_sub(1)).position(selected);
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
