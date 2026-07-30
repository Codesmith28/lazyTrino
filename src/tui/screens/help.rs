use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Paragraph, Wrap},
    Frame,
};

pub fn render(frame: &mut Frame, area: Rect) {
    let block = Block::default()
        .title(" Help & Keybindings ")
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::Yellow));
    frame.render_widget(&block, area);

    let inner = block.inner(area);

    let help_text = vec![
        Line::from(Span::styled(" Active Pane Concept & Focus", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD))),
        Line::from("  Shift+H/J/K/L or Shift+Arrows  Switch active pane focus (Search, Main, Partitions, Schema, Query Log)"),
        Line::from("  Left Click inside pane         Activate / focus clicked pane"),
        Line::from("  Left Drag on vertical border   Resize panel layout width"),
        Line::from(""),
        Line::from(Span::styled(" Unified Active Pane Navigation & Scrolling", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD))),
        Line::from("  j / ↓ / Mouse Wheel Down       Scroll active pane / move selection down"),
        Line::from("  k / ↑ / Mouse Wheel Up         Scroll active pane / move selection up"),
        Line::from("  g / G                          Jump to top / bottom of active pane"),
        Line::from("  h / ← / Esc                    Go back to previous screen (Main Viewer)"),
        Line::from("  l / → / Enter                  Select / drill in (Main Viewer)"),
        Line::from(""),
        Line::from(Span::styled(" Actions & Search", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD))),
        Line::from("  /                      Focus Centralized Top Search Bar"),
        Line::from("  <space>                Enter leader mode for instant actions"),
        Line::from("  <space> d / c / p / P  Describe / Table DDL / Preview / Partitions"),
        Line::from("  ?                      Toggle this help screen"),
        Line::from("  Ctrl+C                 Quit lazyTrino"),
        Line::from(""),
        Line::from(Span::styled(" Press Esc to close help", Style::default().fg(Color::DarkGray))),
    ];

    let paragraph = Paragraph::new(help_text).wrap(Wrap { trim: false });
    frame.render_widget(paragraph, inner);
}
