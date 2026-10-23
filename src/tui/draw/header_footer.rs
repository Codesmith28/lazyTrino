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
                    " j/k:move  l/Enter:select  /:search  V:select  ^E:export  ?:help  Ctrl+C:quit ",
                ),
                Screen::Schema(_) | Screen::Table(_) => Some(
                    " j/k:move  l/Enter:select  h/Esc:back  /:search  V:select  ^E:export  ?:help  Ctrl+C:quit ",
                ),
                Screen::Actions(state) => {
                    let selected_action = ACTIONS.get(state.selected).map(|(_, _, action)| *action);

                    match app.active_panel {
                        ActivePanel::MenuPane => Some(
                            " j/k:move  l/Enter:run  h/Esc:back  A:filters  v/c/e/t/m/p:action  Tab:pane  ?:help  Ctrl+C:quit ",
                        ),
                        ActivePanel::MainViewer => match selected_action {
                            Some(Action::TableView)
                                if state.results.is_some() && state.drilldown.is_some() =>
                            {
                                Some(
                                    " hjkl/←→:cells  <:up  S:sort  C:cols  F:filter  A:filters  u:clear sel  g/G:top/btm  q/:query  Esc:menu  Tab:pane  V/v:select  ^E:export  ?:help  Ctrl+C:quit ",
                                )
                            }
                            Some(Action::TableView) if state.results.is_some() => Some(
                                " hjkl/←→:cells  S:sort  C:cols  F:filter  A:filters  u:clear sel  g/G:top/btm  q/:query  Esc:menu  Tab:pane  V/v:select  ^E:export  ?:help  Ctrl+C:quit ",
                            ),
                            Some(Action::Partitions) if !app.partition_tree_lines.is_empty() => {
                                Some(
                                    " j/k:scroll  g/G:top/btm  Esc:menu  Tab:pane  V/v:select  ^E:export  ?:help  Ctrl+C:quit ",
                                )
                            }
                            Some(Action::Schema) if !app.vertical_schema_cols.is_empty() => Some(
                                " j/k:scroll  g/G:top/btm  q/:query  Esc:menu  Tab:pane  V/v:select  ^E:export  ?:help  Ctrl+C:quit ",
                            ),
                            _ if state.results.is_some() => Some(
                                " hjkl/←→:cells  S:sort  C:cols  F:filter  u:clear sel  g/G:top/btm  q/:query  Esc:menu  Tab:pane  V/v:select  ^E:export  ?:help  Ctrl+C:quit ",
                            ),
                            Some(Action::TableView) if state.drilldown.is_some() => Some(
                                " j/k:move  >/Enter:drill  <:up  u:clear sel  q/:query  Esc:menu  Tab:pane  V/v:select  ^E:export  ?:help  Ctrl+C:quit ",
                            ),
                            Some(Action::TableView) => Some(
                                " q/:query  Esc:menu  Tab:pane  V/v:select  ^E:export  ?:help  Ctrl+C:quit ",
                            ),
                            _ => Some(
                                " q/:query  Esc:menu  Tab:pane  V/v:select  ^E:export  ?:help  Ctrl+C:quit ",
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
    let text = format!(" {} ", sanitize_toast_text(&notice.text));
    let toast_w = (text.chars().count() as u16 + 2).clamp(TOAST_MIN_W, max_w);
    let toast_area = Rect {
        x: area.x + area.width - toast_w - 1,
        y: area.y + 1,
        width: toast_w,
        height: TOAST_H,
    };
    render_toast_card(
        frame,
        toast_area,
        &format!(" {icon} {label} "),
        accent,
        &truncate_middle(&text, toast_w.saturating_sub(2) as usize),
    );
}

/// A toast is a card: accent-colored border and title, message in the
/// terminal's own text color, on a cleared (opaque) area.
fn render_toast_card(
    frame: &mut Frame,
    area: Rect,
    title: &str,
    accent: ratatui::style::Style,
    text: &str,
) {
    frame.render_widget(ratatui::widgets::Clear, area);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(accent)
        .title(Span::styled(title.to_string(), accent));
    let inner = block.inner(area);
    frame.render_widget(block, area);
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            text.to_string(),
            theme::toast_text_style(),
        )))
        .alignment(Alignment::Center),
        inner,
    );
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

    let footer = Paragraph::new(hint_line(&truncate_hint(hint, area.width as usize)))
        .style(theme::footer_style())
        .wrap(ratatui::widgets::Wrap { trim: true });
    frame.render_widget(footer, area);
}

pub const TOAST_MIN_W: u16 = 24;
pub const TOAST_MAX_W: u16 = 60;
pub const TOAST_H: u16 = 3;

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
        &truncate_hint(&text, toast_w.saturating_sub(2) as usize),
    );
}
