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

//! "Copy / export entire view": snapshot whatever the main viewer is showing
//! and either put it on the clipboard or write it to
//! `<Downloads>/lazytrino/<name>_<timestamp>.csv`.

use std::fs::{File, OpenOptions};
use std::io::{BufWriter, ErrorKind, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::app::{
    ACTIONS, Action, App, ExportModalState, Mode, NoticeKind, ResultsState, Screen, ViewExport,
    format_count,
};
use crate::tui::handler::Command;

use super::navigation::list::filter_items;
use super::navigation::normalize_key_code;
use super::query::try_copy_to_clipboard;

/// Above this, copying to the clipboard can visibly freeze the UI, so the
/// modal warns and defaults to the file export instead.
pub(crate) const LARGE_CLIPBOARD_BYTES: usize = 5 * 1024 * 1024;

const EXPORT_DIR_NAME: &str = "lazytrino";
const MAX_STEM_LEN: usize = 100;

pub fn open_export_modal(app: &mut App) {
    match build_view_export(app) {
        Ok(view) => {
            let size_bytes = estimate_size(&view);
            let selected = usize::from(size_bytes > LARGE_CLIPBOARD_BYTES);
            app.export_modal = Some(ExportModalState {
                view,
                selected,
                size_bytes,
            });
            app.mode = Mode::ExportModal;
        }
        Err(message) => app.set_notice(NoticeKind::Error, message),
    }
}

pub fn close_export_modal(app: &mut App) {
    app.export_modal = None;
    app.mode = Mode::Normal;
}

pub fn handle_export_modal_key(app: &mut App, key: KeyEvent) -> Option<Command> {
    let code = normalize_key_code(key.code);
    let mods = key.modifiers;
    if code == KeyCode::Char('c') && mods.contains(KeyModifiers::CONTROL) {
        app.should_quit = true;
        return None;
    }
    // Ctrl+E toggles the modal closed; Cmd+C is the native "copy".
    if code == KeyCode::Char('e') && mods.contains(KeyModifiers::CONTROL) {
        close_export_modal(app);
        return None;
    }
    if code == KeyCode::Char('c') && mods.contains(KeyModifiers::SUPER) {
        finish_export(app, ExportTarget::Clipboard);
        return None;
    }
    // Any other modified key is not one of ours: Ctrl+J must not act as `j`,
    // Alt+E must not export, etc.
    if mods.intersects(KeyModifiers::CONTROL | KeyModifiers::ALT | KeyModifiers::SUPER) {
        return None;
    }

    let selected = app.export_modal.as_ref()?.selected;
    match code {
        KeyCode::Esc | KeyCode::Char('q') => close_export_modal(app),
        KeyCode::Char('c') => finish_export(app, ExportTarget::Clipboard),
        KeyCode::Char('e') => finish_export(app, ExportTarget::CsvFile),
        KeyCode::Enter => finish_export(
            app,
            if selected == 0 {
                ExportTarget::Clipboard
            } else {
                ExportTarget::CsvFile
            },
        ),
        KeyCode::Down | KeyCode::Char('j') | KeyCode::Tab | KeyCode::Up | KeyCode::Char('k') => {
            if let Some(modal) = app.export_modal.as_mut() {
                modal.selected = 1 - modal.selected.min(1);
            }
        }
        _ => {}
    }
    None
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum ExportTarget {
    Clipboard,
    CsvFile,
}

pub(crate) fn finish_export(app: &mut App, target: ExportTarget) {
    let Some(modal) = app.export_modal.take() else {
        return;
    };
    app.mode = Mode::Normal;
    let view = modal.view;

    match target {
        ExportTarget::Clipboard => match try_copy_to_clipboard(&clipboard_text(&view)) {
            Ok(()) => app.set_notice(
                NoticeKind::Success,
                format!("Copied {} to clipboard", describe(&view)),
            ),
            Err(err) => app.set_notice(NoticeKind::Error, format!("Clipboard copy failed: {err}")),
        },
        ExportTarget::CsvFile => match export_to_downloads(&view) {
            Ok(path) => app.set_notice(
                NoticeKind::Success,
                format!("Exported {} to {}", describe(&view), display_path(&path)),
            ),
            Err(err) => app.set_notice(NoticeKind::Error, format!("Export failed: {err}")),
        },
    }
}

/// "1,234 rows × 5 columns" / "42 items".
pub(crate) fn describe(view: &ViewExport) -> String {
    let n = view.rows.len();
    if view.columns.len() <= 1 {
        format!("{} item{}", format_count(n), if n == 1 { "" } else { "s" })
    } else {
        format!(
            "{} row{} × {} columns",
            format_count(n),
            if n == 1 { "" } else { "s" },
            view.columns.len()
        )
    }
}

pub(crate) fn human_bytes(bytes: usize) -> String {
    const KB: f64 = 1024.0;
    let b = bytes as f64;
    if b < KB {
        format!("{bytes} B")
    } else if b < KB * KB {
        format!("{:.1} KB", b / KB)
    } else {
        format!("{:.1} MB", b / (KB * KB))
    }
}

pub(crate) fn estimate_size(view: &ViewExport) -> usize {
    let header: usize = view.columns.iter().map(|c| c.len() + 1).sum();
    let body: usize = view
        .rows
        .iter()
        .map(|r| r.iter().map(|c| c.len() + 1).sum::<usize>())
        .sum();
    header + body
}

/// What lands on the clipboard: raw values for single-column views (so
/// "copy all table names" gives just the names), CSV with a header otherwise.
pub(crate) fn clipboard_text(view: &ViewExport) -> String {
    if view.columns.len() <= 1 {
        view.rows
            .iter()
            .map(|r| r.first().map(String::as_str).unwrap_or(""))
            .collect::<Vec<_>>()
            .join("\n")
    } else {
        rows_to_csv(&view.columns, &view.rows)
    }
}

/// `<Downloads>/lazytrino`, or `None` if the platform has no Downloads dir.
pub(crate) fn export_dir() -> Option<PathBuf> {
    dirs::download_dir()
        .or_else(|| dirs::home_dir().map(|h| h.join("Downloads")))
        .map(|d| d.join(EXPORT_DIR_NAME))
}

/// Where the CSV will land, for the modal (`~/Downloads/lazytrino`).
pub(crate) fn export_dir_display() -> String {
    export_dir().map_or_else(
        || "(Downloads folder not found)".to_string(),
        |d| display_path(&d),
    )
}

fn export_to_downloads(view: &ViewExport) -> Result<PathBuf, String> {
    let dir = export_dir().ok_or("could not determine the Downloads folder")?;
    export_to_dir(view, &dir, &utc_timestamp(now_secs()))
}

pub(crate) fn export_to_dir(
    view: &ViewExport,
    dir: &Path,
    timestamp: &str,
) -> Result<PathBuf, String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let stem = format!("{}_{timestamp}", sanitize_stem(&view.file_stem));
    let (file, path) = create_unique_file(dir, &stem).map_err(|e| e.to_string())?;
    write_csv(file, view).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(path)
}

/// Opens a brand-new `<stem>.csv` (never overwriting), appending `-2`, `-3`…
/// when an export with the same name already exists.
fn create_unique_file(dir: &Path, stem: &str) -> std::io::Result<(File, PathBuf)> {
    for n in 1..=1000 {
        let name = if n == 1 {
            format!("{stem}.csv")
        } else {
            format!("{stem}-{n}.csv")
        };
        let path = dir.join(name);
        match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(file) => return Ok((file, path)),
            Err(err) if err.kind() == ErrorKind::AlreadyExists => continue,
            Err(err) => return Err(err),
        }
    }
    Err(std::io::Error::new(
        ErrorKind::AlreadyExists,
        "too many exports with the same name",
    ))
}

