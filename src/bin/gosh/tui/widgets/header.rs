use ratatui::{
    prelude::*,
    text::Line,
    widgets::Tabs,
};

use super::btop_border::btop_block;
use crate::format::format_speed;
use crate::tui::app::{TuiApp, ViewMode};

pub fn render_header(frame: &mut Frame, area: Rect, app: &TuiApp) {
    let theme = app.theme();

    let speed_str = format!(
        " \u{2193} {}  \u{2191} {}  \u{2502}  {} downloads ",
        format_speed(app.download_speed),
        format_speed(app.upload_speed),
        app.downloads.len()
    );

    let block = btop_block(&format!("gosh v{}", env!("CARGO_PKG_VERSION")), theme, false)
        .title_bottom(Line::from(speed_str).right_aligned().style(Style::default().fg(theme.teal)));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    // Tabs widget for view modes
    let tab_titles = vec!["All", "Active", "Completed"];
    let selected_tab = match app.mode {
        ViewMode::All => 0,
        ViewMode::Active => 1,
        ViewMode::Completed => 2,
    };

    let tabs = Tabs::new(tab_titles)
        .select(selected_tab)
        .highlight_style(Style::default().fg(theme.accent).add_modifier(Modifier::BOLD))
        .style(Style::default().fg(theme.overlay1))
        .divider("\u{2502}")
        .padding(" ", " ");

    frame.render_widget(tabs, inner);
}
