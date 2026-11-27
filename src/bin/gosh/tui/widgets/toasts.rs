//! Short-lived notifications in the top-right corner.

use ratatui::{
    prelude::*,
    widgets::{Block, BorderType, Borders, Clear, Paragraph},
};

use super::{fit, text_width};
use crate::tui::app::{ToastLevel, TuiApp};

pub fn render(frame: &mut Frame, app: &TuiApp) {
    let theme = app.theme();
    let area = frame.area();
    if app.toasts.is_empty() || area.width < 30 || area.height < 5 {
        return;
    }
    let max_width = 60.min(area.width - 2);

    for (index, toast) in app.toasts.iter().rev().enumerate() {
        let y = area.y + 1 + index as u16 * 3;
        if y + 3 > area.height {
            break;
        }
        let (icon, color) = match toast.level {
            ToastLevel::Info => ("i", theme.accent),
            ToastLevel::Success => ("\u{2713}", theme.success),
            ToastLevel::Warning => ("!", theme.warning),
            ToastLevel::Error => ("\u{2717}", theme.error),
        };
        let message = fit(&toast.message, usize::from(max_width) - 6);
        let width = (text_width(&message) as u16 + 6).min(max_width);
        let rect = Rect::new(area.x + area.width - width - 1, y, width, 3);

        // Fade in the last second of its life.
        let fading = toast.created.elapsed().as_secs_f32() > 3.0;
        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(if fading { theme.surface1 } else { color }))
            .style(Style::default().bg(theme.bg_dim));
        frame.render_widget(Clear, rect);
        frame.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled(format!(" {icon} "), Style::default().fg(color)),
                Span::styled(
                    message,
                    Style::default().fg(if fading { theme.overlay0 } else { theme.text }),
                ),
            ]))
            .block(block),
            rect,
        );
    }
}
