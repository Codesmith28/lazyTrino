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

//! A structured description of *where* the user is: catalog → schema →
//! table → action → partition path, plus whether the query on screen is the
//! one the app generates for that place or SQL the user typed.
//!
//! Capturing this ("deconstructing" the app state) instead of the literal SQL
//! lets a saved view be rebuilt step by step ("reconstructed") so every pane
//! before and after it can be navigated, and lets generated queries be
//! regenerated against the table's *current* layout.

use serde::{Deserialize, Serialize};

use crate::app::{ACTIONS, Action, App, Screen};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PartitionStep {
    pub column: String,
    pub value: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ViewLocation {
    pub catalog: String,
    pub schema: String,
    pub table: String,
    pub action: Action,
    /// Partition values fixed in the Table View drill-down, outermost first.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub partition_path: Vec<PartitionStep>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum QuerySource {
    /// Rebuilt from the location when restored.
    #[default]
    Generated,
    /// SQL the user ran from the query bar, kept verbatim.
    Custom { sql: String },
}

impl ViewLocation {
    /// The location and query source of the table view currently on screen.
    pub fn capture(app: &App) -> Option<(ViewLocation, QuerySource)> {
        let Screen::Actions(a) = &app.screen else {
            return None;
        };
        let action = ACTIONS.get(a.selected).map(|(_, _, act)| *act)?;
        let partition_path = match (&a.drilldown, action) {
            (Some(dd), Action::TableView) => dd
                .path
                .iter()
                .map(|(column, value)| PartitionStep {
                    column: column.clone(),
                    value: value.clone(),
                })
                .collect(),
            _ => Vec::new(),
        };
        let query = match &a.custom_sql {
            Some(sql) => QuerySource::Custom { sql: sql.clone() },
            None => QuerySource::Generated,
        };
        Some((
            ViewLocation {
                catalog: a.catalog.clone(),
                schema: a.schema.clone(),
                table: a.table.clone(),
                action,
                partition_path,
            },
            query,
        ))
    }

    /// The partition path as `(column, value)` predicates.
    pub fn filters(&self) -> Vec<(String, String)> {
        self.partition_path
            .iter()
            .map(|s| (s.column.clone(), s.value.clone()))
            .collect()
    }

    /// `catalog › schema › table · Action · col=value › col=value`
    pub fn breadcrumb(&self) -> String {
        let mut out = format!(
            "{} › {} › {} · {}",
            self.catalog,
            self.schema,
            self.table,
            self.action.label()
        );
        if !self.partition_path.is_empty() {
            let path = self
                .partition_path
                .iter()
                .map(|s| format!("{}={}", s.column, s.value))
                .collect::<Vec<_>>()
                .join(" › ");
            out.push_str(" · ");
            out.push_str(&path);
        }
        out
    }

    /// How many leading steps of the saved partition path still match the
    /// table's current partition columns, in order. Anything after the first
    /// mismatch (renamed, reordered or removed column) cannot be restored.
    pub fn matching_partition_depth(&self, partitioned_by: &[String]) -> usize {
        self.partition_path
            .iter()
            .zip(partitioned_by)
            .take_while(|(step, col)| same_partition_column(&step.column, col))
            .count()
    }
}

/// Partition columns compare case-insensitively and ignoring quoting and
/// whitespace, so `"Date"`, `date` and transforms such as `day( ts )` vs
/// `day(ts)` match.
pub fn same_partition_column(a: &str, b: &str) -> bool {
    normalize_partition_column(a) == normalize_partition_column(b)
}

fn normalize_partition_column(s: &str) -> String {
    s.chars()
        .filter(|c| !c.is_whitespace() && !matches!(c, '"' | '\'' | '`'))
        .flat_map(char::to_lowercase)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{ActionState, DrillDownState};

    fn loc(path: &[(&str, &str)]) -> ViewLocation {
        ViewLocation {
            catalog: "c".into(),
            schema: "s".into(),
            table: "t".into(),
            action: Action::TableView,
            partition_path: path
                .iter()
                .map(|(c, v)| PartitionStep {
                    column: c.to_string(),
                    value: v.to_string(),
                })
                .collect(),
        }
    }

    #[test]
    fn capture_reads_the_table_screen() {
        let mut app = App::new(crate::config::ConnectionConfig::default(), false);
        assert!(ViewLocation::capture(&app).is_none());

        app.screen = Screen::Actions(ActionState {
            catalog: "c".into(),
            schema: "s".into(),
            table: "t".into(),
            selected: Action::TableView.index(),
            drilldown: Some(DrillDownState {
                partition_cols: vec!["date".into(), "svc".into()],
                path: vec![("date".into(), "2026-10-13".into())],
                ..Default::default()
            }),
            ..Default::default()
        });
        let (location, query) = ViewLocation::capture(&app).unwrap();
        assert_eq!(location, loc(&[("date", "2026-10-13")]));
        assert_eq!(query, QuerySource::Generated);

        // custom SQL is kept; partition paths only belong to Table View
        if let Screen::Actions(a) = &mut app.screen {
            a.custom_sql = Some("SELECT 1 FROM t".into());
            a.selected = Action::Sample.index();
        }
        let (location, query) = ViewLocation::capture(&app).unwrap();
        assert_eq!(location.action, Action::Sample);
        assert!(location.partition_path.is_empty());
        assert_eq!(
            query,
            QuerySource::Custom {
                sql: "SELECT 1 FROM t".into()
            }
        );
    }

    #[test]
    fn partition_prefix_survives_cosmetic_differences_and_stops_at_changes() {
        let l = loc(&[("Date", "d"), ("day( ts )", "x"), ("svc", "s")]);
        // same layout, different spelling
        assert_eq!(
            l.matching_partition_depth(&["date".into(), "day(ts)".into(), "\"SVC\"".into()]),
            3
        );
        // a column was renamed / reordered: keep what still matches
        assert_eq!(
            l.matching_partition_depth(&["date".into(), "service".into()]),
            1
        );
        assert_eq!(
            l.matching_partition_depth(&["svc".into(), "date".into()]),
            0
        );
        // table no longer partitioned
        assert_eq!(l.matching_partition_depth(&[]), 0);
        // more levels than were saved: everything saved still matches
        let short = loc(&[("date", "d")]);
        assert_eq!(
            short.matching_partition_depth(&["date".into(), "svc".into()]),
            1
        );
    }

    #[test]
    fn serializes_actions_by_name() {
        let json = serde_json::to_string(&loc(&[("date", "d")])).unwrap();
        assert!(json.contains("\"action\":\"table_view\""), "{json}");
        let ddl = ViewLocation {
            action: Action::TableDDL,
            ..loc(&[])
        };
        let json = serde_json::to_string(&ddl).unwrap();
        assert!(json.contains("\"table_ddl\""));
        assert!(!json.contains("partition_path"));
        let back: ViewLocation = serde_json::from_str(&json).unwrap();
        assert_eq!(back, ddl);
    }

    #[test]
    fn breadcrumb_reads_naturally() {
        assert_eq!(
            loc(&[("date", "d"), ("svc", "s")]).breadcrumb(),
            "c › s › t · Table View Mode · date=d › svc=s"
        );
    }
}
