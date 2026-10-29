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
    layout::{Alignment, Rect},
    style::Style,
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Paragraph},
};

use crate::app::{ACTIONS, Action, ActivePanel, App, Mode, Screen};
use crate::tui::theme;

pub const SPINNER_FRAMES: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

pub fn spinner(app: &App) -> String {
    let idx = (app.frame_count / 2) as usize % SPINNER_FRAMES.len();
    SPINNER_FRAMES[idx].to_string()
}

pub fn footer_hint(app: &App) -> Option<&'static str> {
    match app.mode {
        Mode::ColumnModal | Mode::FilterModal => {
            Some(" Space:toggle  a:all/none  Enter:apply  Esc:cancel  j/k:move ")
        }
        Mode::AppliedFiltersModal => {
            Some(" j/k:move  d/x:remove filter  c:clear all  Esc/Enter/q:close ")
        }
        Mode::SavedViewsModal => {
            Some(" Space:toggle  a:all/none  Enter:restore  d/Del:delete  j/k:move  Esc:close ")
        }
        Mode::ExportModal => Some(" c:clipboard  e:CSV file  j/k:move  Enter:confirm  Esc:cancel "),
        Mode::SaveViewPrompt => Some(" Type:name  Bksp:del  Enter:save  Esc:cancel "),
        Mode::Search => Some(" Type:filter  Bksp:del  Enter:close  Esc:clear "),
        Mode::QueryInput => Some(match app.query_vim_mode() {
            crate::app::VimMode::Insert => {
                " Enter:run  Esc:normal  ←→↑↓:move  Opt/Alt+←→:word  Cmd+←→:ends  Ctrl+A:all  Ctrl+V:paste "
            }
            crate::app::VimMode::Normal => {
                " Enter:run  i/a/I/A:insert  hjkl/w/b/e/0/$:move  d/c/y+motion  x/D/C  p:paste  u/^R:undo  v:visual  Esc:cancel "
            }
            crate::app::VimMode::Visual => {
                " hjkl/w/b/e/0/$:extend  iw/i\":object  y:yank  d/x:delete  c:change  o:swap  Esc:normal "
            }
        }),
        Mode::HelpModal | Mode::Normal => {
            let screen = if let Screen::Help = &app.screen {
                app.prev_screen.as_deref().unwrap_or(&app.screen)
            } else {
                &app.screen
            };
            match screen {
                Screen::Connect(_) => Some(" Tab:next field  Enter:connect  ?:help  Ctrl+C:quit "),
                Screen::Catalog(_) => Some(
                    " j/k:move  l/Enter:select  /:search  V:select  ^E:export  r:refresh  ?:help  Ctrl+C:quit ",
                ),
                Screen::Schema(_) | Screen::Table(_) => Some(
                    " j/k:move  l/Enter:select  h/Esc:back  /:search  V:select  ^E:export  r:refresh  ?:help  Ctrl+C:quit ",
                ),
                Screen::Actions(state) => {
                    let selected_action = ACTIONS.get(state.selected).map(|(_, _, action)| *action);

                    match app.active_panel {
                        ActivePanel::MenuPane => Some(
                            " j/k:move  l/Enter:run  h/Esc:back  A:filters  v/c/e/t/m/p:action  Tab:pane  r:refresh  ?:help  Ctrl+C:quit ",
                        ),
                        ActivePanel::MainViewer => match selected_action {
                            Some(Action::TableView)
                                if state.results.is_some() && state.drilldown.is_some() =>
                            {
                                Some(
                                    " hjkl/←→:cells  <:up  S:sort  C:cols  F:filter  A:filters  u:clear sel  g/G:top/btm  q/:query  Esc:menu  Tab:pane  V/v:select  ^E:export  r:refresh  ?:help  Ctrl+C:quit ",
                                )
                            }
                            Some(Action::TableView) if state.results.is_some() => Some(
                                " hjkl/←→:cells  S:sort  C:cols  F:filter  A:filters  u:clear sel  g/G:top/btm  q/:query  Esc:menu  Tab:pane  V/v:select  ^E:export  r:refresh  ?:help  Ctrl+C:quit ",
                            ),
                            Some(Action::Partitions) if !app.partition_tree_lines.is_empty() => {
                                Some(
                                    " j/k:scroll  g/G:top/btm  q/:query  Esc:menu  Tab:pane  V/v:select  ^E:export  r:refresh  ?:help  Ctrl+C:quit ",
                                )
                            }
                            Some(Action::Schema) if !app.vertical_schema_cols.is_empty() => Some(
                                " j/k:scroll  g/G:top/btm  q/:query  Esc:menu  Tab:pane  V/v:select  ^E:export  r:refresh  ?:help  Ctrl+C:quit ",
                            ),
                            _ if state.results.is_some() => Some(
                                " hjkl/←→:cells  S:sort  C:cols  F:filter  u:clear sel  g/G:top/btm  q/:query  Esc:menu  Tab:pane  V/v:select  ^E:export  r:refresh  ?:help  Ctrl+C:quit ",
                            ),
                            Some(Action::TableView) if state.drilldown.is_some() => Some(
                                " j/k:move  >/Enter:drill  <:up  u:clear sel  q/:query  Esc:menu  Tab:pane  V/v:select  ^E:export  r:refresh  ?:help  Ctrl+C:quit ",
                            ),
                            Some(Action::TableView) => Some(
                                " q/:query  Esc:menu  Tab:pane  V/v:select  ^E:export  r:refresh  ?:help  Ctrl+C:quit ",
                            ),
                            _ => Some(
                                " q/:query  Esc:menu  Tab:pane  V/v:select  ^E:export  r:refresh  ?:help  Ctrl+C:quit ",
                            ),
                        },
                    }
                }
                Screen::Help => Some(" /:search  Esc/?:close  Ctrl+C:quit "),
            }
        }
    }
}

