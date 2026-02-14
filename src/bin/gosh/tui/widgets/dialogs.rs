use ratatui::{
    prelude::*,
    widgets::{Block, Clear, Paragraph, Wrap},
};

use super::btop_border::btop_block;
use crate::tui::app::{DialogState, TuiApp};
use crate::util::truncate_str;

pub fn render_help_dialog(frame: &mut Frame, app: &TuiApp) {
    let theme = app.theme();

    let area = centered_rect(60, 70, frame.area());
    frame.render_widget(Clear, area);

    let help_text = "\
    Keyboard Shortcuts\n\
    \u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\n\
    \n\
    Navigation:\n\
      \u{2191}/k      Select previous\n\
      \u{2193}/j      Select next\n\
      J/K      Reorder (move down/up)\n\
      PgUp     Page up\n\
      PgDn     Page down\n\
    \n\
    Actions:\n\
      a        Add new download\n\
      A        Batch import URLs\n\
      p        Pause selected\n\
      r        Resume selected\n\
      c        Cancel selected\n\
      d        Cancel and delete files\n\
      /        Search/filter downloads\n\
      S        Open settings\n\
    \n\
    Views:\n\
      1        All downloads\n\
      2        Active only\n\
      3        Completed only\n\
    \n\
    Other:\n\
      ?        Toggle this help\n\
      q/Ctrl+C Quit\n\
    \n\
    Press any key to close";

    let block = btop_block("Help", theme, true);

    let paragraph = Paragraph::new(help_text)
        .block(block)
        .style(Style::default().fg(theme.text).bg(theme.bg))
        .wrap(Wrap { trim: true });

    frame.render_widget(paragraph, area);
}

pub fn render_dialog(frame: &mut Frame, dialog: &DialogState, app: &TuiApp) {
    let theme = app.theme();

    match dialog {
        DialogState::AddUrl { input, cursor } => {
            let area = centered_rect(65, 20, frame.area());
            frame.render_widget(Clear, area);

            let block = btop_block("Add Download", theme, true)
                .style(Style::default().bg(theme.bg));

            let inner = block.inner(area);
            frame.render_widget(block, area);

            // Prompt text
            let prompt = Line::from(vec![
                Span::styled(
                    "  Enter URL, magnet link, or torrent file path:",
                    Style::default().fg(theme.subtext0),
                ),
            ]);
            let prompt_area = Rect::new(inner.x, inner.y, inner.width, 1);
            frame.render_widget(Paragraph::new(prompt), prompt_area);

            // Input field with distinct background
            let input_y = inner.y + 2;
            let input_area = Rect::new(inner.x + 1, input_y, inner.width - 2, 1);
            let input_block_area = Rect::new(inner.x + 1, input_y, inner.width - 2, 1);
            frame.render_widget(
                Block::default().style(Style::default().bg(theme.surface0)),
                input_block_area,
            );

            let input_text = Paragraph::new(Span::styled(
                format!(" {}", input),
                Style::default().fg(theme.text).bg(theme.surface0),
            ));
            frame.render_widget(input_text, input_area);

            // Show cursor position
            let cursor_x = input_area.x + 1 + *cursor as u16;
            if cursor_x < input_area.x + input_area.width {
                frame.set_cursor_position(Position::new(cursor_x, input_y));
            }

            // Buttons
            let btn_y = inner.y + 4;
            if btn_y < inner.y + inner.height {
                let btn_area = Rect::new(inner.x + 2, btn_y, inner.width - 4, 1);
                let buttons = Line::from(vec![
                    Span::styled(
                        " Enter ",
                        Style::default().fg(theme.bg_deep).bg(theme.accent),
                    ),
                    Span::styled(" Add  ", Style::default().fg(theme.subtext0)),
                    Span::styled(
                        " Esc ",
                        Style::default().fg(theme.bg_deep).bg(theme.surface2),
                    ),
                    Span::styled(" Cancel ", Style::default().fg(theme.subtext0)),
                ]);
                frame.render_widget(Paragraph::new(buttons), btn_area);
            }
        }
        DialogState::ConfirmCancel { id, delete_files } => {
            let area = centered_rect(50, 20, frame.area());
            frame.render_widget(Clear, area);

            let action = if *delete_files {
                "cancel and DELETE FILES for"
            } else {
                "cancel"
            };

            let gid = id.to_gid();
            let content = vec![
                Line::from(""),
                Line::from(vec![
                    Span::styled("  Are you sure you want to ", Style::default().fg(theme.text)),
                    Span::styled(
                        action,
                        if *delete_files {
                            Style::default().fg(theme.error).add_modifier(Modifier::BOLD)
                        } else {
                            Style::default().fg(theme.warning)
                        },
                    ),
                ]),
                Line::from(vec![
                    Span::styled(
                        format!("  download {}?", truncate_str(&gid, 16)),
                        Style::default().fg(theme.text),
                    ),
                ]),
                Line::from(""),
                Line::from(vec![
                    Span::raw("  "),
                    Span::styled(" y ", Style::default().fg(theme.bg_deep).bg(theme.success)),
                    Span::styled(" Yes  ", Style::default().fg(theme.subtext0)),
                    Span::styled(" n ", Style::default().fg(theme.bg_deep).bg(theme.error)),
                    Span::styled(" No ", Style::default().fg(theme.subtext0)),
                ]),
            ];

            let block = btop_block("Confirm", theme, true)
                .style(Style::default().bg(theme.bg));

            let paragraph = Paragraph::new(content).block(block);
            frame.render_widget(paragraph, area);
        }
        DialogState::Error { message } => {
            let area = centered_rect(50, 20, frame.area());
            frame.render_widget(Clear, area);

            let content = vec![
                Line::from(""),
                Line::from(Span::styled(
                    format!("  {}", message),
                    Style::default().fg(theme.text),
                )),
                Line::from(""),
                Line::from(Span::styled(
                    "  Press any key to close",
                    Style::default().fg(theme.overlay0),
                )),
            ];

            let block = btop_block("Error", theme, true)
                .style(Style::default().bg(theme.bg));

            let paragraph = Paragraph::new(content).block(block);
            frame.render_widget(paragraph, area);
        }
        // Settings and BatchImport are rendered by their own modules
        _ => {}
    }
}

/// Apply DIM modifier to all cells (darkens background behind overlays)
pub fn dim_background(frame: &mut Frame) {
    let area = frame.area();
    let buf = frame.buffer_mut();
    for y in area.top()..area.bottom() {
        for x in area.left()..area.right() {
            let cell = &mut buf[(x, y)];
            cell.set_style(cell.style().add_modifier(Modifier::DIM));
        }
    }
}

/// Create a centered rect
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
