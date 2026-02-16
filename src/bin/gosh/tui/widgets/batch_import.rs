use ratatui::{
    prelude::*,
    widgets::{Clear, Paragraph},
};

use super::btop_border::btop_block;
use super::dialogs::centered_rect;
use crate::tui::app::{BatchPhase, DialogState, TuiApp};

pub fn render_batch_import(frame: &mut Frame, dialog: &DialogState, app: &TuiApp) {
    let DialogState::BatchImport { phase } = dialog else {
        return;
    };

    let theme = app.theme();
    let area = centered_rect(65, 70, frame.area());
    frame.render_widget(Clear, area);

    match phase {
        BatchPhase::Input {
            text,
            cursor_line,
            cursor_col,
        } => {
            let block = btop_block("Batch Import - Enter URLs", theme, true)
                .style(Style::default().bg(theme.bg));
            let inner = block.inner(area);
            frame.render_widget(block, area);

            if inner.height < 4 {
                return;
            }

            // Instructions
            let hint = Line::from(vec![
                Span::styled(
                    "  Enter one URL per line. ",
                    Style::default().fg(theme.subtext0),
                ),
                Span::styled(
                    "Ctrl+Enter",
                    Style::default()
                        .fg(theme.accent)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(" to review. ", Style::default().fg(theme.subtext0)),
                Span::styled(
                    "Esc",
                    Style::default()
                        .fg(theme.surface2)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(" to cancel.", Style::default().fg(theme.subtext0)),
            ]);
            frame.render_widget(
                Paragraph::new(hint),
                Rect::new(inner.x, inner.y, inner.width, 1),
            );

            // Text area
            let text_y = inner.y + 2;
            let text_height = inner.height.saturating_sub(3);
            let text_area = Rect::new(inner.x + 1, text_y, inner.width - 2, text_height);

            // Background for text area
            frame.render_widget(
                ratatui::widgets::Block::default().style(Style::default().bg(theme.surface0)),
                text_area,
            );

            // Render text lines
            let lines: Vec<&str> = if text.is_empty() {
                vec![""]
            } else {
                text.lines().collect()
            };
            for (i, line) in lines.iter().enumerate() {
                if i as u16 >= text_height {
                    break;
                }
                let line_y = text_y + i as u16;
                let display = if line.is_empty() && i == *cursor_line {
                    " ".to_string()
                } else {
                    format!(" {}", line)
                };
                frame.render_widget(
                    Paragraph::new(Span::styled(
                        display,
                        Style::default().fg(theme.text).bg(theme.surface0),
                    )),
                    Rect::new(text_area.x, line_y, text_area.width, 1),
                );
            }

            // Cursor
            let cursor_x = text_area.x + 1 + *cursor_col as u16;
            let cursor_y = text_y + *cursor_line as u16;
            if cursor_x < text_area.x + text_area.width && cursor_y < text_y + text_height {
                frame.set_cursor_position(Position::new(cursor_x, cursor_y));
            }

            // Line count
            let line_count = text.lines().count().max(1);
            let counter = Line::from(Span::styled(
                format!(
                    "  {} line{}",
                    line_count,
                    if line_count == 1 { "" } else { "s" }
                ),
                Style::default().fg(theme.overlay0),
            ));
            let counter_y = inner.y + inner.height - 1;
            frame.render_widget(
                Paragraph::new(counter),
                Rect::new(inner.x, counter_y, inner.width, 1),
            );
        }
        BatchPhase::Review { entries, selected } => {
            let block = btop_block("Batch Import - Review", theme, true)
                .style(Style::default().bg(theme.bg));
            let inner = block.inner(area);
            frame.render_widget(block, area);

            if inner.height < 4 {
                return;
            }

            // Header
            let header = Line::from(vec![
                Span::styled("  ", Style::default()),
                Span::styled(
                    "Space",
                    Style::default()
                        .fg(theme.accent)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(": toggle  ", Style::default().fg(theme.subtext0)),
                Span::styled(
                    "Enter",
                    Style::default()
                        .fg(theme.accent)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(": start  ", Style::default().fg(theme.subtext0)),
                Span::styled(
                    "Esc",
                    Style::default()
                        .fg(theme.surface2)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(": back", Style::default().fg(theme.subtext0)),
            ]);
            frame.render_widget(
                Paragraph::new(header),
                Rect::new(inner.x, inner.y, inner.width, 1),
            );

            // Entries
            let list_y = inner.y + 2;
            let list_height = inner.height.saturating_sub(4);

            for (i, entry) in entries.iter().enumerate() {
                if i as u16 >= list_height {
                    break;
                }
                let y = list_y + i as u16;
                let is_sel = i == *selected;

                let checkbox = if entry.selected { "[x]" } else { "[ ]" };
                let checkbox_style = if entry.selected {
                    Style::default().fg(theme.success)
                } else {
                    Style::default().fg(theme.surface2)
                };

                let url_style = if !entry.valid {
                    Style::default().fg(theme.error)
                } else if is_sel {
                    Style::default().fg(theme.text)
                } else {
                    Style::default().fg(theme.subtext0)
                };

                let kind_style = Style::default().fg(theme.accent);

                let bg = if is_sel { theme.surface0 } else { Color::Reset };

                let max_url_width = (inner.width as usize).saturating_sub(20);
                let url_display = if entry.url.len() > max_url_width {
                    format!("{}...", &entry.url[..max_url_width.saturating_sub(3)])
                } else {
                    entry.url.clone()
                };

                let mut spans = vec![
                    Span::styled(format!("  {} ", checkbox), checkbox_style.bg(bg)),
                    Span::styled(format!("[{}] ", entry.kind), kind_style.bg(bg)),
                    Span::styled(url_display, url_style.bg(bg)),
                ];

                if let Some(ref err) = entry.error {
                    spans.push(Span::styled(
                        format!(" ({})", err),
                        Style::default().fg(theme.error).bg(bg),
                    ));
                }

                let line = Line::from(spans);
                frame.render_widget(Paragraph::new(line), Rect::new(inner.x, y, inner.width, 1));
            }

            // Summary
            let selected_count = entries.iter().filter(|e| e.selected && e.valid).count();
            let total = entries.len();
            let summary = Line::from(Span::styled(
                format!("  {}/{} selected", selected_count, total),
                Style::default().fg(theme.overlay0),
            ));
            let summary_y = inner.y + inner.height - 1;
            frame.render_widget(
                Paragraph::new(summary),
                Rect::new(inner.x, summary_y, inner.width, 1),
            );
        }
    }
}
