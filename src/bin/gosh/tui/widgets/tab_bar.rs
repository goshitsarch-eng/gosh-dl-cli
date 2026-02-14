use ratatui::prelude::*;
use ratatui::widgets::Tabs;

use super::btop_border::btop_block;
use crate::tui::app::{TuiApp, ViewMode};

pub fn render_tab_bar(frame: &mut Frame, area: Rect, app: &TuiApp) {
    let theme = app.theme();
    let all_count = app.downloads.len();
    let tab_titles = vec![
        format!("All ({})", all_count),
        "Active".to_string(),
        "Completed".to_string(),
    ];
    let selected_tab = match app.mode {
        ViewMode::All => 0,
        ViewMode::Active => 1,
        ViewMode::Completed => 2,
    };
    let block = btop_block("Views", theme, false);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let tabs = Tabs::new(tab_titles)
        .select(selected_tab)
        .highlight_style(Style::default().fg(theme.accent).add_modifier(Modifier::BOLD))
        .style(Style::default().fg(theme.overlay1))
        .divider("\u{2502}")
        .padding(" ", " ");
    frame.render_widget(tabs, inner);
}
