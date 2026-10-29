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

use ratatui::{
    Frame,
    layout::Rect,
    text::{Line, Span},
    widgets::{Paragraph, Wrap},
};

use crate::app::ExportModalState;
use crate::tui::handler::export::{
    LARGE_CLIPBOARD_BYTES, describe, export_dir_display, human_bytes,
};
use crate::tui::screens::modal;
use crate::tui::theme;

pub fn render(frame: &mut Frame, area: Rect, state: &ExportModalState) {
    let is_large = state.size_bytes > LARGE_CLIPBOARD_BYTES;

    let mut lines: Vec<Line> = vec![Line::from(vec![
        Span::styled(" ", theme::text_style()),
        Span::styled(describe(&state.view), theme::info_bold_style()),
        Span::styled(
            format!("  ·  ~{}", human_bytes(state.size_bytes)),
            theme::muted_style(),
        ),
    ])];
    if let Some(note) = &state.view.note {
        lines.push(Line::from(Span::styled(
            format!(" ⚠ {note}"),
            theme::warning_bold_style(),
        )));
    }
    lines.push(Line::from(""));

    let option = |idx: usize, key: &str, label: &str| {
        let selected = state.selected == idx;
        let style = if selected {
            theme::selection_style_for(true)
        } else {
            theme::text_style()
        };
        Line::from(Span::styled(
            format!(" {} [{key}] {label} ", if selected { "▸" } else { " " }),
            style,
        ))
    };
    lines.push(option(0, "c", "Copy to clipboard"));
    lines.push(option(1, "e", "Export to CSV file"));
    lines.push(Line::from(Span::styled(
        format!("       → {}/", export_dir_display()),
        theme::muted_style(),
    )));

    if is_large {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            " ⚠ Large view: copying to the clipboard may freeze the UI for a while — CSV export recommended.",
            theme::warning_bold_style(),
        )));
    }

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        " Enter:confirm  c/e:choose  j/k:move  Esc:cancel",
        theme::muted_style(),
    )));

    // 2 border rows plus content; wrapped lines (note / large warning) get headroom.
    let width = 72u16.min(area.width.saturating_sub(2)).max(30);
    let wrap_extra = 1 + u16::from(is_large) + u16::from(state.view.note.is_some());
    let height = (lines.len() as u16 + 2 + wrap_extra).min(area.height);
    let popup = modal::centered_rect(width, height, area);

    let title = format!(
        " Copy / Export — {} ",
        clip(&state.view.title, width as usize - 20)
    );
    let inner = modal::render_modal_frame(frame, popup, &title, true);
    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), inner);
}

