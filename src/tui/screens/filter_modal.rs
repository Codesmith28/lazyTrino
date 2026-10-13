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

use crate::app::FilterModalState;
use crate::tui::theme;

pub fn render(frame: &mut Frame, area: Rect, modal: &FilterModalState) {
    let modal_width = (area.width * 60 / 100).clamp(35, 75).min(area.width);
    let max_height = area.height.saturating_sub(4).max(8);
    let needed_height = (modal.values.len() as u16 + 5).min(max_height);
    let modal_height = needed_height.clamp(8, 22).min(area.height);

    let popup_area = Rect {
        x: area.x + (area.width.saturating_sub(modal_width)) / 2,
        y: area.y + (area.height.saturating_sub(modal_height)) / 2,
        width: modal_width,
        height: modal_height,
    };

    frame.render_widget(Clear, popup_area);

    let num_selected = modal.selected_values.len();
    let num_total = modal.values.len();
    let title = format!(
        " Filter: {} ({} of {} selected) ",
        modal.column_name, num_selected, num_total
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

    if modal.values.is_empty() {
        lines.push(Line::from(Span::styled(
            "No values found in column",
            theme::muted_style(),
        )));
    } else {
        for (idx, val) in modal
            .values
            .iter()
            .enumerate()
            .skip(modal.scroll)
            .take(visible_count)
        {
            let is_checked = modal.selected_values.contains(val);
            let check_mark = if is_checked { "[x] " } else { "[ ] " };
            let is_cursor = idx == modal.cursor;

            let display_val = if val.is_empty() {
                "<empty>"
            } else {
                val.as_str()
            };

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

            let text_span = Span::styled(
                display_val,
                if is_cursor {
                    theme::selection_style()
                } else {
                    theme::text_style()
                },
            );

            let line = Line::from(vec![prefix_span, text_span]);
            lines.push(line);
        }
    }

    let paragraph = Paragraph::new(lines);
    frame.render_widget(paragraph, list_area);

    if modal.values.len() > visible_count {
        let mut scrollbar_state =
            ScrollbarState::new(modal.values.len().saturating_sub(1)).position(modal.cursor);
        frame.render_stateful_widget(
            Scrollbar::default()
                .orientation(ScrollbarOrientation::VerticalRight)
                .begin_symbol(Some("▲"))
                .end_symbol(Some("▼")),
            list_area,
            &mut scrollbar_state,
        );
    }

    let hint = Line::from(Span::styled(
        "Space:toggle • a:all/none • Enter:apply • Esc:cancel",
        theme::muted_style(),
    ));
    frame.render_widget(Paragraph::new(hint), hint_area);
}