/// Self-describing status toast for copy/export results and failures. Wider
/// than the plain `Copied:` toast because it can carry a file path.
pub fn render_notice_toast(frame: &mut Frame, area: Rect, app: &App) {
    let Some(notice) = app.notice_toast.as_ref().filter(|n| n.is_active()) else {
        return;
    };
    if area.width < TOAST_MIN_W + 2 || area.height < TOAST_H + 1 {
        return;
    }

    let max_w = area.width.saturating_sub(2).min(110);
    let (icon, label, accent) = match notice.kind {
        crate::app::NoticeKind::Success => ("✔", "Done", theme::toast_style()),
        crate::app::NoticeKind::Error => ("✖", "Error", theme::error_toast_style()),
    };
    let message = sanitize_toast_text(&notice.text);
    // Long messages (a file path, an error with detail) wrap onto up to
    // MAX_TOAST_LINES rows instead of being cut in the middle.
    let toast_w = (message.chars().count() as u16 + 4).clamp(TOAST_MIN_W, max_w);
    let text_w = toast_w.saturating_sub(4) as usize;
    let mut lines = crate::tui::screens::results::wrap_text(&message, text_w.max(1));
    let room = (area.height.saturating_sub(2) as usize).saturating_sub(2);
    let max_lines = MAX_TOAST_LINES.min(room.max(1));
    if lines.len() > max_lines {
        lines.truncate(max_lines);
        if let Some(last) = lines.last_mut() {
            *last = truncate_middle(&format!("{last}…"), text_w.max(1));
        }
    }
    let toast_area = Rect {
        x: area.x + area.width - toast_w - 1,
        y: area.y + 1,
        width: toast_w,
        height: lines.len().max(1) as u16 + 2,
    };
    render_toast_card(
        frame,
        toast_area,
        &format!(" {icon} {label} "),
        accent,
        &lines,
    );
}

/// A toast is a card: accent-colored border and title, message in the
/// terminal's own text color, on a cleared (opaque) area.
fn render_toast_card(
    frame: &mut Frame,
    area: Rect,
    title: &str,
    accent: ratatui::style::Style,
    text: &[String],
) {
    frame.render_widget(ratatui::widgets::Clear, area);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(accent)
        .title(Span::styled(title.to_string(), accent));
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let lines: Vec<Line> = text
        .iter()
        .map(|t| Line::from(Span::styled(t.clone(), theme::toast_text_style())))
        .collect();
    frame.render_widget(Paragraph::new(lines).alignment(Alignment::Center), inner);
}

/// Keeps both ends (action and file name) when a message is too wide.
pub(crate) fn truncate_middle(text: &str, width: usize) -> String {
    let chars: Vec<char> = text.chars().collect();
    if chars.len() <= width {
        return text.to_string();
    }
    if width <= 1 {
        return "…".to_string();
    }
    let keep = width - 1;
    let head = keep / 2 + keep % 2;
    let tail = keep - head;
    let mut out: String = chars[..head].iter().collect();
    out.push('…');
    out.extend(&chars[chars.len() - tail..]);
    out
}

pub(crate) fn sanitize_toast_text(s: &str) -> String {
    s.chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect()
}

