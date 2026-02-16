use ratatui::{
    prelude::*,
    text::Line,
    widgets::{Block, BorderType, Borders, Clear, Paragraph},
};

use crate::tui::app::{ToastLevel, TuiApp};
use crate::util::truncate_str;

pub fn render_toasts(frame: &mut Frame, app: &TuiApp) {
    let theme = app.theme();
    let area = frame.area();

    if app.toasts.is_empty() || area.width < 30 {
        return;
    }

    let toast_width = 44_u16.min(area.width - 2);

    for (i, toast) in app.toasts.iter().rev().enumerate() {
        let y = area.y + 1 + (i as u16 * 3);
        if y + 2 >= area.height {
            break;
        }

        let toast_area = Rect::new(area.width - toast_width - 1, y, toast_width, 3);

        // Fade based on age (dim after 3 seconds)
        let age = toast.created.elapsed().as_secs_f32();
        let fading = age > 3.0;

        let (icon, border_color) = match toast.level {
            ToastLevel::Success => ("\u{2713} ", theme.success),
            ToastLevel::Error => ("\u{2717} ", theme.error),
        };

        let fg = if fading { theme.overlay0 } else { theme.text };
        let border_fg = if fading { theme.surface1 } else { border_color };

        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(border_fg))
            .style(Style::default().bg(theme.bg_dim));

        let content = Line::from(vec![
            Span::styled(icon, Style::default().fg(border_color)),
            Span::styled(
                truncate_str(&toast.message, (toast_width - 6) as usize),
                Style::default().fg(fg),
            ),
        ]);

        frame.render_widget(Clear, toast_area);
        frame.render_widget(Paragraph::new(content).block(block), toast_area);
    }
}
