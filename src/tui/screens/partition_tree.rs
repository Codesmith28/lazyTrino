use ratatui::{
    Frame,
    layout::Rect,
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, List, ListItem, ListState},
};

use crate::tui::theme;

#[derive(Default)]
struct TreeNode {
    name: String,
    children: Vec<TreeNode>,
}

impl TreeNode {
    fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            children: Vec::new(),
        }
    }

    fn insert_path(&mut self, segments: &[&str]) {
        if segments.is_empty() {
            return;
        }
        let head = segments[0];
        let tail = &segments[1..];

        if let Some(child) = self.children.iter_mut().find(|c| c.name == head) {
            child.insert_path(tail);
        } else {
            let mut child = TreeNode::new(head);
            child.insert_path(tail);
            self.children.push(child);
        }
    }
}

fn format_tree_node(
    node: &TreeNode,
    prefix: &str,
    is_last: bool,
    depth: usize,
    lines: &mut Vec<String>,
) {
    if depth > 0 {
        let branch = if is_last { "└── " } else { "├── " };
        let level_tag = format!("  (Level {})", depth);
        lines.push(format!("{prefix}{branch}{}/{level_tag}", node.name));
    }

    let child_prefix = if depth == 0 {
        " ".to_string()
    } else if is_last {
        format!("{prefix}    ")
    } else {
        format!("{prefix}│   ")
    };

    if node.children.is_empty() && depth > 0 {
        lines.push(format!(
            "{child_prefix}├── .hoodie/                (Apache Hudi Metadata)"
        ));
        lines.push(format!(
            "{child_prefix}└── data_files.parquet       (Apache Parquet Data Files)"
        ));
    } else {
        let total_children = node.children.len();
        for (i, child) in node.children.iter().enumerate() {
            let child_is_last = i == total_children - 1;
            format_tree_node(child, &child_prefix, child_is_last, depth + 1, lines);
        }
    }
}

/// Parses a `SHOW CREATE TABLE` DDL string into the ordered list of
/// partition column names (as declared in `partitioned_by`/`partitioning`)
/// and the table's storage location. This is the single source of truth
/// for whether/how a given table is partitioned — it is derived purely
/// from the live DDL response for that specific table, never hardcoded by
/// table/schema name. Returns an empty `Vec` for `partition_cols` when the
/// table is unpartitioned (or the clause is absent).
pub fn parse_partitioned_by(ddl: &str) -> (Vec<String>, String) {
    let mut location = "s3://warehouse/table_data/".to_string();
    let mut partition_cols: Vec<String> = Vec::new();

    for line in ddl.lines() {
        let trimmed = line.trim();
        if (trimmed.contains("location =") || trimmed.contains("external_location ="))
            && let Some(start) = trimmed.find('\'')
            && let Some(end) = trimmed[start + 1..].find('\'')
        {
            location = trimmed[start + 1..start + 1 + end].to_string();
        }
        if (trimmed.contains("partitioned_by =") || trimmed.contains("partitioning ="))
            && let Some(start) = trimmed.find("ARRAY[")
            && let Some(end) = trimmed[start..].find(']')
        {
            let arr_str = &trimmed[start + 6..start + end];
            partition_cols = arr_str
                .split(',')
                .map(|s| s.trim().trim_matches('\'').trim_matches('"').to_string())
                .filter(|s| !s.is_empty())
                .collect();
        }
    }

    if !location.ends_with('/') {
        location.push('/');
    }

    (partition_cols, location)
}

pub fn parse_show_create_to_tree_lines(ddl: &str) -> Vec<String> {
    let mut lines = Vec::new();
    let (partition_cols, location) = parse_partitioned_by(ddl);
    lines.push(format!(" {location}"));

    if partition_cols.is_empty() {
        lines.push(" └── (Non-partitioned Table)".to_string());
        lines.push("     ├── .hoodie/                (Apache Hudi Metadata)".to_string());
        lines.push("     └── data_files.parquet       (Apache Parquet Data Files)".to_string());
    } else {
        let total = partition_cols.len();
        let mut prefix = " ".to_string();
        for (depth, col) in partition_cols.iter().enumerate() {
            let is_last = depth == total - 1;
            let branch = if is_last { "└── " } else { "├── " };
            let val_placeholder = match col.to_lowercase().as_str() {
                "date" | "dt" | "day" => "<YYYY-MM-DD>",
                "service" | "service_name" => "<service_name>",
                "account" | "account_id" | "accountid" => "<account_id>",
                _ => "<value>",
            };
            let level_tag = format!("  (Partition Level {})", depth + 1);
            lines.push(format!(
                "{prefix}{branch}{col}={val_placeholder}/{level_tag}"
            ));
            if is_last {
                prefix.push_str("    ");
            } else {
                prefix.push_str("│   ");
            }
        }

        lines.push(format!(
            "{prefix}├── .hoodie/                (Apache Hudi Metadata)"
        ));
        lines.push(format!(
            "{prefix}└── data_files.parquet       (Apache Parquet Data Files)"
        ));
    }

    lines
}

