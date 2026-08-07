use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Layout, Rect},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Paragraph},
};

use crate::app::{ACTIONS, Action, ActivePanel, App, Mode, Screen};

use super::{screens, theme};

const SPINNER_FRAMES: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

fn spinner(app: &App) -> String {
    let idx = (app.frame_count / 2) as usize % SPINNER_FRAMES.len();
    SPINNER_FRAMES[idx].to_string()
}

fn render_search_bar(frame: &mut Frame, area: Rect, app: &App) {
    let is_editing = matches!(app.mode, Mode::Search);
    let title = if is_editing {
        " Centralized Search [EDITING - Press Enter/Esc to finish] "
    } else {
        " Centralized Search [Press / to search] "
    };

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(theme::border_style(is_editing));

    let search_text = if app.search_query.is_empty() {
        Span::styled(
            "Type to filter catalogs, schemas, tables, and columns...",
            theme::muted_style(),
        )
    } else {
        Span::styled(&app.search_query, theme::bold_text_style())
    };

    let p = Paragraph::new(Line::from(vec![Span::raw(" / "), search_text]))
        .block(block)
        .wrap(ratatui::widgets::Wrap { trim: false });
    frame.render_widget(p, area);

    if is_editing {
        let inner_width = area.width.saturating_sub(2).max(1) as usize;
        let cursor_index = 3 + app.search_query.len();
        let line_offset = cursor_index / inner_width;
        let col_offset = cursor_index % inner_width;
        let cursor_x = area.x + 1 + (col_offset as u16);
        let cursor_y = area.y + 1 + (line_offset as u16);
        if cursor_y < area.y + area.height - 1 && cursor_x < area.x + area.width - 1 {
            frame.set_cursor_position((cursor_x, cursor_y));
        }
    }
}

fn render_query_bar(frame: &mut Frame, area: Rect, app: &App) {
    let is_editing = matches!(app.mode, Mode::QueryInput);
    let is_table_view = matches!(
        &app.screen,
        Screen::Actions(state) if state.results.as_ref().is_some_and(|results| results.is_paginated)
    );

    let title = if is_editing {
        " Table Query Bar [EDITING - Press Enter to run, Esc to cancel] "
    } else if is_table_view {
        " Table Query Bar [Press 'q' or ':' to write query] "
    } else {
        " Table Query Bar [Disabled - Active only in full data table view] "
    };

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(theme::style(theme::query_bar_border_color(
            is_editing,
            is_table_view,
        )));

    let (buf, cursor, sel_range) = match &app.screen {
        Screen::Actions(state) => {
            if let Some(ref res) = state.results {
                (
                    res.query_buffer.as_str(),
                    res.query_cursor,
                    res.selection_range(),
                )
            } else {
                (state.query_buffer.as_str(), state.query_cursor, None)
            }
        }
        _ => ("", 0, None),
    };

    let (spans, cursor_pos) = if buf.is_empty() {
        (
            vec![Span::styled(
                "Write query (e.g. SELECT * FROM table)...",
                theme::muted_style(),
            )],
            if is_editing { Some(0) } else { None },
        )
    } else if is_editing {
        if let Some((sel_start, sel_end)) = sel_range {
            let sel_start = sel_start.min(buf.len());
            let sel_end = sel_end.min(buf.len());
            let before = &buf[..sel_start];
            let selected = &buf[sel_start..sel_end];
            let after = &buf[sel_end..];
            (
                vec![
                    Span::styled(before, theme::bold_text_style()),
                    Span::styled(selected, theme::query_selection_style()),
                    Span::styled(after, theme::bold_text_style()),
                ],
                Some(cursor),
            )
        } else {
            (
                vec![Span::styled(buf, theme::bold_text_style())],
                Some(cursor),
            )
        }
    } else {
        (vec![Span::styled(buf, theme::bold_text_style())], None)
    };

    let inner_w = area.width.saturating_sub(2).max(1) as usize;
    let visible_lines = area.height.saturating_sub(2).max(1) as usize;

    let mut scroll_y: u16 = 0;
    if is_editing && let Some(pos) = cursor_pos {
        let cursor_index = 7 + pos;
        let line_offset = cursor_index / inner_w;
        if line_offset >= visible_lines {
            scroll_y = (line_offset - visible_lines + 1) as u16;
        }
    }

    let mut line_spans = vec![Span::styled(" SQL > ", theme::warning_style())];
    line_spans.extend(spans);

    let p = Paragraph::new(Line::from(line_spans))
        .block(block)
        .wrap(ratatui::widgets::Wrap { trim: false })
        .scroll((scroll_y, 0));
    frame.render_widget(p, area);

    if is_editing && let Some(pos) = cursor_pos {
        let cursor_index = 7 + pos;
        let line_offset = cursor_index / inner_w;
        let col_offset = cursor_index % inner_w;

        let rel_line = line_offset.saturating_sub(scroll_y as usize);
        let cursor_x = area.x + 1 + (col_offset as u16);
        let cursor_y = area.y + 1 + (rel_line as u16);
        if cursor_y < area.y + area.height - 1 && cursor_x < area.x + area.width - 1 {
            frame.set_cursor_position((cursor_x, cursor_y));
        }
    }
}

