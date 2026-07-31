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
    widgets::{Block, BorderType, Borders, Paragraph, Wrap},
};

use crate::{app::ConnectState, tui::theme};

pub fn render(frame: &mut Frame, area: Rect, state: &ConnectState, spinner: String) {
    let block = Block::default()
        .title(" lazyTrino — Trino Browser ")
        .title_alignment(Alignment::Center)
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(theme::style(theme::ACCENT_BORDER));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let chunks = ratatui::layout::Layout::vertical([
        ratatui::layout::Constraint::Length(3),
        ratatui::layout::Constraint::Length(3),
        ratatui::layout::Constraint::Length(3),
        ratatui::layout::Constraint::Length(3),
        ratatui::layout::Constraint::Length(3),
        ratatui::layout::Constraint::Min(0),
    ])
    .split(inner);

    let title = Paragraph::new("Connect to Trino")
        .style(theme::header_style())
        .alignment(Alignment::Center);
    frame.render_widget(title, chunks[0]);

    let url_field = Paragraph::new(Line::from(vec![
        Span::styled("URL: ", theme::warning_bold_style()),
        Span::styled(&state.url, theme::input_field_style(state.focused == 0)),
    ]));
    frame.render_widget(url_field, chunks[1]);

    let user_field = Paragraph::new(Line::from(vec![
        Span::styled("User: ", theme::warning_bold_style()),
        Span::styled(&state.user, theme::input_field_style(state.focused == 1)),
    ]));
    frame.render_widget(user_field, chunks[2]);

    let pass_display: String = state.password.chars().map(|_| '*').collect();
    let pass_field = Paragraph::new(Line::from(vec![
        Span::styled("Pass: ", theme::warning_bold_style()),
        Span::styled(pass_display, theme::input_field_style(state.focused == 2)),
    ]));
    frame.render_widget(pass_field, chunks[3]);

    if state.loading {
        let loading = Paragraph::new(format!("{} Connecting...", spinner))
            .style(theme::info_style())
            .alignment(Alignment::Center);
        frame.render_widget(loading, chunks[4]);
    } else {
        let connect_btn = Paragraph::new(Line::from(vec![
            Span::styled(" [ ", theme::muted_style()),
            Span::styled("Connect", theme::header_style()),
            Span::styled(" ] ", theme::muted_style()),
        ]))
        .alignment(Alignment::Center);
        frame.render_widget(connect_btn, chunks[4]);
    }

    if let Some(ref err) = state.error {
        let error = Paragraph::new(err.as_str())
            .style(theme::error_style())
            .wrap(Wrap { trim: true });
        frame.render_widget(error, chunks[5]);
    }
}
