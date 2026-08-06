use ratatui::{Frame, layout::Rect};

use crate::app::{App, TableState};

use super::catalog::render_selectable_list;

pub fn render(
    frame: &mut Frame,
    area: Rect,
    state: &TableState,
    search: &str,
    is_active: bool,
    app: &App,
) {
    let title = format!(" Tables — {}.{} ", state.catalog, state.schema);
    render_selectable_list(
        frame,
        area,
        &title,
        &state.items,
        state.selected,
        search,
        is_active,
        app,
    );
}