pub fn build_tree_lines(raw_partitions: &[String]) -> Vec<String> {
    if raw_partitions.is_empty() {
        return vec![" (No partitions found)".to_string()];
    }

    if raw_partitions.len() == 1
        && (raw_partitions[0].contains("CREATE TABLE") || raw_partitions[0].contains("WITH ("))
    {
        return parse_show_create_to_tree_lines(&raw_partitions[0]);
    }

    let mut lines = Vec::new();
    let mut base_prefix = "s3://warehouse/table_data/".to_string();

    if let Some(first) = raw_partitions.first()
        && (first.starts_with("s3://") || first.starts_with("hdfs://") || first.starts_with('/'))
    {
        let parts: Vec<&str> = first.split('/').collect();
        if parts.len() > 3 {
            base_prefix = parts[..parts.len().saturating_sub(2)].join("/") + "/";
        }
    }

    lines.push(format!(" {base_prefix}"));

    let mut root = TreeNode::new("root");
    for p_str in raw_partitions.iter().take(20) {
        let clean = p_str.trim_matches('/');
        let segments: Vec<&str> = clean.split('/').filter(|s| !s.is_empty()).collect();
        if !segments.is_empty() {
            root.insert_path(&segments);
        }
    }

    format_tree_node(&root, "", true, 0, &mut lines);

    lines
}

fn is_tree_char(c: char) -> bool {
    c == '│' || c == '├' || c == '─' || c == '└' || c == ' '
}

fn split_tree_line(line: &str) -> (String, String) {
    let mut split_idx = 0;
    for (i, c) in line.char_indices() {
        if is_tree_char(c) {
            split_idx = i + c.len_utf8();
        } else {
            break;
        }
    }
    let branch_part = line[..split_idx].to_string();
    let content_part = line[split_idx..].to_string();
    (branch_part, content_part)
}

pub fn render(
    frame: &mut Frame,
    area: Rect,
    raw_partitions: &[String],
    table_name: &str,
    scroll: usize,
    is_active: bool,
    app: &crate::app::App,
) {
    let title = if table_name.is_empty() {
        " Partitions (Tree View) ".to_string()
    } else {
        format!(" Partitions — {table_name} ")
    };

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(theme::border_style(is_active));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let tree_lines = raw_partitions;

    let items: Vec<ListItem> = tree_lines
        .iter()
        .skip(scroll)
        .take(inner.height as usize)
        .enumerate()
        .map(|(idx, line_str)| {
            let line_y = inner.y + idx as u16;
            // See actions.rs for why this must be gated by pane focus.
            let is_mouse_sel =
                is_active && app.is_area_mouse_selected(inner.x, inner.width, line_y);

            let (branch_part, content_part) = split_tree_line(line_str);

            let content_style = if is_mouse_sel {
                theme::selection_style()
            } else if content_part.contains("s3://")
                || content_part.contains("hdfs://")
                || content_part.starts_with('/')
            {
                theme::warning_bold_style()
            } else if content_part.contains("Level") {
                theme::info_style()
            } else if content_part.contains(".hoodie") || content_part.contains(".parquet") {
                theme::success_style()
            } else {
                theme::muted_style()
            };

            let line = Line::from(vec![
                Span::styled(branch_part, theme::text_style()),
                Span::styled(content_part, content_style),
            ]);

            ListItem::new(line)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_partitioned_by_extracts_ordered_columns_and_location() {
        let ddl = r#"
CREATE TABLE datalake."tenant".events (
   date varchar COMMENT '',
   service varchar COMMENT '',
   account_id varchar COMMENT ''
)
WITH (
   location = 's3://bucket/lakehouse/data/events',
   partitioned_by = ARRAY['date','service','account_id']
)
"#;
        let (cols, location) = parse_partitioned_by(ddl);
        assert_eq!(cols, vec!["date", "service", "account_id"]);
        assert_eq!(location, "s3://bucket/lakehouse/data/events/");
    }

    #[test]
    fn parse_partitioned_by_returns_empty_cols_for_unpartitioned_table() {
        let ddl = r#"
CREATE TABLE datalake."tenant".lookup (
   id varchar COMMENT ''
)
WITH (
   location = 's3://bucket/lakehouse/data/lookup'
)
"#;
        let (cols, location) = parse_partitioned_by(ddl);
        assert!(cols.is_empty());
        assert_eq!(location, "s3://bucket/lakehouse/data/lookup/");
    }

    #[test]
    fn parse_partitioned_by_defaults_location_when_absent() {
        let ddl = "CREATE TABLE t (id varchar)";
        let (cols, location) = parse_partitioned_by(ddl);
        assert!(cols.is_empty());
        assert_eq!(location, "s3://warehouse/table_data/");
    }
}
