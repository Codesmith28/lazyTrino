use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, List, ListItem, ListState},
    Frame,
};

pub fn parse_show_create_to_tree_lines(ddl: &str) -> Vec<String> {
    let mut lines = Vec::new();
    let mut location = "s3://warehouse/table_data/".to_string();
    let mut partition_cols: Vec<String> = Vec::new();

    for line in ddl.lines() {
        let trimmed = line.trim();
        if trimmed.contains("location =") || trimmed.contains("external_location =") {
            if let Some(start) = trimmed.find('\'') {
                if let Some(end) = trimmed[start + 1..].find('\'') {
                    location = trimmed[start + 1..start + 1 + end].to_string();
                }
            }
        }
        if trimmed.contains("partitioned_by =") || trimmed.contains("partitioning =") {
            if let Some(start) = trimmed.find("ARRAY[") {
                if let Some(end) = trimmed[start..].find(']') {
                    let arr_str = &trimmed[start + 6..start + end];
                    partition_cols = arr_str
                        .split(',')
                        .map(|s| s.trim().trim_matches('\'').trim_matches('"').to_string())
                        .filter(|s| !s.is_empty())
                        .collect();
                }
            }
        }
    }

    if !location.ends_with('/') {
        location.push('/');
    }
    lines.push(format!(" {location}"));

    if partition_cols.is_empty() {
        lines.push(" └── (Non-partitioned Table)".to_string());
        lines.push("     ├── .hoodie/                (Apache Hudi Metadata)".to_string());
        lines.push("     └── data_files.parquet       (Apache Parquet Data Files)".to_string());
    } else {
        let total = partition_cols.len();
        for (depth, col) in partition_cols.iter().enumerate() {
            let indent = "    ".repeat(depth + 1);
            let branch = if depth == total - 1 { "└── " } else { "├── " };
            let val_placeholder = match col.to_lowercase().as_str() {
                "date" | "dt" | "day" => "<YYYY-MM-DD>",
                "service" | "service_name" => "<service_name>",
                "account" | "account_id" | "accountid" => "<account_id>",
                _ => "<value>",
            };
            let level_tag = format!("  (Partition Level {})", depth + 1);
            lines.push(format!("{indent}{branch}{col}={val_placeholder}/{level_tag}"));
        }

        let file_indent = "    ".repeat(total + 1);
        lines.push(format!("{file_indent}├── .hoodie/                (Apache Hudi Metadata)"));
        lines.push(format!("{file_indent}└── data_files.parquet       (Apache Parquet Data Files)"));
        lines.push("    ──────".to_string());
    }

    lines
}

pub fn build_tree_lines(raw_partitions: &[String]) -> Vec<String> {
    if raw_partitions.is_empty() {
        return vec![" (No partitions found)".to_string()];
    }

    if raw_partitions.len() == 1 && (raw_partitions[0].contains("CREATE TABLE") || raw_partitions[0].contains("WITH (")) {
        return parse_show_create_to_tree_lines(&raw_partitions[0]);
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
