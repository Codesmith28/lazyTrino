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
    widgets::{Block, BorderType, Borders, Paragraph, Wrap},
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
            "  j / ↓ / Mouse Wheel Down       Move menu selection down / scroll active preview pane",
            "  k / ↑ / Mouse Wheel Up         Move menu selection up / scroll active preview pane",
            "  g / G                          Jump to top / bottom of active preview pane",
            "  < / >                          Move cell cursor horizontally (left / right) across columns",
            "  C                              Open column visibility modal (filter which columns to see in view)",
            "  F                              Open filter modal for the current column",
            "  y                              Copy loaded result grid as TSV (Y exports it as CSV)",
            "  h / ← / Esc                    Switch focus to Menu Pane / Go back to Tables list",
            "  l / → / Enter                  Select menu option & focus Preview Pane",
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
            "  l / → / Enter                  Drill into the highlighted partition value (go one level deeper)",
            "  h / ←                          Go up one partition level (no-op at the top level)",
            "                                 In the leaf record grid, h still goes up a level; l is a",
            "                                 no-op there — use < / > to scroll columns instead",
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

pub fn render(frame: &mut Frame, area: Rect, search_query: &str, is_searching: bool) {
    let block = Block::default()
        .title(" Help & Keybindings ")
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(theme::border_style(true));
    frame.render_widget(&block, area);

    let inner = block.inner(area);
    if inner.height < 2 {
        return;
    }

    let show_search = is_searching || !search_query.is_empty();
    let (search_area, content_area, hint_area) = if show_search {
        let chunks = Layout::vertical([
            Constraint::Length(1),
            Constraint::Min(0),
            Constraint::Length(1),
        ])
        .split(inner);
        (Some(chunks[0]), chunks[1], chunks[2])
    } else {
        let chunks = Layout::vertical([Constraint::Min(0), Constraint::Length(1)]).split(inner);
        (None, chunks[0], chunks[1])
    };

    if let Some(s_area) = search_area {
        let prompt_span = Span::styled("/ ", theme::warning_bold_style());
        let query_span = Span::styled(search_query, theme::text_style());
        let cursor_span = if is_searching {
            Span::styled("█", theme::warning_bold_style())
        } else {
            Span::styled(" (press / to filter)", theme::muted_style())
        };
        let line = Line::from(vec![prompt_span, query_span, cursor_span]);
        frame.render_widget(Paragraph::new(line), s_area);
    }

    let query_lower = search_query.trim().to_lowercase();
    let mut help_text: Vec<Line> = Vec::new();

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

    let paragraph = Paragraph::new(help_text).wrap(Wrap { trim: false });
    frame.render_widget(paragraph, content_area);

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
            Span::styled(":filter help  ", theme::muted_style()),
            Span::styled("Esc/?", theme::selection_style()),
            Span::styled(":close help", theme::muted_style()),
        ])
    };
    frame.render_widget(Paragraph::new(hint_line), hint_area);
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
}
