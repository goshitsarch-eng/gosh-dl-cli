// Help dialog widget

use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::Style,
    widgets::{Block, Borders, Clear, Paragraph, Widget, Wrap},
};

pub struct HelpDialog<'a> {
    title: &'a str,
    content: &'a str,
    title_style: Style,
    border_style: Style,
    content_style: Style,
}

impl<'a> HelpDialog<'a> {
    pub fn new(content: &'a str) -> Self {
        Self {
            title: " Help ",
            content,
            title_style: Style::default(),
            border_style: Style::default(),
            content_style: Style::default(),
        }
    }

    pub fn title(mut self, title: &'a str) -> Self {
        self.title = title;
        self
    }

    pub fn title_style(mut self, style: Style) -> Self {
        self.title_style = style;
        self
    }

    pub fn border_style(mut self, style: Style) -> Self {
        self.border_style = style;
        self
    }

    pub fn content_style(mut self, style: Style) -> Self {
        self.content_style = style;
        self
    }
}

impl<'a> Widget for HelpDialog<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // Clear the area
        Clear.render(area, buf);

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(self.border_style)
            .title(self.title)
            .title_style(self.title_style);

        let paragraph = Paragraph::new(self.content)
            .block(block)
            .style(self.content_style)
            .wrap(Wrap { trim: true });

        paragraph.render(area, buf);
    }
}

/// Calculate a centered rectangle for dialogs
pub fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}
