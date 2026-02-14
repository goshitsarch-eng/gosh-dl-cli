use ratatui::prelude::*;
use ratatui::widgets::Paragraph;

use crate::format::format_speed;
use crate::tui::app::TuiApp;

pub fn render_logo(frame: &mut Frame, area: Rect, app: &TuiApp) {
    let theme = app.theme();
    let lines = vec![
        Line::from(Span::styled(
            format!(" gosh v{}", env!("CARGO_PKG_VERSION")),
            Style::default().fg(theme.accent).add_modifier(Modifier::BOLD),
        )),
        Line::from(vec![
            Span::styled(" \u{2193} ", Style::default().fg(theme.teal)),
            Span::styled(
                format!("{}/s", format_speed(app.download_speed)),
                Style::default().fg(theme.text),
            ),
            Span::styled("  \u{2191} ", Style::default().fg(theme.peach)),
            Span::styled(
                format!("{}/s", format_speed(app.upload_speed)),
                Style::default().fg(theme.text),
            ),
        ]),
        Line::from(Span::styled(
            format!(" {} downloads", app.downloads.len()),
            Style::default().fg(theme.subtext0),
        )),
    ];
    frame.render_widget(Paragraph::new(lines), area);
}
