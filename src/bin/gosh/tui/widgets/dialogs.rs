//! Small popups: add, confirmations, verify and errors.

use ratatui::{
    prelude::*,
    widgets::{Block, BorderType, Borders, Clear, Paragraph, Wrap},
};

use super::{centered, fit, fit_left, key_line, pad, popup, spinner_symbol, text_width};
use crate::tui::app::{detect_input, AddKind, DialogState, TuiApp, VerifyOutcome};
use crate::tui::model::{tilde_path, RowId};

/// The `index`-th line of `area`, if it exists.
pub fn line_at(area: Rect, index: u16) -> Option<Rect> {
    (index < area.height).then(|| Rect::new(area.x, area.y + index, area.width, 1))
}

/// Draw a popup frame and return its padded content area.
pub fn open_popup(frame: &mut Frame, area: Rect, app: &TuiApp, title: &str) -> Rect {
    frame.render_widget(Clear, area);
    let block = popup(app.theme(), title);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    Rect::new(
        inner.x + 1.min(inner.width),
        inner.y,
        inner.width.saturating_sub(2),
        inner.height,
    )
}

fn put(frame: &mut Frame, area: Rect, index: u16, line: Line<'_>) {
    if let Some(rect) = line_at(area, index) {
        frame.render_widget(Paragraph::new(line), rect);
    }
}

/// Draw the dialogs this module owns; settings and batch import have their
/// own modules.
pub fn render(frame: &mut Frame, dialog: &DialogState, app: &TuiApp) {
    match dialog {
        DialogState::AddUrl {
            input,
            cursor,
            single_file,
        } => render_add(frame, app, input, *cursor, *single_file),
        DialogState::ConfirmCancel {
            target,
            name,
            delete_files,
        } => {
            let (what, consequence) = match (target, delete_files) {
                (RowId::Download(_), true) => (
                    "Cancel this download and delete its files?",
                    "Everything it wrote to disk is removed.",
                ),
                (RowId::Download(_), false) => {
                    ("Cancel this download?", "Data already on disk is kept.")
                }
                (RowId::Mirror(_), true) => (
                    "Remove this mirror and delete its files?",
                    "Every file it downloaded is removed from disk.",
                ),
                (RowId::Mirror(_), false) => (
                    "Remove this mirror?",
                    "All of its downloads are cancelled; files on disk are kept.",
                ),
            };
            render_confirm(
                frame,
                app,
                if *delete_files { "Delete" } else { "Cancel" },
                &[what, name, "", consequence],
                *delete_files,
            );
        }
        DialogState::ConfirmCancelAll => render_confirm(
            frame,
            app,
            "Cancel all",
            &[
                "Cancel every download and mirror?",
                "",
                "Data already on disk is kept.",
            ],
            true,
        ),
        DialogState::ConfirmRepair { name, .. } => render_confirm(
            frame,
            app,
            "Repair",
            &[
                "Verify this download and repair it if needed?",
                name,
                "",
                "Damaged data is downloaded again.",
            ],
            false,
        ),
        DialogState::Verify {
            name,
            tag,
            repair,
            outcome,
            ..
        } => render_verify(frame, app, name, tag, *repair, outcome.as_ref()),
        DialogState::Error { message } => render_error(frame, app, message),
        DialogState::Settings { .. } | DialogState::BatchImport { .. } => {}
    }
}

