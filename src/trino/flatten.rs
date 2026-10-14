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

use std::collections::{BTreeMap, HashSet};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum NestedType {
    Json,
    Map,
    Row,
}

impl NestedType {
    pub fn label(&self) -> &'static str {
        match self {
            NestedType::Json => "JSON",
            NestedType::Map => "MAP",
            NestedType::Row => "ROW",
        }
    }
}

/// Identifies if a column name / data type / sample values represent a nested field container.
pub fn detect_nested_type(
    _col_name: &str,
    data_type: &str,
    sample_values: &[String],
) -> Option<NestedType> {
    let dt = data_type.trim().to_ascii_lowercase();
    if dt.contains("json") {
        return Some(NestedType::Json);
    }
    if dt.starts_with("map(") {
        return Some(NestedType::Map);
    }
    if dt.starts_with("row(") {
        return Some(NestedType::Row);
    }

    // Heuristic inspection on sample values if data type is varchar or unspecified
    let non_empty: Vec<&str> = sample_values
        .iter()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty() && !s.eq_ignore_ascii_case("null"))
        .take(10)
        .collect();

    if non_empty.is_empty() {
        return None;
    }

    // Check if non-empty samples parse as JSON Objects
    let all_json_objects = non_empty.iter().all(|s| {
        if s.starts_with('{') && s.ends_with('}') {
            serde_json::from_str::<serde_json::Value>(s)
                .map(|v| v.is_object())
                .unwrap_or(false)
        } else {
            false
        }
    });
    if all_json_objects {
        return Some(NestedType::Json);
    }

    // Check if non-empty samples match Trino map format: {key1=val1, key2=val2}
    let all_trino_maps = non_empty.iter().all(|s| {
        if s.starts_with('{') && s.ends_with('}') {
            let inner = &s[1..s.len() - 1];
            inner.contains('=') && !inner.contains(':')
        } else {
            false
        }
    });
    if all_trino_maps {
        return Some(NestedType::Map);
    }

    None
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FlattenedField {
    pub parent_col: String,
    pub path: Vec<String>,
    pub full_name: String,
    pub display_name: String,
    pub nested_type: NestedType,
    pub expr_sql: String,
}

impl FlattenedField {
    pub fn new(parent_col: &str, path: &[String], nested_type: NestedType) -> Self {
        let display_name = path.join(".");
        let full_name = format!("{parent_col}.{display_name}");
        let expr_sql = build_sql_expression(parent_col, path, nested_type);

        Self {
            parent_col: parent_col.to_string(),
            path: path.to_vec(),
            full_name,
            display_name,
            nested_type,
            expr_sql,
        }
    }
}

fn quote_identifier(id: &str) -> String {
    format!("\"{}\"", id.trim().replace('"', "\"\""))
}

/// Builds the Trino SQL projection/predicate expression for a nested field.
pub fn build_sql_expression(parent_col: &str, path: &[String], nested_type: NestedType) -> String {
    let quoted_parent = quote_identifier(parent_col);
    match nested_type {
        NestedType::Json => {
            let json_path = format!("$.{}", path.join("."));
            format!("json_extract_scalar({quoted_parent}, '{json_path}')")
        }
        NestedType::Map => {
            let key = path.first().map(|s| s.as_str()).unwrap_or("");
            let escaped_key = key.replace('\'', "''");
            format!("{quoted_parent}['{escaped_key}']")
        }
        NestedType::Row => {
            let sub_path = path
                .iter()
                .map(|p| quote_identifier(p))
                .collect::<Vec<_>>()
                .join(".");
            format!("{quoted_parent}.{sub_path}")
        }
    }
}

