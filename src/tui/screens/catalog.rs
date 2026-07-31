use ratatui::{
    Frame,
    layout::Rect,
    text::Line,
    widgets::{
        Block, BorderType, Borders, List, ListItem, ListState, Scrollbar, ScrollbarOrientation,
        ScrollbarState,
    },
};

use crate::{app::CatalogState, tui::theme};

pub fn render(frame: &mut Frame, area: Rect, state: &CatalogState, search: &str, is_active: bool) {
    render_selectable_list(
        frame,
        area,
        " Catalogs ",
        &state.items,
        state.selected,
        search,
        is_active,
    );
}

pub(crate) fn render_selectable_list(
    frame: &mut Frame,
    area: Rect,
    title: &str,
    items: &[String],
    selected: usize,
    search: &str,
    is_active: bool,
) {
    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(theme::border_style(is_active));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let filtered: Vec<(usize, &String)> = items
        .iter()
        .enumerate()
        .filter(|(_, name)| {
            search.is_empty() || name.to_lowercase().contains(&search.to_lowercase())
        })
        .collect();

    if filtered.is_empty() {
        return;
    }

    let items: Vec<ListItem> = filtered
        .iter()
        .map(|(orig_idx, name)| {
            let prefix = format!("{:>3} ", orig_idx + 1);
            let line = if *orig_idx == selected {
                Line::styled(format!("{prefix}{name}"), theme::selection_style())
            } else {
                Line::styled(format!("{prefix}{name}"), theme::text_style())
            };
            ListItem::new(line)
        })
        .collect();

    let mut list_state = ListState::default().with_selected(Some(selected));
    let list = List::new(items).highlight_style(theme::selection_style());

    frame.render_stateful_widget(list, inner, &mut list_state);

    if filtered.len() > 1 {
        let mut scrollbar_state =
            ScrollbarState::new(filtered.len().saturating_sub(1)).position(selected);
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
