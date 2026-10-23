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

use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
    time::SystemTime,
};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::app::{Action, ColumnFilter, NumericFilter, NumericOp, ResultsState, SortDirection};
use crate::trino::queries;
use crate::view_location::{PartitionStep, QuerySource, ViewLocation};

const APP_DIR: &str = "lazytrino";
const SAVED_VIEWS_FILE: &str = "saved_views.json";
/// Copy of a pre-v2 file, written once before it is migrated in place.
const LEGACY_BACKUP_FILE: &str = "saved_views.v1.json";
pub const SAVED_VIEWS_VERSION: u32 = 2;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum StoredColumnFilter {
    Values(Vec<String>),
    Numeric {
        op: String,
        value: f64,
        raw_value: String,
    },
}

impl From<&ColumnFilter> for StoredColumnFilter {
    fn from(filter: &ColumnFilter) -> Self {
        match filter {
            ColumnFilter::Values(set) => {
                let mut v: Vec<String> = set.iter().cloned().collect();
                v.sort();
                StoredColumnFilter::Values(v)
            }
            ColumnFilter::Numeric(num) => StoredColumnFilter::Numeric {
                op: num.op.as_str().to_string(),
                value: num.value,
                raw_value: num.raw_value.clone(),
            },
        }
    }
}

impl StoredColumnFilter {
    pub fn to_column_filter(&self) -> ColumnFilter {
        match self {
            StoredColumnFilter::Values(vals) => {
                let set: HashSet<String> = vals.iter().cloned().collect();
                ColumnFilter::Values(set)
            }
            StoredColumnFilter::Numeric {
                op,
                value,
                raw_value,
            } => {
                let numeric_op = match op.as_str() {
                    "<" => NumericOp::Lt,
                    "<=" => NumericOp::Lte,
                    ">" => NumericOp::Gt,
                    ">=" => NumericOp::Gte,
                    _ => NumericOp::Eq,
                };
                ColumnFilter::Numeric(NumericFilter {
                    op: numeric_op,
                    value: *value,
                    raw_value: raw_value.clone(),
                })
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum StoredSortDirection {
    Asc,
    Desc,
}

impl From<SortDirection> for StoredSortDirection {
    fn from(s: SortDirection) -> Self {
        match s {
            SortDirection::Asc => StoredSortDirection::Asc,
            SortDirection::Desc => StoredSortDirection::Desc,
        }
    }
}

impl From<StoredSortDirection> for SortDirection {
    fn from(s: StoredSortDirection) -> Self {
        match s {
            StoredSortDirection::Asc => SortDirection::Asc,
            StoredSortDirection::Desc => SortDirection::Desc,
        }
    }
}

/// How the results grid was arranged: re-applied on top of the restored
/// query through the same path as the filter / column modals.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct GridState {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub column_filters: Vec<(String, StoredColumnFilter)>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub hidden_columns: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sort: Option<(String, StoredSortDirection)>,
}

impl GridState {
    pub fn is_empty(&self) -> bool {
        self.column_filters.is_empty() && self.hidden_columns.is_empty() && self.sort.is_none()
    }

    pub fn capture(res: &ResultsState) -> Self {
        let mut column_filters: Vec<_> = res
            .column_filters
            .iter()
            .map(|(k, v)| (k.clone(), v.into()))
            .collect();
        column_filters.sort_by(|a: &(String, StoredColumnFilter), b| a.0.cmp(&b.0));
        let mut hidden_columns: Vec<String> = res.hidden_columns.iter().cloned().collect();
        hidden_columns.sort();
        Self {
            column_filters,
            hidden_columns,
            sort: res.sort.as_ref().map(|(c, d)| (c.clone(), (*d).into())),
        }
    }
}

/// A saved view: *where* it is (`location`), *which* query (`query`) and
/// how the grid was arranged (`grid`). See `view_location` for why the
/// location is stored instead of literal SQL.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SavedView {
    pub version: u32,
    pub id: String,
    pub name: String,
    pub saved_at: String,
    /// Trino URL the view was saved against (informational).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub connection: Option<String>,
    pub location: ViewLocation,
    #[serde(default)]
    pub query: QuerySource,
    #[serde(default, skip_serializing_if = "GridState::is_empty")]
    pub grid: GridState,
}

impl SavedView {
    pub fn summary(&self) -> String {
        let mut badges = Vec::new();
        if matches!(self.query, QuerySource::Custom { .. }) {
            badges.push("custom SQL".to_string());
        }
        if !self.grid.column_filters.is_empty() {
            badges.push(format!("{} filter(s)", self.grid.column_filters.len()));
        }
        if !self.grid.hidden_columns.is_empty() {
            badges.push(format!("{} hidden", self.grid.hidden_columns.len()));
        }
        if let Some((ref col, dir)) = self.grid.sort {
            let symbol = match dir {
                StoredSortDirection::Asc => "▲",
                StoredSortDirection::Desc => "▼",
            };
            badges.push(format!("sort: {col} {symbol}"));
        }
        if !self.location.partition_path.is_empty() {
            badges.push(format!(
                "partition depth: {}",
                self.location.partition_path.len()
            ));
        }

        if badges.is_empty() {
            "Default view".to_string()
        } else {
            badges.join(" | ")
        }
    }
}

/// The pre-v2 on-disk shape: literal SQL plus a menu index.
#[derive(Clone, Debug, Deserialize)]
struct SavedViewV1 {
    id: String,
    name: String,
    catalog: String,
    schema: String,
    table: String,
    action_idx: usize,
    query: String,
    #[serde(default)]
    column_filters: Vec<(String, StoredColumnFilter)>,
    #[serde(default)]
    hidden_columns: Vec<String>,
    #[serde(default)]
    sort: Option<(String, StoredSortDirection)>,
    #[serde(default)]
    partition_path: Vec<(String, String)>,
    #[serde(default)]
    saved_at: String,
}

/// The menu order when v1 files were written (`action_idx` meant this).
const V1_ACTIONS: [Action; 6] = [
    Action::TableView,
    Action::TableDDL,
    Action::Schema,
    Action::ShowStats,
    Action::Sample,
    Action::Partitions,
];

impl SavedViewV1 {
    fn migrate(self) -> SavedView {
        let action = V1_ACTIONS
            .get(self.action_idx)
            .copied()
            .unwrap_or(Action::TableView);
        let partition_path = if action == Action::TableView {
            self.partition_path
                .iter()
                .map(|(column, value)| PartitionStep {
                    column: column.clone(),
                    value: value.clone(),
                })
                .collect()
        } else {
            Vec::new()
        };
        let query = if is_generated_v1_query(&self, action) {
            QuerySource::Generated
        } else {
            QuerySource::Custom {
                sql: self.query.clone(),
            }
        };
        SavedView {
            version: SAVED_VIEWS_VERSION,
            id: self.id,
            name: self.name,
            saved_at: self.saved_at,
            connection: None,
            location: ViewLocation {
                catalog: self.catalog,
                schema: self.schema,
                table: self.table,
                action,
                partition_path,
            },
            query,
            grid: GridState {
                column_filters: self.column_filters,
                hidden_columns: self.hidden_columns,
                sort: self.sort,
            },
        }
    }
}

/// True when a v1 view's SQL is exactly what the app generates for its
/// location, so it can be regenerated instead of replayed verbatim. Anything
/// else is kept as custom SQL — never lose what the user wrote.
fn is_generated_v1_query(v: &SavedViewV1, action: Action) -> bool {
    let (c, s, t) = (&v.catalog, &v.schema, &v.table);
    let sql = v.query.trim();
    let mut candidates = vec![action.build_query(c, s, t)];
    if action == Action::TableView {
        // v1 wrote predicates without knowing column types
        let no_types = queries::ColumnTypes::default();
        let path = &v.partition_path;
        candidates.push(queries::filtered_page_query(
            c,
            s,
            t,
            path,
            0,
            100,
            &[],
            &no_types,
        ));
        if let Some(rest) = sql.strip_prefix("SELECT ")
            && let Some((select_list, _)) = rest.split_once(" FROM ")
        {
            // a partition level listing: SELECT DISTINCT <col> ...
            if let Some(col) = select_list.strip_prefix("DISTINCT ") {
                candidates.push(queries::distinct_partition_values(
                    c,
                    s,
                    t,
                    path,
                    col,
                    queries::DRILLDOWN_LEVEL_LIMIT,
                    &no_types,
                ));
            }
            // a leaf page with an explicit (safe) column list
            let cols: Vec<String> = select_list
                .split(", ")
                .map(|c| c.trim_matches('"').replace("\"\"", "\""))
                .collect();
            candidates.push(queries::filtered_page_query(
                c, s, t, path, 0, 100, &cols, &no_types,
            ));
        }
    }
    candidates.iter().any(|cand| cand == sql)
}

pub fn saved_views_path() -> Option<PathBuf> {
    if cfg!(test) {
        // Unit tests must never touch the user's real saved views.
        return Some(test_storage_dir().join(SAVED_VIEWS_FILE));
    }
    dirs::config_dir().map(|dir| dir.join(APP_DIR).join(SAVED_VIEWS_FILE))
}

/// Per-thread scratch location used by unit tests (see `saved_views_path`).
fn test_storage_dir() -> PathBuf {
    let thread = format!("{:?}", std::thread::current().id())
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .collect::<String>();
    std::env::temp_dir().join(format!("lazytrino-test-{}-{thread}", std::process::id()))
}

/// Loads saved views, migrating a pre-v2 file in place (after writing a
/// one-time backup). An unreadable or malformed file is an error rather
/// than an empty list, so a later save can never overwrite it.
pub fn load_saved_views() -> Result<Vec<SavedView>> {
    match saved_views_path() {
        Some(path) => load_from(&path),
        None => Ok(Vec::new()),
    }
}

fn load_from(path: &Path) -> Result<Vec<SavedView>> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let content =
        fs::read_to_string(path).with_context(|| format!("Failed to read {}", path.display()))?;
    if content.trim().is_empty() {
        return Ok(Vec::new());
    }
    let (views, migrated) = parse_saved_views(&content)
        .with_context(|| format!("{} could not be read", path.display()))?;
    if migrated > 0 {
        let backup = path.with_file_name(LEGACY_BACKUP_FILE);
        if !backup.exists() {
            fs::write(&backup, &content).context("Failed to back up saved views")?;
        }
        write_views(path, &views)?;
    }
    Ok(views)
}

/// Parses a saved-views file of any version. Returns the views and how many
/// of them were migrated from the v1 format.
pub fn parse_saved_views(content: &str) -> Result<(Vec<SavedView>, usize)> {
    let raw: Vec<serde_json::Value> =
        serde_json::from_str(content).context("not a JSON list of saved views")?;
    let mut migrated = 0;
    let mut views = Vec::with_capacity(raw.len());
    for (i, value) in raw.into_iter().enumerate() {
        let view = if value.get("location").is_some() {
            serde_json::from_value::<SavedView>(value)
                .with_context(|| format!("saved view #{} is malformed", i + 1))?
        } else {
            migrated += 1;
            serde_json::from_value::<SavedViewV1>(value)
                .with_context(|| format!("saved view #{} is malformed", i + 1))?
                .migrate()
        };
        views.push(view);
    }
    Ok((views, migrated))
}

fn write_views(path: &Path, views: &[SavedView]) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).context("Failed to create lazytrino config directory")?;
    }
    let json = serde_json::to_string_pretty(views).context("Failed to serialize saved views")?;
    // write-then-rename so a crash never leaves a half-written file
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, json).context("Failed to write saved views")?;
    fs::rename(&tmp, path).context("Failed to replace saved_views.json")?;
    Ok(())
}