/// Fits a footer hint into `width`, cutting the contextual keys first so the
/// always-relevant tail (`?:help  Ctrl+C:quit`) stays visible on narrow
/// terminals.
pub fn fit_hint(hint: &str, width: usize) -> String {
    if hint.chars().count() <= width {
        return hint.to_string();
    }
    if let Some(pos) = hint.rfind("  ?:help") {
        let (head, tail) = hint.split_at(pos);
        let tail_w = tail.chars().count();
        if width > tail_w + 6 {
            return format!("{}{tail}", truncate_hint(head, width - tail_w));
        }
    }
    truncate_hint(hint, width)
}

pub fn truncate_hint(hint: &str, width: usize) -> String {
    if hint.chars().count() <= width {
        return hint.to_string();
    }

    if width <= 1 {
        return "…".to_string();
    }

    let mut truncated = hint.chars().take(width - 1).collect::<String>();
    while truncated.ends_with(' ') {
        truncated.pop();
    }
    truncated.push('…');
    truncated
}

/// Colors a ` key:action  key:action ` hint string: keys stand out, the
/// separators and actions stay quiet.
pub fn hint_line(hint: &str) -> Line<'static> {
    let mut spans = vec![Span::raw(" ")];
    for (i, token) in hint.trim().split("  ").enumerate() {
        if i > 0 {
            spans.push(Span::raw("  "));
        }
        match token.split_once(':') {
            Some((key, action)) if !key.is_empty() => {
                spans.push(Span::styled(key.to_string(), theme::key_style()));
                spans.push(Span::styled(":", theme::muted_style()));
                spans.push(Span::styled(action.to_string(), theme::footer_style()));
            }
            _ => spans.push(Span::styled(token.to_string(), theme::footer_style())),
        }
    }
    Line::from(spans)
}

pub fn render_footer(frame: &mut Frame, area: Rect, app: &App) {
    if area.width == 0 || area.height == 0 {
        return;
    }

    let Some(hint) = footer_hint(app) else {
        return;
    };

    let width = area.width as usize;
    app.footer_rect.set(area);
    // A different hint starts from its beginning.
    let hint_key = hint.as_ptr() as usize;
    if app.footer_hint_key.replace(hint_key) != hint_key {
        app.footer_scroll.set(0);
    }
    let max_scroll = hint.chars().count().saturating_sub(width);
    app.footer_max_scroll.set(max_scroll);
    let scroll = app.footer_scroll.get().min(max_scroll);
    app.footer_scroll.set(scroll);
    let focused = app.footer_focused && app.mode == Mode::Normal;

    // Untouched, a bar that is too narrow keeps the always-relevant
    // `?:help  Ctrl+C:quit` tail in view; once it is scrolled or focused it
    // shows the full hints, a window at a time.
    if max_scroll > 0 && (scroll > 0 || focused) {
        let mut line = hint_line(hint);
        line.spans.push(Span::raw(" "));
        let footer = Paragraph::new(scrolled_line(&line, scroll, width, focused))
            .style(theme::footer_style());
        frame.render_widget(footer, area);
        return;
    }

    let footer = Paragraph::new(hint_line(&fit_hint(hint, width)))
        .style(theme::footer_style())
        .wrap(ratatui::widgets::Wrap { trim: true });
    frame.render_widget(footer, area);
}

/// `width` columns of `line` starting `scroll` columns in. The edge cells
/// become `‹` / `›` where more hints lie beyond them; they light up while the
/// bar is focused.
fn scrolled_line(
    line: &Line<'static>,
    scroll: usize,
    width: usize,
    focused: bool,
) -> Line<'static> {
    let cells: Vec<(char, Style)> = line
        .spans
        .iter()
        .flat_map(|span| span.content.chars().map(move |c| (c, span.style)))
        .collect();
    let start = scroll.min(cells.len());
    let end = (start + width).min(cells.len());
    let mut window = cells[start..end].to_vec();

    let marker = if focused {
        theme::border_style(true)
    } else {
        theme::muted_style()
    };
    if start > 0
        && let Some(first) = window.first_mut()
    {
        *first = ('‹', marker);
    }
    if end < cells.len()
        && let Some(last) = window.last_mut()
    {
        *last = ('›', marker);
    }

    let mut spans: Vec<Span<'static>> = Vec::new();
    for (c, style) in window {
        match spans.last_mut() {
            Some(prev) if prev.style == style => prev.content.to_mut().push(c),
            _ => spans.push(Span::styled(c.to_string(), style)),
        }
    }
    Line::from(spans)
}

