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

pub fn preview(catalog: &str, schema: &str, table: &str) -> String {
    format!("SELECT * FROM {}.{}.{} LIMIT 10", q(catalog), q(schema), q(table))
}

pub fn partitions(catalog: &str, schema: &str, table: &str) -> String {
    format!("SELECT * FROM {}.{}.{}", q(catalog), q(schema), q(&format!("{table}$partitions")))
}

#[allow(dead_code)]
pub fn show_partitions(catalog: &str, schema: &str, table: &str) -> String {
    format!(
        "SELECT * FROM {}.information_schema.partitions WHERE table_schema = '{}' AND table_name = '{}'",
        q(catalog),
        schema.trim().replace('\'', "''"),
        table.trim().replace('\'', "''")
    )
}

#[allow(dead_code)]
pub fn files(catalog: &str, schema: &str, table: &str) -> String {
    format!(
        "SELECT * FROM {}.{}.{} LIMIT 50",
        q(catalog), q(schema), q(&format!("{table}$files"))
    )
}

#[allow(dead_code)]
pub fn properties(catalog: &str, schema: &str, table: &str) -> String {
    format!("SELECT * FROM {}.{}.{}", q(catalog), q(schema), q(&format!("{table}$properties")))
}

#[allow(dead_code)]
pub fn snapshots(catalog: &str, schema: &str, table: &str) -> String {
    format!(
        "SELECT * FROM {}.{}.{}",
        q(catalog), q(schema), q(&format!("{table}$snapshots"))
    )
}

#[allow(dead_code)]
pub fn history(catalog: &str, schema: &str, table: &str) -> String {
    format!(
        "SELECT * FROM {}.{}.{}",
        q(catalog), q(schema), q(&format!("{table}$history"))
    )
}

#[allow(dead_code)]
pub fn metadata_log(catalog: &str, schema: &str, table: &str) -> String {
    format!("SELECT * FROM {}.{}.{}", q(catalog), q(schema), q(&format!("{table}$metadata_log")))
}

