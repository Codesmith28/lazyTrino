use std::time::{Instant, SystemTime, UNIX_EPOCH};

use crate::app::{ACTIONS, Action, App, ResultsState, Screen};

use super::query::copy_to_clipboard;

pub(super) fn copy_results_to_clipboard(app: &mut App) {
    let message = {
        match active_results_state(app).and_then(validate_exportable_state) {
            Ok(state) => {
                let tsv = rows_to_tsv(&state.columns, &state.rows);
                copy_to_clipboard(&tsv);
                if state.rows.is_empty() {
                    "Copied headers to clipboard (TSV)".to_string()
                } else {
                    format!("Copied {} rows to clipboard (TSV)", state.rows.len())
                }
            }
            Err(message) => message,
        }
    };

    set_export_toast(app, message);
}

pub(super) fn export_results_to_csv_file(app: &mut App) {
    let message = {
        match active_results_state(app).and_then(validate_exportable_state) {
            Ok(state) => {
                let csv = rows_to_csv(&state.columns, &state.rows);
                let file_name = export_file_name(state);
                match std::fs::write(&file_name, csv) {
                    Ok(()) => format!("Exported results to {file_name}"),
                    Err(err) => format!("Export failed: {err}"),
                }
            }
            Err(message) => message,
        }
    };

    set_export_toast(app, message);
}

fn set_export_toast(app: &mut App, message: String) {
    app.copied_toast = Some((message, Instant::now()));
}

fn active_results_state(app: &App) -> Result<&ResultsState, String> {
    match &app.screen {
        Screen::Actions(action_state)
            if action_state.selected < ACTIONS.len()
                && !matches!(
                    ACTIONS[action_state.selected].2,
                    Action::Partitions | Action::Schema
                ) =>
        {
            action_state
                .results
                .as_ref()
                .ok_or_else(|| "No query results to export".to_string())
        }
        Screen::Actions(_) => Err("Current view has no tabular results to export".to_string()),
        _ => Err("No query results to export".to_string()),
    }
}

fn validate_exportable_state(state: &ResultsState) -> Result<&ResultsState, String> {
    if state.loading {
        return Err("Results are still loading".to_string());
    }

    if state.invalid_query_error.is_some() {
        return Err("Fix the invalid query before exporting".to_string());
    }

    if state.error.is_some() {
        return Err("Cannot export a failed query result".to_string());
    }

    if state.columns.is_empty() {
        return Err("No tabular results available to export".to_string());
    }

    Ok(state)
}

pub(crate) fn rows_to_csv(columns: &[String], rows: &[Vec<String>]) -> String {
    rows_to_delimited(columns, rows, ',')
}

fn rows_to_tsv(columns: &[String], rows: &[Vec<String>]) -> String {
    rows_to_delimited(columns, rows, '\t')
}

fn rows_to_delimited(columns: &[String], rows: &[Vec<String>], delimiter: char) -> String {
    let mut lines = Vec::with_capacity(rows.len() + 1);
    lines.push(format_row(columns.iter().map(String::as_str), delimiter));
    lines.extend(
        rows.iter()
            .map(|row| format_row(row.iter().map(String::as_str), delimiter)),
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

fn export_file_name(state: &ResultsState) -> String {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_millis());

    format!(
        "lazytrino_export_{}_{}_{}_{}.csv",
        sanitize_filename_component(&state.catalog),
        sanitize_filename_component(&state.schema),
        sanitize_filename_component(&state.table),
        timestamp
    )
}

fn sanitize_filename_component(value: &str) -> String {
    let sanitized = value
        .trim()
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch
            } else {
                '_'
            }
        })
        .collect::<String>()
        .trim_matches('_')
        .to_string();

    if sanitized.is_empty() {
        "unknown".to_string()
    } else {
        sanitized
    }
}

#[cfg(test)]
mod tests {
    use super::rows_to_csv;

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
}