fn clip(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    let kept: String = s.chars().take(max.saturating_sub(1)).collect();
    format!("{kept}…")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{App, CatalogState, NoticeKind, Screen, ViewExport};
    use crate::config::ConnectionConfig;
    use ratatui::{Terminal, backend::TestBackend};

    fn dump(buf: &ratatui::buffer::Buffer) -> String {
        (0..buf.area.height)
            .map(|y| {
                (0..buf.area.width)
                    .map(|x| buf.cell((x, y)).unwrap().symbol())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn state(size_bytes: usize, note: Option<&str>) -> ExportModalState {
        ExportModalState {
            view: ViewExport {
                title: "Partitions — c.s.t".into(),
                file_stem: "x".into(),
                columns: vec!["date".into(), "records".into()],
                rows: vec![vec!["d".into(), "1".into()]; 3],
                note: note.map(String::from),
            },
            selected: 0,
            size_bytes,
        }
    }

    #[test]
    fn modal_shows_both_choices_size_and_destination() {
        let mut t = Terminal::new(TestBackend::new(100, 30)).unwrap();
        t.draw(|f| render(f, f.area(), &state(2048, None))).unwrap();
        let out = dump(t.backend().buffer());
        assert!(out.contains("Copy to clipboard"), "{out}");
        assert!(out.contains("Export to CSV file"), "{out}");
        assert!(out.contains("3 rows × 2 columns"), "{out}");
        assert!(out.contains("2.0 KB"), "{out}");
        assert!(out.contains("lazytrino/"), "{out}");
        assert!(!out.contains("Large view"), "{out}");
    }

    #[test]
    fn modal_warns_for_large_views_and_partial_loads() {
        let mut t = Terminal::new(TestBackend::new(100, 30)).unwrap();
        let s = state(
            LARGE_CLIPBOARD_BYTES + 1,
            Some("Only the 100 rows loaded so far"),
        );
        t.draw(|f| render(f, f.area(), &s)).unwrap();
        let out = dump(t.backend().buffer());
        assert!(out.contains("Large view"), "{out}");
        assert!(out.contains("Only the 100 rows loaded"), "{out}");
    }

    #[test]
    fn modal_fits_a_tiny_terminal_without_panicking() {
        let mut t = Terminal::new(TestBackend::new(32, 8)).unwrap();
        t.draw(|f| render(f, f.area(), &state(10, Some("note"))))
            .unwrap();
    }

    #[test]
    fn notice_toast_renders_success_and_wraps_long_paths_without_losing_any_of_them() {
        let mut app = App::new(ConnectionConfig::default(), false);
        app.set_notice(
            NoticeKind::Success,
            "Exported 12 rows to ~/Downloads/lazytrino/some_really_long_catalog_schema_table_partitions_20261023-101500Z.csv",
        );
        let mut t = Terminal::new(TestBackend::new(80, 20)).unwrap();
        t.draw(|f| crate::tui::draw::render_notice_toast(f, f.area(), &app))
            .unwrap();
        let out = dump(t.backend().buffer());
        assert!(out.contains("Exported"), "{out}");
        // the long path is wrapped across rows, so every part of it is on
        // screen (the old single-line toast cut its middle out with "…")
        let text: String = out
            .lines()
            .filter_map(|l| {
                l.trim_matches(|c: char| c == '│' || c.is_whitespace())
                    .into()
            })
            .collect::<Vec<&str>>()
            .join("");
        let compact = text.replace(' ', "");
        assert!(
            compact.contains(
                "~/Downloads/lazytrino/some_really_long_catalog_schema_table_partitions_20261023-101500Z.csv"
            ),
            "{out}"
        );
        assert!(!out.contains('…'), "{out}");
    }

    #[test]
    fn selected_list_rows_are_highlighted_and_labelled_and_copyable_from_the_frame() {
        use crate::tui::handler::{handle_key_sync, live_line_selection};
        use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

        let mut app = App::new(ConnectionConfig::default(), false);
        app.screen = Screen::Catalog(CatalogState {
            items: vec!["alpha".into(), "beta".into(), "gamma".into()],
            selected: 0,
        });
        handle_key_sync(
            &mut app,
            KeyEvent::new(KeyCode::Char('V'), KeyModifiers::SHIFT),
        );
        handle_key_sync(
            &mut app,
            KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE),
        );
        assert_eq!(live_line_selection(&app).unwrap().range(1), (0, 1));

        let mut t = Terminal::new(TestBackend::new(80, 20)).unwrap();
        t.draw(|f| crate::tui::draw::ui(f, &app)).unwrap();
        let buf = t.backend().buffer().clone();
        let out = dump(&buf);
        assert!(
            out.contains("VISUAL LINE"),
            "mode indicator missing:\n{out}"
        );

        // Selected rows use the terminal's own colors via reverse video (no
        // theme colors); the unselected row must not be reversed.
        let cell_of = |needle: &str| {
            let y = (0..buf.area.height)
                .find(|&y| {
                    (0..buf.area.width)
                        .map(|x| buf.cell((x, y)).unwrap().symbol())
                        .collect::<String>()
                        .contains(needle)
                })
                .unwrap();
            let first = needle.chars().next().unwrap().to_string();
            let x = (0..buf.area.width)
                .find(|&x| buf.cell((x, y)).unwrap().symbol() == first)
                .unwrap();
            buf.cell((x, y)).unwrap().clone()
        };
        for name in ["alpha", "beta"] {
            let c = cell_of(name);
            assert!(
                c.modifier.contains(ratatui::style::Modifier::REVERSED),
                "{name}"
            );
            assert_eq!(c.fg, ratatui::style::Color::Reset, "{name}");
            assert_eq!(c.bg, ratatui::style::Color::Reset, "{name}");
        }
        assert!(
            !cell_of("gamma")
                .modifier
                .contains(ratatui::style::Modifier::REVERSED)
        );

        // screen fallback: dragging over rows reads text straight from the frame
        app.last_frame = buf;
        let text = crate::tui::handler::extract_selected_text(&app, (0, 0), (79, 19));
        assert!(text.contains("alpha") && text.contains("gamma"), "{text}");
    }
}
