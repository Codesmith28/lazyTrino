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
    layout::Rect,
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, List, ListItem, ListState},
};

use crate::{
    app::{ACTIONS, App},
    tui::theme,
};

#[allow(clippy::too_many_arguments)]
pub fn render(
    frame: &mut Frame,
    area: Rect,
    _catalog: &str,
    _schema: &str,
    table: &str,
    selected: usize,
    is_active: bool,
    app: &App,
) {
    let block = Block::default()
        .title(if area.width < 25 {
            " Menu ".to_string()
        } else {
            format!(" Menu — {table} ")
        })
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(theme::border_style(is_active));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let avail_w = (inner.width.saturating_sub(1) as usize).max(10);
    let items: Vec<ListItem> = ACTIONS
        .iter()
        .enumerate()
        .map(|(i, (key, label, _))| {
            let item_y = inner.y + i as u16;
            let is_mouse_sel =
                is_active && app.is_area_mouse_selected(inner.x, inner.width, item_y);

            let is_selected = i == selected;
            let raw_lines = wrap_action_item(*key, label, is_selected, avail_w);
            let styled_lines: Vec<Line> = raw_lines
                .into_iter()
                .map(|t| {
                    if is_mouse_sel {
                        Line::styled(t, theme::text_selection_style())
                    } else if is_selected {
                        Line::styled(t, theme::selection_style_for(is_active))
                    } else {
                        menu_item_line(&t)
                    }
                })
                .collect();
            ListItem::new(styled_lines)
        })
        .collect();

    let mut list_state = ListState::default().with_selected(Some(selected));
    let list = List::new(items).highlight_style(theme::selection_style_for(is_active));

    frame.render_stateful_widget(list, inner, &mut list_state);

    use ratatui::widgets::{Scrollbar, ScrollbarOrientation, ScrollbarState};
    if ACTIONS.len() > 1 {
        let mut scrollbar_state =
            ScrollbarState::new(ACTIONS.len().saturating_sub(1)).position(selected);
        frame.render_stateful_widget(
            Scrollbar::default()
                .orientation(ScrollbarOrientation::VerticalRight)
                .begin_symbol(Some("▲"))
                .end_symbol(Some("▼")),
            inner,
            &mut scrollbar_state,
        );
    }
}

/// ` [k] Label`: the hotkey in the key color, the label in plain text.
fn menu_item_line(text: &str) -> Line<'static> {
    match (text.find('['), text.find(']')) {
        (Some(open), Some(close))
            if open < close
                && text[..open]
                    .trim()
                    .trim_start_matches('▸')
                    .trim()
                    .is_empty() =>
        {
            Line::from(vec![
                Span::styled(text[..open].to_string(), theme::text_style()),
                Span::styled(text[open..=close].to_string(), theme::key_style()),
                Span::styled(text[close + 1..].to_string(), theme::text_style()),
            ])
        }
        _ => Line::styled(text.to_string(), theme::text_style()),
    }
}

pub fn wrap_action_item(
    key: char,
    label: &str,
    is_selected: bool,
    max_width: usize,
) -> Vec<String> {
    let prefix = if is_selected { "▸ " } else { "  " };
    let lead = format!("{prefix}[{key}] ");
    if max_width <= lead.len() + 3 {
        return vec![format!("{lead}{label}")];
    }
    let content_w = max_width.saturating_sub(lead.len());
    let words = crate::tui::screens::results::wrap_text(label, content_w);
    let mut lines = Vec::new();
    for (i, part) in words.into_iter().enumerate() {
        if i == 0 {
            lines.push(format!("{lead}{part}"));
        } else {
            lines.push(format!("{:indent$}{part}", "", indent = lead.len()));
        }
    }
    if lines.is_empty() {
        vec![format!("{lead}{label}")]
    } else {
        lines
    }
}

pub fn item_line_counts(width: u16) -> Vec<usize> {
    let avail_w = (width.saturating_sub(1) as usize).max(10);
    ACTIONS
        .iter()
        .map(|(key, label, _)| wrap_action_item(*key, label, false, avail_w).len())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wrap_action_item_fits_on_one_line() {
        let lines = wrap_action_item('v', "Table View Mode", false, 40);
        assert_eq!(lines, vec!["  [v] Table View Mode"]);
    }

    #[test]
    fn test_wrap_action_item_wraps_when_narrow() {
        let lines = wrap_action_item('m', "Sample Mode (20 rows)", true, 16);
        assert!(lines.len() >= 2);
        assert!(lines[0].starts_with("▸ [m] "));
        assert!(lines[1].starts_with("      "));
    }
}
