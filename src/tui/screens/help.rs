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
    layout::{Constraint, Layout, Rect},
    text::{Line, Span},
    widgets::{
        Block, BorderType, Borders, Clear, Paragraph, Scrollbar, ScrollbarOrientation,
        ScrollbarState, Wrap,
    },
};

use crate::tui::theme;

struct HelpSection {
    title: &'static str,
    items: &'static [&'static str],
}

const HELP_SECTIONS: &[HelpSection] = &[
    HelpSection {
        title: "Active Pane Concept & Layout",
        items: &[
            "  All Tables View               Left: Catalogs/Schemas/Tables | Right: Help Pane",
            "  Inside Table View             Left: Menu Pane (15%) | Right: Query & Main Preview (85%)",
            "  Shift+H/L / Shift+←/→ / Tab / Click  Switch focus between Menu Pane (left) and Preview Pane (right)",
            "  Option Selection              Selecting a menu option shifts focus to Preview Pane while option stays highlighted",
        ],
    },
    HelpSection {
        title: "Navigation & Scrolling",
        items: &[
            "  j / ↓ / Mouse Wheel Down       Move menu selection down / move cell cursor down / scroll pane",
            "  k / ↑ / Mouse Wheel Up         Move menu selection up / move cell cursor up / scroll pane",
            "  PgDn / Ctrl+d / Ctrl+f         Scroll page down",
            "  PgUp / Ctrl+u / Ctrl+b         Scroll page up",
            "  h / ← / [ / ,                  Move cell cursor left / scroll columns left",
            "  l / → / ] / .                  Move cell cursor right / scroll columns right",
            "  H / Ctrl+← / Shift+←           Jump 5 columns left",
            "  L / Ctrl+→ / Shift+→           Jump 5 columns right",
            "  Home / 0                       Jump to first column / beginning of list",
            "  End / $                        Jump to last column / end of list",
            "  Mouse Wheel / Drag             Wheel on scrollbar/header or drag scrollbar to scroll columns",
            "  g / G                          Jump to top / bottom of active preview pane",
            "  C                              Open column visibility modal (filter which columns to see in view)",
            "  F                              Open filter modal for the current column",
            "  y                              Copy loaded result grid as TSV (Y exports it as CSV)",
            "  Esc                            Switch focus to Menu Pane / Go back to Tables list",
            "  Enter                          Select menu option & focus Preview Pane",
        ],
    },
    HelpSection {
        title: "Table View Options & Hotkeys",
        items: &[
            "  v                              Table View Mode (Infinite Scroll grid)",
            "  c / i                          Table DDL / Info Schema",
            "  s / p                          Show Stats / Sample (20 rows)",
            "  P / S                          Partitions Tree / Vertical Schema Inspector",
        ],
    },
    HelpSection {
        title: "Partitioned Table Drill-Down (cd/ls style)",
        items: &[
            "  (auto)                         Entering a table runs recon (SHOW CREATE TABLE, etc.) up front;",
            "                                 Partitions/Schema/Table View wait for it to finish",
            "  v on a partitioned table       Table View browses partition values one level at a time",
            "                                 instead of querying every row at once (avoids split errors)",
            "  > / Enter / →                  Drill into the highlighted partition value (go one level deeper)",
            "  < / ←                          Go up one partition level (no-op at the top level)",
            "                                 In the leaf record grid, < still goes up a level;",
            "                                 h / j / k / l navigates cells across rows and columns",
            "  Esc                            Always returns to the Menu Pane, regardless of drill-down depth",
        ],
    },
    HelpSection {
        title: "Search & Custom Query",
        items: &[
            "  /                              Search / filter current view or modal",
            "  q or :                         Write custom SQL in Query Bar (in Table View)",
            "  Ctrl+C                         Quit lazyTrino",
        ],
    },
];

pub fn build_help_lines(search_query: &str) -> Vec<Line<'static>> {
    let query_lower = search_query.trim().to_lowercase();
    let mut help_text: Vec<Line<'static>> = Vec::new();

    if query_lower.is_empty() {
        for (i, section) in HELP_SECTIONS.iter().enumerate() {
            if i > 0 {
                help_text.push(Line::from(""));
            }
            help_text.push(Line::from(Span::styled(
                format!(" {}", section.title),
                theme::header_style(),
            )));
            for item in section.items {
                help_text.push(Line::from(*item));
            }
        }
    } else {
        let mut found_any = false;
        for section in HELP_SECTIONS {
            let section_title_matches = section.title.to_lowercase().contains(&query_lower);
            let matching_items: Vec<&str> = section
                .items
                .iter()
                .copied()
                .filter(|item| section_title_matches || item.to_lowercase().contains(&query_lower))
                .collect();

            if !matching_items.is_empty() {
                if found_any {
                    help_text.push(Line::from(""));
                }
                found_any = true;
                help_text.push(Line::from(Span::styled(
                    format!(" {}", section.title),
                    theme::header_style(),
                )));
                for item in matching_items {
                    help_text.push(Line::from(item));
                }
            }
        }

        if !found_any {
            help_text.push(Line::from(Span::styled(
                format!("  No matching shortcuts or help entries for \"{search_query}\""),
                theme::muted_style(),
            )));
        }
    }

    help_text
}

