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

pub fn count(catalog: &str, schema: &str, table: &str) -> String {
    format!(
        "SELECT COUNT(*) AS total_records FROM {}.{}.{}",
        q(catalog),
        q(schema),
        q(table)
    )
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

/// Builds a `WHERE` clause fragment (without the leading `WHERE`) from an
/// ordered list of `(column, value)` partition predicates, quoting/escaping
/// each value as a string literal. Returns `None` when there are no filters.
fn build_where_clause(filters: &[(String, String)]) -> Option<String> {
    if filters.is_empty() {
        return None;
    }
    Some(
        filters
            .iter()
            .map(|(col, val)| format!("{} = '{}'", col.trim(), val.trim().replace('\'', "''")))
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
    fn info_schema_query_escapes_single_quotes() {
        assert_eq!(
            info_schema_columns("ice\"berg", "o'hare", "customer's orders"),
            "SELECT column_name, data_type, is_nullable, comment FROM \"ice\"\"berg\".information_schema.columns WHERE table_schema = 'o''hare' AND table_name = 'customer''s orders' ORDER BY ordinal_position"
        );
    }
}
