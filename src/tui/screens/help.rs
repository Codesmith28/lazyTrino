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
    widgets::{Block, BorderType, Borders, Paragraph, Wrap},
};

use crate::tui::theme;

pub fn render(frame: &mut Frame, area: Rect) {
    let block = Block::default()
        .title(" Help & Keybindings ")
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(theme::border_style(true));
    frame.render_widget(&block, area);

    let inner = block.inner(area);

    let help_text = vec![
        Line::from(Span::styled(
            " Active Pane Concept & Layout",
            theme::header_style(),
        )),
        Line::from(
            "  All Tables View               Left: Catalogs/Schemas/Tables | Right: Help Pane",
        ),
        Line::from(
            "  Inside Table View             Left: Menu Pane (10%) | Right: Search, Query & Main Preview (90%)",
        ),
        Line::from(
            "  Shift+H/L / Shift+←/→ / Tab / Click  Switch focus between Menu Pane (left) and Preview Pane (right)",
        ),
        Line::from(
            "  Option Selection              Selecting a menu option shifts focus to Preview Pane while option stays highlighted",
        ),
        Line::from(""),
        Line::from(Span::styled(
            " Navigation & Scrolling",
            theme::header_style(),
        )),
        Line::from(
            "  j / ↓ / Mouse Wheel Down       Move menu selection down / scroll active preview pane",
        ),
        Line::from(
            "  k / ↑ / Mouse Wheel Up         Move menu selection up / scroll active preview pane",
        ),
        Line::from("  g / G                          Jump to top / bottom of active preview pane"),
        Line::from(
            "  h / ← / Esc                    Switch focus to Menu Pane / Go back to Tables list",
        ),
        Line::from("  l / → / Enter                  Select menu option & focus Preview Pane"),
        Line::from(""),
        Line::from(Span::styled(
            " Table View Options & Hotkeys",
            theme::header_style(),
        )),
        Line::from("  v                              Table View Mode (Infinite Scroll grid)"),
        Line::from("  d / c / i                      Describe / Table DDL / Info Schema"),
        Line::from("  s / n / p                      Show Stats / Row Count / Sample (20 rows)"),
        Line::from("  P / S                          Partitions Tree / Vertical Schema Inspector"),
        Line::from(""),
        Line::from(Span::styled(
            " Search & Custom Query",
            theme::header_style(),
        )),
        Line::from("  /                              Focus Centralized Search Bar"),
        Line::from(
            "  q or :                         Write custom SQL in Query Bar (in Table View)",
        ),
        Line::from("  Ctrl+C                         Quit lazyTrino"),
    ];

    let paragraph = Paragraph::new(help_text).wrap(Wrap { trim: false });
    frame.render_widget(paragraph, inner);
}
