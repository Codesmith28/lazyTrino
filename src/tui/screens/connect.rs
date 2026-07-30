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
    layout::{Alignment, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Paragraph, Wrap},
    Frame,
};

use crate::app::ConnectState;

pub fn render(frame: &mut Frame, area: Rect, state: &ConnectState, spinner: String) {
    let block = Block::default()
        .title(" lazyTrino — Trino Browser ")
        .title_alignment(Alignment::Center)
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::Cyan));

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
        .style(Style::default().fg(Color::Green).add_modifier(Modifier::BOLD))
        .alignment(Alignment::Center);
    frame.render_widget(title, chunks[0]);

    let url_style = if state.focused == 0 {
        Style::default().fg(Color::White).bg(Color::DarkGray)
    } else {
        Style::default().fg(Color::Gray)
    };
    let url_field = Paragraph::new(Line::from(vec![
        Span::styled("URL: ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        Span::styled(&state.url, url_style),
    ]));
    frame.render_widget(url_field, chunks[1]);

    let user_style = if state.focused == 1 {
        Style::default().fg(Color::White).bg(Color::DarkGray)
    } else {
        Style::default().fg(Color::Gray)
    };
    let user_field = Paragraph::new(Line::from(vec![
        Span::styled("User: ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        Span::styled(&state.user, user_style),
    ]));
    frame.render_widget(user_field, chunks[2]);

    let pass_display: String = state.password.chars().map(|_| '*').collect();
    let pass_style = if state.focused == 2 {
        Style::default().fg(Color::White).bg(Color::DarkGray)
    } else {
        Style::default().fg(Color::Gray)
    };
    let pass_field = Paragraph::new(Line::from(vec![
        Span::styled("Pass: ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        Span::styled(pass_display, pass_style),
    ]));
    frame.render_widget(pass_field, chunks[3]);

    if state.loading {
        let loading = Paragraph::new(format!("{} Connecting...", spinner))
            .style(Style::default().fg(Color::Cyan))
            .alignment(Alignment::Center);
        frame.render_widget(loading, chunks[4]);
    } else {
        let connect_btn = Paragraph::new(Line::from(vec![
            Span::styled(" [ ", Style::default().fg(Color::DarkGray)),
            Span::styled("Connect", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
            Span::styled(" ] ", Style::default().fg(Color::DarkGray)),
        ]))
        .alignment(Alignment::Center);
        frame.render_widget(connect_btn, chunks[4]);
    }

    if let Some(ref err) = state.error {
        let error = Paragraph::new(err.as_str())
            .style(Style::default().fg(Color::Red))
            .wrap(Wrap { trim: true });
        frame.render_widget(error, chunks[5]);
    }
}
