use ratatui::{
    prelude::*,
    text::{Line, Span},
    widgets::Paragraph,
};

use super::btop_border::btop_block;
use crate::tui::app::{ActivityLevel, TuiApp};

pub fn render_activity_log(frame: &mut Frame, area: Rect, app: &TuiApp) {
    let theme = app.theme();
    let block = btop_block("Activity Log", theme, app.show_activity_log);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if app.activity_log.is_empty() {
        let msg = Paragraph::new("No activity yet")
            .style(theme.muted_style())
            .alignment(Alignment::Center);
        frame.render_widget(msg, inner);
        return;
    }

    let lines: Vec<Line> = app
        .activity_log
        .iter()
        .rev()
        .skip(app.activity_log_scroll)
        .take(inner.height as usize)
        .map(|entry| {
            let elapsed = entry.timestamp.elapsed().as_secs();
            let time_str = if elapsed < 60 {
                format!("{:>3}s", elapsed)
            } else if elapsed < 3600 {
                format!("{:>2}m", elapsed / 60)
            } else {
                format!("{:>2}h", elapsed / 3600)
            };

            let (icon, color) = match entry.level {
                ActivityLevel::Info => ("\u{2139}", theme.info),
                ActivityLevel::Success => ("\u{2713}", theme.success),
                ActivityLevel::Warning => ("\u{26a0}", theme.warning),
                ActivityLevel::Error => ("\u{2717}", theme.error),
            };

            Line::from(vec![
                Span::styled(format!(" {} ", time_str), Style::default().fg(theme.overlay1)),
                Span::styled(format!("{} ", icon), Style::default().fg(color)),
                Span::styled(entry.message.clone(), Style::default().fg(theme.text)),
            ])
        })
        .collect();

    frame.render_widget(Paragraph::new(lines), inner);
}