pub const TOAST_MIN_W: u16 = 24;
pub const TOAST_MAX_W: u16 = 60;
pub const TOAST_H: u16 = 3;
/// Most message rows a notification toast grows to.
pub const MAX_TOAST_LINES: usize = 6;

pub fn toast_envelope(area: Rect) -> Option<Rect> {
    if area.width < TOAST_MAX_W + 2 || area.height < TOAST_H + 1 {
        return None;
    }
    Some(Rect {
        x: area.x + area.width - TOAST_MAX_W - 1,
        y: area.y + 1,
        width: TOAST_MAX_W,
        height: TOAST_H,
    })
}

pub fn render_copied_toast(frame: &mut Frame, area: Rect, app: &App) {
    let Some(envelope) = toast_envelope(area) else {
        return;
    };

    let Some((ref msg, ref instant)) = app.copied_toast else {
        return;
    };
    if instant.elapsed().as_secs() >= 2 {
        return;
    }

    let text = format!(" \"{}\" ", sanitize_toast_text(msg));
    let content_w = text.chars().count() as u16 + 2;
    let toast_w = content_w.clamp(TOAST_MIN_W, TOAST_MAX_W);
    let toast_area = Rect {
        x: envelope.x + envelope.width - toast_w,
        y: envelope.y,
        width: toast_w,
        height: envelope.height,
    };
    render_toast_card(
        frame,
        toast_area,
        " ✔ Copied ",
        theme::toast_style(),
        &[truncate_hint(&text, toast_w.saturating_sub(2) as usize)],
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{Terminal, backend::TestBackend};

    fn toast_text(app: &App, w: u16, h: u16) -> String {
        let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
        term.draw(|f| render_notice_toast(f, Rect::new(0, 0, w, h), app))
            .unwrap();
        let buf = term.backend().buffer().clone();
        (0..h)
            .map(|y| {
                (0..w)
                    .map(|x| buf.cell((x, y)).unwrap().symbol())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn app_with(text: &str, kind: crate::app::NoticeKind) -> App {
        let mut app = App::new(crate::config::ConnectionConfig::default(), false);
        app.set_notice(kind, text);
        app
    }

    #[test]
    fn a_long_notification_wraps_instead_of_losing_its_middle() {
        let words: Vec<String> = (0..24).map(|i| format!("word{i:02}")).collect();
        let message = format!("Export failed: {}", words.join(" "));
        let app = app_with(&message, crate::app::NoticeKind::Error);
        let screen = toast_text(&app, 80, 20);
        for w in &words {
            assert!(screen.contains(w.as_str()), "{w} was dropped:\n{screen}");
        }
        assert!(!screen.contains('…'), "nothing needed cutting:\n{screen}");
        // it is a multi-row card, not one squeezed line
        assert!(screen.lines().filter(|l| l.contains("word")).count() >= 2);
    }

    #[test]
    fn an_enormous_notification_is_capped_with_an_ellipsis() {
        let message = "x ".repeat(2_000);
        let app = app_with(&message, crate::app::NoticeKind::Error);
        let screen = toast_text(&app, 80, 20);
        let rows = screen.lines().filter(|l| l.contains('x')).count();
        assert!(rows <= MAX_TOAST_LINES, "{rows} rows");
        assert!(screen.contains('…'));
    }

    #[test]
    fn short_notifications_stay_a_three_row_card() {
        let app = app_with("Copied 3 rows", crate::app::NoticeKind::Success);
        let screen = toast_text(&app, 80, 20);
        assert!(screen.contains("Copied 3 rows"));
        let border_rows = screen
            .lines()
            .filter(|l| l.contains('╭') || l.contains('╰'))
            .count();
        assert_eq!(border_rows, 2);
        assert_eq!(screen.lines().filter(|l| l.contains('│')).count(), 1);
    }

    #[test]
    fn narrow_footers_keep_help_and_quit_visible() {
        let hint = " hjkl/←→:cells  S:sort  C:cols  F:filter  A:filters  u:clear sel  g/G:top/btm  q/:query  Esc:menu  Tab:pane  V/v:select  ^E:export  ?:help  Ctrl+C:quit ";
        for width in [60usize, 80, 100, 120] {
            let fitted = fit_hint(hint, width);
            assert!(fitted.chars().count() <= width, "{width}: {fitted:?}");
            assert!(
                fitted.contains("?:help") && fitted.ends_with("Ctrl+C:quit "),
                "{width}: {fitted:?}"
            );
            assert!(fitted.contains('…'), "{width}: the contextual part was cut");
        }
        // when it all fits nothing changes
        assert_eq!(fit_hint(hint, 400), hint);
        // hints without a help tail fall back to plain truncation
        let q = " Enter:run  Esc:normal  ←→↑↓:move  Opt/Alt+←→:word  Cmd+←→:ends  Ctrl+A:all  Ctrl+V:paste ";
        let fitted = fit_hint(q, 40);
        assert!(fitted.chars().count() <= 40 && fitted.ends_with('…'));
        // absurdly narrow widths never panic
        for w in 0..12 {
            fit_hint(hint, w);
        }
    }

    fn footer_app() -> App {
        let mut app = App::new(
            crate::config::ConnectionConfig {
                url: "http://localhost:8080".to_string(),
                user: "test".to_string(),
                password: String::new(),
                query_cache_limit: crate::config::DEFAULT_QUERY_CACHE_LIMIT,
            },
            false,
        );
        app.screen = Screen::Catalog(crate::app::CatalogState {
            items: Vec::new(),
            selected: 0,
        });
        app
    }

    fn footer_text(app: &App, width: u16) -> String {
        let mut term = Terminal::new(TestBackend::new(width, 1)).unwrap();
        term.draw(|f| render_footer(f, Rect::new(0, 0, width, 1), app))
            .unwrap();
        let buf = term.backend().buffer().clone();
        (0..width)
            .map(|x| buf.cell((x, 0)).unwrap().symbol().to_string())
            .collect()
    }

    #[test]
    fn a_narrow_footer_scrolls_through_all_its_hints() {
        let mut app = footer_app();

        // untouched: the always-relevant tail stays in view, as before
        let untouched = footer_text(&app, 40);
        assert!(untouched.contains("Ctrl+C:quit"), "{untouched:?}");
        assert!(!untouched.contains('›') && !untouched.contains('‹'));
        assert!(app.footer_max_scroll.get() > 0, "it can be scrolled");

        // scrolled into the middle: more on both sides
        app.footer_scroll.set(10);
        let middle = footer_text(&app, 40);
        assert!(
            middle.starts_with('‹') && middle.ends_with('›'),
            "{middle:?}"
        );

        // scrolled to the end: the tail and nothing beyond it
        app.footer_scroll.set(usize::MAX);
        let end = footer_text(&app, 40);
        assert!(end.starts_with('‹') && !end.contains('›'), "{end:?}");
        assert!(end.contains("Ctrl+C:quit"), "{end:?}");
        assert_eq!(app.footer_scroll.get(), app.footer_max_scroll.get());

        // focused at the start: the first hints, with more to the right
        app.footer_scroll.set(0);
        app.footer_focused = true;
        let start = footer_text(&app, 40);
        assert!(
            start.contains("j/k:move") && start.ends_with('›'),
            "{start:?}"
        );
        assert!(!start.starts_with('‹'), "{start:?}");
    }

    #[test]
    fn a_footer_that_fits_has_nothing_to_scroll() {
        let mut app = footer_app();
        app.footer_focused = true;
        app.footer_scroll.set(5);
        let text = footer_text(&app, 200);
        assert_eq!(app.footer_max_scroll.get(), 0);
        assert_eq!(app.footer_scroll.get(), 0, "the offset is clamped away");
        assert!(!text.contains('‹') && !text.contains('›'), "{text:?}");
        assert!(text.contains("Ctrl+C:quit"));
    }

    #[test]
    fn a_different_hint_starts_from_its_beginning() {
        let mut app = footer_app();
        footer_text(&app, 40);
        app.footer_scroll.set(20);
        assert!(footer_text(&app, 40).starts_with('‹'));

        app.screen = Screen::Schema(crate::app::SchemaState {
            catalog: "datalake".to_string(),
            items: Vec::new(),
            selected: 0,
        });
        footer_text(&app, 40);
        assert_eq!(app.footer_scroll.get(), 0);
    }

    #[test]
    fn the_footer_records_where_it_was_drawn_for_hit_testing() {
        let app = footer_app();
        let mut term = Terminal::new(TestBackend::new(60, 5)).unwrap();
        term.draw(|f| render_footer(f, Rect::new(0, 4, 60, 1), &app))
            .unwrap();
        assert_eq!(app.footer_rect.get(), Rect::new(0, 4, 60, 1));
        assert!(crate::tui::handler::footer::contains(&app, 10, 4));
        assert!(!crate::tui::handler::footer::contains(&app, 10, 3));
    }
}
