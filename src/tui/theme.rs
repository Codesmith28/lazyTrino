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

use ratatui::style::{Color, Modifier, Style};

pub const ACTIVE_BORDER: Color = Color::Yellow;
pub const INACTIVE_BORDER: Color = Color::DarkGray;
pub const ACCENT_BORDER: Color = Color::Cyan;

pub const HIGHLIGHT_FG: Color = Color::Yellow;
pub const INFO_FG: Color = Color::Cyan;
pub const HEADER_FG: Color = Color::Green;
pub const ERROR_FG: Color = Color::Red;
pub const TEXT_FG: Color = Color::White;
pub const MUTED_FG: Color = Color::DarkGray;
pub const SECONDARY_FG: Color = Color::Gray;
pub const DETAIL_FG: Color = Color::Magenta;

pub const SELECTION_FG: Color = Color::Black;
pub const SELECTION_BG: Color = Color::Cyan;
pub const QUERY_SELECTION_BG: Color = Color::LightYellow;

pub fn style(color: Color) -> Style {
    Style::default().fg(color)
}

pub fn bold_style(color: Color) -> Style {
    style(color).add_modifier(Modifier::BOLD)
}

pub fn border_color(is_active: bool) -> Color {
    if is_active {
        ACTIVE_BORDER
    } else {
        INACTIVE_BORDER
    }
}

pub fn border_style(is_active: bool) -> Style {
    style(border_color(is_active))
}

pub fn query_bar_border_color(is_editing: bool, is_table_view: bool) -> Color {
    if is_editing {
        ACTIVE_BORDER
    } else if is_table_view {
        ACCENT_BORDER
    } else {
        INACTIVE_BORDER
    }
}

pub fn selection_style() -> Style {
    Style::default()
        .fg(SELECTION_FG)
        .bg(SELECTION_BG)
        .add_modifier(Modifier::BOLD)
}

/// Muted stand-in for `selection_style` used to mark the *last* selected
/// row in a list whose pane is no longer focused. Keeping some visual
/// marker (instead of dropping the highlight entirely) preserves "where was
/// I" context when tabbing back, but it must never be confused with the
/// bright active-selection color — otherwise two panes can appear to have
/// a "current" selection at once, which reads as the selection persisting/
/// leaking across panes.
pub fn inactive_selection_style() -> Style {
    Style::default().fg(TEXT_FG).bg(MUTED_FG)
}

/// Picks the appropriate selection highlight for a row's keyboard-cursor
/// state based on whether the row's own pane currently has focus.
pub fn selection_style_for(is_active: bool) -> Style {
    if is_active {
        selection_style()
    } else {
        inactive_selection_style()
    }
}

pub fn query_selection_style() -> Style {
    Style::default()
        .fg(SELECTION_FG)
        .bg(QUERY_SELECTION_BG)
        .add_modifier(Modifier::BOLD)
}

pub fn header_style() -> Style {
    bold_style(HEADER_FG)
}

pub fn text_style() -> Style {
    style(TEXT_FG)
}

pub fn bold_text_style() -> Style {
    bold_style(TEXT_FG)
}

pub fn muted_style() -> Style {
    style(MUTED_FG)
}

pub fn secondary_style() -> Style {
    style(SECONDARY_FG)
}

pub fn footer_style() -> Style {
    Style::default().fg(TEXT_FG).bg(INACTIVE_BORDER)
}

pub fn error_style() -> Style {
    style(ERROR_FG)
}

pub fn error_bold_style() -> Style {
    bold_style(ERROR_FG)
}

pub fn info_style() -> Style {
    style(INFO_FG)
}

pub fn info_bold_style() -> Style {
    bold_style(INFO_FG)
}

pub fn warning_style() -> Style {
    style(HIGHLIGHT_FG)
}

pub fn warning_bold_style() -> Style {
    bold_style(HIGHLIGHT_FG)
}

pub fn success_style() -> Style {
    style(HEADER_FG)
}

pub fn success_bold_style() -> Style {
    bold_style(HEADER_FG)
}

/// Solid-background style for toast/notification popups. Unlike
/// `success_bold_style`, this sets an explicit background so the toast box
/// fully opaquely overwrites whatever was rendered underneath it, with no
/// gaps of terminal-default background showing through.
pub fn toast_style() -> Style {
    Style::default()
        .fg(HEADER_FG)
        .bg(Color::Black)
        .add_modifier(Modifier::BOLD)
}

pub fn detail_style() -> Style {
    style(DETAIL_FG)
}

pub fn input_field_style(is_focused: bool) -> Style {
    if is_focused {
        text_style().bg(MUTED_FG)
    } else {
        secondary_style()
    }
}
