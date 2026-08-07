use ratatui::{
    Frame,
    layout::Rect,
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, List, ListItem, ListState},
};

use crate::{app::DrillDownState, tui::theme};

/// Renders the cd/ls-style breadcrumb ("date=2026-08-06 › service=smb3 ›
/// (choose account_id)") followed by the selectable list of distinct
/// values at the level currently being browsed. Used only while browsing
/// a partitioned table's partition hierarchy — once every partition
/// column has a fixed value, rendering falls back to the ordinary
/// (infinite-scroll) results grid for the leaf records.
pub fn render(
    frame: &mut Frame,
    area: Rect,
    table_name: &str,
    dd: &DrillDownState,
    is_active: bool,
    app: &crate::app::App,
) {
    let depth = dd.depth();
    let mut breadcrumb = dd
        .path
        .iter()
        .map(|(col, val)| format!("{col}={val}"))
        .collect::<Vec<_>>()
        .join(" › ");
    let next_label = dd
        .partition_cols
        .get(depth)
        .map(|c| format!("(choose {c})"))
        .unwrap_or_default();
    if !breadcrumb.is_empty() {
        breadcrumb.push_str(" › ");
    }
    breadcrumb.push_str(&next_label);

    let title = format!(" Table View — {table_name} : {breadcrumb} ");
    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(theme::border_style(is_active));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if let Some(err) = &dd.error {
        let p = ratatui::widgets::Paragraph::new(Line::from(Span::styled(
            format!("✖ {err}"),
            theme::error_style(),
        )))
        .wrap(ratatui::widgets::Wrap { trim: false });
        frame.render_widget(p, inner);
        return;
    }

    if dd.loading {
        let p = ratatui::widgets::Paragraph::new(Line::from(Span::styled(
            " Loading partition values... ",
            theme::info_bold_style(),
        )));
        frame.render_widget(p, inner);
        return;
    }

    let items_area = if dd.truncated && inner.height > 1 {
        let notice = Rect { height: 1, ..inner };
        let list_area = Rect {
            y: inner.y + 1,
            height: inner.height.saturating_sub(1),
            ..inner
        };
        let notice_p = ratatui::widgets::Paragraph::new(Line::from(Span::styled(
            " ⚠ truncated — showing first 200 values ",
            theme::warning_bold_style(),
        )));
        frame.render_widget(notice_p, notice);
        list_area
    } else {
        inner
    };

    let empty = Vec::new();
    let values = dd.levels_cache.get(depth).unwrap_or(&empty);

    if values.is_empty() {
        let p = ratatui::widgets::Paragraph::new("No partition values found")
            .style(theme::secondary_style())
            .alignment(ratatui::layout::Alignment::Center);
        frame.render_widget(p, items_area);
        return;
    }

    let items: Vec<ListItem> = values
        .iter()
        .enumerate()
        .map(|(i, val)| {
            let item_y = items_area.y + i as u16;
            let is_mouse_sel = app.is_area_mouse_selected(items_area.x, items_area.width, item_y);
            let is_selected = i == dd.selected;
            let prefix = if is_selected { "▸ " } else { "  " };
            let text = format!("{prefix}{val}");
            let line = if is_mouse_sel || is_selected {
                Line::styled(text, theme::selection_style())
            } else {
                Line::styled(text, theme::text_style())
            };
            ListItem::new(line)
        })
        .collect();

    let mut list_state = ListState::default().with_selected(Some(dd.selected));
    let list = List::new(items).highlight_style(theme::selection_style());
    frame.render_stateful_widget(list, items_area, &mut list_state);

    use ratatui::widgets::{Scrollbar, ScrollbarOrientation, ScrollbarState};
    if values.len() > 1 {
        let mut scrollbar_state =
            ScrollbarState::new(values.len().saturating_sub(1)).position(dd.selected);
        frame.render_stateful_widget(
            Scrollbar::default()
                .orientation(ScrollbarOrientation::VerticalRight)
                .begin_symbol(Some("▲"))
                .end_symbol(Some("▼")),
            items_area,
            &mut scrollbar_state,
        );
    }
}
