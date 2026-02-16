use gosh_dl::{DownloadState, DownloadStatus};
use ratatui::{
    prelude::*,
    style::Color,
    text::Line,
    widgets::{Block, Paragraph},
};

use super::gradient_bar::render_gradient_bar;
use crate::format::{format_duration, format_speed, format_state};
use crate::tui::theme::Theme;
use crate::util::truncate_str;

/// Extract current spinner symbol from throbber state
pub fn spinner_symbol(state: &throbber_widgets_tui::ThrobberState) -> &'static str {
    let set = &throbber_widgets_tui::BRAILLE_SIX;
    let len = set.symbols.len() as i8;
    let idx = ((state.index() % len) + len) % len;
    set.symbols[idx as usize]
}

pub fn render_download_item(
    frame: &mut Frame,
    area: Rect,
    dl: &DownloadStatus,
    is_selected: bool,
    theme: &Theme,
    spinner: &str,
) {
    // Use animated spinner for active states, static icons for rest
    let state_icon = match &dl.state {
        DownloadState::Downloading => spinner,
        DownloadState::Connecting => spinner,
        DownloadState::Seeding => "\u{2191}",
        DownloadState::Paused => "\u{23f8}",
        DownloadState::Queued => "\u{25f7}",
        DownloadState::Completed => "\u{2713}",
        DownloadState::Error { .. } => "\u{2717}",
    };

    let state_color = theme.state_color(&dl.state);
    let name = truncate_str(&dl.metadata.name, area.width.saturating_sub(20) as usize);
    let state_label = format_state(&dl.state);

    let selector = if is_selected { "\u{25b6}" } else { " " };
    let bg = if is_selected {
        theme.surface0
    } else {
        theme.bg
    };

    // Line 1: selector + icon + name + state
    let line1 = Line::from(vec![
        Span::styled(
            format!(" {} ", selector),
            Style::default().fg(theme.lavender).bg(bg),
        ),
        Span::styled(
            format!("{} ", state_icon),
            Style::default().fg(state_color).bg(bg),
        ),
        Span::styled(name, Style::default().fg(theme.text).bg(bg)),
        Span::raw("  "),
        Span::styled(state_label, Style::default().fg(state_color).bg(bg)),
    ]);

    let line1_area = Rect::new(area.x, area.y, area.width, 1);
    // Fill background for line 1
    frame.render_widget(Block::default().style(Style::default().bg(bg)), line1_area);
    frame.render_widget(Paragraph::new(line1), line1_area);

    // Line 2: progress bar + percentage + speed + ETA
    if area.height >= 2 {
        let line2_area = Rect::new(area.x, area.y + 1, area.width, 1);
        frame.render_widget(Block::default().style(Style::default().bg(bg)), line2_area);

        let progress = dl.progress.percentage();

        let speed = if dl.progress.download_speed > 0 {
            format!(" {}/s", format_speed(dl.progress.download_speed))
        } else {
            String::new()
        };

        let eta = dl
            .progress
            .eta_seconds
            .map(|s| format!(" ETA {}", format_duration(s)))
            .unwrap_or_default();

        let label = format!("{:>5.1}%{}{}", progress, speed, eta);

        // Indent to align under the name
        let gauge_left = 5_u16; // "  > X " prefix width
        let label_width = label.len() as u16 + 1;
        let gauge_width = area.width.saturating_sub(gauge_left + label_width);

        if gauge_width > 4 {
            let gauge_area = Rect::new(area.x + gauge_left, area.y + 1, gauge_width, 1);
            render_gradient_bar(frame.buffer_mut(), gauge_area, progress / 100.0, theme);

            let label_area = Rect::new(
                area.x + gauge_left + gauge_width,
                area.y + 1,
                label_width,
                1,
            );
            let label_widget = Paragraph::new(Span::styled(
                format!(" {}", label),
                Style::default().fg(theme.subtext0).bg(bg),
            ));
            frame.render_widget(label_widget, label_area);
        }
    }
}

/// Connection quality bar based on peer count
pub fn connection_quality(connections: u32) -> (&'static str, Color) {
    if connections > 50 {
        (
            "\u{25b0}\u{25b0}\u{25b0}\u{25b0}\u{25b0}",
            Color::Rgb(166, 227, 161),
        ) // success green
    } else if connections > 20 {
        (
            "\u{25b0}\u{25b0}\u{25b0}\u{25b0}\u{25b1}",
            Color::Rgb(137, 180, 250),
        ) // accent blue
    } else if connections > 5 {
        (
            "\u{25b0}\u{25b0}\u{25b0}\u{25b1}\u{25b1}",
            Color::Rgb(148, 226, 213),
        ) // teal
    } else if connections > 1 {
        (
            "\u{25b0}\u{25b0}\u{25b1}\u{25b1}\u{25b1}",
            Color::Rgb(249, 226, 175),
        ) // warning yellow
    } else if connections > 0 {
        (
            "\u{25b0}\u{25b1}\u{25b1}\u{25b1}\u{25b1}",
            Color::Rgb(250, 179, 135),
        ) // peach
    } else {
        (
            "\u{25b1}\u{25b1}\u{25b1}\u{25b1}\u{25b1}",
            Color::Rgb(108, 112, 134),
        ) // muted
    }
}