/// Extracts all nested paths discovered from sample values of a column.
pub fn discover_nested_paths(
    values: &[String],
    nested_type: NestedType,
) -> Vec<Vec<String>> {
    let mut discovered: BTreeMap<String, Vec<String>> = BTreeMap::new();

    for raw in values {
        let raw = raw.trim();
        if raw.is_empty() || raw.eq_ignore_ascii_case("null") {
            continue;
        }

        match nested_type {
            NestedType::Json => {
                if let Ok(v) = serde_json::from_str::<serde_json::Value>(raw) {
                    collect_json_paths(&v, Vec::new(), &mut discovered);
                }
            }
            NestedType::Map => {
                // Could be JSON map or Trino {k=v} map
                if let Ok(serde_json::Value::Object(map)) = serde_json::from_str::<serde_json::Value>(raw) {
                    for key in map.keys() {
                        let path = vec![key.clone()];
                        discovered.entry(key.clone()).or_insert(path);
                    }
                } else if raw.starts_with('{') && raw.ends_with('}') {
                    let inner = &raw[1..raw.len() - 1];
                    for pair in parse_trino_map_pairs(inner) {
                        let path = vec![pair.0.clone()];
                        discovered.entry(pair.0).or_insert(path);
                    }
                }
            }
            NestedType::Row => {
                if let Ok(serde_json::Value::Object(map)) = serde_json::from_str::<serde_json::Value>(raw) {
                    collect_json_paths(&serde_json::Value::Object(map), Vec::new(), &mut discovered);
                } else if raw.starts_with('{') && raw.ends_with('}') {
                    let inner = &raw[1..raw.len() - 1];
                    for pair in parse_trino_map_pairs(inner) {
                        let path = vec![pair.0.clone()];
                        discovered.entry(pair.0).or_insert(path);
                    }
                }
            }
        }
    }

    discovered.into_values().collect()
}

fn collect_json_paths(
    val: &serde_json::Value,
    current_path: Vec<String>,
    discovered: &mut BTreeMap<String, Vec<String>>,
) {
    if let serde_json::Value::Object(map) = val {
        for (k, child) in map {
            let mut next_path = current_path.clone();
            next_path.push(k.clone());
            let full_key = next_path.join(".");
            discovered.entry(full_key).or_insert_with(|| next_path.clone());

            // Recurse into nested objects up to depth 3
            if next_path.len() < 3 && child.is_object() {
                collect_json_paths(child, next_path, discovered);
            }
        }
    }
}

fn parse_trino_map_pairs(inner: &str) -> Vec<(String, String)> {
    let mut pairs = Vec::new();
    let mut current_key = String::new();
    let mut current_val = String::new();
    let mut in_value = false;
    let mut paren_depth: usize = 0;
    let mut in_quotes = false;

    for ch in inner.chars() {
        match ch {
            '\'' | '"' => {
                in_quotes = !in_quotes;
                if in_value {
                    current_val.push(ch);
                } else {
                    current_key.push(ch);
                }
            }
            '(' | '[' | '{' if !in_quotes => {
                paren_depth += 1;
                if in_value {
                    current_val.push(ch);
                } else {
                    current_key.push(ch);
                }
            }
            ')' | ']' | '}' if !in_quotes => {
                paren_depth = paren_depth.saturating_sub(1);
                if in_value {
                    current_val.push(ch);
                } else {
                    current_key.push(ch);
                }
            }
            '=' if !in_quotes && paren_depth == 0 && !in_value => {
                in_value = true;
            }
            ',' if !in_quotes && paren_depth == 0 => {
                let k = current_key.trim().trim_matches('\'').trim_matches('"').to_string();
                let v = current_val.trim().to_string();
                if !k.is_empty() {
                    pairs.push((k, v));
                }
                current_key.clear();
                current_val.clear();
                in_value = false;
            }
            _ => {
                if in_value {
                    current_val.push(ch);
                } else {
                    current_key.push(ch);
                }
            }
        }
    }

    let k = current_key.trim().trim_matches('\'').trim_matches('"').to_string();
    let v = current_val.trim().to_string();
    if !k.is_empty() {
        pairs.push((k, v));
    }

    pairs
}

