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

use std::collections::{HashMap, HashSet};

use crate::app::ColumnFilter;

fn q(id: &str) -> String {
    format!("\"{}\"", id.trim().replace('"', "\"\""))
}

pub fn show_catalogs() -> String {
    "SHOW CATALOGS".to_string()
}

pub fn show_schemas(catalog: &str) -> String {
    format!("SHOW SCHEMAS FROM {}", q(catalog))
}

pub fn show_tables(catalog: &str, schema: &str) -> String {
    format!("SHOW TABLES FROM {}.{}", q(catalog), q(schema))
}

pub fn describe(catalog: &str, schema: &str, table: &str) -> String {
    format!("DESCRIBE {}.{}.{}", q(catalog), q(schema), q(table))
}

pub fn show_create(catalog: &str, schema: &str, table: &str) -> String {
    format!(
        "SHOW CREATE TABLE {}.{}.{}",
        q(catalog),
        q(schema),
        q(table)
    )
}

pub fn info_schema_columns(catalog: &str, schema: &str, table: &str) -> String {
    format!(
        "SELECT column_name, data_type, is_nullable, comment \
         FROM {}.information_schema.columns \
         WHERE table_schema = '{}' AND table_name = '{}' \
         ORDER BY ordinal_position",
        q(catalog),
        schema.trim().replace('\'', "''"),
        table.trim().replace('\'', "''"),
    )
}

pub fn show_stats(catalog: &str, schema: &str, table: &str) -> String {
    format!("SHOW STATS FOR {}.{}.{}", q(catalog), q(schema), q(table))
}

pub fn filtered_count(
    catalog: &str,
    schema: &str,
    table: &str,
    filters: &[(String, String)],
) -> String {
    let where_clause = build_where_clause(filters)
        .map(|c| format!(" WHERE {c}"))
        .unwrap_or_default();
    format!(
        "SELECT COUNT(*) AS total_records FROM {}.{}.{}{where_clause}",
        q(catalog),
        q(schema),
        q(table)
    )
}

pub fn count(catalog: &str, schema: &str, table: &str) -> String {
    filtered_count(catalog, schema, table, &[])
}

pub fn sample(catalog: &str, schema: &str, table: &str) -> String {
    format!(
        "SELECT * FROM {}.{}.{} LIMIT 20",
        q(catalog),
        q(schema),
        q(table)
    )
}

pub fn page_query(catalog: &str, schema: &str, table: &str, offset: usize, limit: usize) -> String {
    format!(
        "SELECT * FROM {}.{}.{} OFFSET {offset} LIMIT {limit}",
        q(catalog),
        q(schema),
        q(table)
    )
}

fn is_numeric_literal(val: &str) -> bool {
    let s = val.trim();
    if s.is_empty() {
        return false;
    }
    s.parse::<i64>().is_ok() || s.parse::<f64>().is_ok()
}

pub fn is_partition_column(partitioned_by: &[String], col_name: &str) -> bool {
    let col = col_name.trim().to_ascii_lowercase();
    if col.is_empty() {
        return false;
    }
    partitioned_by.iter().any(|p| {
        let p_trimmed = p.trim().to_ascii_lowercase();
        if p_trimmed == col {
            return true;
        }
        if let Some(open) = p_trimmed.find('(')
            && let Some(close) = p_trimmed[open + 1..].rfind(')')
        {
            let inside = &p_trimmed[open + 1..open + 1 + close];
            let first_arg = inside.split(',').next().unwrap_or(inside).trim();
            let clean_arg = first_arg.trim_matches('"').trim_matches('\'').trim();
            if clean_arg == col {
                return true;
            }
        }
        false
    })
}

pub(crate) fn format_predicate(col: &str, val: &str) -> String {
    let col = col.trim();
    let val = val.trim();
    if val.eq_ignore_ascii_case("null") {
        format!("{col} IS NULL")
    } else if val.eq_ignore_ascii_case("true") || val.eq_ignore_ascii_case("false") {
        format!("{col} = {}", val.to_lowercase())
    } else if (col.contains('(') || is_numeric_literal(val)) && is_numeric_literal(val) {
        // If col is an expression like year(shipdate), month(orderdate), bucket(N, x)
        // or a numeric value, format as numeric literal in SQL without quotes so Trino
        // does not fail with "Cannot apply operator: bigint = varchar(4)".
        format!("{col} = {val}")
    } else {
        format!("{} = '{}'", col, val.replace('\'', "''"))
    }
}

/// Builds a `WHERE` clause fragment (without the leading `WHERE`) from an
/// ordered list of `(column, value)` partition predicates, quoting/escaping
/// each value appropriately (e.g. numeric literals for expressions/numbers,
/// quoted string literals for varchars, IS NULL for nulls).
/// Returns `None` when there are no filters.
fn build_where_clause(filters: &[(String, String)]) -> Option<String> {
    if filters.is_empty() {
        return None;
    }
    Some(
        filters
            .iter()
            .map(|(col, val)| format_predicate(col, val))
            .collect::<Vec<_>>()
            .join(" AND "),
    )
}

