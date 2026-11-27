//! Details of the selected row. Only shows what the engine really reports:
//! there is no per-connection or per-piece telemetry, so nothing here
//! pretends to have it.

use gosh_dl::{DownloadKind, DownloadState, DownloadStatus};
use ratatui::{prelude::*, widgets::Paragraph};

use super::{
    fit, fit_left, gradient_bar, kind_color, pad, panel, panel_title, spinner_symbol, state_color,
    state_icon, text_width,
};
use crate::format::{format_duration, format_size, format_speed};
use crate::tui::app::TuiApp;
use crate::tui::model::{tilde_path, MirrorRow, Row, RowState};

const LABEL: usize = 10;

struct Painter<'a> {
    app: &'a TuiApp,
    area: Rect,
    y: u16,
}

impl<'a> Painter<'a> {
    fn left(&self) -> u16 {
        self.area.y + self.area.height - self.y.min(self.area.y + self.area.height)
    }

    fn full(&self) -> bool {
        self.left() == 0
    }

    fn line(&mut self, frame: &mut Frame, line: Line<'_>) {
        if self.full() {
            return;
        }
        frame.render_widget(
            Paragraph::new(line),
            Rect::new(self.area.x, self.y, self.area.width, 1),
        );
        self.y += 1;
    }

    fn blank(&mut self) {
        if !self.full() {
            self.y += 1;
        }
    }

    fn label(&self, text: &str) -> Span<'static> {
        Span::styled(
            pad(text, LABEL),
            Style::default().fg(self.app.theme().overlay1),
        )
    }

    fn value(&self, text: String) -> Span<'static> {
        Span::styled(text, Style::default().fg(self.app.theme().text))
    }

    /// `Label     value` across the whole width.
    fn field(&mut self, frame: &mut Frame, label: &str, value: &str) {
        let room = (self.area.width as usize).saturating_sub(LABEL);
        let line = Line::from(vec![self.label(label), self.value(fit(value, room))]);
        self.line(frame, line);
    }

    /// Two `Label value` pairs side by side.
    fn pair(&mut self, frame: &mut Frame, left: (&str, Span<'static>), right: (&str, String)) {
        // The left column must hold "999.99 MB / 999.99 MB".
        let width = self.area.width as usize;
        let half = (width * 11 / 20)
            .max(LABEL + 22)
            .min(width.saturating_sub(LABEL + 8));
        let left_room = half.saturating_sub(LABEL + 1);
        let mut left_value = left.1;
        let fitted = fit(&left_value.content, left_room);
        let gap = half.saturating_sub(LABEL + text_width(&fitted));
        left_value.content = fitted.into();
        let right_room = (self.area.width as usize).saturating_sub(half + LABEL);
        let line = Line::from(vec![
            self.label(left.0),
            left_value,
            Span::raw(" ".repeat(gap)),
            self.label(right.0),
            self.value(fit(&right.1, right_room)),
        ]);
        self.line(frame, line);
    }

    /// A dim section heading: `── Swarm ─────────`.
    fn section(&mut self, frame: &mut Frame, title: &str) {
        if self.left() < 2 {
            self.y = self.area.y + self.area.height;
            return;
        }
        let theme = self.app.theme();
        let rule = (self.area.width as usize).saturating_sub(text_width(title) + 4);
        let line = Line::from(vec![
            Span::styled("\u{2500}\u{2500} ", Style::default().fg(theme.surface1)),
            Span::styled(
                title.to_string(),
                Style::default()
                    .fg(theme.subtext0)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!(" {}", "\u{2500}".repeat(rule)),
                Style::default().fg(theme.surface1),
            ),
        ]);
        self.line(frame, line);
    }

    fn note(&mut self, frame: &mut Frame, text: &str, color: Color) {
        let line = Line::from(Span::styled(
            fit(text, self.area.width as usize),
            Style::default().fg(color),
        ));
        self.line(frame, line);
    }
}

