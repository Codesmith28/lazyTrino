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
        Cell::from("Key / Partition").style(theme::bold_style(theme::DETAIL_FG)),
        Cell::from("Description").style(theme::bold_style(theme::SECONDARY_FG)),
    ];
    let header = Row::new(header_cells).bottom_margin(1);

    let col_w1 = (inner.width * 25 / 100).max(10) as usize;
    let col_w2 = (inner.width * 20 / 100).max(10) as usize;
    let col_w3 = (inner.width * 18 / 100).max(8) as usize;
    let col_w4 = (inner.width * 33 / 100).max(12) as usize;

    let mut current_row_y = inner.y + 2; // header line + bottom margin 1
    let rows: Vec<Row> = columns
        .iter()
        .skip(scroll)
        .map(|col| {
            let num = format!("{:>2}", col.index);
            let name_lines = crate::tui::screens::results::wrap_text(&col.name, col_w1);
            let dtype_lines = crate::tui::screens::results::wrap_text(&col.data_type, col_w2);
            let key_str = if col.key_meta.is_empty() {
                "-"
            } else {
                &col.key_meta
            };
            let key_lines = crate::tui::screens::results::wrap_text(key_str, col_w3);
            let desc_str = if col.description.is_empty() {
                "-"
            } else {
                &col.description
            };
            let desc_lines = crate::tui::screens::results::wrap_text(desc_str, col_w4);

            let max_h = name_lines
                .len()
                .max(dtype_lines.len())
                .max(key_lines.len())
                .max(desc_lines.len()) as u16;

            let is_mouse_sel = app.is_area_mouse_selected(inner.x, inner.width, current_row_y);
            current_row_y += max_h;

            let row_style = if is_mouse_sel {
                theme::selection_style()
            } else {
                theme::text_style()
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
        Constraint::Percentage(25),
        Constraint::Percentage(20),
        Constraint::Percentage(18),
        Constraint::Percentage(33),
    ];

    let table = Table::new(rows, widths).header(header).column_spacing(1);

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
