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

use crate::app::{ACTIONS, ActivePanel, App, Screen};
use crate::tui::handler::{mouse, query};

use super::list::filter_items;

pub fn format_csv_cell(val: &str) -> String {
    if val.contains(',') || val.contains('"') || val.contains('\n') || val.contains('\r') {
        format!("\"{}\"", val.replace('"', "\"\""))
    } else {
        val.to_string()
    }
}

pub fn format_csv_row<'a>(cells: impl IntoIterator<Item = &'a str>) -> String {
    let cells: Vec<&str> = cells.into_iter().collect();
    if cells.len() <= 1 {
        cells.first().copied().unwrap_or("").to_string()
    } else {
        cells
            .into_iter()
            .map(format_csv_cell)
            .collect::<Vec<_>>()
            .join(",")
    }
}

pub fn copy_active_pane_content(app: &mut App) {
    let mut text_to_copy = String::new();

    if let (Some(anchor), Some(current)) = (app.mouse_selection_anchor, app.mouse_selection_current)
    {
        text_to_copy = mouse::extract_selected_text(app, anchor, current);
    }

    if text_to_copy.is_empty() {
        match &app.screen {
            Screen::Catalog(s) => {
                text_to_copy = filter_items(&s.items, &app.search_query)
                    .iter()
                    .map(|x| x.trim())
                    .collect::<Vec<_>>()
                    .join("\n");
            }
            Screen::Schema(s) => {
                text_to_copy = filter_items(&s.items, &app.search_query)
                    .iter()
                    .map(|x| x.trim())
                    .collect::<Vec<_>>()
                    .join("\n");
            }
            Screen::Table(s) => {
                text_to_copy = filter_items(&s.items, &app.search_query)
                    .iter()
                    .map(|x| x.trim())
                    .collect::<Vec<_>>()
                    .join("\n");
            }
            Screen::Actions(s) => {
                if app.active_panel == ActivePanel::MenuPane {
                    text_to_copy = ACTIONS
                        .iter()
                        .map(|(_, l, _)| l.to_string())
                        .collect::<Vec<_>>()
                        .join("\n");
                } else if app.active_panel == ActivePanel::MainViewer {
                    if matches!(
                        ACTIONS.get(s.selected).map(|(_, _, a)| a),
                        Some(crate::app::Action::Partitions)
                    ) {
                        text_to_copy = app.partition_tree_lines.join("\n");
                    } else if matches!(
                        ACTIONS.get(s.selected).map(|(_, _, a)| a),
                        Some(crate::app::Action::Schema)
                    ) {
                        text_to_copy = app
                            .vertical_schema_cols
                            .iter()
                            .map(|col| {
                                format_csv_row([
                                    col.name.as_str(),
                                    col.data_type.as_str(),
                                    col.is_nullable.as_str(),
                                    col.key_meta.as_str(),
                                    col.description.as_str(),
                                ])
                            })
                            .collect::<Vec<_>>()
                            .join("\n");
                    } else {
                        copy_results_selection_or_cell(app);
                        return;
                    }
                }
            }
            _ => {}
        }
    }

    if !text_to_copy.is_empty() {
        query::copy_to_clipboard(&text_to_copy);
        app.copied_toast = Some((toast_summary(&text_to_copy), std::time::Instant::now()));
    }
}