pub fn render_side_panel(frame: &mut Frame, area: Rect) {
    let block = Block::default()
        .title(" Help & Keybindings ")
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(theme::border_style(false));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let help_lines = build_help_lines("");
    let paragraph = Paragraph::new(help_lines).wrap(Wrap { trim: false });
    frame.render_widget(paragraph, inner);
}

pub fn render_modal(
    frame: &mut Frame,
    area: Rect,
    search_query: &str,
    is_searching: bool,
    scroll: usize,
) {
    let modal_width = (area.width * 70 / 100).clamp(55, 95).min(area.width);
    let max_height = area.height.saturating_sub(4).max(10);
    let modal_height = (area.height * 75 / 100).clamp(16, 28).min(max_height).min(area.height);

    let popup_area = Rect {
        x: area.x + (area.width.saturating_sub(modal_width)) / 2,
        y: area.y + (area.height.saturating_sub(modal_height)) / 2,
        width: modal_width,
        height: modal_height,
    };

    frame.render_widget(Clear, popup_area);

    let block = Block::default()
        .title(" Help & Keybindings ")
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(theme::border_style(true));
    let inner = block.inner(popup_area);
    frame.render_widget(block, popup_area);

    if inner.height < 4 {
        return;
    }

    let chunks = Layout::vertical([
        Constraint::Length(3), // Dedicated search box
        Constraint::Min(0),    // Help content
        Constraint::Length(1), // Hint footer
    ])
    .split(inner);

    let search_area = chunks[0];
    let content_area = chunks[1];
    let hint_area = chunks[2];

    let search_title = if is_searching {
        " Search [/] [Typing — Enter/Esc to finish] "
    } else {
        " Search [/] "
    };
    let search_block = Block::default()
        .title(search_title)
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(theme::border_style(is_searching));
    let s_inner = search_block.inner(search_area);
    frame.render_widget(search_block, search_area);

    let search_line = if search_query.is_empty() {
        if is_searching {
            Line::from(vec![
                Span::styled("/ ", theme::warning_bold_style()),
                Span::styled("█", theme::warning_bold_style()),
            ])
        } else {
            Line::from(vec![
                Span::styled("/ ", theme::muted_style()),
                Span::styled("Type / to search shortcuts...", theme::muted_style()),
            ])
        }
    } else {
        let mut spans = vec![
            Span::styled("/ ", theme::warning_bold_style()),
            Span::styled(search_query, theme::text_style()),
        ];
        if is_searching {
            spans.push(Span::styled("█", theme::warning_bold_style()));
        }
        Line::from(spans)
    };
    frame.render_widget(Paragraph::new(search_line), s_inner);

    let help_lines = build_help_lines(search_query);
    let total_lines = help_lines.len();
    let visible_height = content_area.height as usize;
    let max_scroll = total_lines.saturating_sub(visible_height);
    let effective_scroll = scroll.min(max_scroll);

    let paragraph = Paragraph::new(help_lines).scroll((effective_scroll as u16, 0));
    frame.render_widget(paragraph, content_area);

    if total_lines > visible_height && visible_height > 0 {
        let mut scrollbar_state = ScrollbarState::new(max_scroll).position(effective_scroll);
        frame.render_stateful_widget(
            Scrollbar::default()
                .orientation(ScrollbarOrientation::VerticalRight)
                .begin_symbol(Some("▲"))
                .end_symbol(Some("▼")),
            content_area,
            &mut scrollbar_state,
        );
    }

    let hint_line = if is_searching {
        Line::from(vec![
            Span::styled("Type", theme::selection_style()),
            Span::styled(":filter  ", theme::muted_style()),
            Span::styled("Enter/Esc", theme::selection_style()),
            Span::styled(":done searching", theme::muted_style()),
        ])
    } else {
        Line::from(vec![
            Span::styled("/", theme::selection_style()),
            Span::styled(":search  ", theme::muted_style()),
            Span::styled("j/k", theme::selection_style()),
            Span::styled(":scroll  ", theme::muted_style()),
            Span::styled("Esc/?/q", theme::selection_style()),
            Span::styled(":close help", theme::muted_style()),
        ])
    };
    frame.render_widget(Paragraph::new(hint_line), hint_area);
}

#[allow(dead_code)]
pub fn render(frame: &mut Frame, area: Rect, search_query: &str, is_searching: bool) {
    render_modal(frame, area, search_query, is_searching, 0);
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{Terminal, backend::TestBackend};

    #[test]
    fn test_help_render_empty_search() {
        let backend = TestBackend::new(100, 40);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| {
                render(f, f.area(), "", false);
            })
            .unwrap();
        let buf = terminal.backend().buffer().clone();
        let mut title_found = false;
        for y in 0..40 {
            for x in 0..80 {
                if buf.cell((x, y)).unwrap().symbol() == "H"
                    && buf.cell((x + 1, y)).unwrap().symbol() == "e"
                    && buf.cell((x + 2, y)).unwrap().symbol() == "l"
                    && buf.cell((x + 3, y)).unwrap().symbol() == "p"
                {
                    title_found = true;
                    break;
                }
            }
        }
        assert!(title_found);
    }

    #[test]
    fn test_help_render_with_filter() {
        let backend = TestBackend::new(100, 40);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| {
                render(f, f.area(), "filter", true);
            })
            .unwrap();
    }

    #[test]
    fn test_help_render_side_panel() {
        let backend = TestBackend::new(50, 30);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| {
                render_side_panel(f, f.area());
            })
            .unwrap();
    }
}
