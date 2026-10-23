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
    layout::{Alignment, Constraint, Layout, Rect},
    text::{Line, Span},
    widgets::{Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState},
};

use crate::app::{AppliedFilterKind, AppliedFiltersModalState};
use crate::tui::screens::modal;
use crate::tui::theme;

pub fn render(frame: &mut Frame, area: Rect, modal: &AppliedFiltersModalState) {
    let popup_area = modal::modal_medium(area);
    let total = modal.items.len();
    let title = if total == 0 {
        " Applied Filters in Current View ".to_string()
    } else {
        format!(" Applied Filters in Current View ({total} active) ")
    };

    let inner = modal::render_modal_frame(frame, popup_area, &title, true);

    if inner.height < 3 {
        return;
    }

    let chunks = Layout::vertical([
        Constraint::Min(0),    // Filters list or empty message
        Constraint::Length(1), // Hint footer
    ])
    .split(inner);

    let content_area = chunks[0];
    let hint_area = chunks[1];

    if modal.items.is_empty() {
        let empty_lines = vec![
            Line::from(""),
            Line::from(Span::styled(
                "No filters are currently applied to this view.",
                theme::bold_text_style(),
            )),
            Line::from(""),
            Line::from(Span::styled(
                "• In Table View, press 'f' on any column to add a column filter.",
                theme::muted_style(),
            )),
            Line::from(Span::styled(
                "• Drill down into partitions to filter by partition keys.",
                theme::muted_style(),
            )),
        ];
        let p = Paragraph::new(empty_lines).alignment(Alignment::Center);
        frame.render_widget(p, content_area);
    } else {
        let visible_height = content_area.height as usize;
        let max_scroll = total.saturating_sub(visible_height);
        let selected = modal.selected.min(total.saturating_sub(1));
        let scroll = if selected < visible_height {
            0
        } else {
            selected.saturating_sub(visible_height.saturating_sub(1))
        }
        .min(max_scroll);

        let lines: Vec<Line> = modal
            .items
            .iter()
            .enumerate()
            .skip(scroll)
            .take(visible_height)
            .map(|(idx, item)| {
                let is_sel = idx == selected;
                let (badge_text, badge_style) = match item.kind {
                    AppliedFilterKind::Partition => (" [Partition] ", theme::info_bold_style()),
                    AppliedFilterKind::Column => (" [Column]    ", theme::warning_bold_style()),
                };

                let line_spans = if is_sel {
                    vec![
                        Span::styled(" ▸", theme::selection_style()),
                        Span::styled(badge_text, theme::selection_style()),
                        Span::styled(
                            format!("{}: {}", item.name, item.display_value),
                            theme::selection_style(),
                        ),
                    ]
                } else {
                    vec![
                        Span::styled("  ", theme::text_style()),
                        Span::styled(badge_text, badge_style),
                        Span::styled(&item.name, theme::bold_text_style()),
                        Span::styled(format!(": {}", item.display_value), theme::text_style()),
                    ]
                };
                Line::from(line_spans)
            })
            .collect();

        frame.render_widget(Paragraph::new(lines), content_area);

        if total > visible_height && visible_height > 0 {
            let mut scrollbar_state = ScrollbarState::new(max_scroll).position(scroll);
            frame.render_stateful_widget(
                Scrollbar::default()
                    .orientation(ScrollbarOrientation::VerticalRight)
                    .begin_symbol(Some("▲"))
                    .end_symbol(Some("▼")),
                content_area,
                &mut scrollbar_state,
            );
        }
    }

    let hint_spans = vec![
        Span::styled(" j/k", theme::warning_bold_style()),
        Span::styled(":move  ", theme::muted_style()),
        Span::styled("d/x", theme::warning_bold_style()),
        Span::styled(":remove filter  ", theme::muted_style()),
        Span::styled("c", theme::warning_bold_style()),
        Span::styled(":clear all  ", theme::muted_style()),
        Span::styled("Esc/q", theme::warning_bold_style()),
        Span::styled(":close", theme::muted_style()),
    ];
    frame.render_widget(Paragraph::new(Line::from(hint_spans)), hint_area);
}