fn render_add(frame: &mut Frame, app: &TuiApp, input: &str, cursor: usize, single_file: bool) {
    let theme = app.theme();
    let area = centered(frame.area(), 78, 14);
    let inner = open_popup(frame, area, app, "Add download");
    if inner.width < 10 {
        return;
    }
    let muted = Style::default().fg(theme.overlay1);

    put(
        frame,
        inner,
        0,
        Line::styled(
            "Enter URL, magnet link, or torrent file path:",
            Style::default().fg(theme.subtext1),
        ),
    );

    // Input box, scrolled so the cursor stays in view.
    if inner.height >= 4 {
        let box_area = Rect::new(inner.x, inner.y + 1, inner.width, 3);
        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(theme.surface2))
            .style(Style::default().bg(theme.bg));
        let field = block.inner(box_area);
        frame.render_widget(block, box_area);
        let room = (field.width as usize).saturating_sub(2);
        let before: String = input.chars().take(cursor).collect();
        let mut start = 0;
        while text_width(&before.chars().skip(start).collect::<String>()) > room {
            start += 1;
        }
        let visible: String = input.chars().skip(start).collect();
        let cursor_x = text_width(&before.chars().skip(start).collect::<String>());
        frame.render_widget(
            Paragraph::new(Line::from(vec![
                Span::raw(" "),
                Span::styled(fit(&visible, room + 1), Style::default().fg(theme.text)),
            ])),
            field,
        );
        frame.set_cursor_position((field.x + 1 + cursor_x as u16, field.y));
    }

    let (kind, description) = detect_input(input, single_file);
    let (marker, color) = match kind {
        AddKind::Empty => ("\u{25b8}", theme.overlay1),
        AddKind::Invalid => ("\u{2717}", theme.error),
        AddKind::Directory => ("\u{25b8}", theme.peach),
        AddKind::Magnet | AddKind::Torrent => ("\u{25b8}", theme.mauve),
        AddKind::Http | AddKind::Metalink => ("\u{25b8}", theme.accent),
    };
    put(
        frame,
        inner,
        4,
        Line::styled(
            fit(&format!("{marker} {description}"), inner.width as usize),
            Style::default().fg(color),
        ),
    );

    // What the download will be started with.
    let config = app.config();
    let half = inner.width as usize / 2;
    let cell = |label: &str, value: String, width: usize| {
        vec![
            Span::styled(pad(label, 13), muted),
            Span::styled(
                pad(&value, width.saturating_sub(13)),
                Style::default().fg(theme.text),
            ),
        ]
    };
    let mut first = cell(
        "Save to",
        fit_left(
            &tilde_path(&config.general.download_dir),
            half.saturating_sub(14),
        ),
        half,
    );
    first.extend(cell(
        "Connections",
        match kind {
            AddKind::Magnet | AddKind::Torrent => {
                format!("up to {} peers", config.engine.max_peers)
            }
            _ => format!(
                "up to {} per file",
                config.engine.max_connections_per_download
            ),
        },
        half,
    ));
    put(frame, inner, 6, Line::from(first));
    let mut second = cell(
        "If exists",
        config.general.file_conflict.as_str().to_string(),
        half,
    );
    second.extend(cell(
        "Slots",
        format!("{} at a time", config.engine.max_concurrent_downloads),
        half,
    ));
    put(frame, inner, 7, Line::from(second));

    let is_directory_url = kind == AddKind::Directory || (kind == AddKind::Http && single_file);
    let mut keys: Vec<(&str, &str)> = vec![("Enter", "Add"), ("F3", "Save paused")];
    if is_directory_url {
        keys.push((
            "Tab",
            if single_file {
                "Mirror instead"
            } else {
                "Single file"
            },
        ));
    }
    keys.push(("Esc", "Cancel"));
    put(
        frame,
        inner,
        inner.height.saturating_sub(1),
        key_line(theme, &keys, inner.width as usize),
    );
}

fn render_confirm(frame: &mut Frame, app: &TuiApp, title: &str, lines: &[&str], danger: bool) {
    let theme = app.theme();
    let height = lines.len() as u16 + 4;
    let area = centered(frame.area(), 64, height);
    let inner = open_popup(frame, area, app, title);
    for (index, text) in lines.iter().enumerate() {
        let style = if index == 0 {
            Style::default().fg(theme.text).add_modifier(Modifier::BOLD)
        } else if index == 1 && lines.len() > 3 {
            Style::default().fg(theme.accent)
        } else if danger {
            Style::default().fg(theme.warning)
        } else {
            Style::default().fg(theme.subtext0)
        };
        put(
            frame,
            inner,
            index as u16,
            Line::styled(fit(text, inner.width as usize), style),
        );
    }
    put(
        frame,
        inner,
        inner.height.saturating_sub(1),
        key_line(
            theme,
            &[("y", "Yes"), ("n", "No"), ("Esc", "Cancel")],
            inner.width as usize,
        ),
    );
}

