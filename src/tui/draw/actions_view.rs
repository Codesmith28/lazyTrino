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
    layout::{Alignment, Rect},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Paragraph},
};

use crate::app::{App, Screen};
use crate::tui::screens;
use crate::tui::theme;

#[allow(clippy::too_many_arguments)]
pub fn render_default_results_preview(
    frame: &mut Frame,
    preview_pane_area: Rect,
    app: &App,
    is_loading: bool,
    spin: String,
    table_name: &str,
    selected_idx: usize,
    preview_is_active: bool,
) {
    if let Screen::Actions(state) = &app.screen {
        if let Some(ref res_state) = state.results {
            screens::results::render(
                frame,
                preview_pane_area,
                res_state,
                spin,
                preview_is_active,
                app,
            );
        } else if is_loading {
            let title = format!(
                " Preview — {} ({}) ",
                table_name,
                crate::app::ACTIONS[selected_idx].1
            );
            let block = Block::default()
                .title(title)
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(theme::border_style(preview_is_active));
            let inner = block.inner(preview_pane_area);
            frame.render_widget(block, preview_pane_area);
            let spin_text = Paragraph::new(Line::from(vec![
                Span::styled(format!(" [{spin}] "), theme::warning_bold_style()),
                Span::styled("EXECUTING TRINO QUERY...", theme::info_bold_style()),
            ]))
            .alignment(Alignment::Center);
            frame.render_widget(spin_text, inner);
        } else {
            render_placeholder_preview(
                frame,
                preview_pane_area,
                table_name,
                selected_idx,
                preview_is_active,
            );
        }
    }
}

pub fn render_placeholder_preview(
    frame: &mut Frame,
    area: Rect,
    table_name: &str,
    selected_idx: usize,
    preview_is_active: bool,
) {
    let action_name = if selected_idx < crate::app::ACTIONS.len() {
        crate::app::ACTIONS[selected_idx].1
    } else {
        ""
    };
    let title = format!(" Preview — {table_name} ({action_name}) ");
    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(theme::border_style(preview_is_active));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let info_lines = vec![
        Line::from(Span::styled(
            " Table Preview Area",
            theme::info_bold_style(),
        )),
        Line::from(""),
        Line::from(Span::styled(
            format!(
                " Active Selection: [{}] {action_name}",
                crate::app::ACTIONS[selected_idx].0
            ),
            theme::warning_bold_style(),
        )),
        Line::from(""),
        Line::from(Span::styled(
            " Press Enter (or hit hotkey) to load and display preview output.",
            theme::text_style(),
        )),
        Line::from(""),
        Line::from(Span::styled(" Menu Shortcuts:", theme::muted_style())),
        Line::from("   [v] Table View Mode       [c] Table DDL            [i] Info Schema"),
        Line::from("   [s] Show Stats            [n] Row Count            [p] Sample (20 rows)"),
        Line::from("   [P] Partition Tree        [S] Vertical Schema"),
    ];
    let info_p = Paragraph::new(info_lines).alignment(Alignment::Center);
    frame.render_widget(info_p, inner);
}