/// Streams the CSV to disk row by row instead of building one giant string.
fn write_csv(file: File, view: &ViewExport) -> std::io::Result<()> {
    let mut out = BufWriter::new(file);
    write_csv_row(&mut out, view.columns.iter().map(String::as_str))?;
    for row in &view.rows {
        write_csv_row(&mut out, row.iter().map(String::as_str))?;
    }
    out.flush()
}

fn write_csv_row<'a>(
    out: &mut impl Write,
    fields: impl Iterator<Item = &'a str>,
) -> std::io::Result<()> {
    out.write_all(format_row(fields, ',').as_bytes())?;
    out.write_all(b"\n")
}

fn display_path(path: &Path) -> String {
    if let Some(home) = dirs::home_dir()
        && let Ok(rest) = path.strip_prefix(&home)
    {
        return format!("~/{}", rest.display());
    }
    path.display().to_string()
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

/// Compact UTC timestamp for file names, e.g. `20261023-101500Z`.
pub(crate) fn utc_timestamp(epoch_secs: u64) -> String {
    let days = (epoch_secs / 86_400) as i64;
    let rem = epoch_secs % 86_400;
    let (y, m, d) = civil_from_days(days);
    format!(
        "{y:04}{m:02}{d:02}-{:02}{:02}{:02}Z",
        rem / 3600,
        rem % 3600 / 60,
        rem % 60
    )
}

/// Days since 1970-01-01 → (year, month, day). Howard Hinnant's algorithm.
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let y = yoe + era * 400 + i64::from(m <= 2);
    (y, m, d)
}

