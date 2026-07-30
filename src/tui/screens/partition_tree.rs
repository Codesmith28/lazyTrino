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
    widgets::{Block, BorderType, Borders, List, ListItem, ListState},
    Frame,
};

pub fn build_tree_lines(raw_partitions: &[String]) -> Vec<String> {
    if raw_partitions.is_empty() {
        return vec![" (No partitions found)".to_string()];
    }

    let mut lines = Vec::new();
    let mut base_prefix = "s3://warehouse/table_data/".to_string();

    if let Some(first) = raw_partitions.first() {
        if first.starts_with("s3://") || first.starts_with("hdfs://") || first.starts_with('/') {
            let parts: Vec<&str> = first.split('/').collect();
            if parts.len() > 3 {
                base_prefix = parts[..parts.len().saturating_sub(2)].join("/") + "/";
            }
        }
    }

    lines.push(format!(" {}", base_prefix));

    for (p_idx, p_str) in raw_partitions.iter().take(20).enumerate() {
        let clean = p_str.trim_matches('/');
        let segments: Vec<&str> = clean.split('/').collect();
        let is_last_partition = p_idx == raw_partitions.len().min(20) - 1;

        for (depth, seg) in segments.iter().enumerate() {
            let indent = "    ".repeat(depth + 1);
            let branch = if depth == segments.len() - 1 && is_last_partition {
                "└── "
            } else if depth == 0 {
                "├── "
            } else {
                "└── "
            };

            let level_tag = format!("  (Partition Level {})", depth + 1);
            lines.push(format!("{indent}{branch}{seg}/{level_tag}"));
        }

        let file_indent = "    ".repeat(segments.len() + 1);
        lines.push(format!("{file_indent}├── .hoodie/                 (Apache Hudi Metadata)"));
        lines.push(format!("{file_indent}└── data_files.parquet       (Apache Parquet Data Files)"));
        lines.push("    ──────".to_string());
    }

    lines
}

pub fn render(frame: &mut Frame, area: Rect, raw_partitions: &[String], table_name: &str, scroll: usize) {
    let title = if table_name.is_empty() {
        " Partitions (Tree View) ".to_string()
    } else {
        format!(" Partitions — {table_name} ")
    };

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::Cyan));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let tree_lines = build_tree_lines(raw_partitions);

    let items: Vec<ListItem> = tree_lines
        .iter()
        .skip(scroll)
        .take(inner.height as usize)
        .map(|line_str| {
            let style = if line_str.contains("s3://") || line_str.contains("hdfs://") {
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
            } else if line_str.contains("Partition Level") {
                Style::default().fg(Color::Cyan)
            } else if line_str.contains(".hoodie") || line_str.contains(".parquet") {
                Style::default().fg(Color::Green)
            } else {
                Style::default().fg(Color::DarkGray)
            };

            ListItem::new(Line::from(Span::styled(line_str.clone(), style)))
        })
        .collect();

    let mut state = ListState::default();
    let list = List::new(items);
    frame.render_stateful_widget(list, inner, &mut state);

    use ratatui::widgets::{Scrollbar, ScrollbarOrientation, ScrollbarState};
    let mut scroll_state = ScrollbarState::new(tree_lines.len().saturating_sub(1)).position(scroll);
    frame.render_stateful_widget(
        Scrollbar::default()
            .orientation(ScrollbarOrientation::VerticalRight)
            .begin_symbol(Some("▲"))
            .end_symbol(Some("▼")),
        inner,
        &mut scroll_state,
    );
}
