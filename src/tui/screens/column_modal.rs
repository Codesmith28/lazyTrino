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
    layout::{Constraint, Layout, Rect},
    text::{Line, Span},
    widgets::{
        Block, BorderType, Borders, Clear, Paragraph, Scrollbar, ScrollbarOrientation,
        ScrollbarState,
    },
};

use crate::app::ColumnModalState;
use crate::tui::theme;

pub fn render(
    frame: &mut Frame,
    area: Rect,
    modal: &ColumnModalState,
    has_value_filter: impl Fn(&str) -> bool,
) {
    let modal_width = (area.width * 60 / 100).clamp(35, 75).min(area.width);
    let max_height = area.height.saturating_sub(4).max(8);
    let needed_height = (modal.columns.len() as u16 + 5).min(max_height);
    let modal_height = needed_height.clamp(8, 22).min(area.height);

    let popup_area = Rect {
        x: area.x + (area.width.saturating_sub(modal_width)) / 2,
        y: area.y + (area.height.saturating_sub(modal_height)) / 2,
        width: modal_width,
        height: modal_height,
    };

    frame.render_widget(Clear, popup_area);

    let num_selected = modal.selected_columns.len();
    let num_total = modal.columns.len();
    let title = format!(
        " Visible Columns ({} of {} visible) ",
        num_selected, num_total
    );

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(theme::border_style(true));
    let inner = block.inner(popup_area);
    frame.render_widget(block, popup_area);

    if inner.height < 2 {
        return;
    }

    let chunks = Layout::vertical([Constraint::Min(0), Constraint::Length(1)]).split(inner);

    let list_area = chunks[0];
    let hint_area = chunks[1];

    let visible_count = list_area.height as usize;
    let mut lines = Vec::new();

    if modal.columns.is_empty() {
        lines.push(Line::from(Span::styled(
            "No columns found",
            theme::muted_style(),
        )));
    } else {
        for (idx, col) in modal
            .columns
            .iter()
            .enumerate()
            .skip(modal.scroll)
            .take(visible_count)
        {
            let is_checked = modal.selected_columns.contains(col);
            let check_mark = if is_checked { "[x] " } else { "[ ] " };
            let is_cursor = idx == modal.cursor;
            let filtered = has_value_filter(col);

            let prefix_span = Span::styled(
                check_mark,
                if is_cursor {
                    theme::selection_style()
                } else if is_checked {
                    theme::success_bold_style()
                } else {
                    theme::muted_style()
                },
            );

            let name_span = Span::styled(
                col.as_str(),
                if is_cursor {
                    theme::selection_style()
                } else if is_checked {
                    theme::text_style()
                } else {
                    theme::muted_style()
                },
            );

            let mut line_spans = vec![prefix_span, name_span];
            if filtered {
                line_spans.push(Span::styled(
                    " [F]",
                    if is_cursor {
                        theme::selection_style()
                    } else {
                        theme::success_bold_style()
                    },
                ));
            }

            lines.push(Line::from(line_spans));
        }
    }

    let list_widget = Paragraph::new(lines);
    frame.render_widget(list_widget, list_area);

    if modal.columns.len() > visible_count && visible_count > 0 {
        let mut scrollbar_state =
            ScrollbarState::new(modal.columns.len().saturating_sub(visible_count))
                .position(modal.scroll);
        let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .begin_symbol(Some("▲"))
            .end_symbol(Some("▼"));
        frame.render_stateful_widget(scrollbar, list_area, &mut scrollbar_state);
    }

    let hint = Line::from(vec![
        Span::styled("Space", theme::warning_bold_style()),
        Span::styled(":toggle  ", theme::muted_style()),
        Span::styled("a", theme::warning_bold_style()),
        Span::styled(":all/none  ", theme::muted_style()),
        Span::styled("Enter", theme::warning_bold_style()),
        Span::styled(":apply  ", theme::muted_style()),
        Span::styled("Esc", theme::warning_bold_style()),
        Span::styled(":cancel  ", theme::muted_style()),
        Span::styled("j/k", theme::warning_bold_style()),
        Span::styled(":move", theme::muted_style()),
    ]);
    frame.render_widget(Paragraph::new(hint), hint_area);
}