/// `SELECT DISTINCT <column> FROM t [WHERE k1='v1' AND k2='v2'] ORDER BY <column> LIMIT <limit>`
///
/// Used to list the next level of values in a partition hierarchy (the
/// `ls` step of the cd/ls-style drill-down), scoped by the partition
/// predicates already fixed by the levels above it so Trino can prune
/// partitions instead of scanning the whole table.
pub fn distinct_partition_values(
    catalog: &str,
    schema: &str,
    table: &str,
    filters: &[(String, String)],
    column: &str,
    limit: usize,
) -> String {
    let where_clause = build_where_clause(filters)
        .map(|c| format!(" WHERE {c}"))
        .unwrap_or_default();
    format!(
        "SELECT DISTINCT {} FROM {}.{}.{}{where_clause} ORDER BY {} DESC LIMIT {limit}",
        column.trim(),
        q(catalog),
        q(schema),
        q(table),
        column.trim(),
    )
}

/// `SELECT * FROM t [WHERE k1='v1' AND k2='v2'] OFFSET <offset> LIMIT <limit>`
///
/// The leaf-level query in a partition drill-down: only run once every
/// partition column has been fixed by the levels above it, so Trino never
/// has to plan splits across the whole (potentially huge/broken) partition
/// tree at once.
///
/// `columns` lets the caller select an explicit column list instead of `*`
/// — used to skip columns whose Parquet-encoded type Trino can't safely
/// read (e.g. `map`/`row` columns hitting a schema mismatch), which would
/// otherwise fail the entire query with an "Unsupported ... Parquet
/// column" error. Pass an empty slice to fall back to `SELECT *`.
pub fn filtered_page_query(
    catalog: &str,
    schema: &str,
    table: &str,
    filters: &[(String, String)],
    offset: usize,
    limit: usize,
    columns: &[String],
) -> String {
    let where_clause = build_where_clause(filters)
        .map(|c| format!(" WHERE {c}"))
        .unwrap_or_default();
    let select_list = if columns.is_empty() {
        "*".to_string()
    } else {
        columns
            .iter()
            .map(|c| q(c.trim()))
            .collect::<Vec<_>>()
            .join(", ")
    };
    format!(
        "SELECT {select_list} FROM {}.{}.{}{where_clause} OFFSET {offset} LIMIT {limit}",
        q(catalog),
        q(schema),
        q(table),
    )
}

pub fn partitions(catalog: &str, schema: &str, table: &str) -> String {
    format!(
        "SELECT * FROM {}.{}.{}",
        q(catalog),
        q(schema),
        q(&format!("{table}$partitions"))
    )
}

pub fn resolve_column_sql_ref(col: &str) -> String {
    let col = col.trim();
    if col.starts_with('"') || col.contains('(') || col.contains('[') {
        col.to_string()
    } else if let Some((parent, child)) = col.split_once('.') {
        let path = child.split('.').collect::<Vec<_>>().join(".");
        format!("json_extract_scalar({}, '$.{}')", q(parent), path)
    } else {
        q(col)
    }
}

pub fn format_column_filter_predicate(col: &str, filter: &ColumnFilter) -> String {
    format_column_filter_predicate_with_overrides(col, filter, None)
}

pub fn format_column_filter_predicate_with_overrides(
    col: &str,
    filter: &ColumnFilter,
    expr_overrides: Option<&HashMap<String, String>>,
) -> String {
    let col_ref = if let Some(map) = expr_overrides && let Some(expr) = map.get(col) {
        expr.clone()
    } else {
        resolve_column_sql_ref(col)
    };

    match filter {
        ColumnFilter::Numeric(num) => {
            format!("{col_ref} {} {}", num.op.as_str(), num.raw_value)
        }
        ColumnFilter::Values(selected_values) => {
            format_column_filter_values_predicate_with_ref(&col_ref, selected_values)
        }
    }
}

pub fn format_column_filter_values_predicate(col: &str, selected_values: &HashSet<String>) -> String {
    let col_ref = resolve_column_sql_ref(col);
    format_column_filter_values_predicate_with_ref(&col_ref, selected_values)
}

