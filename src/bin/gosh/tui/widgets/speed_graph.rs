// Speed graph widget (sparkline-style)

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Style},
    widgets::Widget,
};
use std::collections::VecDeque;

const SPARKLINE_CHARS: [char; 8] = ['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];

#[allow(dead_code)]
pub struct SpeedGraph<'a> {
    data: &'a VecDeque<(u64, u64)>,
    style: Style,
    show_upload: bool,
}

#[allow(dead_code)]
impl<'a> SpeedGraph<'a> {
    pub fn new(data: &'a VecDeque<(u64, u64)>) -> Self {
        Self {
            data,
            style: Style::default().fg(Color::Cyan),
            show_upload: false,
        }
    }

    pub fn style(mut self, style: Style) -> Self {
        self.style = style;
        self
    }

    pub fn show_upload(mut self, show: bool) -> Self {
        self.show_upload = show;
        self
    }
}

impl<'a> Widget for SpeedGraph<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.width == 0 || self.data.is_empty() {
            return;
        }

        // Get the values we want to display (download or upload)
        let values: Vec<u64> = self
            .data
            .iter()
            .map(|(d, u)| if self.show_upload { *u } else { *d })
            .collect();

        let max_value = values.iter().copied().max().unwrap_or(1).max(1);

        // Take last N values that fit in the width
        let display_values: Vec<u64> = values
            .iter()
            .rev()
            .take(area.width as usize)
            .rev()
            .copied()
            .collect();

        // Render sparkline
        for (i, &value) in display_values.iter().enumerate() {
            let x = area.x + (area.width as usize - display_values.len() + i) as u16;
            if x < area.x + area.width {
                let ratio = value as f64 / max_value as f64;
                let char_idx = ((ratio * 7.0).round() as usize).min(7);
                let ch = SPARKLINE_CHARS[char_idx];
                buf[(x, area.y)].set_char(ch).set_style(self.style);
            }
        }
    }
}

/// Format a sparkline as a string
pub fn sparkline_string(data: &[u64], width: usize) -> String {
    if data.is_empty() {
        return " ".repeat(width);
    }

    let max_value = data.iter().copied().max().unwrap_or(1).max(1);

    let display_data: Vec<u64> = data.iter().rev().take(width).rev().copied().collect();

    let mut result = String::with_capacity(width);

    // Pad with spaces if needed
    for _ in 0..(width.saturating_sub(display_data.len())) {
        result.push(' ');
    }

    for value in display_data {
        let ratio = value as f64 / max_value as f64;
        let char_idx = ((ratio * 7.0).round() as usize).min(7);
        result.push(SPARKLINE_CHARS[char_idx]);
    }

    result
}