fn footer_hint(app: &App) -> Option<&'static str> {
    match app.mode {
        Mode::Search => Some(" Type:filter  Bksp:del  Enter:close  Esc:clear "),
        Mode::QueryInput => Some(" Enter:run  Esc:cancel  Ctrl+A:all  Ctrl+C:copy  Ctrl+V:paste "),
        Mode::Normal => match &app.screen {
            Screen::Connect(_) => Some(" Tab:next field  Enter:connect  ?:help  Ctrl+C:quit "),
            Screen::Catalog(_) => Some(" j/k:move  l/Enter:select  /:search  ?:help  Ctrl+C:quit "),
            Screen::Schema(_) | Screen::Table(_) => {
                Some(" j/k:move  l/Enter:select  h/Esc:back  /:search  ?:help  Ctrl+C:quit ")
            }
            Screen::Actions(state) => {
                let selected_action = ACTIONS.get(state.selected).map(|(_, _, action)| *action);

                match app.active_panel {
                    ActivePanel::MenuPane => Some(
                        " j/k:move  l/Enter:run  h/Esc:back  v/c/i/s/n/p/P/S:action  Tab:pane  ?:help  Ctrl+C:quit ",
                    ),
                    ActivePanel::MainViewer => match selected_action {
                        Some(Action::TableView) if state.results.is_some() => Some(
                            " j/k:rows  </>:cols  g/G:top/btm  q/:query  Esc:menu  Tab:pane  v/c/i/s/n/p/P/S:action  ?:help  Ctrl+C:quit ",
                        ),
                        Some(Action::Partitions) if !app.partition_tree_lines.is_empty() => Some(
                            " j/k:scroll  g/G:top/btm  Esc:menu  Tab:pane  v/c/i/s/n/p/P/S:action  ?:help  Ctrl+C:quit ",
                        ),
                        Some(Action::Schema) if !app.vertical_schema_cols.is_empty() => Some(
                            " j/k:scroll  g/G:top/btm  Esc:menu  Tab:pane  v/c/i/s/n/p/P/S:action  ?:help  Ctrl+C:quit ",
                        ),
                        _ if state.results.is_some() => Some(
                            " j/k:rows  </>:cols  g/G:top/btm  Esc:menu  Tab:pane  v/c/i/s/n/p/P/S:action  ?:help  Ctrl+C:quit ",
                        ),
                        Some(Action::TableView) => Some(
                            " q/:query  Esc:menu  Tab:pane  v/c/i/s/n/p/P/S:action  ?:help  Ctrl+C:quit ",
                        ),
                        _ => Some(
                            " Esc:menu  Tab:pane  v/c/i/s/n/p/P/S:action  ?:help  Ctrl+C:quit ",
                        ),
                    },
                }
            }
            Screen::Help => None,
        },
    }
}

/// Replaces control characters (tabs, newlines, carriage returns, etc.)
/// with a plain space before a string is embedded in a single-line UI
/// element such as the "Copied" toast or a block title. Copied cell/row
/// text can contain raw tab/newline bytes (e.g. TSV-joined columns); if
/// those are written into the terminal buffer as-is, the terminal itself
/// interprets them (a tab jumps the cursor to the next tab stop instead of
/// advancing one cell), which breaks the surrounding box border/background
/// and spills the text out of its intended bounds.
pub(crate) fn sanitize_toast_text(s: &str) -> String {
    s.chars().map(|c| if c.is_control() { ' ' } else { c }).collect()
}

