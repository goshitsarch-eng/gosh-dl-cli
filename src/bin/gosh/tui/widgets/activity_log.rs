//! What happened recently, newest at the bottom.

use ratatui::{prelude::*, widgets::Paragraph};

use super::{fit, panel, panel_title};
use crate::tui::app::{ActivityLevel, TuiApp};

pub fn render(frame: &mut Frame, area: Rect, app: &TuiApp) {
    let theme = app.theme();
    let detail = if app.activity_log_scroll > 0 {
        format!("L hides \u{b7} ] newer ({} below)", app.activity_log_scroll)
    } else {
        "L hides \u{b7} [ older".to_string()
    };
    let block = panel(theme, panel_title(theme, "Activity", detail), false);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    if inner.height == 0 || inner.width < 16 {
        return;
    }

    let height = inner.height as usize;
    let total = app.activity_log.len();
    let end = total.saturating_sub(app.activity_log_scroll);
    let start = end.saturating_sub(height);
    let room = (inner.width as usize).saturating_sub(11);

    let lines: Vec<Line> = app
        .activity_log
        .iter()
        .skip(start)
        .take(end - start)
        .map(|entry| {
            let color = match entry.level {
                ActivityLevel::Info => theme.subtext1,
                ActivityLevel::Success => theme.success,
                ActivityLevel::Warning => theme.warning,
                ActivityLevel::Error => theme.error,
            };
            Line::from(vec![
                Span::styled(
                    format!(" {} ", entry.time.format("%H:%M:%S")),
                    Style::default().fg(theme.overlay0),
                ),
                Span::styled(fit(&entry.message, room), Style::default().fg(color)),
            ])
        })
        .collect();
    frame.render_widget(Paragraph::new(lines), inner);
}
