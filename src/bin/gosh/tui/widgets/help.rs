//! The `?` overlay: every key, in columns, plus what the CLI can do.

use ratatui::prelude::*;
use ratatui::widgets::Paragraph;

use super::dialogs::open_popup;
use super::{centered, fit, pad};
use crate::tui::app::TuiApp;

type Section = (&'static str, &'static [(&'static str, &'static str)]);

const SECTIONS: [Section; 4] = [
    (
        "Navigate",
        &[
            ("j / \u{2193}", "next"),
            ("k / \u{2191}", "previous"),
            ("PgUp PgDn", "page"),
            ("Home End", "first / last"),
            ("1-4  Tab", "switch view"),
            ("/", "filter the list"),
            ("J / K", "move row"),
        ],
    ),
    (
        "Downloads",
        &[
            ("a", "add download"),
            ("A", "batch import"),
            ("p", "pause"),
            ("r", "resume / retry"),
            ("c", "cancel"),
            ("d", "cancel + delete"),
            ("P / R", "pause/resume all"),
            ("C", "cancel all"),
        ],
    ),
    (
        "Data",
        &[
            ("v", "verify data"),
            ("V", "verify and repair"),
            ("F3", "add: save paused"),
            ("Tab", "add: mirror/file"),
            ("F2", "batch: review"),
        ],
    ),
    (
        "App",
        &[
            ("S", "settings, schedule"),
            ("L", "toggle activity"),
            ("[ / ]", "scroll activity"),
            ("?  F1", "this help"),
            ("q", "quit"),
            ("Ctrl+C", "quit anywhere"),
        ],
    ),
];

const OUTSIDE: [(&str, &str); 6] = [
    ("gosh URL", "download and exit, with a progress bar"),
    (
        "gosh add URL --alt-url MIRROR",
        "queue with a fallback mirror",
    ),
    ("gosh mirror https://host/dir/", "mirror a directory tree"),
    (
        "gosh --on-conflict rename URL",
        "never overwrite an existing file",
    ),
    (
        "gosh verify ID / gosh repair ID",
        "check or fix a finished download",
    ),
    ("gosh list --output json", "script-friendly output"),
];

const KEY_WIDTH: usize = 10;

pub fn render(frame: &mut Frame, app: &TuiApp) {
    let theme = app.theme();
    let screen = frame.area();

    // As many columns as fit; sections wrap into further rows of columns.
    let width = screen.width.min(122);
    let columns = (usize::from(width.saturating_sub(4)) / 29).clamp(1, 4);
    let section_rows: Vec<usize> = SECTIONS
        .chunks(columns)
        .map(|chunk| chunk.iter().map(|s| s.1.len()).max().unwrap_or(0) + 2)
        .collect();
    let keys_height: usize = section_rows.iter().sum();
    let wanted = keys_height + OUTSIDE.len() + 5;
    let area = centered(screen, width, wanted.min(usize::from(u16::MAX)) as u16);
    let inner = open_popup(frame, area, app, "Help");
    if inner.width < 20 || inner.height == 0 {
        return;
    }

    let column_width = inner.width as usize / columns;
    let bottom = inner.y + inner.height;
    let mut y = inner.y;
    for (chunk, rows) in SECTIONS.chunks(columns).zip(&section_rows) {
        for (index, (title, keys)) in chunk.iter().enumerate() {
            let x = inner.x + (index * column_width) as u16;
            let mut line_y = y;
            let mut put = |frame: &mut Frame, line: Line<'_>| {
                if line_y < bottom.saturating_sub(1) {
                    frame.render_widget(
                        Paragraph::new(line),
                        Rect::new(x, line_y, column_width as u16, 1),
                    );
                }
                line_y += 1;
            };
            put(
                frame,
                Line::styled(
                    *title,
                    Style::default()
                        .fg(theme.accent)
                        .add_modifier(Modifier::BOLD),
                ),
            );
            for (key, what) in keys.iter() {
                put(
                    frame,
                    Line::from(vec![
                        Span::styled(pad(key, KEY_WIDTH), Style::default().fg(theme.peach)),
                        Span::styled(
                            fit(what, column_width.saturating_sub(KEY_WIDTH + 1)),
                            Style::default().fg(theme.subtext1),
                        ),
                    ]),
                );
            }
        }
        y += *rows as u16;
    }

    // What the same binary does without the TUI.
    if y + 2 < bottom {
        frame.render_widget(
            Paragraph::new(Line::styled(
                "Outside the TUI",
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD),
            )),
            Rect::new(inner.x, y, inner.width, 1),
        );
        y += 1;
        let command_width = (inner.width as usize / 2).min(34);
        for (command, what) in OUTSIDE {
            if y >= bottom.saturating_sub(2) {
                break;
            }
            frame.render_widget(
                Paragraph::new(Line::from(vec![
                    Span::styled(pad(command, command_width), Style::default().fg(theme.teal)),
                    Span::styled(
                        fit(what, (inner.width as usize).saturating_sub(command_width)),
                        Style::default().fg(theme.subtext0),
                    ),
                ])),
                Rect::new(inner.x, y, inner.width, 1),
            );
            y += 1;
        }
    }

    frame.render_widget(
        Paragraph::new(Line::styled(
            "Press any key to close",
            Style::default().fg(theme.overlay1),
        ))
        .alignment(Alignment::Center),
        Rect::new(inner.x, bottom.saturating_sub(1), inner.width, 1),
    );
}
