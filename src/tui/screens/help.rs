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
        Line::from(Span::styled(" Basic Navigation (Lists & Screens)", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD))),
        Line::from("  j / ↓                  Move selection down"),
        Line::from("  k / ↑                  Move selection up"),
        Line::from("  h / ← / Esc            Go back to previous screen"),
        Line::from("  l / → / Enter          Select / drill in"),
        Line::from(""),
        Line::from(Span::styled(" Inspector Sub-Panel Scrolling", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD))),
        Line::from("  Option+j / Alt+↓       Scroll Vertical Schema down"),
        Line::from("  Option+k / Alt+↑       Scroll Vertical Schema up"),
        Line::from("  Option+Shift+j / Alt+Shift+↓  Scroll Partitions Tree down"),
        Line::from("  Option+Shift+k / Alt+Shift+↑  Scroll Partitions Tree up"),
        Line::from(""),
        Line::from(Span::styled(" Actions & Search", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD))),
        Line::from("  /                      Focus Centralized Top Search Bar"),
        Line::from("  <space>                Enter leader mode for instant actions"),
        Line::from("  <space> d / c / p / P  Describe / Show Create / Preview / Partitions"),
        Line::from("  ?                      Toggle this help screen"),
        Line::from("  Ctrl+C                 Quit lazyTrino"),
        Line::from(""),
        Line::from(Span::styled(" Press Esc to close help", Style::default().fg(Color::DarkGray))),
    ];

    let paragraph = Paragraph::new(help_text).wrap(Wrap { trim: false });
    frame.render_widget(paragraph, inner);
}