// ---------------------------------------------------------------------------
// Building the snapshot for the current view
// ---------------------------------------------------------------------------

pub(crate) fn build_view_export(app: &App) -> Result<ViewExport, String> {
    let view = match &app.screen {
        Screen::Catalog(s) => list_view(
            "Catalogs".to_string(),
            "catalogs".to_string(),
            "catalog",
            &s.items,
            &app.search_query,
        ),
        Screen::Schema(s) => list_view(
            format!("Schemas — {}", s.catalog.trim()),
            format!("{}_schemas", s.catalog),
            "schema",
            &s.items,
            &app.search_query,
        ),
        Screen::Table(s) => list_view(
            format!("Tables — {}.{}", s.catalog.trim(), s.schema.trim()),
            format!("{}_{}_tables", s.catalog, s.schema),
            "table",
            &s.items,
            &app.search_query,
        ),
        Screen::Actions(_) => actions_view(app)?,
        Screen::Connect(_) | Screen::Help => {
            return Err("Nothing to copy or export on this screen".to_string());
        }
    };

    if view.rows.is_empty() {
        return Err("Nothing to copy or export in this view yet".to_string());
    }
    Ok(view)
}

fn list_view(
    title: String,
    file_stem: String,
    column: &str,
    items: &[String],
    search: &str,
) -> ViewExport {
    ViewExport {
        title,
        file_stem,
        columns: vec![column.to_string()],
        rows: filter_items(items, search)
            .into_iter()
            .map(|x| vec![x.trim().to_string()])
            .collect(),
        note: (!search.is_empty()).then(|| format!("Filtered by search: \"{search}\"")),
    }
}

fn actions_view(app: &App) -> Result<ViewExport, String> {
    let Screen::Actions(s) = &app.screen else {
        return Err("Nothing to copy or export on this screen".to_string());
    };
    let (label, action) = ACTIONS
        .get(s.selected)
        .map(|(_, label, action)| (*label, action))
        .ok_or("Nothing to copy or export in this view")?;
    let base = format!("{}_{}_{}", s.catalog, s.schema, s.table);
    let table_name = format!("{}.{}.{}", s.catalog, s.schema, s.table);

    if let Some(dd) = s.drilldown.as_ref()
        && matches!(action, Action::TableView)
        && !dd.is_leaf()
    {
        if dd.loading {
            return Err("Partition values are still loading".to_string());
        }
        let depth = dd.depth();
        let column = dd.next_column().unwrap_or("value").to_string();
        let values = dd.levels_cache.get(depth).cloned().unwrap_or_default();
        let mut missing = 0usize;
        let rows = values
            .iter()
            .map(|val| {
                let mut key = dd.path.clone();
                key.push((column.clone(), val.clone()));
                let count = dd.counts_cache.get(&key).map(|c| c.to_string());
                if count.is_none() {
                    missing += 1;
                }
                vec![val.clone(), count.unwrap_or_default()]
            })
            .collect();
        let path_suffix = path_suffix(&dd.path);
        return Ok(ViewExport {
            title: format!("Partitions — {table_name}{path_suffix}"),
            file_stem: format!("{base}_partitions{path_suffix}"),
            columns: vec![column, "records".to_string()],
            rows,
            note: (missing > 0).then(|| {
                format!("{missing} partition(s) not counted yet — their records cell is empty")
            }),
        });
    }

    match action {
        Action::Partitions => {
            if s.metadata_loading {
                return Err("Partition layout is still loading".to_string());
            }
            Ok(ViewExport {
                title: format!("Partitions — {table_name}"),
                file_stem: format!("{base}_partition_layout"),
                columns: vec!["partition".to_string()],
                rows: app
                    .partition_tree_lines
                    .iter()
                    .map(|l| vec![l.clone()])
                    .collect(),
                note: None,
            })
        }
        Action::Schema => {
            if s.metadata_loading {
                return Err("Schema is still loading".to_string());
            }
            Ok(ViewExport {
                title: format!("Schema — {table_name}"),
                file_stem: format!("{base}_schema"),
                columns: ["name", "type", "nullable", "key", "description"]
                    .map(String::from)
                    .to_vec(),
                rows: app
                    .vertical_schema_cols
                    .iter()
                    .map(|c| {
                        vec![
                            c.name.clone(),
                            c.data_type.clone(),
                            c.is_nullable.clone(),
                            c.key_meta.clone(),
                            c.description.clone(),
                        ]
                    })
                    .collect(),
                note: None,
            })
        }
        _ => {
            let res = s
                .results
                .as_ref()
                .ok_or("No results to copy or export yet")?;
            results_view(res, &base, label, &table_name)
        }
    }
}

