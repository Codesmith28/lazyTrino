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

/// The single palette lazyTrino draws with. It is readable on light *and*
/// dark terminals: body text uses the terminal's own default foreground,
/// accents are mid-tone fixed colors from the 256-color cube (terminal
/// profiles do not remap those, so their contrast is predictable), and every
/// highlight / bar sets an explicit foreground *and* background.
struct Palette {
    text: Color,
    secondary: Color,
    muted: Color,
    highlight: Color,
    info: Color,
    header: Color,
    error: Color,
    detail: Color,
    active_border: Color,
    inactive_border: Color,
    accent_border: Color,
    /// Foreground / background pairs: always set together so a highlight
    /// never depends on the terminal's own background.
    on_dark_fg: Color,
    selection_bg: Color,
    inactive_selection_bg: Color,
    bar_bg: Color,
    toast_bg: Color,
    toast_fg: Color,
    toast_error_fg: Color,
}

// Foregrounds sit between ~3.3:1 and ~6.3:1 against both pure white and
// pure black (checked by `palette_palette_is_legible_on_light_and_dark`).
const PALETTE: Palette = Palette {
    text: Color::Reset,
    secondary: Color::Reset,
    muted: Color::Indexed(243),     // #767676
    highlight: Color::Indexed(136), // #af8700 golden
    info: Color::Indexed(31),       // #0087af teal
    header: Color::Indexed(28),     // #008700 green
    error: Color::Indexed(196),     // #ff0000
    detail: Color::Indexed(133),    // #af5faf
    active_border: Color::Indexed(136),
    inactive_border: Color::Indexed(243),
    accent_border: Color::Indexed(31),
    on_dark_fg: Color::Indexed(231),            // #ffffff
    selection_bg: Color::Indexed(24),           // #005f87
    inactive_selection_bg: Color::Indexed(240), // #585858
    bar_bg: Color::Indexed(240),
    toast_bg: Color::Indexed(16),        // #000000
    toast_fg: Color::Indexed(41),        // #00d75f
    toast_error_fg: Color::Indexed(203), // #ff5f5f
};

fn pal() -> &'static Palette {
    &PALETTE
}

pub fn style(color: Color) -> Style {
    Style::default().fg(color)
}

pub fn bold_style(color: Color) -> Style {
    style(color).add_modifier(Modifier::BOLD)
}

pub fn detail_color() -> Color {
    pal().detail
}

pub fn secondary_color() -> Color {
    pal().secondary
}

pub fn accent_border_color() -> Color {
    pal().accent_border
}

pub fn inactive_border_color() -> Color {
    pal().inactive_border
}

pub fn border_color(is_active: bool) -> Color {
    if is_active {
        pal().active_border
    } else {
        pal().inactive_border
    }
}

pub fn border_style(is_active: bool) -> Style {
    style(border_color(is_active))
}

pub fn query_bar_border_color(is_editing: bool, is_table_view: bool) -> Color {
    if is_editing {
        pal().active_border
    } else if is_table_view {
        pal().accent_border
    } else {
        pal().inactive_border
    }
}