fn truncate_hint(hint: &str, width: usize) -> String {
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

fn render_footer(frame: &mut Frame, area: Rect, app: &App) {
    if area.width == 0 || area.height == 0 {
        return;
    }

    let Some(hint) = footer_hint(app) else {
        return;
    };

    let footer = Paragraph::new(Line::from(truncate_hint(hint, area.width as usize)))
        .style(theme::footer_style())
        .wrap(ratatui::widgets::Wrap { trim: true });
    frame.render_widget(footer, area);
}

/// Renders the "Copied to clipboard" notification as a small floating
/// popup overlay (top-right corner) instead of taking over the one-line
/// footer — the footer must always stay a single line of key hints.
///
/// Hybrid sizing: the visible box scales with the message length (within
/// `TOAST_MIN_W..=TOAST_MAX_W`) so short messages don't waste space and the
/// box doesn't look awkwardly empty, while messages longer than
/// `TOAST_MAX_W` are truncated with `truncate_hint` rather than growing the
/// box further. `toast_envelope` defines the max region the box is ever
/// allowed to occupy (right-anchored); the box itself is only drawn (and
/// only `Clear`ed) within that envelope while a toast is actually active —
/// see the comment in `render_copied_toast` for why an unconditional clear
/// every frame is both unnecessary and actively harmful.
const TOAST_MIN_W: u16 = 24;
const TOAST_MAX_W: u16 = 60;
const TOAST_H: u16 = 3;

fn toast_envelope(area: Rect) -> Option<Rect> {
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


fn render_copied_toast(frame: &mut Frame, area: Rect, app: &App) {
    let Some(envelope) = toast_envelope(area) else {
        return;
    };

    let Some((ref msg, ref instant)) = app.copied_toast else {
        return;
    };
    if instant.elapsed().as_secs() >= 2 {
        return;
    }

    let text = format!(" Copied: \"{}\" ", sanitize_toast_text(msg));
    // Scale the box width to the content (plus 2 for borders), clamped to
    // the envelope so it never exceeds the region it's allowed to occupy.
    let content_w = text.chars().count() as u16 + 2;
    let toast_w = content_w.clamp(TOAST_MIN_W, TOAST_MAX_W);
    let toast_area = Rect {
        x: envelope.x + envelope.width - toast_w,
        y: envelope.y,
        width: toast_w,
        height: envelope.height,
    };

    // `ratatui::Terminal::draw` builds a brand-new buffer from scratch on
    // every call, so there is no need to pre-`Clear` this rect defensively
    // "just in case" a toast was drawn here on a previous frame — nothing
    // can leak across frames. Doing so unconditionally actually caused a
    // real bug: this envelope sits in the top-right corner, overlapping the
    // search/query bar borders underneath it, so clearing it on every frame
    // (even when no toast was active) blanked out chunks of those borders'
    // dash characters after they'd already been drawn earlier in the same
    // frame. `Clear` here is now only needed (and only used) to blank the
    // exact toast box before drawing over it, and only while a toast is
    // actually active.
    frame.render_widget(ratatui::widgets::Clear, toast_area);

    // Use a style with an explicit background so the box is fully opaque:
    // applying it via `Block::style` (not just `border_style`) paints the
    // background over the *entire* toast_area, including the interior, so
    // nothing underneath can show through the borders or padding.
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .style(theme::toast_style());
    let inner = block.inner(toast_area);
    frame.render_widget(block, toast_area);
    // Render the paragraph over the full inner rect (not just a single
    // line) so every cell in the interior gets the solid background style,
    // even the padding cells the text doesn't reach.
    let toast_text = Paragraph::new(Line::from(truncate_hint(&text, inner.width as usize)))
        .style(theme::toast_style())
        .alignment(Alignment::Center);
    frame.render_widget(toast_text, inner);
}


pub(super) fn ui(frame: &mut Frame, app: &App) {
    match app.screen {
        Screen::Help => {
            screens::help::render(frame, frame.area());
        }
        _ => {
            let is_in_table = matches!(app.screen, Screen::Actions(_));

            let outer_chunks =
                Layout::vertical([Constraint::Min(0), Constraint::Length(7)]).split(frame.area());
            let bottom_chunks = Layout::vertical([Constraint::Min(0), Constraint::Length(1)])
                .split(outer_chunks[1]);

            if !is_in_table {
                // Phase 1: Default All Tables View (Connect, Catalog, Schema, Table) -> Default 60% list ratio
                let list_pct = if app.main_panel_pct <= 30 {
                    60
                } else {
                    app.main_panel_pct
                };
                let main_chunks = Layout::horizontal([
                    Constraint::Percentage(list_pct),
                    Constraint::Percentage(100 - list_pct),
                ])
                .split(outer_chunks[0]);

                let search_active = matches!(app.mode, Mode::Search);
                let inner_w = main_chunks[0].width.saturating_sub(2).max(1) as usize;
                let search_height = if search_active {
                    let total_chars = 3 + app.search_query.len();
                    let lines = total_chars.div_ceil(inner_w);
                    (lines as u16 + 2).clamp(3, 8)
                } else {
                    3
                };

                let left_chunks =
                    Layout::vertical([Constraint::Length(search_height), Constraint::Min(0)])
                        .split(main_chunks[0]);

                render_search_bar(frame, left_chunks[0], app);
                screens::help::render(frame, main_chunks[1]);

                let main = left_chunks[1];
                let main_is_active = true;

                match &app.screen {
                    Screen::Connect(state) => {
                        screens::connect::render(frame, main, state, spinner(app));
                    }
                    Screen::Catalog(state) => {
                        screens::catalog::render(
                            frame,
                            main,
                            state,
                            &app.search_query,
                            main_is_active,
                            app,
                        );
                    }
                    Screen::Schema(state) => {
                        screens::schema::render(
                            frame,
                            main,
                            state,
                            &app.search_query,
                            main_is_active,
                            app,
                        );
                    }
                    Screen::Table(state) => {
                        screens::table::render(
                            frame,
                            main,
                            state,
                            &app.search_query,
                            main_is_active,
                            app,
                        );
                    }
                    _ => unreachable!(),
                }
            } else {
                // Phase 2: Inside Table View -> Default 15% Menu : 85% Preview (Resizable)
                let menu_pct = if app.main_panel_pct > 30 {
                    15
                } else {
                    app.main_panel_pct.clamp(8, 30)
                };
                let main_chunks = Layout::horizontal([
                    Constraint::Percentage(menu_pct),
                    Constraint::Percentage(100 - menu_pct),
                ])
                .split(outer_chunks[0]);

                let menu_area = main_chunks[0];
                let preview_column = main_chunks[1];

                let search_active = matches!(app.mode, Mode::Search);
                let query_active = matches!(app.mode, Mode::QueryInput);
                let inner_w = preview_column.width.saturating_sub(2).max(1) as usize;

                let search_height = if search_active {
                    let total_chars = 3 + app.search_query.len();
                    let lines = total_chars.div_ceil(inner_w);
                    (lines as u16 + 2).clamp(3, 8)
                } else {
                    3
                };

                let selected_idx = match &app.screen {
                    Screen::Actions(a) => a.selected,
                    _ => 0,
                };

                let query_height = if query_active {
                    let total_chars = 7 + match &app.screen {
                        Screen::Actions(a) => a
                            .results
                            .as_ref()
                            .map(|r| r.query_buffer.len())
                            .unwrap_or_else(|| a.query_buffer.len()),
                        _ => 0,
                    };
                    let lines = total_chars.div_ceil(inner_w);
                    (lines as u16 + 2).clamp(3, 7)
                } else {
                    3
                };

                let preview_chunks = Layout::vertical([
                    Constraint::Length(search_height),
                    Constraint::Length(query_height),
                    Constraint::Min(0),
                ])
                .split(preview_column);

                render_search_bar(frame, preview_chunks[0], app);
                render_query_bar(frame, preview_chunks[1], app);
                let preview_pane_area = preview_chunks[2];

                let menu_is_active = app.active_panel == ActivePanel::MenuPane;
                let preview_is_active = app.active_panel == ActivePanel::MainViewer;

                if let Screen::Actions(state) = &app.screen {
                    screens::actions::render(
                        frame,
                        menu_area,
                        &state.catalog,
                        &state.schema,
                        &state.table,
                        state.selected,
                        menu_is_active,
                        app,
                    );
                }

                if selected_idx < crate::app::ACTIONS.len() {
                    let action = &crate::app::ACTIONS[selected_idx].2;
                    let table_name = match &app.screen {
                        Screen::Actions(a) => a.table.as_str(),
                        _ => "",
                    };

                    match action {
                        crate::app::Action::Partitions => {
                            if app.loading && selected_idx == 6 {
                                let title = format!(" Preview — {table_name} (Partitions) ");
                                let block = Block::default()
                                    .title(title)
                                    .borders(Borders::ALL)
                                    .border_type(BorderType::Rounded)
                                    .border_style(theme::border_style(preview_is_active));
                                let inner = block.inner(preview_pane_area);
                                frame.render_widget(block, preview_pane_area);
                                let spin = spinner(app);
                                let spin_text = Paragraph::new(Line::from(vec![
                                    Span::styled(
                                        format!(" [{spin}] "),
                                        theme::warning_bold_style(),
                                    ),
                                    Span::styled(
                                        "FETCHING PARTITION METADATA...",
                                        theme::info_bold_style(),
                                    ),
                                ]))
                                .alignment(Alignment::Center);
                                frame.render_widget(spin_text, inner);
                            } else if !app.partition_tree_lines.is_empty() {
                                screens::partition_tree::render(
                                    frame,
                                    preview_pane_area,
                                    &app.partition_tree_lines,
                                    table_name,
                                    app.partition_scroll,
                                    preview_is_active,
                                    app,
                                );
                            } else {
                                render_placeholder_preview(
                                    frame,
                                    preview_pane_area,
                                    table_name,
                                    selected_idx,
                                    preview_is_active,
                                );
                            }
                        }
                        crate::app::Action::Schema => {
                            if app.loading && selected_idx == 7 {
                                let title = format!(" Preview — {table_name} (Schema) ");
                                let block = Block::default()
                                    .title(title)
                                    .borders(Borders::ALL)
                                    .border_type(BorderType::Rounded)
                                    .border_style(theme::border_style(preview_is_active));
                                let inner = block.inner(preview_pane_area);
                                frame.render_widget(block, preview_pane_area);
                                let spin = spinner(app);
                                let spin_text = Paragraph::new(Line::from(vec![
                                    Span::styled(
                                        format!(" [{spin}] "),
                                        theme::warning_bold_style(),
                                    ),
                                    Span::styled(
                                        "FETCHING SCHEMA METADATA...",
                                        theme::info_bold_style(),
                                    ),
                                ]))
                                .alignment(Alignment::Center);
                                frame.render_widget(spin_text, inner);
                            } else if !app.vertical_schema_cols.is_empty() {
                                screens::vertical_schema::render(
                                    frame,
                                    preview_pane_area,
                                    &app.vertical_schema_cols,
                                    table_name,
                                    app.schema_scroll,
                                    preview_is_active,
                                    app,
                                );
                            } else {
                                render_placeholder_preview(
                                    frame,
                                    preview_pane_area,
                                    table_name,
                                    selected_idx,
                                    preview_is_active,
                                );
                            }
                        }
                        crate::app::Action::TableView => {
                            let drilldown_browsing = match &app.screen {
                                Screen::Actions(state) => {
                                    state.drilldown.as_ref().filter(|d| !d.is_leaf()).cloned()
                                }
                                _ => None,
                            };
                            if let Some(dd) = drilldown_browsing {
                                screens::drilldown::render(
                                    frame,
                                    preview_pane_area,
                                    table_name,
                                    &dd,
                                    preview_is_active,
                                    app,
                                );
                            } else {
                                render_default_results_preview(
                                    frame,
                                    preview_pane_area,
                                    app,
                                    app.loading,
                                    spinner(app),
                                    table_name,
                                    selected_idx,
                                    preview_is_active,
                                );
                            }
                        }
                        crate::app::Action::TableDDL => {
                            // Mirror Partitions/Schema: DDL text is already
                            // cached from recon (`SHOW CREATE TABLE` fetched
                            // on table entry), so render it directly the
                            // moment this menu item is highlighted — no
                            // Enter/hotkey needed, and no dependency on
                            // `state.results` (which previously only got
                            // populated once `trigger_action` ran via
                            // Enter/hotkey, unlike Partitions/Schema which
                            // read straight from `app`-level caches).
                            let ddl_text = match &app.screen {
                                Screen::Actions(a) => {
                                    a.metadata.as_ref().map(|m| m.ddl_text.clone())
                                }
                                _ => None,
                            };
                            if app.loading && selected_idx == 1 {
                                let title = format!(" Preview — {table_name} (Table DDL) ");
                                let block = Block::default()
                                    .title(title)
                                    .borders(Borders::ALL)
                                    .border_type(BorderType::Rounded)
                                    .border_style(theme::border_style(preview_is_active));
                                let inner = block.inner(preview_pane_area);
                                frame.render_widget(block, preview_pane_area);
                                let spin = spinner(app);
                                let spin_text = Paragraph::new(Line::from(vec![
                                    Span::styled(
                                        format!(" [{spin}] "),
                                        theme::warning_bold_style(),
                                    ),
                                    Span::styled("FETCHING TABLE DDL...", theme::info_bold_style()),
                                ]))
                                .alignment(Alignment::Center);
                                frame.render_widget(spin_text, inner);
                            } else if let Some(ddl_text) = ddl_text {
                                let title = format!(" Preview — {table_name} (Table DDL) ");
                                let block = Block::default()
                                    .title(title)
                                    .borders(Borders::ALL)
                                    .border_type(BorderType::Rounded)
                                    .border_style(theme::border_style(preview_is_active));
                                let inner = block.inner(preview_pane_area);
                                frame.render_widget(block, preview_pane_area);
                                let paragraph =
                                    Paragraph::new(ddl_text).style(theme::detail_style());
                                frame.render_widget(paragraph, inner);
                            } else {
                                render_default_results_preview(
                                    frame,
                                    preview_pane_area,
                                    app,
                                    app.loading,
                                    spinner(app),
                                    table_name,
                                    selected_idx,
                                    preview_is_active,
                                );
                            }
                        }
                        _ => {
                            render_default_results_preview(
                                frame,
                                preview_pane_area,
                                app,
                                app.loading,
                                spinner(app),
                                table_name,
                                selected_idx,
                                preview_is_active,
                            );
                        }
                    }
                }
            }

            if bottom_chunks[0].height > 0 {
                screens::query_inspector::render(frame, bottom_chunks[0], app);
            }
            render_footer(frame, bottom_chunks[1], app);
        }
    }
    // Rendered unconditionally, after every screen variant (including Help),
    // so the toast's fixed rect is always cleared on every frame regardless
    // of which branch drew the rest of the UI. This is what prevents stray
    // leftover styled cells: the clear no longer depends on a particular
    // screen branch being taken.
    render_copied_toast(frame, frame.area(), app);
}

#[allow(clippy::too_many_arguments)]
fn render_default_results_preview(
    frame: &mut Frame,
    preview_pane_area: Rect,
    app: &App,
    is_loading: bool,
    spin: String,
    table_name: &str,
    selected_idx: usize,
    preview_is_active: bool,
) {
    if let Screen::Actions(state) = &app.screen {
        if let Some(ref res_state) = state.results {
            screens::results::render(
                frame,
                preview_pane_area,
                res_state,
                spin,
                preview_is_active,
                app,
            );
        } else if is_loading {
            let title = format!(
                " Preview — {} ({}) ",
                table_name,
                crate::app::ACTIONS[selected_idx].1
            );
            let block = Block::default()
                .title(title)
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(theme::border_style(preview_is_active));
            let inner = block.inner(preview_pane_area);
            frame.render_widget(block, preview_pane_area);
            let spin_text = Paragraph::new(Line::from(vec![
                Span::styled(format!(" [{spin}] "), theme::warning_bold_style()),
                Span::styled("EXECUTING TRINO QUERY...", theme::info_bold_style()),
            ]))
            .alignment(Alignment::Center);
            frame.render_widget(spin_text, inner);
        } else {
            render_placeholder_preview(
                frame,
                preview_pane_area,
                table_name,
                selected_idx,
                preview_is_active,
            );
        }
    }
}

fn render_placeholder_preview(
    frame: &mut Frame,
    area: Rect,
    table_name: &str,
    selected_idx: usize,
    preview_is_active: bool,
) {
    let action_name = if selected_idx < crate::app::ACTIONS.len() {
        crate::app::ACTIONS[selected_idx].1
    } else {
        ""
    };
    let title = format!(" Preview — {table_name} ({action_name}) ");
    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(theme::border_style(preview_is_active));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let info_lines = vec![
        Line::from(Span::styled(
            " Table Preview Area",
            theme::info_bold_style(),
        )),
        Line::from(""),
        Line::from(Span::styled(
            format!(
                " Active Selection: [{}] {action_name}",
                crate::app::ACTIONS[selected_idx].0
            ),
            theme::warning_bold_style(),
        )),
        Line::from(""),
        Line::from(Span::styled(
            " Press Enter (or hit hotkey) to load and display preview output.",
            theme::text_style(),
        )),
        Line::from(""),
        Line::from(Span::styled(" Menu Shortcuts:", theme::muted_style())),
        Line::from("   [v] Table View Mode       [c] Table DDL            [i] Info Schema"),
        Line::from("   [s] Show Stats            [n] Row Count            [p] Sample (20 rows)"),
        Line::from("   [P] Partition Tree        [S] Vertical Schema"),
    ];
    let info_p = Paragraph::new(info_lines).alignment(Alignment::Center);
    frame.render_widget(info_p, inner);
}


#[cfg(test)]
mod toast_border_regression {
    use super::*;
    use crate::app::{ActionState, App};
    use ratatui::{Terminal, backend::TestBackend};

    /// Regression test for a bug where the copy-toast overlay's `Clear`
    /// widget ran unconditionally on every frame (even with no toast
    /// active), blanking out chunks of the search bar / query bar borders
    /// that happen to sit under its fixed top-right rectangle. The toast
    /// must never touch cells outside its own active rendering, so with no
    /// toast pending the search/query bar borders must be fully intact
    /// dashes all the way across, not broken by unrelated blank gaps.
    #[test]
    fn search_and_query_bar_borders_stay_intact_without_active_toast() {
        let mut app = App::new(crate::config::ConnectionConfig::default(), false);
        app.mode = Mode::QueryInput;
        app.screen = Screen::Actions(ActionState {
            catalog: "datalake".into(),
            schema: "s".into(),
            table: "t".into(),
            selected: 4,
            query_buffer: "SELECT 1".into(),
            query_cursor: 8,
            ..Default::default()
        });
        app.main_panel_pct = 15;
        assert!(app.copied_toast.is_none());

        let backend = TestBackend::new(200, 40);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|f| ui(f, &app)).unwrap();
        let buf = terminal.backend().buffer().clone();

        // Rows 0 (search bar top border) and 3 (query bar top border) each
        // contain a titled border line ending in a rounded corner. The
        // corner must be immediately preceded by a border dash — if the
        // toast overlay's `Clear` punches a blank gap right before the
        // corner (as it did when it ran unconditionally every frame), the
        // cell just left of the corner is a stray blank space instead.
        for y in [0u16, 3u16] {
            let mut corner_x = None;
            for x in (0..200u16).rev() {
                let sym = buf.cell((x, y)).unwrap().symbol().to_string();
                if sym != " " {
                    corner_x = Some(x);
                    break;
                }
            }
            let corner_x = corner_x.expect("row should have a border corner");
            assert!(corner_x > 0, "row {y} corner at unexpected x=0");
            let prev_sym = buf.cell((corner_x - 1, y)).unwrap().symbol().to_string();
            assert_eq!(
                prev_sym, "─",
                "row {y}: cell just before the border corner (x={}) is {:?}, expected a \
                 dash — the toast overlay is punching a blank gap into the border",
                corner_x - 1,
                prev_sym
            );
        }
    }

    /// Regression test for a bug where the copy-toast had no explicit
    /// background color, so its interior (padding cells not covered by the
    /// text itself) let whatever was drawn underneath show through instead
    /// of a solid box. Every cell inside the toast's border must carry the
    /// toast's background color once a toast is active.
    #[test]
    fn active_toast_has_solid_background_with_no_spillover() {
        let mut app = App::new(crate::config::ConnectionConfig::default(), false);
        app.mode = Mode::QueryInput;
        app.screen = Screen::Actions(ActionState {
            catalog: "datalake".into(),
            schema: "s".into(),
            table: "t".into(),
            selected: 4,
            query_buffer: "SELECT 1".into(),
            query_cursor: 8,
            ..Default::default()
        });
        app.main_panel_pct = 15;
        app.copied_toast = Some(("hello".into(), std::time::Instant::now()));

        let width = 200u16;
        let height = 40u16;
        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|f| ui(f, &app)).unwrap();
        let buf = terminal.backend().buffer().clone();

        let area = Rect::new(0, 0, width, height);
        let envelope = toast_envelope(area).expect("envelope should exist at this size");

        // Locate the toast box: it is right-anchored within `envelope` and
        // spans `envelope.height` rows. Scan row `envelope.y` for the
        // left/right rounded corners to find its exact bounds.
        let y = envelope.y;
        let mut left_x = None;
        let mut right_x = None;
        for x in envelope.x..envelope.x + envelope.width {
            let sym = buf.cell((x, y)).unwrap().symbol().to_string();
            if sym == "╭" {
                left_x = Some(x);
            }
            if sym == "╮" {
                right_x = Some(x);
                break;
            }
        }
        let left_x = left_x.expect("toast top-left corner not found");
        let right_x = right_x.expect("toast top-right corner not found");
        assert!(right_x > left_x);

        let expected_bg = theme::toast_style().bg.expect("toast_style must set a bg");
        for row in y..y + envelope.height {
            for col in left_x..=right_x {
                let cell = buf.cell((col, row)).unwrap();
                assert_eq!(
                    cell.bg, expected_bg,
                    "cell ({col},{row}) inside toast box lacks the solid toast background"
                );
            }
        }

        // No spillover: cells immediately outside the toast box's right
        // edge (if any remain within the terminal) must NOT carry the
        // toast's background color.
        if right_x + 1 < width {
            let cell = buf.cell((right_x + 1, y)).unwrap();
            assert_ne!(
                cell.bg, expected_bg,
                "toast background spilled over its right border"
            );
        }
    }
}