pub fn format_column_filter_values_predicate_with_ref(
    col_ref: &str,
    selected_values: &HashSet<String>,
) -> String {

    if selected_values.is_empty() {
        return "1 = 0".to_string();
    }

    let mut vals: Vec<&String> = selected_values.iter().collect();
    vals.sort();

    let has_null = vals.iter().any(|v| v.eq_ignore_ascii_case("null") || v.is_empty());
    let non_nulls: Vec<&&String> = vals
        .iter()
        .filter(|v| !v.eq_ignore_ascii_case("null") && !v.is_empty())
        .collect();

    let non_null_predicate = if non_nulls.is_empty() {
        None
    } else if non_nulls.len() == 1 {
        let v = non_nulls[0];
        if v.eq_ignore_ascii_case("true") || v.eq_ignore_ascii_case("false") {
            Some(format!("{col_ref} = {}", v.to_lowercase()))
        } else if is_numeric_literal(v) {
            Some(format!("{col_ref} = {v}"))
        } else {
            Some(format!("{col_ref} = '{}'", v.replace('\'', "''")))
        }
    } else {
        let all_numeric = non_nulls.iter().all(|v| is_numeric_literal(v));
        if all_numeric {
            let joined = non_nulls.iter().map(|v| v.as_str()).collect::<Vec<_>>().join(", ");
            Some(format!("{col_ref} IN ({joined})"))
        } else {
            let joined = non_nulls
                .iter()
                .map(|v| format!("'{}'", v.replace('\'', "''")))
                .collect::<Vec<_>>()
                .join(", ");
            Some(format!("{col_ref} IN ({joined})"))
        }
    };

    match (non_null_predicate, has_null) {
        (Some(pred), false) => pred,
        (Some(pred), true) => format!("({pred} OR {col_ref} IS NULL)"),
        (None, true) => format!("{col_ref} IS NULL"),
        (None, false) => "1 = 0".to_string(),
    }
}

pub fn split_sql_clauses(sql: &str) -> (&str, Option<&str>, &str) {
    let bytes = sql.as_bytes();
    let len = bytes.len();
    let mut in_single_quote = false;
    let mut in_double_quote = false;
    let mut paren_depth: usize = 0;

    let mut where_pos: Option<usize> = None;
    let mut trailing_pos: Option<usize> = None;

    let trailing_keywords = ["GROUP BY", "HAVING", "ORDER BY", "OFFSET", "LIMIT"];

    let mut i = 0;
    while i < len {
        let b = bytes[i];
        if b == b'\'' && !in_double_quote {
            in_single_quote = !in_single_quote;
            i += 1;
            continue;
        }
        if b == b'"' && !in_single_quote {
            in_double_quote = !in_double_quote;
            i += 1;
            continue;
        }
        if in_single_quote || in_double_quote {
            i += 1;
            continue;
        }
        if b == b'(' {
            paren_depth += 1;
            i += 1;
            continue;
        }
        if b == b')' {
            paren_depth = paren_depth.saturating_sub(1);
            i += 1;
            continue;
        }

        if paren_depth == 0 {
            let at_word_start =
                i == 0 || bytes[i - 1].is_ascii_whitespace() || bytes[i - 1] == b')';
            if at_word_start {
                let rest = &sql[i..];
                if where_pos.is_none()
                    && rest.len() >= 5
                    && rest[..5].eq_ignore_ascii_case("WHERE")
                {
                    let after = &rest[5..];
                    if after.is_empty()
                        || after.as_bytes()[0].is_ascii_whitespace()
                        || after.as_bytes()[0] == b'('
                    {
                        where_pos = Some(i);
                        i += 5;
                        continue;
                    }
                }

                let mut found_trailing = false;
                for kw in &trailing_keywords {
                    if rest.len() >= kw.len() && rest[..kw.len()].eq_ignore_ascii_case(kw) {
                        let after = &rest[kw.len()..];
                        if after.is_empty()
                            || after.as_bytes()[0].is_ascii_whitespace()
                            || after.as_bytes()[0] == b'('
                        {
                            trailing_pos = Some(i);
                            found_trailing = true;
                            break;
                        }
                    }
                }
                if found_trailing {
                    break;
                }
            }
        }
        i += 1;
    }

    match (where_pos, trailing_pos) {
        (Some(w_start), Some(t_start)) => {
            let head = sql[..w_start].trim();
            let where_clause = sql[w_start + 5..t_start].trim();
            let tail = sql[t_start..].trim();
            (head, Some(where_clause), tail)
        }
        (Some(w_start), None) => {
            let head = sql[..w_start].trim();
            let where_clause = sql[w_start + 5..].trim();
            (head, Some(where_clause), "")
        }
        (None, Some(t_start)) => {
            let head = sql[..t_start].trim();
            let tail = sql[t_start..].trim();
            (head, None, tail)
        }
        (None, None) => (sql.trim(), None, ""),
    }
}



pub fn replace_select_columns(
    head: &str,
    visible_columns: &[String],
    all_columns_count: usize,
) -> String {
    replace_select_columns_with_overrides(head, visible_columns, all_columns_count, None)
}

