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
    layout::{Constraint, Rect},
    widgets::{Block, BorderType, Borders, Cell, Row, Table},
};

use crate::{app::VerticalColumn, tui::theme};

pub fn render(
    frame: &mut Frame,
    area: Rect,
    columns: &[VerticalColumn],
    table_name: &str,
    scroll: usize,
    is_active: bool,
    app: &crate::app::App,
) {
    let title = if table_name.is_empty() {
        " Schema (Vertical Table Format) ".to_string()
    } else {
        format!(" Schema — {table_name} ")
    };

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(theme::border_style(is_active));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    if columns.is_empty() {
        return;
    }

    let header_cells = vec![
        Cell::from(" # ").style(theme::warning_bold_style()),
        Cell::from("Column Name").style(theme::info_bold_style()),
        Cell::from("Data Type").style(theme::header_style()),
        Cell::from("Nullable").style(theme::bold_style(theme::detail_color())),
        Cell::from("Key / Partition").style(theme::bold_style(theme::detail_color())),
        Cell::from("Description").style(theme::bold_style(theme::secondary_color())),
    ];
    let header = Row::new(header_cells).bottom_margin(1);

    let col_w1 = (inner.width * 24 / 100).max(10) as usize;
    let col_w2 = (inner.width * 18 / 100).max(10) as usize;
    let col_w4 = (inner.width * 18 / 100).max(8) as usize;
    let col_w5 = (inner.width * 30 / 100).max(12) as usize;

    let mut current_row_y = inner.y + 2; // header line + bottom margin 1
    let rows: Vec<Row> = columns
        .iter()
        .skip(scroll)
        .map(|col| {
            let num = format!("{:>2}", col.index);
            let name_lines = crate::tui::screens::results::wrap_text(&col.name, col_w1);
            let dtype_lines = crate::tui::screens::results::wrap_text(&col.data_type, col_w2);
            let nullable_str = if col.is_nullable.is_empty() {
                "-"
            } else {
                &col.is_nullable
            };
            let key_str = if col.key_meta.is_empty() {
                "-"
            } else {
                &col.key_meta
            };
            let key_lines = crate::tui::screens::results::wrap_text(key_str, col_w4);
            let desc_str = if col.description.is_empty() {
                "-"
            } else {
                &col.description
            };
            let desc_lines = crate::tui::screens::results::wrap_text(desc_str, col_w5);

            let max_h = name_lines
                .len()
                .max(dtype_lines.len())
                .max(key_lines.len())
                .max(desc_lines.len()) as u16;

            // See actions.rs for why this must be gated by pane focus.
            let is_mouse_sel =
                is_active && app.is_area_mouse_selected(inner.x, inner.width, current_row_y);
            current_row_y += max_h;

            let row_style = if is_mouse_sel {
                theme::text_selection_style()
            } else {
                theme::text_style()
            };

            let nullable_style = if is_mouse_sel {
                row_style
            } else if col.is_nullable.eq_ignore_ascii_case("no") {
                theme::warning_bold_style()
            } else {
                theme::secondary_style()
            };

            let cells = vec![
                Cell::from(num).style(if is_mouse_sel {
                    row_style
                } else {
                    theme::muted_style()
                }),
                Cell::from(name_lines.join("\n")).style(if is_mouse_sel {
                    row_style
                } else {
                    theme::bold_text_style()
                }),
                Cell::from(dtype_lines.join("\n")).style(if is_mouse_sel {
                    row_style
                } else {
                    theme::success_style()
                }),
                Cell::from(nullable_str).style(nullable_style),
                Cell::from(key_lines.join("\n")).style(if is_mouse_sel {
                    row_style
                } else {
                    theme::detail_style()
                }),
                Cell::from(desc_lines.join("\n")).style(if is_mouse_sel {
                    row_style
                } else {
                    theme::secondary_style()
                }),
            ];
            Row::new(cells).height(max_h)
        })
        .collect();

    let widths = [
        Constraint::Length(4),
        Constraint::Percentage(24),
        Constraint::Percentage(18),
        Constraint::Length(10),
        Constraint::Percentage(18),
        Constraint::Percentage(30),
    ];

    let table = Table::new(rows, widths).header(header).column_spacing(1);

    frame.render_widget(table, inner);

    use ratatui::widgets::{ScrollbarOrientation, ScrollbarState};
    let mut scroll_state = ScrollbarState::new(columns.len().saturating_sub(1)).position(scroll);
    frame.render_stateful_widget(
        crate::tui::theme::scrollbar()
            .orientation(ScrollbarOrientation::VerticalRight)
            .begin_symbol(Some("▲"))
            .end_symbol(Some("▼")),
        inner,
        &mut scroll_state,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{Terminal, backend::TestBackend, layout::Rect};

    #[test]
    fn test_vertical_schema_renders_nullable_column() {
        let cols = vec![
            VerticalColumn {
                index: 1,
                name: "id".to_string(),
                data_type: "bigint".to_string(),
                is_nullable: "NO".to_string(),
                key_meta: "PK".to_string(),
                description: "Primary key identifier".to_string(),
            },
            VerticalColumn {
                index: 2,
                name: "email".to_string(),
                data_type: "varchar".to_string(),
                is_nullable: "YES".to_string(),
                key_meta: String::new(),
                description: "Customer email".to_string(),
            },
        ];

        let backend = TestBackend::new(120, 20);
        let mut terminal = Terminal::new(backend).unwrap();
        let app = crate::app::App::new(crate::config::ConnectionConfig::default(), false);

        terminal
            .draw(|f| {
                render(f, Rect::new(0, 0, 120, 20), &cols, "users", 0, true, &app);
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

        assert!(
            content.contains("Nullable"),
            "Header should contain 'Nullable': {content}"
        );
        assert!(
            content.contains("NO"),
            "Should display 'NO' for id column: {content}"
        );
        assert!(
            content.contains("YES"),
            "Should display 'YES' for email column: {content}"
        );
    }
}