fn results_view(
    res: &ResultsState,
    base: &str,
    label: &str,
    table_name: &str,
) -> Result<ViewExport, String> {
    if res.loading {
        return Err("Results are still loading".to_string());
    }
    if res.invalid_query_error.is_some() {
        return Err("Fix the invalid query before exporting".to_string());
    }
    if res.error.is_some() {
        return Err("Cannot export a failed query result".to_string());
    }
    if res.columns.is_empty() {
        return Err("No tabular results available".to_string());
    }

    let visible = res.visible_columns();
    let columns = visible.iter().map(|(_, name)| (*name).clone()).collect();
    let rows = res
        .rows
        .iter()
        .map(|row| {
            visible
                .iter()
                .map(|(i, _)| row.get(*i).cloned().unwrap_or_default())
                .collect()
        })
        .collect();

    let filter_values: Vec<(String, String)> = res.filters.clone();
    let suffix = path_suffix(&filter_values);
    let note = if res.has_more_rows {
        Some(format!(
            "Only the {} rows loaded so far — scroll to load more first",
            format_count(res.rows.len())
        ))
    } else {
        None
    };
    Ok(ViewExport {
        title: format!("{label} — {table_name}{suffix}"),
        file_stem: format!("{base}_{}{suffix}", label_slug(label)),
        columns,
        rows,
        note,
    })
}

/// ` [date=2026-08-06, service=smb3]` for titles and `_date-2026-08-06_…`-ish
/// stems; sanitized later when used as a file name.
fn path_suffix(path: &[(String, String)]) -> String {
    if path.is_empty() {
        return String::new();
    }
    format!(
        " [{}]",
        path.iter()
            .map(|(c, v)| format!("{c}={v}"))
            .collect::<Vec<_>>()
            .join(", ")
    )
}

fn label_slug(label: &str) -> String {
    // "Sample Mode (20 rows)" -> "sample_mode"; keep it short and readable.
    label
        .split('(')
        .next()
        .unwrap_or(label)
        .trim()
        .to_lowercase()
        .replace(' ', "_")
}

// ---------------------------------------------------------------------------
// CSV helpers
// ---------------------------------------------------------------------------

pub(crate) fn rows_to_csv(columns: &[String], rows: &[Vec<String>]) -> String {
    let mut lines = Vec::with_capacity(rows.len() + 1);
    lines.push(format_row(columns.iter().map(String::as_str), ','));
    lines.extend(
        rows.iter()
            .map(|row| format_row(row.iter().map(String::as_str), ',')),
    );
    lines.join("\n")
}

fn format_row<'a>(fields: impl Iterator<Item = &'a str>, delimiter: char) -> String {
    let separator = delimiter.to_string();
    fields
        .map(|field| escape_delimited_field(field, delimiter))
        .collect::<Vec<_>>()
        .join(&separator)
}

fn escape_delimited_field(field: &str, delimiter: char) -> String {
    let needs_quotes = field.contains(delimiter)
        || field.contains('"')
        || field.contains('\n')
        || field.contains('\r');

    if needs_quotes {
        format!("\"{}\"", field.replace('"', "\"\""))
    } else {
        field.to_string()
    }
}