/// In-memory extractor for nested values from raw parent cell content.
pub fn extract_nested_value(raw: &str, path: &[String], nested_type: NestedType) -> String {
    let raw = raw.trim();
    if raw.is_empty() || raw.eq_ignore_ascii_case("null") {
        return String::new();
    }

    match nested_type {
        NestedType::Json | NestedType::Row => {
            if let Ok(mut cur) = serde_json::from_str::<serde_json::Value>(raw) {
                for segment in path {
                    match cur {
                        serde_json::Value::Object(mut map) => {
                            if let Some(next) = map.remove(segment) {
                                cur = next;
                            } else {
                                return String::new();
                            }
                        }
                        _ => return String::new(),
                    }
                }
                format_json_leaf_value(&cur)
            } else if nested_type == NestedType::Row && raw.starts_with('{') && raw.ends_with('}') {
                extract_trino_map_value(raw, path)
            } else {
                String::new()
            }
        }
        NestedType::Map => {
            if let Ok(serde_json::Value::Object(map)) =
                serde_json::from_str::<serde_json::Value>(raw)
                && let Some(key) = path.first()
                && let Some(val) = map.get(key)
            {
                return format_json_leaf_value(val);
            }
            extract_trino_map_value(raw, path)
        }
    }
}

fn extract_trino_map_value(raw: &str, path: &[String]) -> String {
    if !raw.starts_with('{') || !raw.ends_with('}') {
        return String::new();
    }
    let inner = &raw[1..raw.len() - 1];
    let key = match path.first() {
        Some(k) => k.as_str(),
        None => return String::new(),
    };

    for (k, v) in parse_trino_map_pairs(inner) {
        if k.eq_ignore_ascii_case(key) {
            return v;
        }
    }
    String::new()
}

fn format_json_leaf_value(val: &serde_json::Value) -> String {
    match val {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Number(n) => n.to_string(),
        serde_json::Value::Bool(b) => b.to_string(),
        serde_json::Value::Null => "null".to_string(),
        serde_json::Value::Array(_) | serde_json::Value::Object(_) => val.to_string(),
    }
}

/// A node in a column tree view for modals.
#[derive(Clone, Debug, PartialEq)]
pub struct ColumnTreeNode {
    pub name: String,
    pub label: String,
    pub depth: usize,
    pub is_parent: bool,
    pub is_expanded: bool,
    pub children: Vec<String>,
}

/// Builds a structured tree representation from root columns and flattened fields.
pub fn build_column_tree(
    root_columns: &[String],
    flattened_fields: &[FlattenedField],
) -> Vec<ColumnTreeNode> {
    let mut nodes = Vec::new();

    // Map parent_col -> list of child fields
    let mut field_map: BTreeMap<String, Vec<&FlattenedField>> = BTreeMap::new();
    for f in flattened_fields {
        field_map.entry(f.parent_col.clone()).or_default().push(f);
    }

    for root in root_columns {
        let children = field_map.get(root);
        let has_children = children.is_some_and(|c| !c.is_empty());

        let all_child_names: Vec<String> = children
            .map(|c| c.iter().map(|f| f.full_name.clone()).collect())
            .unwrap_or_default();

        nodes.push(ColumnTreeNode {
            name: root.clone(),
            label: root.clone(),
            depth: 0,
            is_parent: has_children,
            is_expanded: true,
            children: all_child_names,
        });

        if let Some(child_fields) = children {
            for child in child_fields {
                nodes.push(ColumnTreeNode {
                    name: child.full_name.clone(),
                    label: child.display_name.clone(),
                    depth: 1,
                    is_parent: false,
                    is_expanded: false,
                    children: Vec::new(),
                });
            }
        }
    }

    nodes
}

