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

    let table_count_str = if let Some(tc) = dd.table_count {
        format!(" [{}]", crate::app::format_count(tc))
    } else if dd.table_count_loading {
        " [counting...]".to_string()
    } else {
        String::new()
    };

    let mut breadcrumb = dd
        .path
        .iter()
        .enumerate()
        .map(|(i, (col, val))| {
            let prefix = dd.path[..=i].to_vec();
            if let Some(c) = dd.counts_cache.get(&prefix) {
                format!("{col}={val} [{}]", crate::app::format_count(*c))
            } else if i == depth.saturating_sub(1) && dd.current_count_loading {
                format!("{col}={val} [counting...]")
            } else {
                format!("{col}={val}")
            }
        })
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

    let title = format!(" Table View — {table_name}{table_count_str} : {breadcrumb} ");
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

    let (content_area, _banner_area) = if inner.height >= 4 {
        let chunks = ratatui::layout::Layout::vertical([
            ratatui::layout::Constraint::Length(1),
            ratatui::layout::Constraint::Min(1),
        ])
        .split(inner);

        let table_total_display = if let Some(tc) = dd.table_count {
            format!("{} records", crate::app::format_count(tc))
        } else if dd.table_count_loading {
            "counting...".to_string()
        } else {
            "unknown".to_string()
        };

        let mut banner_spans = vec![
            Span::styled(" Total Table: ", theme::muted_style()),
            Span::styled(table_total_display, theme::info_bold_style()),
        ];

        if depth > 0 && let Some((curr_col, curr_val)) = dd.path.last() {
            let part_count_display = if let Some(pc) = dd.counts_cache.get(&dd.path) {
                format!("{} records", crate::app::format_count(*pc))
            } else if dd.current_count_loading {
                "counting...".to_string()
            } else {
                "unknown".to_string()
            };

            banner_spans.push(Span::styled("  │  ", theme::muted_style()));
            banner_spans.push(Span::styled(
                format!("Partition ({curr_col}={curr_val}): "),
                theme::muted_style(),
            ));
            banner_spans.push(Span::styled(part_count_display, theme::success_bold_style()));
        }

        let banner_p = ratatui::widgets::Paragraph::new(Line::from(banner_spans));
        frame.render_widget(banner_p, chunks[0]);
        (chunks[1], Some(chunks[0]))
    } else {
        (inner, None)
    };

    if dd.loading {
        let p = ratatui::widgets::Paragraph::new(Line::from(Span::styled(
            " Loading partition values... ",
            theme::info_bold_style(),
        )));
        frame.render_widget(p, content_area);
        return;
    }

    let items_area = if dd.truncated && content_area.height > 1 {
        let notice = Rect { height: 1, ..content_area };
        let list_area = Rect {
            y: content_area.y + 1,
            height: content_area.height.saturating_sub(1),
            ..content_area
        };
        let notice_p = ratatui::widgets::Paragraph::new(Line::from(Span::styled(
            " ⚠ truncated — showing first 200 values ",
            theme::warning_bold_style(),
        )));
        frame.render_widget(notice_p, notice);
        list_area
    } else {
        content_area
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

    let max_val_len = values.iter().map(|v| v.len()).max().unwrap_or(10);
    let fixed_gap = 4;
    let name_col_width = max_val_len + fixed_gap;

    let items: Vec<ListItem> = values
        .iter()
        .enumerate()
        .map(|(i, val)| {
            let item_y = items_area.y + i as u16;
            // See actions.rs for why this must be gated by pane focus.
            let is_mouse_sel =
                is_active && app.is_area_mouse_selected(items_area.x, items_area.width, item_y);
            let is_selected = i == dd.selected;
            let prefix = if is_selected { "▸ " } else { "  " };

            let mut item_filter = dd.path.clone();
            if let Some(next_col) = dd.next_column() {
                item_filter.push((next_col.to_string(), val.clone()));
            }

            let count_text = if let Some(cnt) = dd.counts_cache.get(&item_filter) {
                format!("{} records", crate::app::format_count(*cnt))
            } else if is_selected {
                "counting records...".to_string()
            } else {
                String::new()
            };

            let name_text = format!("{prefix}{val}");
            let pad_len = (name_col_width + 2).saturating_sub(name_text.len());
            let padding = " ".repeat(pad_len);

            let name_style = if is_mouse_sel {
                theme::selection_style()
            } else if is_selected {
                theme::selection_style_for(is_active)
            } else {
                theme::text_style()
            };

            let count_style = if is_mouse_sel || is_selected {
                theme::style(ratatui::style::Color::Cyan)
                    .add_modifier(ratatui::style::Modifier::ITALIC)
            } else {
                theme::muted_style().add_modifier(ratatui::style::Modifier::ITALIC)
            };

            let line = if count_text.is_empty() {
                Line::from(vec![Span::styled(name_text, name_style)])
            } else {
                Line::from(vec![
                    Span::styled(name_text, name_style),
                    Span::raw(padding),
                    Span::styled(count_text, count_style),
                ])
            };

            ListItem::new(line)
        })
        .collect();

    let mut list_state = ListState::default().with_selected(Some(dd.selected));
    let list = List::new(items).highlight_style(theme::selection_style_for(is_active));
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{App, DrillDownState};
    use ratatui::{Terminal, backend::TestBackend, layout::Rect};
    use std::collections::HashMap;

    #[test]
    fn drilldown_renders_table_and_partition_counts_in_title_and_banner() {
        let mut counts = HashMap::new();
        counts.insert(
            vec![("date".to_string(), "2026-08-06".to_string())],
            45120,
        );

        let dd = DrillDownState {
            partition_cols: vec!["date".to_string(), "service".to_string()],
            path: vec![("date".to_string(), "2026-08-06".to_string())],
            levels_cache: vec![
                vec!["2026-08-06".to_string()],
                vec!["smb3".to_string(), "auth".to_string()],
            ],
            selected: 0,
            loading: false,
            error: None,
            truncated: false,
            table_count: Some(1234567),
            table_count_loading: false,
            counts_cache: counts,
            current_count_loading: false,
        };

        let app = App::new(crate::config::ConnectionConfig::default(), false);
        let backend = TestBackend::new(120, 20);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|f| {
                render(f, Rect::new(0, 0, 120, 20), "orders", &dd, true, &app);
            })
            .unwrap();

        let buffer = terminal.backend().buffer().clone();
        let mut content = String::new();
        for y in 0..buffer.area.height {
            for x in 0..buffer.area.width {
                content.push_str(buffer.cell((x, y)).unwrap().symbol());
            }
            content.push('\n');
        }

        // Check title includes table count and partition count
        assert!(
            content.contains("orders [1,234,567]"),
            "Title should contain table count formatted: {content}"
        );
        assert!(
            content.contains("date=2026-08-06 [45,120]"),
            "Breadcrumb should contain partition count formatted: {content}"
        );
        assert!(
            content.contains("(choose service)"),
            "Breadcrumb should contain next partition choice: {content}"
        );

        // Check banner contains table records and current partition count
        assert!(
            content.contains("Total Table: 1,234,567 records"),
            "Banner should contain total table count: {content}"
        );
        assert!(
            content.contains("Partition (date=2026-08-06): 45,120 records"),
            "Banner should contain current partition count: {content}"
        );
    }
}