pub fn save_saved_views(views: &[SavedView]) -> Result<()> {
    let path = saved_views_path().context("Could not determine config directory")?;
    write_views(&path, views)
}

pub fn add_saved_view(view: SavedView) -> Result<()> {
    let mut views = load_saved_views()?;
    if let Some(pos) = views.iter().position(|v| v.id == view.id) {
        views[pos] = view;
    } else {
        views.insert(0, view);
    }
    save_saved_views(&views)
}

pub fn delete_saved_view(id: &str) -> Result<()> {
    let mut views = load_saved_views()?;
    views.retain(|v| v.id != id);
    save_saved_views(&views)
}

pub fn current_timestamp_str() -> String {
    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default();
    let secs = now.as_secs();
    let days = secs / 86400;
    let time_of_day = secs % 86400;
    let hours = time_of_day / 3600;
    let minutes = (time_of_day % 3600) / 60;
    let seconds = time_of_day % 60;

    let (year, month, day) = days_to_ymd(days);
    format!("{year:04}-{month:02}-{day:02} {hours:02}:{minutes:02}:{seconds:02} UTC")
}

fn days_to_ymd(days: u64) -> (u64, u64, u64) {
    // Gregorian calendar date calculation from epoch day
    let z = days + 719468;
    let era = z / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y, m, d)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_view() -> SavedView {
        SavedView {
            version: SAVED_VIEWS_VERSION,
            id: "view_test_1".to_string(),
            name: "iceberg.sales.orders".to_string(),
            saved_at: "2026-10-14 08:00:00 UTC".to_string(),
            connection: Some("https://trino.example".to_string()),
            location: ViewLocation {
                catalog: "iceberg".to_string(),
                schema: "sales".to_string(),
                table: "orders".to_string(),
                action: Action::TableView,
                partition_path: vec![PartitionStep {
                    column: "dt".to_string(),
                    value: "2026-10-14".to_string(),
                }],
            },
            query: QuerySource::Generated,
            grid: GridState {
                column_filters: vec![
                    (
                        "price".to_string(),
                        StoredColumnFilter::Numeric {
                            op: ">=".to_string(),
                            value: 50.0,
                            raw_value: "50".to_string(),
                        },
                    ),
                    (
                        "status".to_string(),
                        StoredColumnFilter::Values(vec!["COMPLETE".to_string()]),
                    ),
                ],
                hidden_columns: vec!["internal_id".to_string()],
                sort: Some(("order_date".to_string(), StoredSortDirection::Desc)),
            },
        }
    }

    #[test]
    fn test_saved_view_serialization_roundtrip() {
        let view = sample_view();
        let json = serde_json::to_string(&view).unwrap();
        let (parsed, migrated) = parse_saved_views(&format!("[{json}]")).unwrap();
        assert_eq!(migrated, 0);
        assert_eq!(parsed, vec![view.clone()]);
        assert_eq!(
            view.summary(),
            "2 filter(s) | 1 hidden | sort: order_date ▼ | partition depth: 1"
        );
    }

    /// The exact file the app wrote before v2, including a view saved while
    /// browsing a partition level (its SQL is a `SELECT DISTINCT`).
    const V1_FILE: &str = r#"[
      {
        "id": "datalake_x_events_1",
        "name": "datalake.x.events",
        "catalog": "datalake",
        "schema": "x",
        "table": "events",
        "action_idx": 0,
        "query": "SELECT DISTINCT account_id FROM \"datalake\".\"x\".\"events\" WHERE date = '2026-10-13' AND service = 'onedrivebiz' ORDER BY account_id DESC LIMIT 200",
        "column_filters": [],
        "hidden_columns": [],
        "sort": null,
        "partition_path": [["date", "2026-10-13"], ["service", "onedrivebiz"]],
        "saved_at": "2026-10-23 12:28:15 UTC"
      },
      {
        "id": "v1",
        "name": "custom one",
        "catalog": "iceberg",
        "schema": "sales",
        "table": "orders",
        "action_idx": 4,
        "query": "SELECT id FROM iceberg.sales.orders WHERE id > 3",
        "column_filters": [["status", {"Values": ["OK"]}]],
        "hidden_columns": ["internal_id"],
        "sort": ["id", "Asc"],
        "partition_path": [],
        "saved_at": "2026-10-14 08:00:00 UTC"
      }
    ]"#;

    #[test]
    fn v1_views_migrate_to_locations() {
        let (views, migrated) = parse_saved_views(V1_FILE).unwrap();
        assert_eq!(migrated, 2);

        let level = &views[0];
        assert_eq!(level.version, SAVED_VIEWS_VERSION);
        assert_eq!(level.location.action, Action::TableView);
        assert_eq!(
            level.location.filters(),
            vec![
                ("date".to_string(), "2026-10-13".to_string()),
                ("service".to_string(), "onedrivebiz".to_string())
            ]
        );
        // the app generated that SQL, so it is regenerated on restore
        assert_eq!(level.query, QuerySource::Generated);

        let custom = &views[1];
        assert_eq!(custom.location.action, Action::Sample);
        assert_eq!(
            custom.query,
            QuerySource::Custom {
                sql: "SELECT id FROM iceberg.sales.orders WHERE id > 3".into()
            }
        );
        assert_eq!(custom.grid.hidden_columns, vec!["internal_id"]);
        assert_eq!(
            custom.grid.sort,
            Some(("id".to_string(), StoredSortDirection::Asc))
        );
    }

    #[test]
    fn v1_generated_queries_are_recognised() {
        let v = |action_idx: usize, query: String, path: Vec<(String, String)>| SavedViewV1 {
            id: "i".into(),
            name: "n".into(),
            catalog: "c".into(),
            schema: "s".into(),
            table: "t".into(),
            action_idx,
            query,
            column_filters: vec![],
            hidden_columns: vec![],
            sort: None,
            partition_path: path,
            saved_at: String::new(),
        };
        let path = vec![("dt".to_string(), "2026-01-01".to_string())];
        let leaf = queries::filtered_page_query(
            "c",
            "s",
            "t",
            &path,
            0,
            100,
            &["id".to_string(), "name".to_string()],
            &crate::trino::queries::ColumnTypes::default(),
        );
        assert_eq!(
            v(0, leaf, path.clone()).migrate().query,
            QuerySource::Generated
        );
        assert_eq!(
            v(0, queries::page_query("c", "s", "t", 0, 100), vec![])
                .migrate()
                .query,
            QuerySource::Generated
        );
        assert_eq!(
            v(3, queries::show_stats("c", "s", "t"), vec![])
                .migrate()
                .query,
            QuerySource::Generated
        );
        // a leaf page whose LIMIT was edited by hand stays custom
        let edited = queries::filtered_page_query(
            "c",
            "s",
            "t",
            &path,
            0,
            5,
            &[],
            &crate::trino::queries::ColumnTypes::default(),
        );
        assert!(matches!(
            v(0, edited, path).migrate().query,
            QuerySource::Custom { .. }
        ));
    }

    #[test]
    fn loading_a_v1_file_migrates_it_once_and_keeps_a_backup() {
        let dir = test_storage_dir().join("migrate");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join(SAVED_VIEWS_FILE);
        fs::write(&path, V1_FILE).unwrap();

        let views = load_from(&path).unwrap();
        assert_eq!(views.len(), 2);
        assert_eq!(
            fs::read_to_string(dir.join(LEGACY_BACKUP_FILE)).unwrap(),
            V1_FILE
        );
        // the file is now v2 and loads without migrating again
        let (again, migrated) = parse_saved_views(&fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(migrated, 0);
        assert_eq!(again, views);
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_malformed_file_is_an_error_and_is_never_overwritten() {
        let dir = test_storage_dir().join("corrupt");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join(SAVED_VIEWS_FILE);
        fs::write(&path, "{ not json").unwrap();
        assert!(load_from(&path).is_err());
        fs::write(&path, r#"[{"id": 3}]"#).unwrap();
        assert!(load_from(&path).is_err());
        assert_eq!(fs::read_to_string(&path).unwrap(), r#"[{"id": 3}]"#);
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn add_and_delete_use_the_isolated_test_store() {
        let path = saved_views_path().unwrap();
        assert!(path.starts_with(std::env::temp_dir()));
        let _ = fs::remove_file(&path);

        add_saved_view(sample_view()).unwrap();
        assert_eq!(load_saved_views().unwrap().len(), 1);
        delete_saved_view("view_test_1").unwrap();
        assert!(load_saved_views().unwrap().is_empty());
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn grid_state_capture_is_stable() {
        let mut res = ResultsState::default();
        res.hidden_columns.insert("b".into());
        res.hidden_columns.insert("a".into());
        res.sort = Some(("x".into(), SortDirection::Desc));
        let g = GridState::capture(&res);
        assert_eq!(g.hidden_columns, vec!["a", "b"]);
        assert_eq!(g.sort, Some(("x".to_string(), StoredSortDirection::Desc)));
        assert!(GridState::default().is_empty());
    }

    #[test]
    fn test_stored_column_filter_conversion() {
        let mut set = HashSet::new();
        set.insert("A".to_string());
        set.insert("B".to_string());
        let orig = ColumnFilter::Values(set.clone());
        let stored = StoredColumnFilter::from(&orig);
        assert_eq!(
            stored,
            StoredColumnFilter::Values(vec!["A".to_string(), "B".to_string()])
        );
        assert_eq!(stored.to_column_filter(), orig);

        let num_filter = ColumnFilter::Numeric(NumericFilter {
            op: NumericOp::Gt,
            value: 42.0,
            raw_value: "42".to_string(),
        });
        let stored_num = StoredColumnFilter::from(&num_filter);
        assert_eq!(stored_num.to_column_filter(), num_filter);
    }

    #[test]
    fn test_current_timestamp_format() {
        let ts = current_timestamp_str();
        assert!(ts.ends_with(" UTC"));
        assert_eq!(ts.len(), 23); // "YYYY-MM-DD HH:MM:SS UTC"
    }
}