pub fn render(frame: &mut Frame, area: Rect, app: &TuiApp) {
    let theme = app.theme();
    let row = app.selected_row();
    let title = match row {
        Some(row) => Line::from(vec![
            Span::raw(" "),
            Span::styled("Details", theme.title_style()),
            Span::raw(" "),
            Span::styled(
                row.tag(),
                Style::default()
                    .fg(kind_color(theme, row.kind()))
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" "),
        ]),
        None => panel_title(theme, "Details", String::new()),
    };
    let block = panel(theme, title, false);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    if inner.height == 0 || inner.width < 20 {
        return;
    }
    // One cell of breathing room on each side.
    let inner = Rect::new(inner.x + 1, inner.y, inner.width - 2, inner.height);

    let Some(row) = row else {
        frame.render_widget(
            Paragraph::new("Select a download to see its details.")
                .style(Style::default().fg(theme.overlay1)),
            inner,
        );
        return;
    };

    let mut p = Painter {
        app,
        area: inner,
        y: inner.y,
    };
    render_summary(frame, &mut p, row);
    match row {
        Row::Download(d) => match d.kind {
            DownloadKind::Http => render_http(frame, &mut p, d),
            DownloadKind::Torrent | DownloadKind::Magnet => render_torrent(frame, &mut p, d),
        },
        Row::Mirror(m) => render_mirror(frame, &mut p, m),
    }
}

/// The grid every kind of row shares, plus the one-line hint for its state.
fn render_summary(frame: &mut Frame, p: &mut Painter<'_>, row: &Row) {
    let app = p.app;
    let theme = app.theme();
    let state = row.state();
    let color = state_color(theme, state);

    p.field(frame, "Name", &row.name());
    p.pair(
        frame,
        (
            "State",
            Span::styled(
                format!(
                    "{} {}",
                    state_icon(state, spinner_symbol(&app.throbber_state)),
                    state.label()
                ),
                Style::default().fg(color),
            ),
        ),
        ("Progress", format!("{:.1}%", row.percent())),
    );
    let size = match row.total_size() {
        Some(total) => format!(
            "{} / {}",
            format_size(row.completed_size()),
            format_size(total)
        ),
        None => format!("{} / ?", format_size(row.completed_size())),
    };
    let eta = match state {
        RowState::Downloading => row
            .eta_seconds()
            .map(format_duration)
            .unwrap_or_else(|| "\u{2014}".to_string()),
        _ => "\u{2014}".to_string(),
    };
    p.pair(frame, ("Size", p.value(size)), ("ETA", eta));
    let speed = if row.upload_speed() > 0 || state == RowState::Seeding {
        format!(
            "\u{2193} {}/s  \u{2191} {}/s",
            format_speed(row.download_speed()),
            format_speed(row.upload_speed())
        )
    } else {
        format!("\u{2193} {}/s", format_speed(row.download_speed()))
    };
    p.pair(
        frame,
        ("Speed", p.value(speed)),
        ("Priority", row.priority()),
    );

    let room = (p.area.width as usize).saturating_sub(LABEL);
    match row {
        Row::Download(d) => {
            let path = d
                .metadata
                .save_dir
                .join(d.metadata.filename.as_deref().unwrap_or(&d.metadata.name));
            let line = Line::from(vec![
                p.label("Path"),
                p.value(fit_left(&tilde_path(&path), room)),
            ]);
            p.line(frame, line);
            p.field(frame, "ID", &d.id.to_gid());
        }
        Row::Mirror(m) => {
            let dir = m
                .save_dir()
                .map(tilde_path)
                .unwrap_or_else(|| tilde_path(&app.config().general.download_dir));
            let line = Line::from(vec![p.label("Into"), p.value(fit_left(&dir, room))]);
            p.line(frame, line);
            p.field(frame, "ID", &m.id.to_string());
        }
    }

    let note: Option<(String, Color)> = match (row, state) {
        (_, RowState::Paused) => Some((
            match row {
                Row::Download(_) => {
                    "\u{2016} Paused. v verifies data on disk, r resumes.".to_string()
                }
                Row::Mirror(_) => "\u{2016} Paused. r resumes every paused file.".to_string(),
            },
            theme.warning,
        )),
        (_, RowState::Queued) => Some((
            format!(
                "\u{25f7} Waiting for a free slot (max_concurrent_downloads = {}).",
                app.config().engine.max_concurrent_downloads
            ),
            theme.overlay1,
        )),
        (Row::Download(d), RowState::Error) => {
            let message = match &d.state {
                DownloadState::Error { message, .. } => message.trim_end_matches('.').to_string(),
                _ => "Failed".to_string(),
            };
            Some((
                format!("\u{2717} {message}. r retries, c cancels."),
                theme.error,
            ))
        }
        (Row::Mirror(_), RowState::Error) => Some((
            "\u{2717} Every file failed. r retries them, c removes the job.".to_string(),
            theme.error,
        )),
        (_, RowState::Partial) => Some((
            "! Finished with failures. r retries the failed files.".to_string(),
            theme.peach,
        )),
        (Row::Download(_), RowState::Completed) => Some((
            "\u{2713} Complete. v verifies the data on disk.".to_string(),
            theme.success,
        )),
        (_, RowState::Seeding) => Some((
            "\u{2191} Seeding. p pauses, c stops and keeps the files.".to_string(),
            theme.teal,
        )),
        (Row::Download(d), RowState::Connecting) if d.kind == DownloadKind::Magnet => Some((
            "Fetching torrent metadata from peers\u{2026}".to_string(),
            theme.sky,
        )),
        _ => None,
    };
    if let Some((text, color)) = note {
        p.blank();
        p.note(frame, &text, color);
    }
}

