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
    widgets::{Block, BorderType, Borders, Clear},
};

use crate::tui::theme;

#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModalPreset {
    Small,
    Medium,
    Large,
}

pub fn centered_rect(width: u16, height: u16, area: Rect) -> Rect {
    let w = width.min(area.width);
    let h = height.min(area.height);
    Rect {
        x: area.x + area.width.saturating_sub(w) / 2,
        y: area.y + area.height.saturating_sub(h) / 2,
        width: w,
        height: h,
    }
}

/// Small preset: compact dialog for confirmations (e.g. y/n delete), alerts, and quick prompts.
pub fn modal_small(area: Rect) -> Rect {
    let width = 50.min(area.width.saturating_sub(4)).max(24).min(area.width);
    let height = 8.min(area.height.saturating_sub(2)).max(6).min(area.height);
    centered_rect(width, height, area)
}

/// Medium preset: balanced dialog for list pickers, column visibility, and value filters.
pub fn modal_medium(area: Rect) -> Rect {
    let width = (area.width * 60 / 100).clamp(45, 80).min(area.width);
    let height = (area.height * 65 / 100).clamp(12, 24).min(area.height);
    centered_rect(width, height, area)
}

/// Large preset: expansive dialog for full browsers, help & shortcut documentation, and saved views.
pub fn modal_large(area: Rect) -> Rect {
    let width = (area.width * 80 / 100).clamp(55, 100).min(area.width);
    let height = (area.height * 80 / 100).clamp(16, 32).min(area.height);
    centered_rect(width, height, area)
}

#[allow(dead_code)]
pub fn modal_area(area: Rect, preset: ModalPreset) -> Rect {
    match preset {
        ModalPreset::Small => modal_small(area),
        ModalPreset::Medium => modal_medium(area),
        ModalPreset::Large => modal_large(area),
    }
}

/// Renders the modal background (`Clear`) and outer rounded border block, returning the inner content area.
pub fn render_modal_frame(frame: &mut Frame, area: Rect, title: &str, is_active: bool) -> Rect {
    frame.render_widget(Clear, area);
    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(theme::border_style(is_active));
    let inner = block.inner(area);
    frame.render_widget(block, area);
    inner
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_modal_preset_sizing() {
        let area = Rect::new(0, 0, 100, 40);

        let small = modal_small(area);
        assert_eq!(small.width, 50);
        assert_eq!(small.height, 8);
        assert_eq!(small.x, 25);
        assert_eq!(small.y, 16);

        let med = modal_medium(area);
        assert_eq!(med.width, 60);
        assert_eq!(med.height, 24); // clamped to 24 max
        assert_eq!(med.x, 20);
        assert_eq!(med.y, 8);

        let large = modal_large(area);
        assert_eq!(large.width, 80);
        assert_eq!(large.height, 32); // clamped to 32 max
        assert_eq!(large.x, 10);
        assert_eq!(large.y, 4);

        assert_eq!(modal_area(area, ModalPreset::Small), small);
        assert_eq!(modal_area(area, ModalPreset::Medium), med);
        assert_eq!(modal_area(area, ModalPreset::Large), large);
    }

    #[test]
    fn test_modal_constrained_terminal() {
        let tiny_area = Rect::new(0, 0, 30, 10);
        let small = modal_small(tiny_area);
        assert!(small.width <= 30);
        assert!(small.height <= 10);

        let med = modal_medium(tiny_area);
        assert!(med.width <= 30);
        assert!(med.height <= 10);

        let large = modal_large(tiny_area);
        assert!(large.width <= 30);
        assert!(large.height <= 10);
    }
}
