//! Drawing code. Everything here only reads `TuiApp`; state changes live in
//! `tui::app`.

pub mod activity_log;
pub mod batch_import;
pub mod details;
pub mod dialogs;
pub mod download_list;
pub mod help;
pub mod net_graph;
pub mod settings;
pub mod status;
pub mod toasts;

use ratatui::{
    prelude::*,
    widgets::{Block, BorderType, Borders},
};

use super::model::{RowKind, RowState};
use super::theme::Theme;

/// Display width of a string in terminal cells.
pub fn text_width(text: &str) -> usize {
    Span::raw(text).width()
}

/// Cut `text` to at most `max` cells, ending in `…` when shortened.
pub fn fit(text: &str, max: usize) -> String {
    if text_width(text) <= max {
        return text.to_string();
    }
    if max == 0 {
        return String::new();
    }
    let mut out = String::new();
    let mut used = 0;
    for ch in text.chars() {
        let w = text_width(ch.encode_utf8(&mut [0; 4]));
        if used + w > max - 1 {
            break;
        }
        out.push(ch);
        used += w;
    }
    out.push('\u{2026}');
    out
}

/// Like [`fit`], but keeps the *end* of the text (for paths and URLs).
pub fn fit_left(text: &str, max: usize) -> String {
    if text_width(text) <= max {
        return text.to_string();
    }
    if max == 0 {
        return String::new();
    }
    let mut tail: Vec<char> = Vec::new();
    let mut used = 0;
    for ch in text.chars().rev() {
        let w = text_width(ch.encode_utf8(&mut [0; 4]));
        if used + w > max - 1 {
            break;
        }
        tail.push(ch);
        used += w;
    }
    std::iter::once('\u{2026}')
        .chain(tail.into_iter().rev())
        .collect()
}

/// `text` padded with spaces (or cut) to exactly `width` cells.
pub fn pad(text: &str, width: usize) -> String {
    let fitted = fit(text, width);
    let gap = width.saturating_sub(text_width(&fitted));
    format!("{fitted}{}", " ".repeat(gap))
}

/// `text` right-aligned in exactly `width` cells.
pub fn pad_left(text: &str, width: usize) -> String {
    let fitted = fit(text, width);
    let gap = width.saturating_sub(text_width(&fitted));
    format!("{}{fitted}", " ".repeat(gap))
}

/// A rounded panel with a title; `focused` panels get the accent border.
pub fn panel<'a>(theme: &Theme, title: Line<'a>, focused: bool) -> Block<'a> {
    Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(if focused {
            theme.border_focused_style()
        } else {
            theme.border_style()
        })
        .title(title)
        .style(Style::default().bg(theme.bg))
}

/// Panel title: ` Name detail ` with the detail in a quieter colour.
pub fn panel_title<'a>(theme: &Theme, name: &'a str, detail: String) -> Line<'a> {
    let mut spans = vec![
        Span::raw(" "),
        Span::styled(name, theme.title_style()),
        Span::raw(" "),
    ];
    if !detail.is_empty() {
        spans.push(Span::styled(detail, Style::default().fg(theme.overlay1)));
        spans.push(Span::raw(" "));
    }
    Line::from(spans)
}

/// A popup frame: rounded, accent border, darker background.
pub fn popup<'a>(theme: &Theme, title: &'a str) -> Block<'a> {
    Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(theme.border_focused_style())
        .title(Line::from(vec![
            Span::raw(" "),
            Span::styled(title, theme.title_style()),
            Span::raw(" "),
        ]))
        .style(Style::default().bg(theme.bg_dim).fg(theme.text))
}

/// A rectangle of at most `width` x `height`, centred in `area`.
pub fn centered(area: Rect, width: u16, height: u16) -> Rect {
    let width = width.min(area.width);
    let height = height.min(area.height);
    Rect::new(
        area.x + (area.width - width) / 2,
        area.y + (area.height - height) / 2,
        width,
        height,
    )
}