fn render_http(frame: &mut Frame, p: &mut Painter<'_>, d: &DownloadStatus) {
    let theme = p.app.theme();
    p.blank();
    p.section(frame, "Transfer");
    if let Some(url) = &d.metadata.url {
        p.field(frame, "URL", url);
    }
    let active = matches!(
        d.state,
        DownloadState::Connecting | DownloadState::Downloading
    );
    let connections = if active {
        format!(
            "{} open (limit {})",
            d.progress.connections,
            p.app.config().engine.max_connections_per_download
        )
    } else {
        "none open".to_string()
    };
    p.field(frame, "Streams", &connections);
    match &d.metadata.checksum {
        Some(sum) => {
            let verified = matches!(d.state, DownloadState::Completed);
            let line = Line::from(vec![
                p.label("Checksum"),
                Span::styled(
                    format!(
                        "{} {}",
                        sum.algorithm.to_string().to_lowercase(),
                        fit(&sum.value, 13)
                    ),
                    Style::default().fg(theme.text),
                ),
                Span::styled(
                    if verified {
                        "  \u{2713} matched"
                    } else {
                        "  checked when done"
                    },
                    Style::default().fg(if verified {
                        theme.success
                    } else {
                        theme.overlay1
                    }),
                ),
            ]);
            p.line(frame, line);
        }
        None => p.field(frame, "Checksum", "none given (--checksum)"),
    }
    let mirrors = match d.metadata.mirrors.len() {
        0 => "none (--alt-url adds fallbacks)".to_string(),
        1 => "1 fallback URL".to_string(),
        n => format!("{n} fallback URLs"),
    };
    p.field(frame, "Mirrors", &mirrors);
    let resume = if d.metadata.etag.is_some() {
        "validated by ETag"
    } else if d.metadata.last_modified.is_some() {
        "validated by Last-Modified"
    } else if matches!(d.state, DownloadState::Queued | DownloadState::Connecting) {
        "not known yet"
    } else {
        "no validator sent by the server"
    };
    p.field(frame, "Resume", resume);
    p.field(
        frame,
        "If exists",
        p.app.config().general.file_conflict.as_str(),
    );
}