pub fn replace_select_columns_with_overrides(
    head: &str,
    visible_columns: &[String],
    all_columns_count: usize,
    expr_overrides: Option<&HashMap<String, String>>,
) -> String {
    if visible_columns.len() == all_columns_count || visible_columns.is_empty() {
        return head.to_string();
    }

    let upper = head.to_ascii_uppercase();
    if !upper.starts_with("SELECT ") {
        return head.to_string();
    }

    let mut depth: usize = 0;
    let mut from_pos = None;
    let bytes = head.as_bytes();
    let len = bytes.len();

    for i in 6..len {
        match bytes[i] {
            b'(' => depth += 1,
            b')' => depth = depth.saturating_sub(1),
            _ if depth == 0
                && i + 6 <= len
                && head[i..i + 6].eq_ignore_ascii_case(" FROM ") =>
            {
                from_pos = Some(i);
                break;
            }
            _ => {}
        }
    }

    let Some(f_pos) = from_pos else {
        return head.to_string();
    };

    let from_and_rest = &head[f_pos..];
    let formatted_cols = visible_columns
        .iter()
        .map(|c| {
            if let Some(map) = expr_overrides && let Some(expr) = map.get(c) {
                format!("{expr} AS {}", q(c))
            } else if c.starts_with('"') || c.contains('(') || c.contains('[') {
                c.to_string()
            } else if let Some((parent, child)) = c.split_once('.') {
                let path = child.split('.').collect::<Vec<_>>().join(".");
                format!("json_extract_scalar({}, '$.{}') AS {}", q(parent), path, q(c))
            } else {
                q(c)
            }
        })
        .collect::<Vec<_>>()
        .join(", ");

    format!("SELECT {formatted_cols}{from_and_rest}")
}

pub fn build_query_with_all_filters(
    base_query: &str,
    all_columns: &[String],
    hidden_columns: &HashSet<String>,
    column_filters: &HashMap<String, ColumnFilter>,
    col_distinct_values: &HashMap<String, Vec<String>>,
) -> String {
    build_query_with_all_filters_and_overrides(
        base_query,
        all_columns,
        hidden_columns,
        column_filters,
        col_distinct_values,
        None,
    )
}

pub fn build_query_with_all_filters_and_overrides(
    base_query: &str,
    all_columns: &[String],
    hidden_columns: &HashSet<String>,
    column_filters: &HashMap<String, ColumnFilter>,
    col_distinct_values: &HashMap<String, Vec<String>>,
    expr_overrides: Option<&HashMap<String, String>>,
) -> String {
    let mut predicates = Vec::new();
    let mut cols: Vec<&String> = column_filters.keys().collect();
    cols.sort();

    for col in cols {
        let filter = &column_filters[col];
        match filter {
            ColumnFilter::Values(selected) => {
                if let Some(distinct) = col_distinct_values.get(col)
                    && selected.len() == distinct.len()
                {
                    continue;
                }
                predicates.push(format_column_filter_predicate_with_overrides(
                    col,
                    filter,
                    expr_overrides,
                ));
            }
            ColumnFilter::Numeric(_) => {
                predicates.push(format_column_filter_predicate_with_overrides(
                    col,
                    filter,
                    expr_overrides,
                ));
            }
        }
    }

    let (head, existing_where, tail) = split_sql_clauses(base_query);

    let visible_cols: Vec<String> = all_columns
        .iter()
        .filter(|c| !hidden_columns.contains(*c))
        .cloned()
        .collect();
    let updated_head = replace_select_columns_with_overrides(
        head,
        &visible_cols,
        all_columns.len(),
        expr_overrides,
    );

    let where_clause = match (existing_where, predicates.is_empty()) {
        (Some(orig), false) if !orig.trim().is_empty() => {
            let preds = predicates.join(" AND ");
            format!("WHERE ({}) AND ({})", orig.trim(), preds)
        }
        (Some(orig), true) if !orig.trim().is_empty() => {
            format!("WHERE {}", orig.trim())
        }
        (_, false) => {
            let preds = predicates.join(" AND ");
            format!("WHERE {preds}")
        }
        (_, true) => String::new(),
    };

    let mut parts = Vec::new();
    if !updated_head.is_empty() {
        parts.push(updated_head.as_str());
    }
    if !where_clause.is_empty() {
        parts.push(&where_clause);
    }
    if !tail.is_empty() {
        parts.push(tail);
    }
    parts.join(" ")
}

pub fn build_query_with_column_filters(
    base_query: &str,
    column_filters: &HashMap<String, ColumnFilter>,
    col_distinct_values: &HashMap<String, Vec<String>>,
) -> String {
    build_query_with_all_filters(
        base_query,
        &[],
        &HashSet::new(),
        column_filters,
        col_distinct_values,
    )
}

