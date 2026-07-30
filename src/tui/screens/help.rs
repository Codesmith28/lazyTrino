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
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};

pub fn render(frame: &mut Frame, area: Rect) {
    let block = Block::default()
        .title(" Help ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow));
    frame.render_widget(&block, area);

    let inner = block.inner(area);

    let help_text = vec![
        Line::from(Span::styled(" Navigation", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD))),
        Line::from("  j / ↓         Move down"),
        Line::from("  k / ↑         Move up"),
        Line::from("  h / ← / Esc   Go back / parent"),
        Line::from("  l / → / Enter Select / drill in"),
        Line::from("  g             Jump to first"),
        Line::from("  G             Jump to last"),
        Line::from("  N + Enter     Jump to item N"),
        Line::from(""),
        Line::from(Span::styled(" Actions", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD))),
        Line::from("  <space>       Enter leader mode"),
        Line::from("  <space> d     Describe table"),
        Line::from("  <space> c     Show CREATE"),
        Line::from("  <space> s     Show STATS"),
        Line::from("  <space> p     Preview (LIMIT 10)"),
        Line::from("  <space> f     Files ($files)"),
        Line::from("  <space> P     Partitions ($partitions)"),
        Line::from("  <space> S     Snapshots ($snapshots)"),
        Line::from(""),
        Line::from(Span::styled(" General", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD))),
        Line::from("  /             Search (tables)"),
        Line::from("  ?             Toggle this help"),
        Line::from("  q             Quit"),
        Line::from(""),
        Line::from(Span::styled(" Results View", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD))),
        Line::from("  j/k           Scroll vertically"),
        Line::from("  h/l           Scroll horizontally"),
        Line::from("  g/G           Top / bottom"),
        Line::from("  Esc / h       Back to actions"),
        Line::from(""),
        Line::from(Span::styled(" Press Esc or q to close help", Style::default().fg(Color::DarkGray))),
    ];

    let paragraph = Paragraph::new(help_text).wrap(Wrap { trim: false });
    frame.render_widget(paragraph, inner);
}