fn render_torrent(frame: &mut Frame, p: &mut Painter<'_>, d: &DownloadStatus) {
    let app = p.app;
    let theme = app.theme();
    if !app.show_peers() {
        return;
    }
    p.blank();
    p.section(frame, "Swarm");

    let private = d.torrent_info.as_ref().is_some_and(|info| info.private);
    let engine = &app.config().engine;
    let flag = |on: bool, name: &'static str| {
        vec![
            Span::styled(
                if on { "\u{25cf} " } else { "\u{25cb} " },
                Style::default().fg(if on { theme.success } else { theme.overlay0 }),
            ),
            Span::styled(
                format!("{name}  "),
                Style::default().fg(if on { theme.text } else { theme.overlay0 }),
            ),
        ]
    };
    let mut flags = vec![p.label("Discovery")];
    flags.extend(flag(engine.enable_dht && !private, "DHT"));
    flags.extend(flag(engine.enable_pex && !private, "PEX"));
    flags.extend(flag(engine.enable_lpd && !private, "LPD"));
    if private {
        flags.push(Span::styled(
            "private torrent",
            Style::default().fg(theme.warning),
        ));
    }
    p.line(frame, Line::from(flags));

    p.field(
        frame,
        "Peers",
        &format!(
            "{} connected \u{b7} {} seeding (limit {})",
            d.progress.peers, d.progress.seeders, engine.max_peers
        ),
    );
    if let Some(hash) = &d.metadata.info_hash {
        p.field(frame, "Info hash", hash);
    }

    let Some(info) = &d.torrent_info else {
        return;
    };
    if info.pieces_count > 0 {
        let have = (row_ratio(d) * info.pieces_count as f64).floor() as usize;
        p.field(
            frame,
            "Pieces",
            &format!(
                "\u{2248} {have} of {} \u{b7} {} each",
                info.pieces_count,
                format_size(info.piece_length)
            ),
        );
    }

    if info.files.len() > 1 && p.left() >= 3 {
        p.blank();
        let selected = info.files.iter().filter(|f| f.selected).count();
        p.section(
            frame,
            &format!("Files \u{b7} {selected} of {} selected", info.files.len()),
        );
        let room = p.left() as usize;
        for (index, file) in info.files.iter().enumerate() {
            if index + 1 == room && info.files.len() > room {
                p.note(
                    frame,
                    &format!("\u{2026} and {} more", info.files.len() - index),
                    theme.overlay1,
                );
                break;
            }
            let pct = if file.size > 0 {
                file.completed as f64 / file.size as f64 * 100.0
            } else {
                100.0
            };
            let tail = if file.selected {
                format!("{:>5.1}%  {:>10}", pct, format_size(file.size))
            } else {
                format!("{:>6}  {:>10}", "skip", format_size(file.size))
            };
            let name_room = (p.area.width as usize).saturating_sub(text_width(&tail) + 1);
            let line = Line::from(vec![
                Span::styled(
                    pad(&file.path.display().to_string(), name_room),
                    Style::default().fg(if file.selected {
                        theme.subtext1
                    } else {
                        theme.overlay0
                    }),
                ),
                Span::raw(" "),
                Span::styled(tail, Style::default().fg(theme.overlay1)),
            ]);
            p.line(frame, line);
        }
    }
}

fn row_ratio(d: &DownloadStatus) -> f64 {
    if matches!(d.state, DownloadState::Completed | DownloadState::Seeding) {
        1.0
    } else {
        (d.progress.percentage() / 100.0).clamp(0.0, 1.0)
    }
}

fn render_mirror(frame: &mut Frame, p: &mut Painter<'_>, m: &MirrorRow) {
    let app = p.app;
    let theme = app.theme();
    let progress = &m.status.progress;
    p.blank();
    p.section(frame, "Mirror");
    p.field(frame, "Root", &m.root_url);

    let count = |n: usize, label: &'static str, color: Color| {
        vec![
            Span::styled(
                n.to_string(),
                Style::default()
                    .fg(if n > 0 { color } else { theme.overlay0 })
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(format!(" {label}   "), Style::default().fg(theme.overlay1)),
        ]
    };
    let mut counts = vec![p.label("Files")];
    counts.extend(count(progress.completed_children, "done", theme.success));
    counts.extend(count(progress.active_children, "active", theme.pink));
    counts.extend(count(progress.queued_children, "queued", theme.subtext0));
    if progress.paused_children > 0 {
        counts.extend(count(progress.paused_children, "paused", theme.warning));
    }
    counts.extend(count(
        progress.failed_children + progress.missing_children,
        "failed",
        theme.error,
    ));
    p.line(frame, Line::from(counts));

    if p.left() < 3 || m.children.is_empty() {
        return;
    }
    p.blank();
    p.section(frame, "Files");

    // What is happening now is more interesting than what is finished:
    // active first, then failed, paused, queued, and completed last.
    let rank = |state: &DownloadState| match RowState::from_download(state) {
        RowState::Downloading | RowState::Connecting => 0,
        RowState::Error => 1,
        RowState::Paused => 2,
        RowState::Queued => 3,
        _ => 4,
    };
    let mut files: Vec<&DownloadStatus> = m.children.iter().collect();
    files.sort_by_key(|c| rank(&c.state));

    let spinner = spinner_symbol(&app.throbber_state);
    let room = p.left() as usize;
    for (index, child) in files.iter().enumerate() {
        if index + 1 == room && files.len() > room {
            p.note(
                frame,
                &format!("\u{2026} and {} more", files.len() - index),
                theme.overlay1,
            );
            break;
        }
        let state = RowState::from_download(&child.state);
        let tail = match state {
            RowState::Downloading => format!(
                "{:>5.1}%  {:>10}/s",
                row_ratio(child) * 100.0,
                format_speed(child.progress.download_speed)
            ),
            RowState::Completed => format!(
                "{:>20}",
                format_size(
                    child
                        .progress
                        .total_size
                        .unwrap_or(child.progress.completed_size)
                )
            ),
            other => format!("{:>20}", other.label().to_lowercase()),
        };
        let name_room = (p.area.width as usize).saturating_sub(text_width(&tail) + 3);
        let line = Line::from(vec![
            Span::styled(
                format!("{} ", state_icon(state, spinner)),
                Style::default().fg(state_color(theme, state)),
            ),
            Span::styled(
                pad(&child.metadata.name, name_room),
                Style::default().fg(theme.subtext1),
            ),
            Span::raw(" "),
            Span::styled(tail, Style::default().fg(theme.overlay1)),
        ]);
        p.line(frame, line);
    }
}