/// Toggles a node in the tree and updates selection with atomic parent-child cascade.
/// Selecting a parent selects all its children.
/// Deselecting a parent deselects all its children.
pub fn toggle_tree_node_selection(
    selected_set: &mut HashSet<String>,
    tree: &[ColumnTreeNode],
    node_idx: usize,
) {
    let Some(node) = tree.get(node_idx) else {
        return;
    };

    if node.is_parent {
        let is_selected = selected_set.contains(&node.name)
            || node.children.iter().all(|c| selected_set.contains(c));

        if is_selected {
            // Deselect parent and all children
            selected_set.remove(&node.name);
            for child in &node.children {
                selected_set.remove(child);
            }
        } else {
            // Select parent and all children
            selected_set.insert(node.name.clone());
            for child in &node.children {
                selected_set.insert(child.clone());
            }
        }
    } else {
        // Child node
        let is_selected = selected_set.contains(&node.name);
        if is_selected {
            selected_set.remove(&node.name);
            // Check if parent should be deselected
            for parent in tree.iter().filter(|n| n.is_parent && n.children.contains(&node.name)) {
                if !parent.children.iter().any(|c| selected_set.contains(c)) {
                    selected_set.remove(&parent.name);
                }
            }
        } else {
            selected_set.insert(node.name.clone());
            // Check if parent should be selected
            for parent in tree.iter().filter(|n| n.is_parent && n.children.contains(&node.name)) {
                selected_set.insert(parent.name.clone());
            }
        }
    }
}

