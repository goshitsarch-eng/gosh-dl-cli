//! Batch import: paste a list, review it, import what is selected.

use ratatui::{prelude::*, widgets::Paragraph};

use super::dialogs::{line_at, open_popup};
use super::{centered, fit, key_line, pad, text_width};
use crate::tui::app::{AddKind, BatchPhase, DialogState, TuiApp};

pub fn render(frame: &mut Frame, dialog: &DialogState, app: &TuiApp) {
    let DialogState::BatchImport { phase } = dialog else {
        return;
    };
    let theme = app.theme();
    let area = centered(frame.area(), 84, 20);
    let inner = open_popup(frame, area, app, "Batch import");
    if inner.width < 20 || inner.height < 4 {
        return;
    }
    let muted = Style::default().fg(theme.overlay1);
    let body = Rect::new(
        inner.x,
        inner.y + 2,
        inner.width,
        inner.height.saturating_sub(4),
    );
    let footer = inner.height - 1;

    match phase {
        BatchPhase::Input { text } => {
            frame.render_widget(
                Paragraph::new(Line::styled(
                    "One URL, magnet link or file per line. Lines starting with # are skipped.",
                    Style::default().fg(theme.subtext1),
                )),
                Rect::new(inner.x, inner.y, inner.width, 1),
            );
            // Show the end of the text: typing always appends.
            let lines: Vec<&str> = text.split('\n').collect();
            let visible = body.height as usize;
            let start = lines.len().saturating_sub(visible);
            for (slot, line) in lines.iter().skip(start).enumerate() {
                let Some(rect) = line_at(body, slot as u16) else {
                    break;
                };
                let number = format!("{:>3} ", start + slot + 1);
                let room = (body.width as usize).saturating_sub(text_width(&number) + 1);
                frame.render_widget(
                    Paragraph::new(Line::from(vec![
                        Span::styled(number.clone(), Style::default().fg(theme.overlay0)),
                        Span::styled(super::fit_left(line, room), Style::default().fg(theme.text)),
                    ])),
                    rect,
                );
                if slot + start + 1 == lines.len() {
                    let x = text_width(&number) + text_width(line).min(room);
                    frame.set_cursor_position((rect.x + x as u16, rect.y));
                }
            }
            let count = text
                .lines()
                .map(str::trim)
                .filter(|l| !l.is_empty() && !l.starts_with('#'))
                .count();
            if let Some(rect) = line_at(inner, footer) {
                let keys = key_line(
                    theme,
                    &[("F2", "Review"), ("Enter", "New line"), ("Esc", "Cancel")],
                    (inner.width as usize).saturating_sub(12),
                );
                frame.render_widget(Paragraph::new(keys), rect);
                frame.render_widget(
                    Paragraph::new(
                        Line::styled(
                            format!("{count} item{}", if count == 1 { "" } else { "s" }),
                            muted,
                        )
                        .right_aligned(),
                    ),
                    rect,
                );
            }
        }
        BatchPhase::Review { entries, selected } => {
            let chosen = entries.iter().filter(|e| e.selected).count();
            frame.render_widget(
                Paragraph::new(Line::from(vec![
                    Span::styled(
                        format!("{chosen}/{} selected", entries.len()),
                        Style::default().fg(theme.text).add_modifier(Modifier::BOLD),
                    ),
                    Span::styled("  \u{b7}  Space toggles a line", muted),
                ])),
                Rect::new(inner.x, inner.y, inner.width, 1),
            );
            let visible = body.height as usize;
            let offset = (selected + 1).saturating_sub(visible);
            for (slot, (index, entry)) in entries
                .iter()
                .enumerate()
                .skip(offset)
                .take(visible)
                .enumerate()
            {
                let Some(rect) = line_at(body, slot as u16) else {
                    break;
                };
                let is_selected = index == *selected;
                let kind_color = match entry.kind {
                    AddKind::Invalid | AddKind::Empty => theme.error,
                    AddKind::Magnet | AddKind::Torrent => theme.mauve,
                    AddKind::Directory => theme.peach,
                    AddKind::Http | AddKind::Metalink => theme.accent,
                };
                let error = entry
                    .error
                    .as_ref()
                    .map(|e| format!("  ({e})"))
                    .unwrap_or_default();
                let room = (body.width as usize).saturating_sub(18);
                let url_room = if error.is_empty() {
                    room
                } else {
                    room.saturating_sub(text_width(&error).min(room / 2))
                };
                let line = Line::from(vec![
                    Span::styled(
                        if is_selected { "\u{25b6} " } else { "  " },
                        Style::default().fg(theme.accent),
                    ),
                    Span::styled(
                        if entry.selected { "[x] " } else { "[ ] " },
                        Style::default().fg(if entry.selected {
                            theme.success
                        } else {
                            theme.overlay0
                        }),
                    ),
                    Span::styled(
                        pad(&format!("[{}]", entry.kind.tag()), 11),
                        Style::default().fg(kind_color),
                    ),
                    Span::styled(
                        fit(&entry.url, url_room),
                        Style::default().fg(if entry.valid {
                            theme.text
                        } else {
                            theme.overlay1
                        }),
                    ),
                    Span::styled(
                        fit(
                            &error,
                            room.saturating_sub(url_room.min(text_width(&entry.url))),
                        ),
                        Style::default().fg(theme.error),
                    ),
                ]);
                frame.render_widget(
                    Paragraph::new(line).style(if is_selected {
                        Style::default().bg(theme.surface0)
                    } else {
                        Style::default()
                    }),
                    rect,
                );
            }
            if let Some(rect) = line_at(inner, footer) {
                frame.render_widget(
                    Paragraph::new(key_line(
                        theme,
                        &[
                            ("Enter", "Import selected"),
                            ("Space", "Toggle"),
                            ("Esc", "Back"),
                        ],
                        inner.width as usize,
                    )),
                    rect,
                );
            }
        }
    }
}
