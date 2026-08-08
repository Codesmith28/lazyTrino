use ratatui::{
    Frame,
    layout::Rect,
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, List, ListItem, ListState},
};

use crate::tui::theme;

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

pub fn parse_show_create_to_tree_lines(ddl: &str, override_location: Option<&str>) -> Vec<String> {
    let mut lines = Vec::new();
    let (partition_cols, location) = parse_partitioned_by(ddl);
    let loc = override_location
        .filter(|l| !l.is_empty())
        .map(|l| if l.ends_with('/') { l.to_string() } else { format!("{l}/") })
        .unwrap_or(location);
    lines.push(format!(" {loc}"));

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
            let col_lower = col.to_lowercase();
            let val_placeholder = if col_lower.contains("year") {
                "<YYYY>"
            } else if col_lower.contains("month") {
                "<MM>"
            } else if col_lower.contains("day")
                || col_lower.contains("date")
                || col_lower.contains("dt")
            {
                "<YYYY-MM-DD>"
            } else if col_lower.contains("service") {
                "<service_name>"
            } else if col_lower.contains("account") {
                "<account_id>"
            } else {
                "<value>"
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

    #[test]
    fn parse_show_create_to_tree_lines_parses_partition_hierarchy() {
        let ddl = "CREATE TABLE iceberg.demo.lineitem_partitioned (\n   orderkey bigint\n)\nWITH (\n   location = 's3://warehouse/demo/lineitem/',\n   partitioning = ARRAY['year(shipdate)','shipmode']\n)";
        let lines = parse_show_create_to_tree_lines(ddl, None);
        assert_eq!(lines[0], " s3://warehouse/demo/lineitem/");
        assert!(lines.iter().any(|l| l.contains("year(shipdate)=<YYYY>/")));
        assert!(lines.iter().any(|l| l.contains("shipmode=<value>/")));
        assert!(lines.iter().any(|l| l.contains("data_files.parquet")));
    }

    #[test]
    fn parse_show_create_to_tree_lines_handles_unpartitioned_table() {
        let ddl = "CREATE TABLE iceberg.demo.lookup (\n   id bigint\n)\nWITH (\n   location = 's3://warehouse/demo/lookup/'\n)";
        let lines = parse_show_create_to_tree_lines(ddl, None);
        assert_eq!(lines[0], " s3://warehouse/demo/lookup/");
        assert!(lines.iter().any(|l| l.contains("(Non-partitioned Table)")));
    }
}