/// Keyboard-cursor highlight (the row/cell you are *on*). Explicit white on
/// a deep background so it reads the same on any terminal background.
pub fn selection_style() -> Style {
    Style::default()
        .fg(pal().on_dark_fg)
        .bg(pal().selection_bg)
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
    Style::default()
        .fg(pal().on_dark_fg)
        .bg(pal().inactive_selection_bg)
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

/// Highlight for text that is *selected for copying* (mouse drag, visual
/// modes, list ranges, query-bar selection). It deliberately uses no theme
/// colors: reverse video on the terminal's own default foreground/background,
/// so it reads white-on-black in a dark terminal and black-on-white in a
/// light one, and never looks like the keyboard-cursor highlight.
pub fn text_selection_style() -> Style {
    Style::default()
        .fg(Color::Reset)
        .bg(Color::Reset)
        .add_modifier(Modifier::REVERSED)
}

pub fn query_selection_style() -> Style {
    text_selection_style()
}

pub fn header_style() -> Style {
    bold_style(pal().header)
}

pub fn text_style() -> Style {
    style(pal().text)
}

pub fn bold_text_style() -> Style {
    bold_style(pal().text)
}

pub fn muted_style() -> Style {
    style(pal().muted)
}

pub fn secondary_style() -> Style {
    style(pal().secondary)
}

pub fn footer_style() -> Style {
    Style::default().fg(pal().on_dark_fg).bg(pal().bar_bg)
}

pub fn error_style() -> Style {
    style(pal().error)
}

pub fn error_bold_style() -> Style {
    bold_style(pal().error)
}

pub fn info_style() -> Style {
    style(pal().info)
}

pub fn info_bold_style() -> Style {
    bold_style(pal().info)
}

pub fn warning_style() -> Style {
    style(pal().highlight)
}

pub fn warning_bold_style() -> Style {
    bold_style(pal().highlight)
}

pub fn success_style() -> Style {
    style(pal().header)
}

pub fn success_bold_style() -> Style {
    bold_style(pal().header)
}

/// Solid-background style for toast/notification popups. Unlike
/// `success_bold_style`, this sets an explicit background so the toast box
/// fully opaquely overwrites whatever was rendered underneath it, with no
/// gaps of terminal-default background showing through.
pub fn toast_style() -> Style {
    Style::default()
        .fg(pal().toast_fg)
        .bg(pal().toast_bg)
        .add_modifier(Modifier::BOLD)
}

pub fn error_toast_style() -> Style {
    Style::default()
        .fg(pal().toast_error_fg)
        .bg(pal().toast_bg)
        .add_modifier(Modifier::BOLD)
}

pub fn detail_style() -> Style {
    style(pal().detail)
}

pub fn input_field_style(is_focused: bool) -> Style {
    if is_focused {
        Style::default().fg(pal().on_dark_fg).bg(pal().bar_bg)
    } else {
        secondary_style()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// xterm 256-color cube / gray ramp (fixed values, never remapped).
    fn rgb(color: Color) -> (u8, u8, u8) {
        match color {
            Color::Indexed(i @ 16..=231) => {
                let n = i - 16;
                let lv = [0u8, 95, 135, 175, 215, 255];
                (
                    lv[(n / 36) as usize],
                    lv[((n / 6) % 6) as usize],
                    lv[(n % 6) as usize],
                )
            }
            Color::Indexed(i @ 232..=255) => {
                let v = 8 + 10 * (i - 232);
                (v, v, v)
            }
            other => panic!("{other:?} is not a fixed-RGB color"),
        }
    }

    fn luminance(c: Color) -> f64 {
        let f = |v: u8| {
            let v = f64::from(v) / 255.0;
            if v <= 0.04045 {
                v / 12.92
            } else {
                ((v + 0.055) / 1.055).powf(2.4)
            }
        };
        let (r, g, b) = rgb(c);
        0.2126 * f(r) + 0.7152 * f(g) + 0.0722 * f(b)
    }

    fn contrast(a: Color, b: Color) -> f64 {
        let (la, lb) = (luminance(a), luminance(b));
        (la.max(lb) + 0.05) / (la.min(lb) + 0.05)
    }

    const WHITE: Color = Color::Indexed(231);
    const BLACK: Color = Color::Indexed(16);

    #[test]
    fn palette_palette_is_legible_on_light_and_dark() {
        let p = &PALETTE;
        for (name, c) in [
            ("muted", p.muted),
            ("highlight", p.highlight),
            ("info", p.info),
            ("header", p.header),
            ("error", p.error),
            ("detail", p.detail),
            ("active_border", p.active_border),
            ("inactive_border", p.inactive_border),
            ("accent_border", p.accent_border),
        ] {
            let (on_white, on_black) = (contrast(c, WHITE), contrast(c, BLACK));
            assert!(
                on_white >= 3.3,
                "{name} too faint on a light terminal: {on_white:.2}"
            );
            assert!(
                on_black >= 4.4,
                "{name} too dim on a dark terminal: {on_black:.2}"
            );
        }
    }

    #[test]
    fn palette_text_uses_the_terminal_default_foreground() {
        // Hard-coded White/Black text is what made light terminals unreadable.
        assert_eq!(PALETTE.text, Color::Reset);
        assert_eq!(PALETTE.secondary, Color::Reset);
    }

    #[test]
    fn palette_highlights_set_both_colors_with_strong_contrast() {
        let p = &PALETTE;
        for (name, bg) in [
            ("selection", p.selection_bg),
            ("inactive selection", p.inactive_selection_bg),
            ("bars / focused input", p.bar_bg),
            ("toast", p.toast_bg),
        ] {
            let fg = if name == "toast" {
                p.toast_fg
            } else {
                p.on_dark_fg
            };
            assert!(contrast(fg, bg) >= 4.5, "{name}: {:.2}", contrast(fg, bg));
        }
        assert!(contrast(p.toast_error_fg, p.toast_bg) >= 4.5);

        // and the style helpers really use them, never an unset color
        for style in [
            selection_style(),
            inactive_selection_style(),
            footer_style(),
        ] {
            assert!(style.fg.is_some() && style.bg.is_some());
        }
        assert!(input_field_style(true).bg.is_some());
    }

    #[test]
    fn keyboard_cursor_never_matches_text_selection() {
        assert_ne!(selection_style().bg, text_selection_style().bg);
        assert_ne!(selection_style_for(true), selection_style_for(false));
    }

    #[test]
    fn test_text_selection_uses_terminal_colors_via_reverse_video() {
        let style = text_selection_style();
        // No theme colors: default fg/bg, swapped by the terminal itself.
        assert_eq!(style.fg, Some(Color::Reset));
        assert_eq!(style.bg, Some(Color::Reset));
        assert!(style.add_modifier.contains(Modifier::REVERSED));
        assert_eq!(query_selection_style(), text_selection_style());
    }
}
