//! The one-line bars: status line on top, key hints at the bottom.

use ratatui::{prelude::*, widgets::Paragraph};

use super::{fit_left, key_line, text_width};
use crate::format::format_speed;
use crate::tui::app::{LayoutMode, TuiApp};
use crate::tui::model::{active_schedule_rule, tilde_path};

/// What the schedule is doing right now, e.g.
/// `rule 2 · 09–17 mon-fri · ↓ 2M ↑ 512K`.
pub fn schedule_summary(app: &TuiApp) -> Option<String> {
    let (index, rule) = active_schedule_rule(app.config())?;
    let limit = |value: &Option<String>| value.clone().unwrap_or_else(|| "\u{221e}".to_string());
    Some(format!(
        "rule {} \u{b7} {:02}\u{2013}{:02} {} \u{b7} \u{2193} {} \u{2191} {}",
        index + 1,
        rule.start_hour,
        rule.end_hour,
        rule.days,
        limit(&rule.download_limit),
        limit(&rule.upload_limit),
    ))
}

/// Top line: version, speeds, counts, mirrors, schedule, storage and folder.
pub fn render_status_line(frame: &mut Frame, area: Rect, app: &TuiApp) {
    let theme = app.theme();
    let muted = Style::default().fg(theme.overlay1);
    let sep = || Span::styled(" \u{2502} ", Style::default().fg(theme.surface2));
    let compact = app.layout_mode != LayoutMode::Wide;

    let mut left = vec![
        Span::styled(
            " gosh",
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(concat!(" v", env!("CARGO_PKG_VERSION")), muted),
        sep(),
        Span::styled(
            format!("\u{2193} {}/s", format_speed(app.download_speed)),
            Style::default().fg(theme.teal),
        ),
        Span::raw(" "),
        Span::styled(
            format!("\u{2191} {}/s", format_speed(app.upload_speed)),
            Style::default().fg(theme.peach),
        ),
        sep(),
        Span::styled(
            format!(
                "{} download{}",
                app.counts[0],
                if app.counts[0] == 1 { "" } else { "s" }
            ),
            Style::default().fg(theme.text),
        ),
    ];

    if !compact {
        let running = app.counts[3];
        let (done, total) = app.mirror_files;
        let crawling = app.pending_mirrors();
        if running > 0 || crawling > 0 {
            left.push(sep());
            let mut text = format!(
                "\u{27f3} {running} mirror{}",
                if running == 1 { "" } else { "s" }
            );
            if total > 0 {
                text.push_str(&format!(" ({done}/{total} files)"));
            }
            if crawling > 0 {
                text.push_str(&format!(" \u{b7} {crawling} discovering"));
            }
            left.push(Span::styled(text, Style::default().fg(theme.peach)));
        }
        left.push(sep());
        match schedule_summary(app) {
            Some(summary) => left.push(Span::styled(
                format!("\u{23f1} {summary}"),
                Style::default().fg(theme.warning),
            )),
            None if app.config().schedule.rules.is_empty() => {
                left.push(Span::styled("\u{23f1} no schedule", muted));
            }
            None => left.push(Span::styled("\u{23f1} no schedule rule active", muted)),
        }
        if app.integrity_running() {
            left.push(sep());
            left.push(Span::styled(
                "verifying\u{2026}",
                Style::default().fg(theme.sky),
            ));
        }
    }

    let left = Line::from(left);
    let used = left.width();
    frame.render_widget(
        Paragraph::new(left).style(Style::default().bg(theme.bg_dim)),
        area,
    );

    // Right side: where things are stored, if there is room for it.
    let room = (area.width as usize).saturating_sub(used + 3);
    if room >= 12 {
        let storage = format!("{} \u{b7} ", app.storage_label());
        let dir = tilde_path(&app.config().general.download_dir);
        let dir = fit_left(&dir, room.saturating_sub(text_width(&storage)));
        let right = Line::from(vec![
            Span::styled(storage, muted),
            Span::styled(dir, Style::default().fg(theme.subtext0)),
            Span::raw(" "),
        ])
        .right_aligned();
        frame.render_widget(Paragraph::new(right), area);
    }
}

/// `(key, label, rank)`: shown in this order; when the line is too narrow
/// the highest ranks are dropped first, so help and quit always survive.
const WIDE_KEYS: &[(&str, &str, u8)] = &[
    ("a", "add", 0),
    ("A", "batch", 6),
    ("p", "pause", 2),
    ("r", "resume", 3),
    ("v", "verify", 5),
    ("V", "repair", 8),
    ("S", "settings", 4),
    ("L", "log", 9),
    ("1-4", "views", 7),
    ("?", "help", 1),
    ("q", "quit", 1),
];

const COMPACT_KEYS: &[(&str, &str, u8)] = &[
    ("a", "add", 0),
    ("A", "batch", 6),
    ("p/r", "pause/resume", 2),
    ("v", "verify", 5),
    ("S", "set", 4),
    ("?", "help", 1),
    ("q", "quit", 1),
];

/// The keys that fit in `width`, most important first, in display order.
fn keys_that_fit(
    keys: &[(&'static str, &'static str, u8)],
    width: usize,
) -> Vec<(&'static str, &'static str)> {
    let cost = |key: &str, label: &str| text_width(key) + text_width(label) + 5;
    let mut order: Vec<usize> = (0..keys.len()).collect();
    order.sort_by_key(|&i| keys[i].2);
    let mut keep = vec![false; keys.len()];
    let mut used = 0;
    for i in order {
        let needed = cost(keys[i].0, keys[i].1);
        if used + needed <= width + 2 {
            keep[i] = true;
            used += needed;
        }
    }
    keys.iter()
        .zip(keep)
        .filter(|(_, keep)| *keep)
        .map(|((key, label, _), _)| (*key, *label))
        .collect()
}

/// Bottom line: the keys that matter most, as many as fit.
pub fn render_key_bar(frame: &mut Frame, area: Rect, app: &TuiApp) {
    let theme = app.theme();
    let keys = if app.layout_mode == LayoutMode::Wide {
        WIDE_KEYS
    } else {
        COMPACT_KEYS
    };
    let width = (area.width as usize).saturating_sub(1);
    let mut line = key_line(theme, &keys_that_fit(keys, width), width);
    line.spans.insert(0, Span::raw(" "));
    frame.render_widget(
        Paragraph::new(line).style(Style::default().bg(theme.bg_dim)),
        area,
    );
}