/// The short details box of the compact layout: name, bar, two fact lines.
pub fn render_compact(frame: &mut Frame, area: Rect, app: &TuiApp) {
    let theme = app.theme();
    let row = app.selected_row();
    let title = match row {
        Some(row) => Line::from(vec![
            Span::raw(" "),
            Span::styled(
                row.tag(),
                Style::default()
                    .fg(kind_color(theme, row.kind()))
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" "),
            Span::styled(
                row.state().label(),
                Style::default().fg(state_color(theme, row.state())),
            ),
            Span::raw(" "),
        ]),
        None => panel_title(theme, "Details", String::new()),
    };
    let block = panel(theme, title, false);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    if inner.height == 0 || inner.width < 20 {
        return;
    }
    let inner = Rect::new(inner.x + 1, inner.y, inner.width - 2, inner.height);
    let Some(row) = row else {
        frame.render_widget(
            Paragraph::new("Press a to add a URL, magnet link or torrent.")
                .style(Style::default().fg(theme.overlay1)),
            inner,
        );
        return;
    };

    let mut p = Painter {
        app,
        area: inner,
        y: inner.y,
    };
    p.note(frame, &row.name(), theme.text);
    if !p.full() {
        let pct = format!(" {:>5.1}%", row.percent());
        let bar = (inner.width as usize).saturating_sub(text_width(&pct));
        gradient_bar(
            frame.buffer_mut(),
            Rect::new(inner.x, p.y, bar as u16, 1),
            row.percent() / 100.0,
            theme,
        );
        frame.render_widget(
            Paragraph::new(pct).style(Style::default().fg(theme.subtext0)),
            Rect::new(inner.x + bar as u16, p.y, inner.width - bar as u16, 1),
        );
        p.y += 1;
    }
    let size = match row.total_size() {
        Some(total) => format!(
            "{} / {}",
            format_size(row.completed_size()),
            format_size(total)
        ),
        None => format_size(row.completed_size()),
    };
    p.pair(
        frame,
        ("Size", p.value(size)),
        (
            "Speed",
            format!("\u{2193} {}/s", format_speed(row.download_speed())),
        ),
    );
    let (label, text) = match row {
        Row::Download(d) => match &d.state {
            DownloadState::Error { message, .. } => ("Error", message.clone()),
            _ => (
                "Path",
                tilde_path(
                    &d.metadata
                        .save_dir
                        .join(d.metadata.filename.as_deref().unwrap_or(&d.metadata.name)),
                ),
            ),
        },
        Row::Mirror(m) => (
            "Files",
            format!(
                "{} done \u{b7} {} active \u{b7} {} queued \u{b7} {} failed",
                m.status.progress.completed_children,
                m.status.progress.active_children,
                m.status.progress.queued_children,
                m.status.progress.failed_children + m.status.progress.missing_children
            ),
        ),
    };
    let room = (inner.width as usize).saturating_sub(LABEL);
    let value = if label == "Path" {
        fit_left(&text, room)
    } else {
        fit(&text, room)
    };
    let color = if label == "Error" {
        theme.error
    } else {
        theme.text
    };
    let line = Line::from(vec![
        p.label(label),
        Span::styled(value, Style::default().fg(color)),
    ]);
    p.line(frame, line);
    let eta = row
        .eta_seconds()
        .filter(|_| row.state() == RowState::Downloading)
        .map(format_duration)
        .unwrap_or_else(|| "\u{2014}".to_string());
    p.pair(frame, ("ETA", p.value(eta)), ("Priority", row.priority()));
}
