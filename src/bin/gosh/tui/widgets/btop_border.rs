use ratatui::prelude::*;
use ratatui::widgets::{Block, BorderType, Borders};

use crate::tui::theme::Theme;

pub fn btop_block<'a>(title: &str, theme: &Theme, focused: bool) -> Block<'a> {
    let border_color = if focused {
        theme.accent
    } else {
        theme.surface1
    };
    Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(border_color))
        .title(Line::from(vec![
            Span::styled("─ ", Style::default().fg(border_color)),
            Span::styled(
                title.to_string(),
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" "),
        ]))
}
