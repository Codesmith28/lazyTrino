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
    format!("SHOW CREATE TABLE {}.{}.{}", q(catalog), q(schema), q(table))
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
    format!("SELECT COUNT(*) AS total_records FROM {}.{}.{}", q(catalog), q(schema), q(table))
}

pub fn sample(catalog: &str, schema: &str, table: &str) -> String {
    format!("SELECT * FROM {}.{}.{} LIMIT 20", q(catalog), q(schema), q(table))
}

pub fn page_query(catalog: &str, schema: &str, table: &str, offset: usize, limit: usize) -> String {
    format!("SELECT * FROM {}.{}.{} OFFSET {offset} LIMIT {limit}", q(catalog), q(schema), q(table))
}

pub fn partitions(catalog: &str, schema: &str, table: &str) -> String {
    format!("SELECT * FROM {}.{}.{}", q(catalog), q(schema), q(&format!("{table}$partitions")))
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
    fn info_schema_query_escapes_single_quotes() {
        assert_eq!(
            info_schema_columns("ice\"berg", "o'hare", "customer's orders"),
            "SELECT column_name, data_type, is_nullable, comment FROM \"ice\"\"berg\".information_schema.columns WHERE table_schema = 'o''hare' AND table_name = 'customer''s orders' ORDER BY ordinal_position"
        );
    }
}