fn render_verify(
    frame: &mut Frame,
    app: &TuiApp,
    name: &str,
    tag: &str,
    repair: bool,
    outcome: Option<&VerifyOutcome>,
) {
    let theme = app.theme();
    let area = centered(frame.area(), 70, 12);
    let inner = open_popup(frame, area, app, if repair { "Repair" } else { "Verify" });
    let width = inner.width as usize;
    let torrent = !tag.starts_with("HTTP");

    put(
        frame,
        inner,
        0,
        Line::from(vec![
            Span::styled(
                format!("{tag} "),
                Style::default()
                    .fg(if torrent { theme.mauve } else { theme.accent })
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                fit(name, width.saturating_sub(text_width(tag) + 1)),
                Style::default().fg(theme.text),
            ),
        ]),
    );

    let (status, color, detail) = match outcome {
        None => (
            format!(
                "{} Checking the data on disk\u{2026}",
                spinner_symbol(&app.throbber_state)
            ),
            theme.sky,
            "This can take a while for large downloads. Esc keeps it running.".to_string(),
        ),
        Some(VerifyOutcome::Report(report)) if report.valid => (
            "\u{2713} The data on disk is intact".to_string(),
            theme.success,
            report.detail.clone(),
        ),
        Some(VerifyOutcome::Report(report)) if repair => (
            "\u{21bb} Repair started: damaged data is being downloaded again".to_string(),
            theme.warning,
            report.detail.clone(),
        ),
        Some(VerifyOutcome::Report(report)) => (
            "\u{2717} The data on disk is NOT valid".to_string(),
            theme.error,
            report.detail.clone(),
        ),
        Some(VerifyOutcome::Failed(error)) => (
            "\u{2717} The check could not be completed".to_string(),
            theme.error,
            error.clone(),
        ),
    };
    put(
        frame,
        inner,
        2,
        Line::styled(
            fit(&status, width),
            Style::default().fg(color).add_modifier(Modifier::BOLD),
        ),
    );
    if inner.height > 5 {
        frame.render_widget(
            Paragraph::new(detail)
                .style(Style::default().fg(theme.subtext0))
                .wrap(Wrap { trim: true }),
            Rect::new(inner.x, inner.y + 3, inner.width, 2.min(inner.height - 5)),
        );
    }
    if let Some(VerifyOutcome::Report(report)) = outcome {
        if report.bad_pieces > 0 {
            put(
                frame,
                inner,
                5,
                Line::styled(
                    format!(
                        "{} piece{} failed the hash check",
                        report.bad_pieces,
                        if report.bad_pieces == 1 { "" } else { "s" }
                    ),
                    Style::default().fg(theme.error),
                ),
            );
        }
    }
    put(
        frame,
        inner,
        7,
        Line::styled(
            fit(
                if torrent {
                    "Torrents: every piece is re-hashed against the torrent's piece hashes."
                } else {
                    "HTTP: the file is checked against its size and its checksum, if one was given."
                },
                width,
            ),
            Style::default().fg(theme.overlay1),
        ),
    );

    let can_repair =
        !repair && matches!(outcome, Some(VerifyOutcome::Report(report)) if !report.valid);
    let keys: &[(&str, &str)] = if can_repair {
        &[("V", "Repair"), ("Esc", "Close")]
    } else if outcome.is_none() {
        &[("Esc", "Run in background")]
    } else {
        &[("Esc", "Close")]
    };
    put(
        frame,
        inner,
        inner.height.saturating_sub(1),
        key_line(theme, keys, width),
    );
}

fn render_error(frame: &mut Frame, app: &TuiApp, message: &str) {
    let theme = app.theme();
    let width = 68.min(frame.area().width);
    let text_width = width.saturating_sub(4).max(1) as usize;
    // Rough wrap estimate so the box is as tall as the message needs.
    let lines: usize = message
        .lines()
        .map(|line| super::text_width(line).div_ceil(text_width).max(1))
        .sum();
    let area = centered(frame.area(), width, (lines as u16).clamp(1, 12) + 4);
    frame.render_widget(Clear, area);
    let block = popup(theme, "Error").border_style(Style::default().fg(theme.error));
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let inner = Rect::new(
        inner.x + 1.min(inner.width),
        inner.y,
        inner.width.saturating_sub(2),
        inner.height,
    );
    if inner.height > 2 {
        frame.render_widget(
            Paragraph::new(message.to_string())
                .style(Style::default().fg(theme.text))
                .wrap(Wrap { trim: false }),
            Rect::new(inner.x, inner.y, inner.width, inner.height - 2),
        );
    }
    put(
        frame,
        inner,
        inner.height.saturating_sub(1),
        key_line(theme, &[("Enter", "Close")], inner.width as usize),
    );
}