/// Bulk select / deselect all items in the tree atomically, preventing race conditions.
pub fn toggle_all_tree_selection(
    selected_set: &mut HashSet<String>,
    tree: &[ColumnTreeNode],
    all_columns: &[String],
    cursor_node_idx: Option<usize>,
) {
    let all_selected = all_columns.iter().all(|c| selected_set.contains(c));
    if all_selected {
        selected_set.clear();
        if let Some(idx) = cursor_node_idx
            && let Some(node) = tree.get(idx)
        {
            selected_set.insert(node.name.clone());
            for child in &node.children {
                selected_set.insert(child.clone());
            }
            for parent in tree.iter().filter(|n| n.is_parent && n.children.contains(&node.name)) {
                selected_set.insert(parent.name.clone());
            }
        }
    } else {
        *selected_set = all_columns.iter().cloned().collect();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_nested_type() {
        assert_eq!(
            detect_nested_type("col", "json", &[]),
            Some(NestedType::Json)
        );
        assert_eq!(
            detect_nested_type("col", "map(varchar, varchar)", &[]),
            Some(NestedType::Map)
        );
        assert_eq!(
            detect_nested_type("col", "row(a int, b varchar)", &[]),
            Some(NestedType::Row)
        );

        let json_samples = vec![
            r#"{"user": "alice", "age": 30}"#.to_string(),
            r#"{"user": "bob", "age": 25}"#.to_string(),
        ];
        assert_eq!(
            detect_nested_type("data", "varchar", &json_samples),
            Some(NestedType::Json)
        );

        let map_samples = vec![
            "{k1=v1, k2=v2}".to_string(),
            "{k1=v3, k2=v4}".to_string(),
        ];
        assert_eq!(
            detect_nested_type("properties", "varchar", &map_samples),
            Some(NestedType::Map)
        );
    }

    #[test]
    fn test_discover_json_paths_and_extract() {
        let samples = vec![
            r#"{"user": {"name": "Alice", "id": 1}, "status": "active"}"#.to_string(),
            r#"{"user": {"name": "Bob", "role": "admin"}, "status": "pending"}"#.to_string(),
        ];

        let paths = discover_nested_paths(&samples, NestedType::Json);
        let path_strings: Vec<String> = paths.iter().map(|p| p.join(".")).collect();
        assert!(path_strings.contains(&"user.name".to_string()));
        assert!(path_strings.contains(&"user.id".to_string()));
        assert!(path_strings.contains(&"user.role".to_string()));
        assert!(path_strings.contains(&"status".to_string()));

        // Extraction
        let val1 = extract_nested_value(
            &samples[0],
            &["user".to_string(), "name".to_string()],
            NestedType::Json,
        );
        assert_eq!(val1, "Alice");

        let val2 = extract_nested_value(
            &samples[0],
            &["user".to_string(), "id".to_string()],
            NestedType::Json,
        );
        assert_eq!(val2, "1");

        let val3 = extract_nested_value(
            &samples[1],
            &["user".to_string(), "role".to_string()],
            NestedType::Json,
        );
        assert_eq!(val3, "admin");
    }

    #[test]
    fn test_discover_map_paths_and_extract() {
        let samples = vec![
            "{env=prod, region=us-east-1}".to_string(),
            "{env=staging, region=eu-west-1, tier=gold}".to_string(),
        ];

        let paths = discover_nested_paths(&samples, NestedType::Map);
        let path_strings: Vec<String> = paths.iter().map(|p| p.join(".")).collect();
        assert!(path_strings.contains(&"env".to_string()));
        assert!(path_strings.contains(&"region".to_string()));
        assert!(path_strings.contains(&"tier".to_string()));

        // Extraction
        let env_val = extract_nested_value(&samples[0], &["env".to_string()], NestedType::Map);
        assert_eq!(env_val, "prod");
        let tier_val = extract_nested_value(&samples[1], &["tier".to_string()], NestedType::Map);
        assert_eq!(tier_val, "gold");
    }

    #[test]
    fn test_sql_expression_builders() {
        let json_expr = build_sql_expression(
            "metadata",
            &["user".to_string(), "name".to_string()],
            NestedType::Json,
        );
        assert_eq!(
            json_expr,
            "json_extract_scalar(\"metadata\", '$.user.name')"
        );

        let map_expr = build_sql_expression(
            "attributes",
            &["cluster_id".to_string()],
            NestedType::Map,
        );
        assert_eq!(map_expr, "\"attributes\"['cluster_id']");
    }

    #[test]
    fn test_tree_cascading_selection() {
        let root_columns = vec!["id".to_string(), "metadata".to_string()];
        let fields = vec![
            FlattenedField::new("metadata", &["user".to_string()], NestedType::Json),
            FlattenedField::new("metadata", &["status".to_string()], NestedType::Json),
        ];

        let tree = build_column_tree(&root_columns, &fields);
        assert_eq!(tree.len(), 4); // id, metadata, metadata.user, metadata.status

        let mut selected: HashSet<String> = HashSet::new();

        // 1. Select parent "metadata" (node_idx 1) -> selects metadata and both children
        toggle_tree_node_selection(&mut selected, &tree, 1);
        assert!(selected.contains("metadata"));
        assert!(selected.contains("metadata.user"));
        assert!(selected.contains("metadata.status"));

        // 2. Deselect parent "metadata" -> deselects metadata and both children
        toggle_tree_node_selection(&mut selected, &tree, 1);
        assert!(!selected.contains("metadata"));
        assert!(!selected.contains("metadata.user"));
        assert!(!selected.contains("metadata.status"));

        // 3. Select one child "metadata.user" (node_idx 2)
        toggle_tree_node_selection(&mut selected, &tree, 2);
        assert!(selected.contains("metadata.user"));
        assert!(selected.contains("metadata")); // parent is also marked

        // 4. Test atomic toggle_all_tree_selection
        let all_cols = vec![
            "id".to_string(),
            "metadata".to_string(),
            "metadata.user".to_string(),
            "metadata.status".to_string(),
        ];
        toggle_all_tree_selection(&mut selected, &tree, &all_cols, Some(0));
        assert_eq!(selected.len(), 4);

        // Toggle all again when all are selected -> keeps cursor (0: "id")
        toggle_all_tree_selection(&mut selected, &tree, &all_cols, Some(0));
        assert_eq!(selected.len(), 1);
        assert!(selected.contains("id"));
    }
}