#[cfg(test)]
mod query_bar_height_regression {
    use super::*;
    use crate::app::{ActionState, App};
    use ratatui::{Terminal, backend::TestBackend};

    /// The query bar should grow beyond its default 3-row height (1 line of
    /// text + 2 border rows) up to a max of 5 rows (3 wrapped text lines)
    /// once the in-progress query is long enough to wrap past one line.
    #[test]
    fn long_query_expands_bar_up_to_five_rows() {
        let mut app = App::new(crate::config::ConnectionConfig::default(), false);
        app.mode = Mode::QueryInput;
        // Long enough to wrap across several lines at typical widths.
        let long_query = "SELECT * FROM datalake.sales.orders WHERE ".to_string()
            + &"customer_id = 'abc123' AND ".repeat(14)
            + "1=1";
        let cursor = long_query.len();
        app.screen = Screen::Actions(ActionState {
            catalog: "datalake".into(),
            schema: "sales".into(),
            table: "orders".into(),
            selected: 4,
            query_buffer: long_query,
            query_cursor: cursor,
            ..Default::default()
        });
        app.main_panel_pct = 15;

        let backend = TestBackend::new(120, 40);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|f| ui(f, &app)).unwrap();
        let buf = terminal.backend().buffer().clone();

        // Query bar starts right after the search bar (default height 3,
        // since Mode::QueryInput doesn't affect search_height) at y=3.
        // Scan downward from there for the query bar's own bottom border
        // (rounded corner "╰") to measure its actual rendered height.
        let query_bar_top = 3u16;
        let mut bottom_y = query_bar_top;
        for y in query_bar_top..40u16 {
            let has_corner = (0..120u16).any(|x| buf.cell((x, y)).unwrap().symbol() == "╰");
            if has_corner {
                bottom_y = y;
                break;
            }
        }
        let height = bottom_y - query_bar_top + 1;
        assert_eq!(
            height, 7,
            "expected the query bar to expand to its max of 5 text rows + 2 border rows (7 total) for a long wrapped query, got {height}"
        );
    }

    /// A short query must not force the bar to expand — it should stay at
    /// the default single-line height of 3 rows.
    #[test]
    fn short_query_keeps_default_three_row_height() {
        let mut app = App::new(crate::config::ConnectionConfig::default(), false);
        app.mode = Mode::QueryInput;
        let short_query = "SELECT 1".to_string();
        let cursor = short_query.len();
        app.screen = Screen::Actions(ActionState {
            catalog: "datalake".into(),
            schema: "sales".into(),
            table: "orders".into(),
            selected: 4,
            query_buffer: short_query,
            query_cursor: cursor,
            ..Default::default()
        });
        app.main_panel_pct = 15;

        let backend = TestBackend::new(120, 40);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|f| ui(f, &app)).unwrap();
        let buf = terminal.backend().buffer().clone();

        let query_bar_top = 3u16;
        let mut bottom_y = query_bar_top;
        for y in query_bar_top..40u16 {
            let has_corner = (0..120u16).any(|x| buf.cell((x, y)).unwrap().symbol() == "╰");
            if has_corner {
                bottom_y = y;
                break;
            }
        }
        let height = bottom_y - query_bar_top + 1;
        assert_eq!(height, 3, "short query should keep the default 3-row height, got {height}");
    }
}
