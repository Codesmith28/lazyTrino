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

use std::{collections::HashSet, fs, path::PathBuf, time::SystemTime};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::app::{ColumnFilter, NumericFilter, NumericOp, SortDirection};

const APP_DIR: &str = "lazytrino";
const SAVED_VIEWS_FILE: &str = "saved_views.json";

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

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SavedView {
    pub id: String,
    pub name: String,
    pub catalog: String,
    pub schema: String,
    pub table: String,
    pub action_idx: usize,
    pub query: String,
    #[serde(default)]
    pub column_filters: Vec<(String, StoredColumnFilter)>,
    #[serde(default)]
    pub hidden_columns: Vec<String>,
    #[serde(default)]
    pub sort: Option<(String, StoredSortDirection)>,
    #[serde(default)]
    pub partition_path: Vec<(String, String)>,
    pub saved_at: String,
}

impl SavedView {
    pub fn summary(&self) -> String {
        let mut badges = Vec::new();
        if !self.column_filters.is_empty() {
            badges.push(format!("{} filter(s)", self.column_filters.len()));
        }
        if !self.hidden_columns.is_empty() {
            badges.push(format!("{} hidden", self.hidden_columns.len()));
        }
        if let Some((ref col, dir)) = self.sort {
            let symbol = match dir {
                StoredSortDirection::Asc => "▲",
                StoredSortDirection::Desc => "▼",
            };
            badges.push(format!("sort: {col} {symbol}"));
        }
        if !self.partition_path.is_empty() {
            badges.push(format!("partition depth: {}", self.partition_path.len()));
        }

        if badges.is_empty() {
            "Default view".to_string()
        } else {
            badges.join(" | ")
        }
    }
}

pub fn saved_views_path() -> Option<PathBuf> {
    dirs::config_dir().map(|dir| dir.join(APP_DIR).join(SAVED_VIEWS_FILE))
}

pub fn load_saved_views() -> Vec<SavedView> {
    let Some(path) = saved_views_path() else {
        return Vec::new();
    };
    if !path.exists() {
        return Vec::new();
    }
    match fs::read_to_string(&path) {
        Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
        Err(_) => Vec::new(),
    }
}

pub fn save_saved_views(views: &[SavedView]) -> Result<()> {
    let path = saved_views_path().context("Could not determine config directory")?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).context("Failed to create lazytrino config directory")?;
    }
    let json = serde_json::to_string_pretty(views).context("Failed to serialize saved views")?;
    fs::write(&path, json).context("Failed to write saved_views.json")?;
    Ok(())
}

pub fn add_saved_view(view: SavedView) -> Result<()> {
    let mut views = load_saved_views();
    if let Some(pos) = views.iter().position(|v| v.id == view.id) {
        views[pos] = view;
    } else {
        views.insert(0, view);
    }
    save_saved_views(&views)
}

pub fn delete_saved_view(id: &str) -> Result<()> {
    let mut views = load_saved_views();
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

    #[test]
    fn test_saved_view_serialization_roundtrip() {
        let view = SavedView {
            id: "view_test_1".to_string(),
            name: "iceberg.sales.orders".to_string(),
            catalog: "iceberg".to_string(),
            schema: "sales".to_string(),
            table: "orders".to_string(),
            action_idx: 0,
            query: "SELECT * FROM iceberg.sales.orders LIMIT 100".to_string(),
            column_filters: vec![
                (
                    "status".to_string(),
                    StoredColumnFilter::Values(vec!["COMPLETE".to_string()]),
                ),
                (
                    "price".to_string(),
                    StoredColumnFilter::Numeric {
                        op: ">=".to_string(),
                        value: 50.0,
                        raw_value: "50".to_string(),
                    },
                ),
            ],
            hidden_columns: vec!["internal_id".to_string()],
            sort: Some(("order_date".to_string(), StoredSortDirection::Desc)),
            partition_path: vec![("dt".to_string(), "2026-10-14".to_string())],
            saved_at: "2026-10-14 08:00:00 UTC".to_string(),
        };

        let json = serde_json::to_string(&view).unwrap();
        let deserialized: SavedView = serde_json::from_str(&json).unwrap();
        assert_eq!(view, deserialized);
        assert_eq!(
            view.summary(),
            "2 filter(s) | 1 hidden | sort: order_date ▼ | partition depth: 1"
        );
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
