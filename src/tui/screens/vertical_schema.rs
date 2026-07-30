use ratatui::{
    layout::{Constraint, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, BorderType, Borders, Cell, Row, Table},
    Frame,
};

use crate::app::VerticalColumn;

pub fn render(frame: &mut Frame, area: Rect, columns: &[VerticalColumn], table_name: &str, scroll: usize) {
    let title = if table_name.is_empty() {
        " Schema (Vertical Table Format) ".to_string()
    } else {
        format!(" Schema — {table_name} ")
    };

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::Cyan));

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

    let rows: Vec<Row> = columns
        .iter()
        .skip(scroll)
        .enumerate()
        .map(|(idx, col)| {
            let num = format!("{:>2}", scroll + idx + 1);
            let name = col.name.clone();
            let dtype = col.data_type.clone();
            let key = if col.key_meta.is_empty() {
                "-".to_string()
            } else {
                col.key_meta.clone()
            };
            let desc = if col.description.is_empty() {
                "-".to_string()
            } else {
                col.description.clone()
            };

            let cells = vec![
                Cell::from(num).style(Style::default().fg(Color::DarkGray)),
                Cell::from(name).style(Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
                Cell::from(dtype).style(Style::default().fg(Color::Green)),
                Cell::from(key).style(Style::default().fg(Color::Magenta)),
                Cell::from(desc).style(Style::default().fg(Color::Gray)),
            ];
            Row::new(cells)
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
