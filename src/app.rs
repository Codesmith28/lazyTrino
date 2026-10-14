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

use std::cell::Cell;
use std::collections::{HashMap, HashSet};

use crate::config::ConnectionConfig;
use crate::trino::client::TrinoClient;

#[derive(Clone, Debug, PartialEq)]
pub enum QueryStatus {
    Running,
    Success,
    Error,
}

#[derive(Clone, Debug)]
pub struct QueryLogEntry {
    pub id: usize,
    pub sql: String,
    pub status: QueryStatus,
    pub duration_ms: Option<u64>,
    pub row_count: Option<usize>,
    pub error_msg: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct VerticalColumn {
    pub index: usize,
    pub name: String,
    pub data_type: String,
    pub is_nullable: String,
    pub key_meta: String,
    pub description: String,
}

/// Returns the subset of column names safe to include in a `SELECT` list
/// for row-level drill-down queries. Trino/Parquet can fail an entire
/// query with an "Unsupported ... Parquet column" error when a `map` or
/// `row` (struct) typed column's on-disk encoding doesn't match what the
/// catalog metadata declares — a schema-drift issue seen in some Hudi
/// tables. Excluding those columns lets the rest of the row still be
/// readable instead of the whole query failing. Falls back to an empty
/// list (meaning "use `SELECT *`") when there's no cached schema yet.
pub fn safe_select_columns(columns: &[VerticalColumn]) -> Vec<String> {
    if columns.is_empty() {
        return Vec::new();
    }
    let safe: Vec<String> = columns
        .iter()
        .filter(|c| {
            let t = c.data_type.trim().to_ascii_lowercase();
            !(t.starts_with("map(") || t.starts_with("row(") || t == "map" || t == "row")
        })
        .map(|c| c.name.clone())
        .collect();
    // If everything got filtered out (unexpected), fall back to `SELECT *`
    // rather than sending an empty/invalid column list.
    if safe.is_empty() { Vec::new() } else { safe }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ActivePanel {
    MenuPane,
    MainViewer,
}

#[derive(Clone)]
pub struct ConnectState {
    pub url: String,
    pub user: String,
    pub password: String,
    pub focused: usize,
    pub loading: bool,
    pub error: Option<String>,
}

impl Default for ConnectState {
    fn default() -> Self {
        let default_config = ConnectionConfig::default();
        Self {
            url: default_config.url,
            user: default_config.user,
            password: String::new(),
            focused: 0,
            loading: false,
            error: None,
        }
    }
}

#[derive(Clone)]
pub struct CatalogState {
    pub items: Vec<String>,
    pub selected: usize,
}

#[derive(Clone)]
pub struct SchemaState {
    pub catalog: String,
    pub items: Vec<String>,
    pub selected: usize,
}

#[derive(Clone)]
pub struct TableState {
    pub catalog: String,
    pub schema: String,
    pub items: Vec<String>,
    pub selected: usize,
}

pub trait SelectableList {
    fn items(&self) -> &[String];
    fn selected(&self) -> usize;
    fn set_selected(&mut self, selected: usize);
    fn selected_item(&self) -> Option<&str> {
        let items = self.items();
        let idx = self.selected();
        if idx < items.len() {
            Some(&items[idx])
        } else {
            None
        }
    }
}

impl SelectableList for CatalogState {
    fn items(&self) -> &[String] {
        &self.items
    }
    fn selected(&self) -> usize {
        self.selected
    }
    fn set_selected(&mut self, selected: usize) {
        self.selected = selected;
    }
}

impl SelectableList for SchemaState {
    fn items(&self) -> &[String] {
        &self.items
    }
    fn selected(&self) -> usize {
        self.selected
    }
    fn set_selected(&mut self, selected: usize) {
        self.selected = selected;
    }
}

impl SelectableList for TableState {
    fn items(&self) -> &[String] {
        &self.items
    }
    fn selected(&self) -> usize {
        self.selected
    }
    fn set_selected(&mut self, selected: usize) {
        self.selected = selected;
    }
}

/// Ordered partition-column layout + storage location for a table, parsed
/// once (per table) from a live `SHOW CREATE TABLE` response during table
/// entry recon. `partitioned_by` is empty for unpartitioned tables. This is
/// always derived dynamically from that table's own DDL — never hardcoded
/// by table/schema name.
#[derive(Clone, Debug, Default)]
pub struct TableRecon {
    pub partitioned_by: Vec<String>,
    pub location: String,
    /// Raw `SHOW CREATE TABLE` DDL text fetched during recon. Cached here so
    /// `Action::TableDDL` can render it instantly without re-querying Trino.
    pub ddl_text: String,
}

/// Tracks cd/ls-style drill-down progress through a partitioned table's
/// hierarchy: which partition columns exist (from `TableRecon`), which
/// values have been fixed so far (`path`), and the distinct values shown
/// at each previously-visited depth (`levels_cache`, so navigating back up
/// with `h` doesn't need to re-query Trino).
#[derive(Clone, Debug, Default)]
pub struct DrillDownState {
    pub partition_cols: Vec<String>,
    pub path: Vec<(String, String)>,
    pub levels_cache: Vec<Vec<String>>,
    pub selected: usize,
    pub loading: bool,
    pub error: Option<String>,
    pub truncated: bool,
    pub table_count: Option<usize>,
    pub table_count_loading: bool,
    pub counts_cache: HashMap<Vec<(String, String)>, usize>,
    pub current_count_loading: bool,
}

impl DrillDownState {
    /// Depth (0-based) of the level currently being displayed, i.e. how
    /// many partition columns have already been fixed.
    pub fn depth(&self) -> usize {
        self.path.len()
    }

    /// True once every partition column has a fixed value and the caller
    /// should switch to the leaf record view instead of another distinct
    /// value list.
    pub fn is_leaf(&self) -> bool {
        self.path.len() >= self.partition_cols.len()
    }

    /// The next partition column to fetch distinct values for, if any
    /// levels remain.
    pub fn next_column(&self) -> Option<&str> {
        self.partition_cols.get(self.path.len()).map(|s| s.as_str())
    }

    /// Gets the cached count for the current partition path (or table count if path is empty).
    pub fn current_count(&self) -> Option<usize> {
        self.counts_cache
            .get(&self.path)
            .copied()
            .or(if self.path.is_empty() { self.table_count } else { None })
    }

    /// Gets the cached count for a specific prefix of the path up to `depth`.
    pub fn count_at_depth(&self, depth: usize) -> Option<usize> {
        if depth == 0 {
            self.table_count
                .or_else(|| self.counts_cache.get(&vec![]).copied())
        } else {
            let prefix: Vec<(String, String)> = self.path[..depth.min(self.path.len())].to_vec();
            self.counts_cache.get(&prefix).copied()
        }
    }
}

#[derive(Clone, Default)]
pub struct ActionState {
    pub catalog: String,
    pub schema: String,
    pub table: String,
    pub selected: usize,
    pub query_buffer: String,
    pub query_cursor: usize,
    pub results: Option<ResultsState>,
    pub metadata: Option<TableRecon>,
    /// True until `SHOW CREATE TABLE` (the DDL/partition-layout part of
    /// recon) resolves. Only gates `Table DDL` and `Table View`, which only
    /// need `metadata`, not the slower `$partitions`/info-schema queries.
    pub ddl_loading: bool,
    /// True until the `$partitions` and `information_schema.columns` recon
    /// queries resolve. Gates `Partitions` and `Schema`.
    pub metadata_loading: bool,
    pub drilldown: Option<DrillDownState>,
}

#[derive(Clone, Debug, Default)]
pub struct ResultsState {
    pub query_buffer: String,
    pub query_cursor: usize,
    pub columns: Vec<String>,
    pub rows: Vec<Vec<String>>,
    pub scroll_v: Cell<usize>,
    pub scroll_h: Cell<usize>,
    pub selected_row: usize,
    pub selected_col: usize,
    pub last_selected_row: Cell<usize>,
    pub last_selected_col: Cell<usize>,
    pub loading: bool,
    pub error: Option<String>,
    pub is_paginated: bool,
    pub catalog: String,
    pub schema: String,
    pub table: String,
    pub offset: usize,
    pub page_size: usize,
    pub is_fetching_next_page: bool,
    pub has_more_rows: bool,
    pub invalid_query_error: Option<String>,
    pub selection_anchor: Option<usize>,
    /// Partition predicates (`col`, `value`) baked into this view's query,
    /// e.g. when viewing leaf-level records reached via partition
    /// drill-down. Empty for ordinary (non-drill-down) queries. Threaded
    /// through to `FetchNextPage` so infinite scroll keeps the same
    /// predicate on subsequent pages.
    pub filters: Vec<(String, String)>,
    pub total_count: Option<usize>,
    pub total_count_loading: bool,
    pub table_count: Option<usize>,
    pub base_query: Option<String>,
    pub unfiltered_rows: Option<Vec<Vec<String>>>,
    pub column_filters: HashMap<String, ColumnFilter>,
    pub col_distinct_values: HashMap<String, Vec<String>>,
    pub hidden_columns: HashSet<String>,
    pub sort: Option<(String, SortDirection)>,
    pub unsorted_rows: Option<Vec<Vec<String>>>,
}

pub fn format_count(n: usize) -> String {
    let s = n.to_string();
    let mut result = String::with_capacity(s.len() + s.len() / 3);
    let chars: Vec<char> = s.chars().collect();
    let len = chars.len();
    for (i, c) in chars.iter().enumerate() {
        if i > 0 && (len - i).is_multiple_of(3) {
            result.push(',');
        }
        result.push(*c);
    }
    result
}

impl ResultsState {
    pub fn is_column_visible(&self, col_name: &str) -> bool {
        !self.hidden_columns.contains(col_name)
    }

    pub fn visible_columns(&self) -> Vec<(usize, &String)> {
        if self.hidden_columns.is_empty() {
            self.columns.iter().enumerate().collect()
        } else {
            let vis: Vec<_> = self
                .columns
                .iter()
                .enumerate()
                .filter(|(_, c)| !self.hidden_columns.contains(*c))
                .collect();
            if vis.is_empty() {
                self.columns.iter().enumerate().collect()
            } else {
                vis
            }
        }
    }

    pub fn next_visible_col(&self) -> usize {
        let vis = self.visible_columns();
        if vis.is_empty() {
            return self.selected_col;
        }
        if let Some(pos) = vis.iter().position(|(idx, _)| *idx == self.selected_col) {
            let next_pos = (pos + 1).min(vis.len() - 1);
            vis[next_pos].0
        } else {
            vis.iter()
                .find(|(idx, _)| *idx > self.selected_col)
                .map(|(idx, _)| *idx)
                .unwrap_or_else(|| vis.last().map(|(idx, _)| *idx).unwrap_or(0))
        }
    }

    pub fn prev_visible_col(&self) -> usize {
        let vis = self.visible_columns();
        if vis.is_empty() {
            return self.selected_col;
        }
        if let Some(pos) = vis.iter().position(|(idx, _)| *idx == self.selected_col) {
            let prev_pos = pos.saturating_sub(1);
            vis[prev_pos].0
        } else {
            vis.iter()
                .rfind(|(idx, _)| *idx < self.selected_col)
                .map(|(idx, _)| *idx)
                .unwrap_or_else(|| vis.first().map(|(idx, _)| *idx).unwrap_or(0))
        }
    }

    pub fn first_visible_col(&self) -> usize {
        self.visible_columns().first().map(|(idx, _)| *idx).unwrap_or(0)
    }

    pub fn last_visible_col(&self) -> usize {
        self.visible_columns().last().map(|(idx, _)| *idx).unwrap_or(0)
    }

    pub fn selection_range(&self) -> Option<(usize, usize)> {
        if let Some(anchor) = self.selection_anchor
            && anchor != self.query_cursor
        {
            let start = anchor.min(self.query_cursor);
            let end = anchor.max(self.query_cursor);
            return Some((start, end));
        }
        None
    }

    pub fn delete_selection(&mut self) -> bool {
        if let Some((start, end)) = self.selection_range() {
            let start = start.min(self.query_buffer.len());
            let end = end.min(self.query_buffer.len());
            if start < end {
                self.query_buffer.drain(start..end);
                self.query_cursor = start;
                self.selection_anchor = None;
                return true;
            }
        }
        false
    }

    pub fn clear_selection(&mut self) {
        self.selection_anchor = None;
    }

    pub fn select_all(&mut self) {
        self.selection_anchor = Some(0);
        self.query_cursor = self.query_buffer.len();
    }

    pub fn toggle_sort(&mut self, vertical_schema_cols: &[VerticalColumn]) {
        if self.columns.is_empty() || self.rows.is_empty() {
            return;
        }
        let col_idx = self.selected_col.min(self.columns.len().saturating_sub(1));
        let col_name = self.columns[col_idx].clone();

        if self.unsorted_rows.is_none() {
            self.unsorted_rows = Some(self.rows.clone());
        }

        let next_sort = match &self.sort {
            Some((cur_col, SortDirection::Desc)) if cur_col == &col_name => {
                Some((col_name.clone(), SortDirection::Asc))
            }
            Some((cur_col, SortDirection::Asc)) if cur_col == &col_name => None,
            _ => Some((col_name.clone(), SortDirection::Desc)),
        };

        self.sort = next_sort;
        self.apply_sort(vertical_schema_cols);
    }

    pub fn apply_sort(&mut self, vertical_schema_cols: &[VerticalColumn]) {
        if let Some((ref col_name, direction)) = self.sort {
            if let Some(col_idx) = self.columns.iter().position(|c| c == col_name) {
                let data_type = vertical_schema_cols
                    .iter()
                    .find(|c| c.name.eq_ignore_ascii_case(col_name))
                    .map(|c| c.data_type.as_str());

                self.rows.sort_by(|row_a, row_b| {
                    let val_a = row_a.get(col_idx).map(|s| s.as_str()).unwrap_or("");
                    let val_b = row_b.get(col_idx).map(|s| s.as_str()).unwrap_or("");
                    let is_null_a = val_a.trim().is_empty() || val_a.trim().eq_ignore_ascii_case("null");
                    let is_null_b = val_b.trim().is_empty() || val_b.trim().eq_ignore_ascii_case("null");

                    if is_null_a && is_null_b {
                        return std::cmp::Ordering::Equal;
                    }
                    if is_null_a {
                        return std::cmp::Ordering::Greater;
                    }
                    if is_null_b {
                        return std::cmp::Ordering::Less;
                    }

                    let ord = compare_non_null_cell_values(val_a.trim(), val_b.trim(), data_type);
                    match direction {
                        SortDirection::Asc => ord,
                        SortDirection::Desc => ord.reverse(),
                    }
                });
            }
        } else if let Some(ref unsorted) = self.unsorted_rows {
            if self.column_filters.is_empty() {
                self.rows = unsorted.clone();
            } else {
                self.rows = unsorted
                    .iter()
                    .filter(|row| {
                        for (filter_col, filter) in &self.column_filters {
                            if let Some(idx) = self.columns.iter().position(|c| c == filter_col) {
                                let val = row.get(idx).map(|s| s.as_str()).unwrap_or("");
                                if !filter.matches(val) {
                                    return false;
                                }
                            }
                        }
                        true
                    })
                    .cloned()
                    .collect();
            }
            self.unsorted_rows = None;
        }
        self.selected_row = 0;
        self.scroll_v.set(0);
    }
}

// `ActionState` (which embeds the optional `ResultsState` with query text/rows) is
// substantially larger than the other variants. `Screen` is only ever held as a single
// top-level app field (never collected into large vectors or hot loops), so boxing the
// large variant would add pattern-matching overhead across handler.rs/mod.rs for no
// measurable benefit.
#[allow(clippy::large_enum_variant)]
#[derive(Clone)]
pub enum Screen {
    Connect(ConnectState),
    Catalog(CatalogState),
    Schema(SchemaState),
    Table(TableState),
    Actions(ActionState),
    Help,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SortDirection {
    Desc,
    Asc,
}

pub fn is_numeric_type(t: &str) -> bool {
    let t = t.to_lowercase();
    t.starts_with("tinyint")
        || t.starts_with("smallint")
        || t.starts_with("integer")
        || t.starts_with("int")
        || t.starts_with("bigint")
        || t.starts_with("real")
        || t.starts_with("float")
        || t.starts_with("double")
        || t.starts_with("decimal")
        || t.starts_with("numeric")
}

pub fn compare_cell_values(a: &str, b: &str, data_type: Option<&str>) -> std::cmp::Ordering {
    let a_trim = a.trim();
    let b_trim = b.trim();
    let is_null_a = a_trim.is_empty() || a_trim.eq_ignore_ascii_case("null");
    let is_null_b = b_trim.is_empty() || b_trim.eq_ignore_ascii_case("null");

    if is_null_a && is_null_b {
        return std::cmp::Ordering::Equal;
    }
    if is_null_a {
        return std::cmp::Ordering::Greater;
    }
    if is_null_b {
        return std::cmp::Ordering::Less;
    }

    compare_non_null_cell_values(a_trim, b_trim, data_type)
}

pub fn compare_non_null_cell_values(a: &str, b: &str, data_type: Option<&str>) -> std::cmp::Ordering {
    let is_numeric = data_type.map(is_numeric_type).unwrap_or(false)
        || (a.parse::<f64>().is_ok() && b.parse::<f64>().is_ok());

    if is_numeric {
        if let (Ok(num_a), Ok(num_b)) = (a.parse::<f64>(), b.parse::<f64>()) {
            return num_a.partial_cmp(&num_b).unwrap_or(std::cmp::Ordering::Equal);
        }
    }

    if let (Ok(bool_a), Ok(bool_b)) = (a.parse::<bool>(), b.parse::<bool>()) {
        return bool_a.cmp(&bool_b);
    }

    a.to_lowercase().cmp(&b.to_lowercase()).then_with(|| a.cmp(b))
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Mode {
    Normal,
    Search,
    QueryInput,
    FilterModal,
    ColumnModal,
    HelpModal,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NumericOp {
    Lt,
    Lte,
    Gt,
    Gte,
    Eq,
}

impl NumericOp {
    pub const ALL: [NumericOp; 5] = [
        NumericOp::Lt,
        NumericOp::Lte,
        NumericOp::Gt,
        NumericOp::Gte,
        NumericOp::Eq,
    ];

    pub fn as_str(&self) -> &'static str {
        match self {
            NumericOp::Lt => "<",
            NumericOp::Lte => "<=",
            NumericOp::Gt => ">",
            NumericOp::Gte => ">=",
            NumericOp::Eq => "=",
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            NumericOp::Lt => "Less than (<)",
            NumericOp::Lte => "Less than or equal (<=)",
            NumericOp::Gt => "Greater than (>)",
            NumericOp::Gte => "Greater than or equal (>=)",
            NumericOp::Eq => "Equal (=)",
        }
    }

    pub fn matches(&self, val: f64, target: f64) -> bool {
        match self {
            NumericOp::Lt => val < target,
            NumericOp::Lte => val <= target,
            NumericOp::Gt => val > target,
            NumericOp::Gte => val >= target,
            NumericOp::Eq => (val - target).abs() < 1e-9 || val == target,
        }
    }

    pub fn from_str_prefix(s: &str) -> Option<(NumericOp, &str)> {
        let s = s.trim_start();
        if let Some(rest) = s.strip_prefix("<=") {
            Some((NumericOp::Lte, rest.trim()))
        } else if let Some(rest) = s.strip_prefix(">=") {
            Some((NumericOp::Gte, rest.trim()))
        } else if let Some(rest) = s.strip_prefix("<") {
            Some((NumericOp::Lt, rest.trim()))
        } else if let Some(rest) = s.strip_prefix(">") {
            Some((NumericOp::Gt, rest.trim()))
        } else if let Some(rest) = s.strip_prefix("==") {
            Some((NumericOp::Eq, rest.trim()))
        } else if let Some(rest) = s.strip_prefix("=") {
            Some((NumericOp::Eq, rest.trim()))
        } else {
            None
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct NumericFilter {
    pub op: NumericOp,
    pub value: f64,
    pub raw_value: String,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ColumnFilter {
    Values(HashSet<String>),
    Numeric(NumericFilter),
}

impl From<HashSet<String>> for ColumnFilter {
    fn from(s: HashSet<String>) -> Self {
        ColumnFilter::Values(s)
    }
}

impl ColumnFilter {
    pub fn matches(&self, cell_value: &str) -> bool {
        match self {
            ColumnFilter::Values(selected) => selected.contains(cell_value),
            ColumnFilter::Numeric(num) => {
                let s = cell_value.trim();
                if s.is_empty() || s.eq_ignore_ascii_case("null") {
                    return false;
                }
                if let Ok(v) = s.parse::<f64>() {
                    num.op.matches(v, num.value)
                } else {
                    false
                }
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FilterMode {
    Values,
    Numeric,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NumericFocus {
    Operator,
    Value,
}

#[derive(Clone, Debug, PartialEq)]
pub struct FilterModalState {
    pub column_index: usize,
    pub column_name: String,
    pub is_numeric_col: bool,
    pub mode: FilterMode,
    pub values: Vec<String>,
    pub selected_values: HashSet<String>,
    pub cursor: usize,
    pub scroll: usize,
    pub search_query: String,
    pub is_searching: bool,
    pub numeric_op: NumericOp,
    pub numeric_input: String,
    pub numeric_focus: NumericFocus,
    pub numeric_error: Option<String>,
    pub last_op_char: Option<char>,
}

impl FilterModalState {
    pub fn filtered_indices(&self) -> Vec<usize> {
        if self.search_query.is_empty() {
            (0..self.values.len()).collect()
        } else if self.is_numeric_col
            && let Some((op, target_str)) = NumericOp::from_str_prefix(&self.search_query)
            && let Ok(target) = target_str.parse::<f64>()
        {
            self.values
                .iter()
                .enumerate()
                .filter(|(_, v)| {
                    v.trim()
                        .parse::<f64>()
                        .map(|num| op.matches(num, target))
                        .unwrap_or(false)
                })
                .map(|(i, _)| i)
                .collect()
        } else {
            let q = self.search_query.to_lowercase();
            self.values
                .iter()
                .enumerate()
                .filter(|(_, v)| v.to_lowercase().contains(&q))
                .map(|(i, _)| i)
                .collect()
        }
    }

    pub fn parse_numeric_filter(&self) -> Result<Option<(NumericOp, f64, String)>, String> {
        let trimmed = self.numeric_input.trim();
        if trimmed.is_empty() {
            return Ok(None);
        }
        if let Some((op, rest)) = NumericOp::from_str_prefix(trimmed) {
            let rest_trimmed = rest.trim();
            if rest_trimmed.is_empty() {
                return Err("Enter a number after operator (e.g. >= 50)".to_string());
            }
            match rest_trimmed.parse::<f64>() {
                Ok(val) => Ok(Some((op, val, rest_trimmed.to_string()))),
                Err(_) => Err(format!("Invalid number: '{rest_trimmed}'")),
            }
        } else {
            match trimmed.parse::<f64>() {
                Ok(val) => Ok(Some((self.numeric_op, val, trimmed.to_string()))),
                Err(_) => Err(format!("Invalid number: '{trimmed}'")),
            }
        }
    }

    pub fn numeric_matching_count(&self) -> Option<(usize, usize)> {
        if let Ok(Some((op, target, _))) = self.parse_numeric_filter() {
            let mut matching = 0;
            let mut total = 0;
            for v in &self.values {
                let s = v.trim();
                if !s.is_empty()
                    && !s.eq_ignore_ascii_case("null")
                    && let Ok(num) = s.parse::<f64>()
                {
                    total += 1;
                    if op.matches(num, target) {
                        matching += 1;
                    }
                }
            }
            Some((matching, total))
        } else {
            None
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ColumnModalState {
    pub columns: Vec<String>,
    pub selected_columns: HashSet<String>,
    pub cursor: usize,
    pub scroll: usize,
    pub search_query: String,
    pub is_searching: bool,
}

impl ColumnModalState {
    pub fn filtered_indices(&self) -> Vec<usize> {
        if self.search_query.is_empty() {
            (0..self.columns.len()).collect()
        } else {
            let q = self.search_query.to_lowercase();
            self.columns
                .iter()
                .enumerate()
                .filter(|(_, c)| c.to_lowercase().contains(&q))
                .map(|(i, _)| i)
                .collect()
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct HelpModalState {
    pub search_query: String,
    pub is_searching: bool,
    pub scroll: usize,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Action {
    TableView,
    TableDDL,
    Schema,
    ShowStats,
    Sample,
    Partitions,
}

pub const ACTIONS: &[(char, &str, Action)] = &[
    ('v', "Table View Mode", Action::TableView),
    ('c', "Table DDL", Action::TableDDL),
    ('e', "Schema", Action::Schema),
    ('t', "Show Stats", Action::ShowStats),
    ('m', "Sample Mode (20 rows)", Action::Sample),
    ('p', "Partitions", Action::Partitions),
];

impl Action {
    pub fn build_query(&self, catalog: &str, schema: &str, table: &str) -> String {
        match self {
            Action::TableView => crate::trino::queries::page_query(catalog, schema, table, 0, 100),
            Action::TableDDL => crate::trino::queries::show_create(catalog, schema, table),
            Action::Schema => crate::trino::queries::describe(catalog, schema, table),
            Action::ShowStats => crate::trino::queries::show_stats(catalog, schema, table),
            Action::Sample => crate::trino::queries::sample(catalog, schema, table),
            Action::Partitions => crate::trino::queries::show_create(catalog, schema, table),
        }
    }
}

pub struct App {
    pub screen: Screen,
    pub prev_screen: Option<Box<Screen>>,
    pub mode: Mode,
    pub should_quit: bool,
    pub number_buffer: String,
    pub search_query: String,
    pub loading: bool,
    pub trino_client: Option<TrinoClient>,
    pub config: ConnectionConfig,
    pub catalogs: Vec<String>,
    pub schemas: HashMap<String, Vec<String>>,
    pub tables: HashMap<(String, String), Vec<String>>,
    pub frame_count: u64,
    pub query_logs: Vec<QueryLogEntry>,
    pub active_panel: ActivePanel,
    pub partition_tree_lines: Vec<String>,
    pub partition_scroll: usize,
    pub vertical_schema_cols: Vec<VerticalColumn>,
    pub schema_scroll: usize,
    pub auto_connect: bool,
    pub main_panel_pct: u16,
    pub control_panel_split_pct: u16,
    pub is_dragging_resizer: bool,
    pub is_dragging_query_select: bool,
    pub is_dragging_h_scrollbar: bool,
    pub query_inspector_scroll: usize,
    pub mouse_selection_anchor: Option<(u16, u16)>,
    pub mouse_selection_current: Option<(u16, u16)>,
    pub is_selecting_text: bool,
    pub copied_toast: Option<(String, std::time::Instant)>,
    pub filter_modal: Option<FilterModalState>,
    pub column_modal: Option<ColumnModalState>,
    pub help_modal: Option<HelpModalState>,
    pub help_scroll: usize,
}

impl App {
    pub fn new(config: ConnectionConfig, auto_connect: bool) -> Self {
        Self {
            screen: Screen::Connect(ConnectState {
                url: config.url.clone(),
                user: config.user.clone(),
                password: config.password.clone(),
                focused: 0,
                loading: false,
                error: None,
            }),
            prev_screen: None,
            mode: Mode::Normal,
            should_quit: false,
            number_buffer: String::new(),
            search_query: String::new(),
            loading: false,
            trino_client: None,
            config,
            catalogs: Vec::new(),
            schemas: HashMap::new(),
            tables: HashMap::new(),
            frame_count: 0,
            query_logs: Vec::new(),
            active_panel: ActivePanel::MainViewer,
            partition_tree_lines: Vec::new(),
            partition_scroll: 0,
            vertical_schema_cols: Vec::new(),
            schema_scroll: 0,
            auto_connect,
            main_panel_pct: 60,
            control_panel_split_pct: 40,
            is_dragging_resizer: false,
            is_dragging_query_select: false,
            is_dragging_h_scrollbar: false,
            query_inspector_scroll: 0,
            mouse_selection_anchor: None,
            mouse_selection_current: None,
            is_selecting_text: false,
            copied_toast: None,
            filter_modal: None,
            column_modal: None,
            help_modal: None,
            help_scroll: 0,
        }
    }

    pub fn add_query_log(&mut self, sql: String) -> usize {
        let id = self.query_logs.len() + 1;
        self.query_logs.push(QueryLogEntry {
            id,
            sql,
            status: QueryStatus::Running,
            duration_ms: None,
            row_count: None,
            error_msg: None,
        });
        id
    }

    pub fn complete_query_log_success(&mut self, id: usize, duration_ms: u64, row_count: usize) {
        if let Some(entry) = self.query_logs.iter_mut().find(|e| e.id == id) {
            entry.status = QueryStatus::Success;
            entry.duration_ms = Some(duration_ms);
            entry.row_count = Some(row_count);
        }
    }

    pub fn complete_query_log_error(&mut self, id: usize, err: String) {
        if let Some(entry) = self.query_logs.iter_mut().find(|e| e.id == id) {
            entry.status = QueryStatus::Error;
            entry.error_msg = Some(err);
        }
    }

    /// Clears any leftover mouse click/drag highlight. `mouse_selection_anchor`
    /// / `mouse_selection_current` are otherwise only reset by the *next*
    /// mouse click, so without calling this on every keyboard-driven change
    /// of focus (active panel or screen), a row/cell clicked once would keep
    /// rendering as "selected" forever, even after navigating elsewhere.
    pub fn clear_mouse_selection(&mut self) {
        self.mouse_selection_anchor = None;
        self.mouse_selection_current = None;
    }

    /// Switches the focused pane, clearing any stale mouse-click highlight
    /// left over from the pane being unfocused.
    pub fn set_active_panel(&mut self, panel: ActivePanel) {
        if self.active_panel != panel {
            self.clear_mouse_selection();
        }
        self.active_panel = panel;
    }

    /// Replaces the current screen, clearing any stale mouse-click highlight
    /// left over from the previous screen (e.g. a clicked catalog row must
    /// not still appear "selected" once you've drilled into its schemas).
    pub fn set_screen(&mut self, screen: Screen) {
        self.clear_mouse_selection();
        self.screen = screen;
    }

    pub fn is_area_mouse_selected(&self, area_x: u16, area_w: u16, row_y: u16) -> bool {
        self.is_area_mouse_selected_rect(area_x, area_w, row_y, 1)
    }

    pub fn is_area_mouse_selected_rect(
        &self,
        area_x: u16,
        area_w: u16,
        area_y: u16,
        area_h: u16,
    ) -> bool {
        if !self.is_selecting_text {
            return false;
        }
        if let (Some((a_x, a_y)), Some((c_x, c_y))) =
            (self.mouse_selection_anchor, self.mouse_selection_current)
        {
            if a_x == c_x && a_y == c_y {
                return false;
            }
            let start_y = a_y.min(c_y);
            let end_y = a_y.max(c_y);
            let start_x = a_x.min(c_x);
            let end_x = a_x.max(c_x);

            let y_match = area_y <= end_y && area_y.saturating_add(area_h) > start_y;
            let x_match = area_x <= end_x && area_x.saturating_add(area_w) > start_x;
            y_match && x_match
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ConnectionConfig;
    use crate::trino::queries;

    fn sample_config() -> ConnectionConfig {
        ConnectionConfig {
            url: "https://trino.example".to_string(),
            user: "analyst".to_string(),
            password: "secret".to_string(),
        }
    }

    #[test]
    fn app_new_initializes_connect_screen_and_defaults() {
        let config = sample_config();
        let app = App::new(config.clone(), true);

        let Screen::Connect(state) = &app.screen else {
            panic!("expected connect screen");
        };
        assert_eq!(state.url, config.url);
        assert_eq!(state.user, config.user);
        assert_eq!(state.password, config.password);
        assert_eq!(state.focused, 0);
        assert!(!state.loading);
        assert!(state.error.is_none());

        assert!(app.prev_screen.is_none());
        assert!(matches!(app.mode, Mode::Normal));
        assert!(!app.should_quit);
        assert!(app.number_buffer.is_empty());
        assert!(app.search_query.is_empty());
        assert!(!app.loading);
        assert!(app.trino_client.is_none());
        assert_eq!(app.config, config);
        assert!(app.catalogs.is_empty());
        assert!(app.schemas.is_empty());
        assert!(app.tables.is_empty());
        assert_eq!(app.frame_count, 0);
        assert!(app.query_logs.is_empty());
        assert_eq!(app.active_panel, ActivePanel::MainViewer);
        assert!(app.partition_tree_lines.is_empty());
        assert_eq!(app.partition_scroll, 0);
        assert!(app.vertical_schema_cols.is_empty());
        assert_eq!(app.schema_scroll, 0);
        assert!(app.auto_connect);
        assert_eq!(app.main_panel_pct, 60);
        assert_eq!(app.control_panel_split_pct, 40);
        assert!(!app.is_dragging_resizer);
        assert!(!app.is_dragging_query_select);
        assert_eq!(app.query_inspector_scroll, 0);
        assert!(app.mouse_selection_anchor.is_none());
        assert!(app.mouse_selection_current.is_none());
        assert!(!app.is_selecting_text);
        assert!(app.copied_toast.is_none());
    }

    #[test]
    fn query_log_lifecycle_tracks_success_and_error_entries() {
        let mut app = App::new(sample_config(), false);

        let first_id = app.add_query_log("SELECT 1".to_string());
        let second_id = app.add_query_log("SELECT 2".to_string());

        assert_eq!(first_id, 1);
        assert_eq!(second_id, 2);

        app.complete_query_log_success(first_id, 125, 7);
        app.complete_query_log_error(second_id, "boom".to_string());

        assert_eq!(app.query_logs.len(), 2);

        let first = &app.query_logs[0];
        assert_eq!(first.id, 1);
        assert_eq!(first.sql, "SELECT 1");
        assert_eq!(first.status, QueryStatus::Success);
        assert_eq!(first.duration_ms, Some(125));
        assert_eq!(first.row_count, Some(7));
        assert!(first.error_msg.is_none());

        let second = &app.query_logs[1];
        assert_eq!(second.id, 2);
        assert_eq!(second.sql, "SELECT 2");
        assert_eq!(second.status, QueryStatus::Error);
        assert_eq!(second.duration_ms, None);
        assert_eq!(second.row_count, None);
        assert_eq!(second.error_msg.as_deref(), Some("boom"));
    }

    #[test]
    fn action_build_query_matches_queries_module_for_all_variants() {
        let catalog = "iceberg";
        let schema = "sales";
        let table = "orders";
        let cases = vec![
            (
                Action::TableView,
                queries::page_query(catalog, schema, table, 0, 100),
            ),
            (
                Action::TableDDL,
                queries::show_create(catalog, schema, table),
            ),
            (Action::Schema, queries::describe(catalog, schema, table)),
            (
                Action::ShowStats,
                queries::show_stats(catalog, schema, table),
            ),
            (Action::Sample, queries::sample(catalog, schema, table)),
            (
                Action::Partitions,
                queries::show_create(catalog, schema, table),
            ),
        ];

        for (action, expected) in cases {
            assert_eq!(action.build_query(catalog, schema, table), expected);
        }
    }

    #[test]
    fn format_count_adds_comma_separators() {
        assert_eq!(format_count(0), "0");
        assert_eq!(format_count(999), "999");
        assert_eq!(format_count(1000), "1,000");
        assert_eq!(format_count(1234567), "1,234,567");
    }

    fn vcol(name: &str, data_type: &str) -> VerticalColumn {
        VerticalColumn {
            index: 0,
            name: name.to_string(),
            data_type: data_type.to_string(),
            is_nullable: "YES".to_string(),
            key_meta: String::new(),
            description: String::new(),
        }
    }

    #[test]
    fn safe_select_columns_returns_empty_when_no_schema_cached() {
        assert!(safe_select_columns(&[]).is_empty());
    }

    #[test]
    fn safe_select_columns_excludes_map_and_row_typed_columns() {
        let cols = vec![
            vcol("event_type", "varchar"),
            vcol("policy_id", "map(varchar, varchar)"),
            vcol("nested", "row(a varchar, b bigint)"),
            vcol("timestamp", "bigint"),
        ];
        assert_eq!(
            safe_select_columns(&cols),
            vec!["event_type".to_string(), "timestamp".to_string()]
        );
    }

    #[test]
    fn safe_select_columns_falls_back_to_select_star_when_all_columns_unsafe() {
        let cols = vec![vcol("policy_id", "map(varchar, varchar)")];
        assert!(safe_select_columns(&cols).is_empty());
    }
}
