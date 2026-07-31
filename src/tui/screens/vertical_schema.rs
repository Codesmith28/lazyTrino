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
    layout::{Constraint, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, BorderType, Borders, Cell, Row, Table},
    Frame,
};

use crate::app::VerticalColumn;

pub fn render(frame: &mut Frame, area: Rect, columns: &[VerticalColumn], table_name: &str, scroll: usize, is_active: bool) {
    let title = if table_name.is_empty() {
        " Schema (Vertical Table Format) ".to_string()
    } else {
        format!(" Schema — {table_name} ")
    };

    let border_color = if is_active { Color::Yellow } else { Color::DarkGray };

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(border_color));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    if columns.is_empty() {
        return;
    }

    let header_cells = vec![
        Cell::from(" # ").style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        Cell::from("Column Name").style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        Cell::from("Data Type").style(Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
        Cell::from("Key / Partition").style(Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD)),
        Cell::from("Description").style(Style::default().fg(Color::Gray).add_modifier(Modifier::BOLD)),
    ];
    let header = Row::new(header_cells).bottom_margin(1);

    let col_w1 = (inner.width * 25 / 100).max(10) as usize;
    let col_w2 = (inner.width * 20 / 100).max(10) as usize;
    let col_w3 = (inner.width * 18 / 100).max(8) as usize;
    let col_w4 = (inner.width * 33 / 100).max(12) as usize;

    let rows: Vec<Row> = columns
        .iter()
        .skip(scroll)
        .map(|col| {
            let num = format!("{:>2}", col.index);
            let name_lines = crate::tui::screens::results::wrap_text(&col.name, col_w1);
            let dtype_lines = crate::tui::screens::results::wrap_text(&col.data_type, col_w2);
            let key_str = if col.key_meta.is_empty() { "-" } else { &col.key_meta };
            let key_lines = crate::tui::screens::results::wrap_text(key_str, col_w3);
            let desc_str = if col.description.is_empty() { "-" } else { &col.description };
            let desc_lines = crate::tui::screens::results::wrap_text(desc_str, col_w4);

            let max_h = name_lines.len().max(dtype_lines.len()).max(key_lines.len()).max(desc_lines.len()) as u16;

            let cells = vec![
                Cell::from(num).style(Style::default().fg(Color::DarkGray)),
                Cell::from(name_lines.join("\n")).style(Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
                Cell::from(dtype_lines.join("\n")).style(Style::default().fg(Color::Green)),
                Cell::from(key_lines.join("\n")).style(Style::default().fg(Color::Magenta)),
                Cell::from(desc_lines.join("\n")).style(Style::default().fg(Color::Gray)),
            ];
            Row::new(cells).height(max_h)
        })
        .collect();

    let widths = [
        Constraint::Length(4),
        Constraint::Percentage(25),
        Constraint::Percentage(20),
        Constraint::Percentage(18),
        Constraint::Percentage(33),
    ];

    let table = Table::new(rows, widths)
        .header(header)
        .column_spacing(1);

    frame.render_widget(table, inner);

    use ratatui::widgets::{Scrollbar, ScrollbarOrientation, ScrollbarState};
    let mut scroll_state = ScrollbarState::new(columns.len().saturating_sub(1)).position(scroll);
    frame.render_stateful_widget(
        Scrollbar::default()
            .orientation(ScrollbarOrientation::VerticalRight)
            .begin_symbol(Some("▲"))
            .end_symbol(Some("▼")),
        inner,
        &mut scroll_state,
    );
}
