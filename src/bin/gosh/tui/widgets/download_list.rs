use ratatui::{
    prelude::*,
    widgets::{Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState},
};

use super::btop_border::btop_block;
use super::download_item::{render_download_item, spinner_symbol};
use crate::tui::app::TuiApp;

pub fn render_download_list(frame: &mut Frame, area: Rect, app: &mut TuiApp) {
    let theme = app.theme().clone();

    let block = btop_block("Downloads", &theme, false);

    let inner = block.inner(area);
    frame.render_widget(block, area);

    if app.downloads.is_empty() {
        let empty = vec![
            Line::from(""),
            Line::from(Span::styled(
                "No downloads yet",
                Style::default().fg(theme.overlay0),
            )),
            Line::from(""),
            Line::from(vec![
                Span::styled("Press ", Style::default().fg(theme.overlay0)),
                Span::styled(
                    " a ",
                    Style::default()
                        .fg(theme.accent)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled("to add a download", Style::default().fg(theme.overlay0)),
            ]),
        ];
        let paragraph = Paragraph::new(empty).alignment(Alignment::Center);
        frame.render_widget(paragraph, inner);
        return;
    }

    // Each download takes 2 lines
    let lines_per_item = 2;
    let visible_items = (inner.height as usize) / lines_per_item;
    app.last_visible_height = visible_items;
    app.adjust_scroll(visible_items);

    let end = (app.scroll_offset + visible_items).min(app.downloads.len());

    // Get current spinner symbol for animated states
    let spinner = spinner_symbol(&app.throbber_state);

    // Render each download item as 2-line block
    for (i, dl) in app.downloads[app.scroll_offset..end].iter().enumerate() {
        let global_idx = i + app.scroll_offset;
        let is_selected = global_idx == app.selected;
        let y = inner.y + (i * lines_per_item) as u16;

        if y + 1 >= inner.y + inner.height {
            break;
        }

        let item_area = Rect::new(inner.x, y, inner.width, lines_per_item as u16);
        render_download_item(frame, item_area, dl, is_selected, &theme, spinner);
    }

    // Scrollbar
    if app.downloads.len() > visible_items {
        let mut scrollbar_state = ScrollbarState::new(app.downloads.len()).position(app.selected);
        let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .style(Style::default().fg(theme.surface2));
        frame.render_stateful_widget(scrollbar, area, &mut scrollbar_state);
    }
}