/// Dim everything drawn so far (behind a dialog).
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

/// `[key] label` for footers and dialog buttons.
pub fn key_badge<'a>(theme: &Theme, key: &'a str, label: &'a str) -> Vec<Span<'a>> {
    vec![
        Span::styled(
            format!(" {key} "),
            Style::default()
                .fg(theme.accent)
                .bg(theme.surface0)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(format!(" {label}"), Style::default().fg(theme.subtext0)),
    ]
}

/// As many `[key] label` badges as fit in `width`, separated by two spaces.
pub fn key_line<'a>(theme: &Theme, keys: &[(&'a str, &'a str)], width: usize) -> Line<'a> {
    let mut spans = Vec::new();
    let mut used = 0;
    for (key, label) in keys {
        let needed = text_width(key) + text_width(label) + 3 + if used > 0 { 2 } else { 0 };
        if used + needed > width {
            break;
        }
        if used > 0 {
            spans.push(Span::raw("  "));
        }
        spans.extend(key_badge(theme, key, label));
        used += needed;
    }
    Line::from(spans)
}

pub fn state_color(theme: &Theme, state: RowState) -> Color {
    match state {
        RowState::Downloading => theme.pink,
        RowState::Seeding => theme.teal,
        RowState::Paused => theme.warning,
        RowState::Queued | RowState::Empty => theme.overlay1,
        RowState::Connecting => theme.sky,
        RowState::Completed => theme.success,
        RowState::Error => theme.error,
        RowState::Partial => theme.peach,
    }
}

pub fn kind_color(theme: &Theme, kind: RowKind) -> Color {
    match kind {
        RowKind::Http => theme.accent,
        RowKind::Torrent | RowKind::Magnet => theme.mauve,
        RowKind::Mirror => theme.peach,
    }
}

/// Current frame of the activity spinner.
pub fn spinner_symbol(state: &throbber_widgets_tui::ThrobberState) -> &'static str {
    let set = &throbber_widgets_tui::BRAILLE_SIX;
    let len = set.symbols.len() as i8;
    set.symbols[state.index().rem_euclid(len) as usize]
}

/// The icon in front of a row: a spinner while it is working.
pub fn state_icon(state: RowState, spinner: &'static str) -> &'static str {
    state.icon().unwrap_or(spinner)
}

/// A one-line progress bar: red to peach to green, grey remainder.
pub fn gradient_bar(buf: &mut Buffer, area: Rect, ratio: f64, theme: &Theme) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    let ratio = ratio.clamp(0.0, 1.0);
    let mut filled = (f64::from(area.width) * ratio).round() as u16;
    // Never show a full bar before 100 %, or an empty one once started.
    if ratio < 1.0 {
        filled = filled.min(area.width.saturating_sub(1));
    }
    if ratio > 0.0 {
        filled = filled.max(1);
    }
    for x in 0..area.width {
        let cell = &mut buf[(area.x + x, area.y)];
        cell.set_symbol("\u{2501}");
        if x < filled {
            let t = f64::from(x) / f64::from(area.width.max(2) - 1);
            cell.set_fg(theme.progress_gradient(t));
        } else {
            cell.set_fg(theme.surface1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fitting_counts_cells_not_bytes() {
        assert_eq!(fit("hello", 10), "hello");
        assert_eq!(fit("hello world", 6), "hello\u{2026}");
        // Wide characters take two cells each.
        assert_eq!(
            fit("\u{96ea}\u{96ea}\u{96ea}", 5),
            "\u{96ea}\u{96ea}\u{2026}"
        );
        assert_eq!(fit_left("/very/long/path/file.iso", 9), "\u{2026}file.iso");
        assert_eq!(pad("ab", 4), "ab  ");
        assert_eq!(pad_left("ab", 4), "  ab");
        assert_eq!(text_width(&pad("\u{96ea}\u{96ea}\u{96ea}", 4)), 4);
        assert_eq!(fit("abc", 0), "");
    }
}
