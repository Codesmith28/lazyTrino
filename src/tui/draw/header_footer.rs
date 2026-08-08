use ratatui::{
    Frame,
    layout::{Alignment, Rect},
    text::Line,
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

pub fn render_footer(frame: &mut Frame, area: Rect, app: &App) {
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

    let text = format!(" Copied: \"{}\" ", sanitize_toast_text(msg));
    let content_w = text.chars().count() as u16 + 2;
    let toast_w = content_w.clamp(TOAST_MIN_W, TOAST_MAX_W);
    let toast_area = Rect {
        x: envelope.x + envelope.width - toast_w,
        y: envelope.y,
        width: toast_w,
        height: envelope.height,
    };

    frame.render_widget(ratatui::widgets::Clear, toast_area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .style(theme::toast_style());
    let inner = block.inner(toast_area);
    frame.render_widget(block, toast_area);

    let toast_text = Paragraph::new(Line::from(truncate_hint(&text, inner.width as usize)))
        .style(theme::toast_style())
        .alignment(Alignment::Center);
    frame.render_widget(toast_text, inner);
}
