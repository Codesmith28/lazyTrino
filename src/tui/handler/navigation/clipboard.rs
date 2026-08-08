use crate::app::{ACTIONS, ActivePanel, App, Screen};
use crate::tui::handler::{export, mouse, query};

use super::list::filter_items;

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
                    if s.selected == 6 {
                        text_to_copy = app.partition_tree_lines.join("\n");
                    } else if s.selected == 7 {
                        text_to_copy = app
                            .vertical_schema_cols
                            .iter()
                            .map(|col| {
                                format!(
                                    "{}\t{}\t{}\t{}",
                                    col.name, col.data_type, col.key_meta, col.description
                                )
                            })
                            .collect::<Vec<_>>()
                            .join("\n");
                    } else {
                        export::copy_results_to_clipboard(app);
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

pub fn toast_summary(text: &str) -> String {
    let line_count = text.lines().count();
    if line_count > 1 {
        format!("{line_count} lines")
    } else {
        text.chars().take(40).collect()
    }
}
