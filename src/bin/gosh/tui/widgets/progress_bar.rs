// Custom progress bar widget

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Style},
    widgets::Widget,
};

pub struct ProgressBar {
    ratio: f64,
    filled_style: Style,
    empty_style: Style,
}

impl ProgressBar {
    pub fn new(ratio: f64) -> Self {
        Self {
            ratio: ratio.clamp(0.0, 1.0),
            filled_style: Style::default().bg(Color::Cyan),
            empty_style: Style::default().bg(Color::DarkGray),
        }
    }

    pub fn filled_style(mut self, style: Style) -> Self {
        self.filled_style = style;
        self
    }

    pub fn empty_style(mut self, style: Style) -> Self {
        self.empty_style = style;
        self
    }
}

impl Widget for ProgressBar {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.width == 0 {
            return;
        }

        let filled_width = (self.ratio * area.width as f64).round() as u16;

        for x in 0..area.width {
            let style = if x < filled_width {
                self.filled_style
            } else {
                self.empty_style
            };
            buf.get_mut(area.x + x, area.y).set_style(style);
        }
    }
}

/// Create a text-based progress bar string
pub fn text_progress_bar(ratio: f64, width: usize) -> String {
    let ratio = ratio.clamp(0.0, 1.0);
    let filled = (ratio * width as f64).round() as usize;
    let empty = width.saturating_sub(filled);

    format!("[{}{}]", "█".repeat(filled), "░".repeat(empty))
}