/// Incrementally updates the `LIMIT` clause in a SQL query to `new_limit`.
///
/// Accurately parses top-level SQL, ignoring `LIMIT` inside single-quoted strings,
/// double-quoted identifiers, parentheses (subqueries), and line comments.
/// If a top-level `LIMIT <n>` clause is present, it replaces `<n>` with `new_limit`
/// (preserving case, surrounding spacing, and trailing punctuation like `;`).
/// If no `LIMIT` clause is present, it appends `LIMIT {new_limit}`.
pub fn update_query_limit(sql: &str, new_limit: usize) -> String {
    let trimmed = sql.trim();
    if trimmed.is_empty() {
        return String::new();
    }

    let bytes = sql.as_bytes();
    let len = bytes.len();
    let mut in_single_quote = false;
    let mut in_double_quote = false;
    let mut paren_depth: usize = 0;

    let mut last_limit_num_range: Option<(usize, usize)> = None;

    let mut i = 0;
    while i < len {
        let b = bytes[i];

        // Line comment: -- until end of line
        if !in_single_quote && !in_double_quote && b == b'-' && i + 1 < len && bytes[i + 1] == b'-' {
            i += 2;
            while i < len && bytes[i] != b'\n' {
                i += 1;
            }
            continue;
        }

        if b == b'\'' && !in_double_quote {
            if in_single_quote && i + 1 < len && bytes[i + 1] == b'\'' {
                i += 2;
                continue;
            }
            in_single_quote = !in_single_quote;
            i += 1;
            continue;
        }
        if b == b'"' && !in_single_quote {
            if in_double_quote && i + 1 < len && bytes[i + 1] == b'"' {
                i += 2;
                continue;
            }
            in_double_quote = !in_double_quote;
            i += 1;
            continue;
        }
        if in_single_quote || in_double_quote {
            i += 1;
            continue;
        }

        if b == b'(' {
            paren_depth += 1;
            i += 1;
            continue;
        }
        if b == b')' {
            paren_depth = paren_depth.saturating_sub(1);
            i += 1;
            continue;
        }

        if paren_depth == 0 {
            let at_word_start =
                i == 0 || bytes[i - 1].is_ascii_whitespace() || bytes[i - 1] == b')';
            if at_word_start && i + 5 <= len && sql[i..i + 5].eq_ignore_ascii_case("LIMIT") {
                let after_kw = i + 5;
                if after_kw == len || bytes[after_kw].is_ascii_whitespace() {
                    let mut j = after_kw;
                    while j < len && bytes[j].is_ascii_whitespace() {
                        j += 1;
                    }
                    let num_start = j;
                    while j < len && bytes[j].is_ascii_digit() {
                        j += 1;
                    }
                    let num_end = j;
                    if num_end > num_start {
                        last_limit_num_range = Some((num_start, num_end));
                        i = num_end;
                        continue;
                    }
                }
            }
        }
        i += 1;
    }

    if let Some((num_start, num_end)) = last_limit_num_range {
        format!("{}{new_limit}{}", &sql[..num_start], &sql[num_end..])
    } else {
        let trimmed_end = sql.trim_end();
        if let Some(stripped) = trimmed_end.strip_suffix(';') {
            format!("{} LIMIT {new_limit};", stripped.trim_end())
        } else {
            format!("{trimmed_end} LIMIT {new_limit}")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metadata_queries_quote_identifiers() {
        assert_eq!(show_catalogs(), "SHOW CATALOGS");
        assert_eq!(show_schemas("ice berg"), "SHOW SCHEMAS FROM \"ice berg\"");
        assert_eq!(
            show_tables("ice\"berg", "sales data"),
            "SHOW TABLES FROM \"ice\"\"berg\".\"sales data\""
        );
    }

    #[test]
    fn table_inspection_queries_match_expected_sql() {
        let catalog = "ice\"berg";
        let schema = "sales data";
        let table = "orders\"2024";

        assert_eq!(
            describe(catalog, schema, table),
            "DESCRIBE \"ice\"\"berg\".\"sales data\".\"orders\"\"2024\""
        );
        assert_eq!(
            show_create(catalog, schema, table),
            "SHOW CREATE TABLE \"ice\"\"berg\".\"sales data\".\"orders\"\"2024\""
        );
        assert_eq!(
            show_stats(catalog, schema, table),
            "SHOW STATS FOR \"ice\"\"berg\".\"sales data\".\"orders\"\"2024\""
        );
        assert_eq!(
            count(catalog, schema, table),
            "SELECT COUNT(*) AS total_records FROM \"ice\"\"berg\".\"sales data\".\"orders\"\"2024\""
        );
        let filters = vec![("date".to_string(), "2026-08-06".to_string())];
        assert_eq!(
            filtered_count(catalog, schema, table, &filters),
            "SELECT COUNT(*) AS total_records FROM \"ice\"\"berg\".\"sales data\".\"orders\"\"2024\" WHERE date = '2026-08-06'"
        );
        assert_eq!(
            sample(catalog, schema, table),
            "SELECT * FROM \"ice\"\"berg\".\"sales data\".\"orders\"\"2024\" LIMIT 20"
        );
        assert_eq!(
            page_query(catalog, schema, table, 25, 50),
            "SELECT * FROM \"ice\"\"berg\".\"sales data\".\"orders\"\"2024\" OFFSET 25 LIMIT 50"
        );
        assert_eq!(
            partitions(catalog, schema, table),
            "SELECT * FROM \"ice\"\"berg\".\"sales data\".\"orders\"\"2024$partitions\""
        );
    }

    #[test]
    fn distinct_partition_values_builds_query_without_filters() {
        assert_eq!(
            distinct_partition_values("ice berg", "sales", "orders", &[], "date", 200),
            "SELECT DISTINCT date FROM \"ice berg\".\"sales\".\"orders\" ORDER BY date DESC LIMIT 200"
        );
    }

    #[test]
    fn distinct_partition_values_builds_query_with_filters_and_escapes_values() {
        let filters = vec![
            ("date".to_string(), "2026-08-06".to_string()),
            ("service".to_string(), "o'brien".to_string()),
        ];
        assert_eq!(
            distinct_partition_values("datalake", "tenant", "events", &filters, "account_id", 200),
            "SELECT DISTINCT account_id FROM \"datalake\".\"tenant\".\"events\" \
             WHERE date = '2026-08-06' AND service = 'o''brien' ORDER BY account_id DESC LIMIT 200"
        );
    }

    #[test]
    fn filtered_page_query_builds_query_without_filters() {
        assert_eq!(
            filtered_page_query("ice berg", "sales", "orders", &[], 0, 100, &[]),
            "SELECT * FROM \"ice berg\".\"sales\".\"orders\" OFFSET 0 LIMIT 100"
        );
    }

    #[test]
    fn filtered_page_query_builds_query_with_filters() {
        let filters = vec![
            ("date".to_string(), "2026-08-06".to_string()),
            ("service".to_string(), "smb3".to_string()),
            ("account_id".to_string(), "58bfaed0".to_string()),
        ];
        assert_eq!(
            filtered_page_query("datalake", "tenant", "events", &filters, 0, 100, &[]),
            "SELECT * FROM \"datalake\".\"tenant\".\"events\" \
             WHERE date = '2026-08-06' AND service = 'smb3' AND account_id = '58bfaed0' \
             OFFSET 0 LIMIT 100"
        );
    }

    #[test]
    fn filtered_page_query_uses_explicit_column_list_when_provided() {
        let columns = vec!["event_type".to_string(), "source_id".to_string()];
        assert_eq!(
            filtered_page_query("datalake", "tenant", "events", &[], 0, 100, &columns),
            "SELECT \"event_type\", \"source_id\" FROM \"datalake\".\"tenant\".\"events\" \
             OFFSET 0 LIMIT 100"
        );
    }

    #[test]
    fn distinct_partition_values_formats_expression_and_numeric_predicates_unquoted() {
        let filters = vec![("year(shipdate)".to_string(), "1998".to_string())];
        assert_eq!(
            distinct_partition_values(
                "iceberg",
                "demo",
                "lineitem_partitioned",
                &filters,
                "shipmode",
                200
            ),
            "SELECT DISTINCT shipmode FROM \"iceberg\".\"demo\".\"lineitem_partitioned\" \
             WHERE year(shipdate) = 1998 ORDER BY shipmode DESC LIMIT 200"
        );
    }

    #[test]
    fn filtered_page_query_formats_numeric_boolean_and_null_predicates() {
        let filters = vec![
            ("year(shipdate)".to_string(), "1998".to_string()),
            ("shipmode".to_string(), "AIR".to_string()),
            ("is_active".to_string(), "true".to_string()),
            ("deleted_at".to_string(), "NULL".to_string()),
        ];
        assert_eq!(
            filtered_page_query(
                "iceberg",
                "demo",
                "lineitem_partitioned",
                &filters,
                0,
                100,
                &[]
            ),
            "SELECT * FROM \"iceberg\".\"demo\".\"lineitem_partitioned\" \
             WHERE year(shipdate) = 1998 AND shipmode = 'AIR' AND is_active = true AND deleted_at IS NULL \
             OFFSET 0 LIMIT 100"
        );
    }

    #[test]
    fn info_schema_query_escapes_single_quotes() {
        assert_eq!(
            info_schema_columns("ice\"berg", "o'hare", "customer's orders"),
            "SELECT column_name, data_type, is_nullable, comment FROM \"ice\"\"berg\".information_schema.columns WHERE table_schema = 'o''hare' AND table_name = 'customer''s orders' ORDER BY ordinal_position"
        );
    }

    #[test]
    fn test_is_partition_column() {
        let partitioned_by = vec![
            "year(shipdate)".to_string(),
            "shipmode".to_string(),
            "month(orderdate)".to_string(),
            "bucket(account_id, 100)".to_string(),
        ];
        assert!(is_partition_column(&partitioned_by, "shipdate"));
        assert!(is_partition_column(&partitioned_by, "shipmode"));
        assert!(is_partition_column(&partitioned_by, "orderdate"));
        assert!(is_partition_column(&partitioned_by, "account_id"));
        assert!(!is_partition_column(&partitioned_by, "orderkey"));
        assert!(!is_partition_column(&partitioned_by, "extendedprice"));
    }

    #[test]
    fn test_format_column_filter_predicate() {
        let mut set = HashSet::new();
        set.insert("ACTIVE".to_string());
        assert_eq!(format_column_filter_predicate("status", &ColumnFilter::Values(set.clone())), "\"status\" = 'ACTIVE'");

        set.insert("PENDING".to_string());
        assert_eq!(format_column_filter_predicate("status", &ColumnFilter::Values(set)), "\"status\" IN ('ACTIVE', 'PENDING')");

        let mut num_set = HashSet::new();
        num_set.insert("10".to_string());
        num_set.insert("20".to_string());
        assert_eq!(format_column_filter_predicate("amount", &ColumnFilter::Values(num_set)), "\"amount\" IN (10, 20)");

        let mut null_set = HashSet::new();
        null_set.insert("null".to_string());
        assert_eq!(format_column_filter_predicate("status", &ColumnFilter::Values(null_set.clone())), "\"status\" IS NULL");

        null_set.insert("ACTIVE".to_string());
        assert_eq!(
            format_column_filter_predicate("status", &ColumnFilter::Values(null_set)),
            "(\"status\" = 'ACTIVE' OR \"status\" IS NULL)"
        );

        let empty_set: HashSet<String> = HashSet::new();
        assert_eq!(format_column_filter_predicate("status", &ColumnFilter::Values(empty_set)), "1 = 0");

        // Numeric operators: <, <=, >, >=, =
        use crate::app::{NumericFilter, NumericOp};
        let lt = ColumnFilter::Numeric(NumericFilter {
            op: NumericOp::Lt,
            value: 100.0,
            raw_value: "100".to_string(),
        });
        assert_eq!(format_column_filter_predicate("amount", &lt), "\"amount\" < 100");

        let lte = ColumnFilter::Numeric(NumericFilter {
            op: NumericOp::Lte,
            value: 50.5,
            raw_value: "50.5".to_string(),
        });
        assert_eq!(format_column_filter_predicate("amount", &lte), "\"amount\" <= 50.5");

        let gt = ColumnFilter::Numeric(NumericFilter {
            op: NumericOp::Gt,
            value: 0.0,
            raw_value: "0".to_string(),
        });
        assert_eq!(format_column_filter_predicate("amount", &gt), "\"amount\" > 0");

        let gte = ColumnFilter::Numeric(NumericFilter {
            op: NumericOp::Gte,
            value: 10.0,
            raw_value: "10".to_string(),
        });
        assert_eq!(format_column_filter_predicate("amount", &gte), "\"amount\" >= 10");

        let eq = ColumnFilter::Numeric(NumericFilter {
            op: NumericOp::Eq,
            value: 42.0,
            raw_value: "42".to_string(),
        });
        assert_eq!(format_column_filter_predicate("amount", &eq), "\"amount\" = 42");
    }

    #[test]
    fn test_build_query_with_column_filters() {
        use crate::app::{NumericFilter, NumericOp};
        let base_query = "SELECT * FROM \"iceberg\".\"sales\".\"orders\" OFFSET 0 LIMIT 100";
        let mut filters = HashMap::new();
        let mut distinct = HashMap::new();

        let mut sel = HashSet::new();
        sel.insert("ACTIVE".to_string());
        filters.insert("status".to_string(), ColumnFilter::Values(sel));
        distinct.insert("status".to_string(), vec!["ACTIVE".to_string(), "PENDING".to_string()]);

        let sql = build_query_with_column_filters(base_query, &filters, &distinct);
        assert_eq!(
            sql,
            "SELECT * FROM \"iceberg\".\"sales\".\"orders\" WHERE \"status\" = 'ACTIVE' OFFSET 0 LIMIT 100"
        );

        // When all values selected, no WHERE added
        let mut all_sel = HashSet::new();
        all_sel.insert("ACTIVE".to_string());
        all_sel.insert("PENDING".to_string());
        filters.insert("status".to_string(), ColumnFilter::Values(all_sel));
        let sql_all = build_query_with_column_filters(base_query, &filters, &distinct);
        assert_eq!(sql_all, base_query);

        // Query with existing WHERE and numeric filter
        let base_with_where = "SELECT * FROM tbl WHERE date = '2026-08-06' OFFSET 0 LIMIT 100";
        let mut filters2 = HashMap::new();
        filters2.insert(
            "amount".to_string(),
            ColumnFilter::Numeric(NumericFilter {
                op: NumericOp::Gte,
                value: 50.0,
                raw_value: "50".to_string(),
            }),
        );
        let sql_where = build_query_with_column_filters(base_with_where, &filters2, &distinct);
        assert_eq!(
            sql_where,
            "SELECT * FROM tbl WHERE (date = '2026-08-06') AND (\"amount\" >= 50) OFFSET 0 LIMIT 100"
        );
    }

    #[test]
    fn test_replace_select_columns() {
        let head = "SELECT * FROM \"iceberg\".\"sales\".\"orders\"";
        let cols = vec!["id".to_string(), "status".to_string()];
        let replaced = replace_select_columns(head, &cols, 3);
        assert_eq!(replaced, "SELECT \"id\", \"status\" FROM \"iceberg\".\"sales\".\"orders\"");

        // All columns visible preserves original head
        let all_cols = vec!["id".to_string(), "status".to_string(), "amount".to_string()];
        let preserved = replace_select_columns(head, &all_cols, 3);
        assert_eq!(preserved, head);
    }

    #[test]
    fn test_build_query_with_all_filters() {
        let base_query = "SELECT * FROM orders OFFSET 0 LIMIT 100";
        let all_cols = vec!["id".to_string(), "status".to_string(), "amount".to_string()];
        let mut hidden = HashSet::new();
        hidden.insert("amount".to_string());

        let mut filters = HashMap::new();
        let mut distinct = HashMap::new();
        let mut sel = HashSet::new();
        sel.insert("ACTIVE".to_string());
        filters.insert("status".to_string(), ColumnFilter::Values(sel));
        distinct.insert("status".to_string(), vec!["ACTIVE".to_string(), "PENDING".to_string()]);

        let sql = build_query_with_all_filters(base_query, &all_cols, &hidden, &filters, &distinct);
        assert_eq!(
            sql,
            "SELECT \"id\", \"status\" FROM orders WHERE \"status\" = 'ACTIVE' OFFSET 0 LIMIT 100"
        );
    }

    #[test]
    fn test_update_query_limit_replaces_top_level_limit() {
        assert_eq!(
            update_query_limit("SELECT * FROM orders OFFSET 0 LIMIT 100", 200),
            "SELECT * FROM orders OFFSET 0 LIMIT 200"
        );
        assert_eq!(
            update_query_limit("SELECT * FROM orders LIMIT 100", 250),
            "SELECT * FROM orders LIMIT 250"
        );
        assert_eq!(
            update_query_limit("SELECT * FROM orders limit 100", 300),
            "SELECT * FROM orders limit 300"
        );
        assert_eq!(
            update_query_limit("SELECT * FROM orders LIMIT 100;", 200),
            "SELECT * FROM orders LIMIT 200;"
        );
        assert_eq!(
            update_query_limit("SELECT * FROM orders LIMIT   100", 200),
            "SELECT * FROM orders LIMIT   200"
        );
    }

    #[test]
    fn test_update_query_limit_ignores_quoted_and_subquery_limit() {
        // Ignores 'LIMIT 50' in string literal
        let query_with_str = "SELECT * FROM t WHERE comment = 'LIMIT 50' LIMIT 100";
        assert_eq!(
            update_query_limit(query_with_str, 200),
            "SELECT * FROM t WHERE comment = 'LIMIT 50' LIMIT 200"
        );

        // Ignores inner subquery LIMIT
        let query_with_sub = "SELECT * FROM (SELECT id FROM t LIMIT 10) AS s LIMIT 100";
        assert_eq!(
            update_query_limit(query_with_sub, 200),
            "SELECT * FROM (SELECT id FROM t LIMIT 10) AS s LIMIT 200"
        );
    }

    #[test]
    fn test_update_query_limit_appends_when_absent() {
        assert_eq!(
            update_query_limit("SELECT * FROM orders", 200),
            "SELECT * FROM orders LIMIT 200"
        );
        assert_eq!(
            update_query_limit("SELECT * FROM orders;", 200),
            "SELECT * FROM orders LIMIT 200;"
        );
    }

    #[test]
    fn test_build_query_with_flattened_nested_fields() {
        let base_query = "SELECT * FROM orders OFFSET 0 LIMIT 100";
        let all_cols = vec![
            "id".to_string(),
            "meta.user".to_string(),
            "tags.env".to_string(),
            "unwanted".to_string(),
        ];
        let mut hidden = HashSet::new();
        hidden.insert("unwanted".to_string());

        let mut expr_overrides = HashMap::new();
        expr_overrides.insert(
            "meta.user".to_string(),
            "json_extract_scalar(\"meta\", '$.user')".to_string(),
        );
        expr_overrides.insert("tags.env".to_string(), "\"tags\"['env']".to_string());

        let mut filters = HashMap::new();
        let mut user_set = HashSet::new();
        user_set.insert("alice".to_string());
        filters.insert("meta.user".to_string(), ColumnFilter::Values(user_set));

        let distinct = HashMap::new();

        let sql = build_query_with_all_filters_and_overrides(
            base_query,
            &all_cols,
            &hidden,
            &filters,
            &distinct,
            Some(&expr_overrides),
        );

        assert_eq!(
            sql,
            "SELECT \"id\", json_extract_scalar(\"meta\", '$.user') AS \"meta.user\", \"tags\"['env'] AS \"tags.env\" FROM orders WHERE json_extract_scalar(\"meta\", '$.user') = 'alice' OFFSET 0 LIMIT 100"
        );
    }
}

