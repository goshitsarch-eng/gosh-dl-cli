use ratatui::prelude::*;

use crate::tui::theme::Theme;

pub fn render_gradient_bar(buf: &mut Buffer, area: Rect, ratio: f64, theme: &Theme) {
    let filled = ((area.width as f64) * ratio.clamp(0.0, 1.0)) as u16;
    for x in 0..area.width {
        let cell = &mut buf[(area.x + x, area.y)];
        if x < filled {
            let t = if filled > 0 {
                x as f64 / filled as f64
            } else {
                0.0
            };
            let color = theme.progress_gradient(t);
            cell.set_symbol("━");
            cell.set_fg(color);
        } else {
            cell.set_symbol("━");
            cell.set_fg(theme.surface1);
        }
    }
}
