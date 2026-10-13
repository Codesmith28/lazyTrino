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

pub fn format_column_filter_predicate(col: &str, selected_values: &HashSet<String>) -> String {
    let col = col.trim();
    let col_ref = if col.starts_with('"') || col.contains('(') {
        col.to_string()
    } else {
        q(col)
    };

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

pub fn build_query_with_column_filters(
    base_query: &str,
    column_filters: &HashMap<String, HashSet<String>>,
    col_distinct_values: &HashMap<String, Vec<String>>,
) -> String {
    let mut predicates = Vec::new();
    let mut cols: Vec<&String> = column_filters.keys().collect();
    cols.sort();

    for col in cols {
        let selected = &column_filters[col];
        if let Some(distinct) = col_distinct_values.get(col)
            && selected.len() == distinct.len()
        {
            continue;
        }
        predicates.push(format_column_filter_predicate(col, selected));
    }

    let (head, existing_where, tail) = split_sql_clauses(base_query);

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
    if !head.is_empty() {
        parts.push(head);
    }
    if !where_clause.is_empty() {
        parts.push(&where_clause);
    }
    if !tail.is_empty() {
        parts.push(tail);
    }
    parts.join(" ")
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
        assert_eq!(format_column_filter_predicate("status", &set), "\"status\" = 'ACTIVE'");

        set.insert("PENDING".to_string());
        assert_eq!(format_column_filter_predicate("status", &set), "\"status\" IN ('ACTIVE', 'PENDING')");

        let mut num_set = HashSet::new();
        num_set.insert("10".to_string());
        num_set.insert("20".to_string());
        assert_eq!(format_column_filter_predicate("amount", &num_set), "\"amount\" IN (10, 20)");

        let mut null_set = HashSet::new();
        null_set.insert("null".to_string());
        assert_eq!(format_column_filter_predicate("status", &null_set), "\"status\" IS NULL");

        null_set.insert("ACTIVE".to_string());
        assert_eq!(
            format_column_filter_predicate("status", &null_set),
            "(\"status\" = 'ACTIVE' OR \"status\" IS NULL)"
        );

        let empty_set: HashSet<String> = HashSet::new();
        assert_eq!(format_column_filter_predicate("status", &empty_set), "1 = 0");
    }

    #[test]
    fn test_build_query_with_column_filters() {
        let base_query = "SELECT * FROM \"iceberg\".\"sales\".\"orders\" OFFSET 0 LIMIT 100";
        let mut filters = HashMap::new();
        let mut distinct = HashMap::new();

        let mut sel = HashSet::new();
        sel.insert("ACTIVE".to_string());
        filters.insert("status".to_string(), sel);
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
        filters.insert("status".to_string(), all_sel);
        let sql_all = build_query_with_column_filters(base_query, &filters, &distinct);
        assert_eq!(sql_all, base_query);

        // Query with existing WHERE
        let base_with_where = "SELECT * FROM tbl WHERE date = '2026-08-06' OFFSET 0 LIMIT 100";
        let mut sel2 = HashSet::new();
        sel2.insert("ACTIVE".to_string());
        filters.insert("status".to_string(), sel2);
        let sql_where = build_query_with_column_filters(base_with_where, &filters, &distinct);
        assert_eq!(
            sql_where,
            "SELECT * FROM tbl WHERE (date = '2026-08-06') AND (\"status\" = 'ACTIVE') OFFSET 0 LIMIT 100"
        );
    }
}