pub fn copy_results_selection_or_cell(app: &mut App) {
    let Screen::Actions(ref mut s) = app.screen else {
        return;
    };
    let Some(ref mut res) = s.results else {
        app.copied_toast = Some((
            "No query results to copy".to_string(),
            std::time::Instant::now(),
        ));
        return;
    };

    match res.visual_mode {
        crate::app::VisualMode::Line { anchor_row } => {
            let min_r = anchor_row.min(res.selected_row);
            let max_r = anchor_row
                .max(res.selected_row)
                .min(res.rows.len().saturating_sub(1));
            let vis_cols = res.visible_columns();
            let mut lines = Vec::new();
            for r in min_r..=max_r {
                if let Some(row) = res.rows.get(r) {
                    let line = format_csv_row(
                        vis_cols
                            .iter()
                            .map(|(c, _)| row.get(*c).map(|s| s.as_str()).unwrap_or("")),
                    );
                    lines.push(line);
                }
            }
            let text = lines.join("\n");
            query::copy_to_clipboard(&text);
            let count = max_r.saturating_sub(min_r) + 1;
            app.copied_toast = Some((
                format!("Copied {count} selected row(s) (CSV)"),
                std::time::Instant::now(),
            ));
            res.exit_visual_mode();
        }
        crate::app::VisualMode::Block {
            anchor_row,
            anchor_col,
        } => {
            let min_r = anchor_row.min(res.selected_row);
            let max_r = anchor_row
                .max(res.selected_row)
                .min(res.rows.len().saturating_sub(1));
            let min_c = anchor_col.min(res.selected_col);
            let max_c = anchor_col.max(res.selected_col);
            let vis_cols: Vec<_> = res
                .visible_columns()
                .into_iter()
                .filter(|(c, _)| *c >= min_c && *c <= max_c)
                .collect();
            let mut lines = Vec::new();
            for r in min_r..=max_r {
                if let Some(row) = res.rows.get(r) {
                    let line = format_csv_row(
                        vis_cols
                            .iter()
                            .map(|(c, _)| row.get(*c).map(|s| s.as_str()).unwrap_or("")),
                    );
                    lines.push(line);
                }
            }
            let text = lines.join("\n");
            query::copy_to_clipboard(&text);
            let r_count = max_r.saturating_sub(min_r) + 1;
            let c_count = vis_cols.len();
            app.copied_toast = Some((
                format!("Copied {r_count}x{c_count} cell block (CSV)"),
                std::time::Instant::now(),
            ));
            res.exit_visual_mode();
        }
        crate::app::VisualMode::None => {
            let cell_val = res
                .rows
                .get(res.selected_row)
                .and_then(|r| r.get(res.selected_col))
                .cloned()
                .unwrap_or_default();
            query::copy_to_clipboard(&cell_val);
            let summary = if cell_val.len() > 30 {
                format!("{}...", &cell_val[..27])
            } else {
                cell_val
            };
            app.copied_toast = Some((
                format!("Copied cell: {summary}"),
                std::time::Instant::now(),
            ));
        }
    }
}

pub fn yank_whole_row(app: &mut App) {
    let Screen::Actions(ref mut s) = app.screen else {
        return;
    };
    let Some(ref mut res) = s.results else {
        app.copied_toast = Some((
            "No query results to copy".to_string(),
            std::time::Instant::now(),
        ));
        return;
    };

    let (min_r, max_r) = match res.visual_mode {
        crate::app::VisualMode::Line { anchor_row }
        | crate::app::VisualMode::Block { anchor_row, .. } => {
            (anchor_row.min(res.selected_row), anchor_row.max(res.selected_row))
        }
        crate::app::VisualMode::None => (res.selected_row, res.selected_row),
    };
    let max_r = max_r.min(res.rows.len().saturating_sub(1));
    let vis_cols = res.visible_columns();
    let mut lines = Vec::new();
    for r in min_r..=max_r {
        if let Some(row) = res.rows.get(r) {
            let line = format_csv_row(
                vis_cols
                    .iter()
                    .map(|(c, _)| row.get(*c).map(|s| s.as_str()).unwrap_or("")),
            );
            lines.push(line);
        }
    }
    let text = lines.join("\n");
    query::copy_to_clipboard(&text);
    let count = max_r.saturating_sub(min_r) + 1;
    app.copied_toast = Some((
        format!("Copied {count} visible row(s) (CSV)"),
        std::time::Instant::now(),
    ));
    res.exit_visual_mode();
}

pub fn toast_summary(text: &str) -> String {
    let line_count = text.lines().count();
    if line_count > 1 {
        format!("{line_count} lines")
    } else {
        text.chars().take(40).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_csv_row_single_col_is_raw() {
        assert_eq!(format_csv_row(["simple"]), "simple");
        // Even if single cell has a comma or quotes, single column stays raw as is
        assert_eq!(format_csv_row(["has, comma"]), "has, comma");
        assert_eq!(format_csv_row(["\"quotes\""]), "\"quotes\"");
    }

    #[test]
    fn test_format_csv_row_multi_col_escapes_commas_and_quotes() {
        assert_eq!(format_csv_row(["col1", "col2"]), "col1,col2");
        assert_eq!(format_csv_row(["hello, world", "normal"]), "\"hello, world\",normal");
        assert_eq!(format_csv_row(["he said \"hi\"", "bye"]), "\"he said \"\"hi\"\"\",bye");
        assert_eq!(format_csv_row(["line1\nline2", "ok"]), "\"line1\nline2\",ok");
    }
}
