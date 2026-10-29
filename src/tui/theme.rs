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

/// The single palette lazyTrino draws with. Everything is expressed in the
/// terminal's own colors — the default foreground/background plus the 16 ANSI
/// colors — so the terminal profile decides the actual shades: bright ones in
/// a dark profile, dark ones in a light profile. Nothing is hard-coded RGB or
/// from the 256-color cube.
///
/// Cyan is the pane-border / structure color, so syntax-style highlighting
/// (numbers, quoted identifiers) uses *bright* blue instead — it stays
/// distinct from the borders. Plain ANSI blue is too dark to read as text on
/// most dark profiles, so it is only used as a *background* (cursor row).
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
    /// What kind of thing a name is, so catalogs, schemas and tables read
    /// differently at a glance (lists, borders, counts, breadcrumbs).
    catalog: Color,
    schema: Color,
    table: Color,
    /// Value types in the results grid.
    number: Color,
    boolean: Color,
    temporal: Color,
    /// Foreground / background pairs for bars and the keyboard cursor.
    on_accent_fg: Color,
    selection_bg: Color,
    inactive_selection_fg: Color,
    bar_bg: Color,
    toast_ok: Color,
    toast_error: Color,
}

const PALETTE: Palette = Palette {
    text: Color::Reset,
    secondary: Color::Reset,
    muted: Color::DarkGray,
    highlight: Color::Yellow,
    info: Color::LightBlue,
    header: Color::Green,
    error: Color::Red,
    detail: Color::Magenta,
    active_border: Color::Yellow,
    inactive_border: Color::DarkGray,
    accent_border: Color::Cyan,
    catalog: Color::Magenta,
    schema: Color::Cyan,
    table: Color::Green,
    number: Color::LightBlue,
    boolean: Color::Magenta,
    temporal: Color::Green,
    on_accent_fg: Color::White,
    selection_bg: Color::Blue,
    inactive_selection_fg: Color::Cyan,
    bar_bg: Color::DarkGray,
    toast_ok: Color::Green,
    toast_error: Color::LightRed,
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

/// Border of a catalog / schema / table list pane: the entity's color when
/// focused, muted otherwise.
pub fn entity_border_style(entity: Entity, is_active: bool) -> Style {
    if is_active {
        style(entity.color())
    } else {
        style(pal().inactive_border)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Entity {
    Catalog,
    Schema,
    Table,
}

impl Entity {
    pub fn color(self) -> Color {
        match self {
            Entity::Catalog => pal().catalog,
            Entity::Schema => pal().schema,
            Entity::Table => pal().table,
        }
    }

    pub fn style(self) -> Style {
        style(self.color())
    }
}

/// Scrollbars own their cells: a fixed style that clears whatever was drawn
/// underneath, so a scrollbar never picks up the color or weight of the
/// highlighted row, selected cell or value tint it is drawn over.
pub fn scrollbar_style() -> Style {
    Style::default()
        .fg(Color::Reset)
        .bg(Color::Reset)
        .remove_modifier(Modifier::all())
}

/// A scrollbar in the fixed `scrollbar_style`; callers set orientation and symbols.
pub fn scrollbar() -> ratatui::widgets::Scrollbar<'static> {
    ratatui::widgets::Scrollbar::default().style(scrollbar_style())
}

pub fn border_style(is_active: bool) -> Style {
    style(border_color(is_active))
}

/// Border of the query bar: the active color while editing, dimmed while it
/// is disabled, and otherwise the same as the menu pane's border.
pub fn query_bar_border_color(
    is_editing: bool,
    is_table_view: bool,
    menu_is_active: bool,
) -> Color {
    if is_editing {
        pal().active_border
    } else if is_table_view {
        border_color(menu_is_active)
    } else {
        pal().inactive_border
    }
}

/// Keyboard-cursor highlight (the row/cell you are *on*). Explicit white on
/// a deep background so it reads the same on any terminal background.
pub fn selection_style() -> Style {
    Style::default()
        .fg(pal().on_accent_fg)
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
    bold_style(pal().inactive_selection_fg)
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
    style(pal().text)
}

/// The key part of a footer / menu hint (`Esc` in `Esc:close`).
pub fn key_style() -> Style {
    bold_style(pal().highlight)
}

pub fn catalog_style() -> Style {
    style(pal().catalog)
}

pub fn schema_style() -> Style {
    style(pal().schema)
}

pub fn table_style() -> Style {
    style(pal().table)
}

pub fn catalog_bold_style() -> Style {
    bold_style(pal().catalog)
}

pub fn schema_bold_style() -> Style {
    bold_style(pal().schema)
}

pub fn table_bold_style() -> Style {
    bold_style(pal().table)
}

pub fn sql_keyword_style() -> Style {
    bold_style(pal().detail)
}

pub fn sql_string_style() -> Style {
    bold_style(pal().header)
}

/// Quoted identifiers in SQL (`"my column"`), in the syntax blue.
pub fn sql_ident_style() -> Style {
    bold_style(pal().number)
}

pub fn sql_comment_style() -> Style {
    muted_style().add_modifier(Modifier::ITALIC)
}

pub fn number_style() -> Style {
    style(pal().number)
}

pub fn number_bold_style() -> Style {
    bold_style(pal().number)
}

/// Syntax-highlight-style color for a results-grid value, by what it looks like.
pub fn value_style(value: &str) -> Style {
    let v = value.trim();
    if v.is_empty() || v.eq_ignore_ascii_case("null") || v.eq_ignore_ascii_case("none") {
        muted_style().add_modifier(Modifier::ITALIC)
    } else if v.eq_ignore_ascii_case("true") || v.eq_ignore_ascii_case("false") {
        style(pal().boolean)
    } else if is_temporal(v) {
        style(pal().temporal)
    } else if is_number(v) {
        style(pal().number)
    } else {
        style(pal().text)
    }
}

fn is_number(v: &str) -> bool {
    let digits = v.strip_prefix(['-', '+']).unwrap_or(v);
    !digits.is_empty()
        && digits.chars().any(|c| c.is_ascii_digit())
        && digits
            .chars()
            .all(|c| c.is_ascii_digit() || matches!(c, '.' | ',' | '_' | 'e' | 'E' | '-' | '+'))
        && digits.starts_with(|c: char| c.is_ascii_digit() || c == '.')
}

/// `2026-08-06`, `2026-08-06 12:30:00.123`, `2026-08-06T12:30:00Z`, `12:30:00`.
fn is_temporal(v: &str) -> bool {
    let b = v.as_bytes();
    let date = b.len() >= 10
        && b[4] == b'-'
        && b[7] == b'-'
        && b[..4].iter().all(u8::is_ascii_digit)
        && b[5..7].iter().all(u8::is_ascii_digit)
        && b[8..10].iter().all(u8::is_ascii_digit);
    let time = b.len() >= 8
        && b[2] == b':'
        && b[5] == b':'
        && b[..2].iter().all(u8::is_ascii_digit)
        && b[3..5].iter().all(u8::is_ascii_digit);
    date || time
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

/// Accent (border / title) of a success toast. The toast itself is a cleared
/// card: accent border and title, message in the terminal's own text color.
pub fn toast_style() -> Style {
    bold_style(pal().toast_ok)
}

pub fn error_toast_style() -> Style {
    bold_style(pal().toast_error)
}

/// Message text inside a toast: the terminal's own foreground, so it reads
/// on any profile while the border and title carry the accent color.
pub fn toast_text_style() -> Style {
    text_style()
}

pub fn detail_style() -> Style {
    style(pal().detail)
}

pub fn input_field_style(is_focused: bool) -> Style {
    if is_focused {
        Style::default()
            .fg(pal().on_accent_fg)
            .bg(pal().bar_bg)
            .add_modifier(Modifier::BOLD)
    } else {
        secondary_style()
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_terminal_native_colors_are_used() {
        // Default fg/bg or one of the 16 ANSI colors — never RGB / 256-color
        // values, which terminal profiles cannot remap.
        let p = &PALETTE;
        for c in [
            p.text,
            p.secondary,
            p.muted,
            p.highlight,
            p.info,
            p.header,
            p.error,
            p.detail,
            p.active_border,
            p.inactive_border,
            p.accent_border,
            p.catalog,
            p.schema,
            p.table,
            p.number,
            p.boolean,
            p.temporal,
            p.on_accent_fg,
            p.selection_bg,
            p.inactive_selection_fg,
            p.bar_bg,
            p.toast_ok,
            p.toast_error,
        ] {
            assert!(
                !matches!(c, Color::Rgb(..) | Color::Indexed(_)),
                "{c:?} is not a terminal-native color"
            );
        }
    }

    #[test]
    fn plain_ansi_blue_is_never_used_as_text() {
        // Dark-profile ANSI blue is unreadable on a dark background; the
        // syntax blue is the bright variant.
        assert_eq!(PALETTE.number, Color::LightBlue);
        let p = &PALETTE;
        for c in [
            p.muted,
            p.highlight,
            p.info,
            p.header,
            p.error,
            p.detail,
            p.catalog,
            p.schema,
            p.table,
            p.number,
            p.boolean,
            p.temporal,
        ] {
            assert_ne!(c, Color::Blue);
        }
    }

    #[test]
    fn syntax_colors_never_reuse_the_border_color() {
        // Cyan frames the panes, so syntax highlighting must not use it.
        let p = &PALETTE;
        for syntax in [p.number, p.boolean, p.temporal, p.info] {
            assert_ne!(syntax, p.accent_border);
            assert_ne!(syntax, p.schema);
        }
        assert_eq!(sql_ident_style().fg, Some(p.number));
        for style in [
            sql_keyword_style(),
            sql_string_style(),
            sql_ident_style(),
            number_style(),
            value_style("42"),
        ] {
            assert_ne!(style.fg, Some(p.accent_border));
        }
    }

    #[test]
    fn entities_and_value_types_are_told_apart() {
        assert_ne!(Entity::Catalog.color(), Entity::Schema.color());
        assert_ne!(Entity::Schema.color(), Entity::Table.color());
        assert_ne!(Entity::Catalog.color(), Entity::Table.color());

        let text = value_style("hello");
        let number = value_style("-12,345.6");
        let boolean = value_style("TRUE");
        let date = value_style("2026-08-06");
        let ts = value_style("2026-08-06 12:30:00.123");
        let null = value_style("NULL");
        assert_eq!(text.fg, Some(Color::Reset));
        assert_eq!(number.fg, Some(PALETTE.number));
        assert_eq!(boolean.fg, Some(PALETTE.boolean));
        assert_eq!(date.fg, Some(PALETTE.temporal));
        assert_eq!(ts.fg, Some(PALETTE.temporal));
        assert_eq!(null.fg, Some(PALETTE.muted));
        assert!(null.add_modifier.contains(Modifier::ITALIC));
        let kinds = [text.fg, number.fg, boolean.fg, date.fg, null.fg];
        for (i, a) in kinds.iter().enumerate() {
            for b in &kinds[i + 1..] {
                if a != b {
                    continue;
                }
                // number/boolean/temporal each get their own color
                assert!(
                    [number.fg, boolean.fg, date.fg]
                        .iter()
                        .filter(|c| *c == a)
                        .count()
                        <= 1,
                    "value kinds share a color"
                );
            }
        }
    }

    #[test]
    fn values_that_only_look_numeric_stay_plain() {
        for v in ["abc", "12abc", "-", "e", "1-2-3x", "2026", "0x1F", "v1.2"] {
            let fg = value_style(v).fg;
            assert!(
                fg == Some(Color::Reset) || fg == Some(PALETTE.number) && v == "2026",
                "{v}: {fg:?}"
            );
        }
        assert_eq!(value_style("2026").fg, Some(PALETTE.number));
        assert_eq!(value_style("3.14").fg, Some(PALETTE.number));
        assert_eq!(value_style("1e-9").fg, Some(PALETTE.number));
    }

    #[test]
    fn keyboard_cursor_sets_both_colors_and_never_matches_text_selection() {
        assert!(selection_style().fg.is_some() && selection_style().bg.is_some());
        assert_ne!(selection_style().bg, text_selection_style().bg);
        assert_ne!(selection_style_for(true), selection_style_for(false));
        assert!(input_field_style(true).bg.is_some());
    }

    #[test]
    fn test_text_selection_uses_terminal_colors_via_reverse_video() {
        let style = text_selection_style();
        assert_eq!(style.fg, Some(Color::Reset));
        assert_eq!(style.bg, Some(Color::Reset));
        assert!(style.add_modifier.contains(Modifier::REVERSED));
        assert_eq!(query_selection_style(), text_selection_style());
    }

    #[test]
    fn toasts_accent_the_border_and_keep_the_text_readable() {
        for (style, accent) in [
            (toast_style(), PALETTE.toast_ok),
            (error_toast_style(), PALETTE.toast_error),
        ] {
            assert_eq!(style.fg, Some(accent));
            assert!(style.add_modifier.contains(Modifier::BOLD));
            assert!(!style.add_modifier.contains(Modifier::REVERSED));
        }
        assert_ne!(toast_style().fg, error_toast_style().fg);
        // message text is the terminal default, never an accent
        assert_eq!(toast_text_style().fg, Some(Color::Reset));
    }

    #[test]
    fn the_query_bar_border_follows_the_menu_pane() {
        for menu_is_active in [true, false] {
            assert_eq!(
                query_bar_border_color(false, true, menu_is_active),
                border_color(menu_is_active),
                "an idle query bar in table view matches the menu"
            );
        }
        // editing always lights up, and a disabled bar stays dim
        assert_eq!(
            query_bar_border_color(true, true, false),
            PALETTE.active_border
        );
        assert_eq!(
            query_bar_border_color(false, false, true),
            PALETTE.inactive_border
        );
    }
}