fn sanitize_stem(value: &str) -> String {
    let sanitized = value
        .trim()
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' || ch == '.' {
                ch
            } else {
                '_'
            }
        })
        .collect::<String>();
    // Collapse the runs of '_' that punctuation-heavy partition values leave.
    let mut collapsed = String::with_capacity(sanitized.len());
    for ch in sanitized.chars() {
        if ch == '_' && collapsed.ends_with('_') {
            continue;
        }
        collapsed.push(ch);
    }
    let trimmed: String = collapsed
        .trim_matches(|c| c == '_' || c == '.')
        .chars()
        .take(MAX_STEM_LEN)
        .collect();

    if trimmed.is_empty() {
        "export".to_string()
    } else {
        trimmed
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{ActionState, CatalogState, DrillDownState, TableState};
    use crate::config::ConnectionConfig;
    use std::collections::HashMap;

    fn app_with(screen: Screen) -> App {
        let mut app = App::new(ConnectionConfig::default(), false);
        app.screen = screen;
        app
    }

    fn scratch_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "lazytrino-test-{name}-{}-{}",
            std::process::id(),
            now_secs()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn test_rows_to_csv_escapes_commas_quotes_and_newlines() {
        let columns = vec!["name".to_string(), "notes".to_string()];
        let rows = vec![vec![
            "Ada, Jr.".to_string(),
            "Said \"hi\"\nthen left".to_string(),
        ]];

        let csv = rows_to_csv(&columns, &rows);

        assert_eq!(
            csv,
            "name,notes\n\"Ada, Jr.\",\"Said \"\"hi\"\"\nthen left\""
        );
    }

    #[test]
    fn test_utc_timestamp_known_epochs() {
        assert_eq!(utc_timestamp(0), "19700101-000000Z");
        // 2000-02-29 12:34:56 UTC (leap day)
        assert_eq!(utc_timestamp(951_827_696), "20000229-123456Z");
        // 2026-10-23 10:15:00 UTC
        assert_eq!(utc_timestamp(1_792_750_500), "20261023-101500Z");
    }

    #[test]
    fn test_sanitize_stem() {
        assert_eq!(sanitize_stem("a b/c"), "a_b_c");
        assert_eq!(sanitize_stem("x [date=2026-08-06]"), "x_date_2026-08-06");
        assert_eq!(sanitize_stem("///"), "export");
        assert!(sanitize_stem(&"a".repeat(500)).len() <= MAX_STEM_LEN);
    }

    #[test]
    fn test_catalog_view_honors_search_and_copies_names_only() {
        let mut app = app_with(Screen::Catalog(CatalogState {
            items: vec!["hive".into(), "iceberg".into(), "hudi".into()],
            selected: 0,
        }));
        app.search_query = "hi".into();
        let view = build_view_export(&app).unwrap();
        assert_eq!(view.rows, vec![vec!["hive".to_string()]]);
        assert_eq!(clipboard_text(&view), "hive");
        assert!(view.note.is_some());
    }

    #[test]
    fn test_empty_view_is_an_error() {
        let app = app_with(Screen::Table(TableState {
            catalog: "c".into(),
            schema: "s".into(),
            items: vec![],
            selected: 0,
        }));
        assert!(build_view_export(&app).is_err());
    }

    #[test]
    fn test_drilldown_view_includes_counts_beside_values() {
        let mut counts = HashMap::new();
        counts.insert(vec![("date".to_string(), "2026-08-06".to_string())], 45120);
        let dd = DrillDownState {
            partition_cols: vec!["date".into(), "service".into()],
            levels_cache: vec![vec!["2026-08-06".into(), "2026-08-07".into()]],
            counts_cache: counts,
            ..Default::default()
        };
        let app = app_with(Screen::Actions(ActionState {
            catalog: "c".into(),
            schema: "s".into(),
            table: "t".into(),
            selected: 0,
            drilldown: Some(dd),
            ..Default::default()
        }));
        let view = build_view_export(&app).unwrap();
        assert_eq!(view.columns, vec!["date", "records"]);
        assert_eq!(
            view.rows,
            vec![
                vec!["2026-08-06".to_string(), "45120".to_string()],
                vec!["2026-08-07".to_string(), String::new()],
            ]
        );
        assert_eq!(
            clipboard_text(&view),
            "date,records\n2026-08-06,45120\n2026-08-07,"
        );
        assert!(view.note.unwrap().contains("1 partition"));
    }

    #[test]
    fn test_results_view_respects_hidden_columns_and_flags_partial_load() {
        let mut res = ResultsState {
            columns: vec!["a".into(), "b".into(), "c".into()],
            rows: vec![vec!["1".into(), "2".into(), "3".into()]],
            has_more_rows: true,
            ..Default::default()
        };
        res.hidden_columns.insert("b".to_string());
        let view = results_view(&res, "c_s_t", "Sample Mode (20 rows)", "c.s.t").unwrap();
        assert_eq!(view.columns, vec!["a", "c"]);
        assert_eq!(view.rows, vec![vec!["1".to_string(), "3".to_string()]]);
        assert!(view.note.is_some());
        assert_eq!(view.file_stem, "c_s_t_sample_mode");
    }

    #[test]
    fn test_loading_or_failed_results_are_not_exportable() {
        let loading = ResultsState {
            loading: true,
            columns: vec!["a".into()],
            ..Default::default()
        };
        assert!(results_view(&loading, "b", "L", "t").is_err());
        let failed = ResultsState {
            error: Some("boom".into()),
            columns: vec!["a".into()],
            ..Default::default()
        };
        assert!(results_view(&failed, "b", "L", "t").is_err());
    }

    #[test]
    fn test_export_writes_csv_and_never_overwrites() {
        let dir = scratch_dir("export");
        let view = ViewExport {
            title: "T".into(),
            file_stem: "my table/x".into(),
            columns: vec!["name".into(), "n".into()],
            rows: vec![vec!["a, b".into(), "1".into()]],
            note: None,
        };
        let first = export_to_dir(&view, &dir, "20260101-000000Z").unwrap();
        let second = export_to_dir(&view, &dir, "20260101-000000Z").unwrap();
        assert_ne!(first, second);
        assert_eq!(
            first.file_name().unwrap().to_str().unwrap(),
            "my_table_x_20260101-000000Z.csv"
        );
        assert!(
            second
                .file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .ends_with("-2.csv")
        );
        assert_eq!(
            std::fs::read_to_string(&first).unwrap(),
            "name,n\n\"a, b\",1\n"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_modal_flow_defaults_and_cancel() {
        let mut app = app_with(Screen::Catalog(CatalogState {
            items: vec!["hive".into()],
            selected: 0,
        }));
        open_export_modal(&mut app);
        assert_eq!(app.mode, Mode::ExportModal);
        assert_eq!(app.export_modal.as_ref().unwrap().selected, 0);

        handle_export_modal_key(&mut app, KeyEvent::from(KeyCode::Char('j')));
        assert_eq!(app.export_modal.as_ref().unwrap().selected, 1);
        handle_export_modal_key(&mut app, KeyEvent::from(KeyCode::Tab));
        assert_eq!(app.export_modal.as_ref().unwrap().selected, 0);

        handle_export_modal_key(&mut app, KeyEvent::from(KeyCode::Esc));
        assert_eq!(app.mode, Mode::Normal);
        assert!(app.export_modal.is_none());
        assert!(app.notice_toast.is_none());
    }

    #[test]
    fn test_modified_keys_are_not_mistaken_for_plain_ones() {
        let mut app = app_with(Screen::Catalog(CatalogState {
            items: vec!["hive".into()],
            selected: 0,
        }));
        open_export_modal(&mut app);

        // Alt+E / Ctrl+J / Ctrl+K / Alt+C must be inert (in particular, must
        // not trigger an export or a copy).
        for (code, mods) in [
            (KeyCode::Char('e'), KeyModifiers::ALT),
            (KeyCode::Char('c'), KeyModifiers::ALT),
            (KeyCode::Char('j'), KeyModifiers::CONTROL),
            (KeyCode::Char('k'), KeyModifiers::CONTROL),
            (KeyCode::Char('q'), KeyModifiers::ALT),
        ] {
            handle_export_modal_key(&mut app, KeyEvent::new(code, mods));
            assert!(
                app.export_modal.is_some(),
                "{code:?}+{mods:?} acted on the modal"
            );
            assert_eq!(app.export_modal.as_ref().unwrap().selected, 0);
            assert!(app.notice_toast.is_none());
        }

        // Ctrl+E toggles it closed without exporting anything.
        handle_export_modal_key(
            &mut app,
            KeyEvent::new(KeyCode::Char('e'), KeyModifiers::CONTROL),
        );
        assert!(app.export_modal.is_none());
        assert_eq!(app.mode, Mode::Normal);
        assert!(app.notice_toast.is_none());
    }

    #[test]
    fn test_open_modal_on_empty_view_shows_error_notice() {
        let mut app = app_with(Screen::Help);
        open_export_modal(&mut app);
        assert_eq!(app.mode, Mode::Normal);
        assert!(app.export_modal.is_none());
        assert_eq!(app.notice_toast.as_ref().unwrap().kind, NoticeKind::Error);
    }

    #[test]
    fn test_large_view_defaults_to_file_export() {
        let big = "x".repeat(1024);
        let mut app = app_with(Screen::Catalog(CatalogState {
            items: (0..6000).map(|_| big.clone()).collect(),
            selected: 0,
        }));
        open_export_modal(&mut app);
        let modal = app.export_modal.as_ref().unwrap();
        assert!(modal.size_bytes > LARGE_CLIPBOARD_BYTES);
        assert_eq!(modal.selected, 1);
    }
}
