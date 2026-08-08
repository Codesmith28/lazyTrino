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

use crate::app::ActionState;
use crate::trino::queries;
use crate::tui::handler::Command;

pub fn drilldown_drill_into_selected(
    s: &mut ActionState,
    safe_columns: &[String],
) -> Option<Command> {
    let catalog = s.catalog.clone();
    let schema = s.schema.clone();
    let table = s.table.clone();
    let dd = s.drilldown.as_mut()?;
    if dd.loading || dd.is_leaf() {
        return None;
    }
    let depth = dd.depth();
    let value = dd.levels_cache.get(depth)?.get(dd.selected)?.clone();
    let column = dd.partition_cols.get(depth)?.clone();

    dd.path.push((column, value));
    dd.selected = 0;
    dd.error = None;

    if dd.is_leaf() {
        let filters = dd.path.clone();
        s.results = None;
        let query =
            queries::filtered_page_query(&catalog, &schema, &table, &filters, 0, 100, safe_columns);
        let query_len = query.len();
        s.query_buffer = query.clone();
        s.query_cursor = query_len;
        Some(Command::ExecuteQuery {
            query,
            is_paginated: true,
            catalog,
            schema,
            table,
            filters,
        })
    } else {
        dd.loading = true;
        let next_col = dd.next_column()?.to_string();
        let filters = dd.path.clone();
        Some(Command::FetchPartitionLevel {
            catalog,
            schema,
            table,
            filters,
            column: next_col,
        })
    }
}

pub fn drilldown_go_up(s: &mut ActionState) -> Option<Command> {
    let dd = s.drilldown.as_mut()?;
    if dd.loading {
        return None;
    }
    if dd.path.is_empty() {
        return None;
    }

    let popped = dd.path.pop();
    if popped.is_some() {
        dd.selected = 0;
        dd.error = None;
        dd.truncated = false;
        s.results = None;
    }
    None
}
