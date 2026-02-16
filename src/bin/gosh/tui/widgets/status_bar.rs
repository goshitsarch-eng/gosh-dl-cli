use ratatui::{prelude::*, text::Line, widgets::Paragraph};

use crate::tui::app::TuiApp;

pub fn render_status_bar(frame: &mut Frame, area: Rect, app: &TuiApp) {
    let theme = app.theme();

    let keys = vec![
        ("a", "add"),
        ("p", "pause"),
        ("r", "resume"),
        ("/", "search"),
        ("S", "settings"),
        ("A", "batch"),
        ("J/K", "reorder"),
        ("1-3", "views"),
        ("?", "help"),
        ("q", "quit"),
    ];

    let mut spans = Vec::new();
    for (i, (key, desc)) in keys.iter().enumerate() {
        if i > 0 {
            spans.push(Span::styled("  ", Style::default().bg(theme.bg_deep)));
        }
        spans.push(Span::styled(
            format!(" {} ", key),
            Style::default().fg(theme.bg_deep).bg(theme.accent),
        ));
        spans.push(Span::styled(
            format!(" {} ", desc),
            Style::default().fg(theme.subtext0).bg(theme.bg_deep),
        ));
    }

    let status = Paragraph::new(Line::from(spans)).style(Style::default().bg(theme.bg_deep));
    frame.render_widget(status, area);
}
