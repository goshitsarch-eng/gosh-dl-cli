use gosh_dl::{DownloadState, DownloadStatus};
use ratatui::{
    prelude::*,
    text::Line,
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Wrap},
};

use super::app::{DialogState, TuiApp, ViewMode};
use super::widgets::speed_graph;
use crate::format::{format_duration, format_size, format_speed, format_state};
use crate::util::truncate_str;

/// Main render function
pub fn render(frame: &mut Frame, app: &mut TuiApp) {
    let _theme = app.theme();

    // Main layout
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // Header
            Constraint::Fill(1),   // Download list - takes remaining space
            Constraint::Length(8), // Details panel (including speed graph)
            Constraint::Length(1), // Status bar
        ])
        .split(frame.area());

    // Render header
    render_header(frame, chunks[0], app);

    // Render download list (with scrolling)
    render_download_list(frame, chunks[1], app);

    // Render details panel
    render_details(frame, chunks[2], app);

    // Render status bar
    render_status_bar(frame, chunks[3], app);

    // Render overlays
    if app.show_help {
        render_help_dialog(frame, app);
    }

    if let Some(ref dialog) = app.dialog {
        render_dialog(frame, dialog, app);
    }
}

fn render_header(frame: &mut Frame, area: Rect, app: &TuiApp) {
    let theme = app.theme();

    let mode_str = match app.mode {
        ViewMode::All => "[1]All",
        ViewMode::Active => "[2]Active",
        ViewMode::Completed => "[3]Completed",
    };

    let speed_str = format!(
        "↓ {}  ↑ {}",
        format_speed(app.download_speed),
        format_speed(app.upload_speed)
    );

    let count_str = format!("{} downloads", app.downloads.len());

    let header_text = format!(
        " gosh v{}  │  {}  │  {}  │  {}",
        env!("CARGO_PKG_VERSION"),
        mode_str,
        speed_str,
        count_str
    );

    let header = Paragraph::new(header_text).style(theme.header_style());

    frame.render_widget(header, area);
}

fn render_download_list(frame: &mut Frame, area: Rect, app: &mut TuiApp) {
    let theme = app.theme().clone();

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(theme.border_style())
        .title(Line::from(" Downloads ").style(theme.title_style()));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    if app.downloads.is_empty() {
        let empty_msg = Paragraph::new("No downloads. Press 'a' to add one.")
            .style(theme.muted_style())
            .alignment(Alignment::Center);
        frame.render_widget(empty_msg, inner);
        return;
    }

    let visible_height = inner.height as usize;
    app.last_visible_height = visible_height;
    app.adjust_scroll(visible_height);

    let end = (app.scroll_offset + visible_height).min(app.downloads.len());

    // Create list items (only visible range)
    let items: Vec<ListItem> = app.downloads[app.scroll_offset..end]
        .iter()
        .enumerate()
        .map(|(i, dl)| {
            let is_selected = i + app.scroll_offset == app.selected;
            create_download_item(dl, is_selected, &theme)
        })
        .collect();

    let list = List::new(items);
    frame.render_widget(list, inner);
}

fn create_download_item<'a>(
    dl: &DownloadStatus,
    is_selected: bool,
    theme: &super::theme::Theme,
) -> ListItem<'a> {
    let state_icon = match &dl.state {
        DownloadState::Downloading => "▼",
        DownloadState::Seeding => "▲",
        DownloadState::Paused => "⏸",
        DownloadState::Queued => "⏳",
        DownloadState::Connecting => "⟳",
        DownloadState::Completed => "✓",
        DownloadState::Error { .. } => "✗",
    };

    let progress = dl.progress.percentage();
    let progress_bar = create_progress_bar(progress, 20);

    let speed = if dl.progress.download_speed > 0 {
        format!("{}/s", format_speed(dl.progress.download_speed))
    } else {
        String::new()
    };

    let eta = dl
        .progress
        .eta_seconds
        .map(format_duration)
        .unwrap_or_default();

    let name = truncate_str(&dl.metadata.name, 35);
    let _gid = &dl.id.to_gid()[..8];

    let line = format!(
        "{} {} {:<35} {} {:>6.1}% {:>10} {:>8}",
        if is_selected { "▶" } else { " " },
        state_icon,
        name,
        progress_bar,
        progress,
        speed,
        eta
    );

    let style = if is_selected {
        theme.selected_style()
    } else {
        match &dl.state {
            DownloadState::Completed => theme.success_style(),
            DownloadState::Error { .. } => theme.error_style(),
            DownloadState::Paused => theme.warning_style(),
            _ => theme.normal_style(),
        }
    };

    ListItem::new(line).style(style)
}

fn create_progress_bar(percent: f64, width: usize) -> String {
    let filled = ((percent / 100.0) * width as f64) as usize;
    let empty = width.saturating_sub(filled);
    format!("[{}{}]", "█".repeat(filled), "░".repeat(empty))
}

fn render_details(frame: &mut Frame, area: Rect, app: &TuiApp) {
    let theme = app.theme();

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(theme.border_style())
        .title(Line::from(" Details ").style(theme.title_style()));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    if let Some(dl) = app.selected_download() {
        let total = dl
            .progress
            .total_size
            .map(format_size)
            .unwrap_or_else(|| "Unknown".to_string());
        let completed = format_size(dl.progress.completed_size);
        let state = format_state(&dl.state);

        let dl_sparkline = speed_graph::sparkline_string(
            &app.speed_history.iter().map(|(d, _)| *d).collect::<Vec<_>>(),
            30,
        );

        let details = format!(
            "Name: {}\n\
             State: {}  │  Progress: {:.1}%  │  Size: {} / {}\n\
             Speed: {} ↓  {} ↑  │  Connections: {}  │  ETA: {}\n\
             Path: {}\n\
             Speed graph: {}",
            dl.metadata.name,
            state,
            dl.progress.percentage(),
            completed,
            total,
            format_speed(dl.progress.download_speed),
            format_speed(dl.progress.upload_speed),
            dl.progress.connections,
            dl.progress
                .eta_seconds
                .map(format_duration)
                .unwrap_or_else(|| "--".to_string()),
            dl.metadata.save_dir.display(),
            dl_sparkline,
        );

        let paragraph = Paragraph::new(details)
            .style(theme.normal_style())
            .wrap(Wrap { trim: true });

        frame.render_widget(paragraph, inner);
    } else {
        let msg = Paragraph::new("Select a download to view details")
            .style(theme.muted_style())
            .alignment(Alignment::Center);
        frame.render_widget(msg, inner);
    }
}

fn render_status_bar(frame: &mut Frame, area: Rect, app: &TuiApp) {
    let theme = app.theme();

    let help_text =
        " [a]dd  [p]ause  [r]esume  [c]ancel  [d]elete  [↑↓/jk]navigate  [?]help  [q]uit ";

    let status = Paragraph::new(help_text)
        .style(theme.header_style())
        .alignment(Alignment::Center);

    frame.render_widget(status, area);
}

fn render_help_dialog(frame: &mut Frame, app: &TuiApp) {
    let theme = app.theme();

    let area = centered_rect(60, 70, frame.area());

    // Clear the area behind the dialog
    frame.render_widget(Clear, area);

    let help_text = "\
    Keyboard Shortcuts\n\
    ══════════════════\n\
    \n\
    Navigation:\n\
      ↑/k      Select previous\n\
      ↓/j      Select next\n\
      PgUp     Page up\n\
      PgDn     Page down\n\
    \n\
    Actions:\n\
      a        Add new download\n\
      p        Pause selected\n\
      r        Resume selected\n\
      c        Cancel selected\n\
      d        Cancel and delete files\n\
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

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(theme.title_style())
        .title(Line::from(" Help ").style(theme.title_style()));

    let paragraph = Paragraph::new(help_text)
        .block(block)
        .style(theme.normal_style())
        .wrap(Wrap { trim: true });

    frame.render_widget(paragraph, area);
}

fn render_dialog(frame: &mut Frame, dialog: &DialogState, app: &TuiApp) {
    let theme = app.theme();

    match dialog {
        DialogState::AddUrl { input, cursor: _ } => {
            let area = centered_rect(60, 15, frame.area());
            frame.render_widget(Clear, area);

            let block = Block::default()
                .borders(Borders::ALL)
                .border_style(theme.title_style())
                .title(Line::from(" Add Download ").style(theme.title_style()));

            let inner = block.inner(area);
            frame.render_widget(block, area);

            let text = format!(
                "Enter URL, magnet link, or torrent file path:\n\n> {}\n\n[Enter] Add  [Esc] Cancel",
                input
            );

            let paragraph = Paragraph::new(text)
                .style(theme.normal_style())
                .wrap(Wrap { trim: true });

            frame.render_widget(paragraph, inner);
        }
        DialogState::ConfirmCancel { id, delete_files } => {
            let area = centered_rect(50, 20, frame.area());
            frame.render_widget(Clear, area);

            let action = if *delete_files {
                "cancel and DELETE FILES for"
            } else {
                "cancel"
            };

            let text = format!(
                "Are you sure you want to {} download {}?\n\n[y] Yes  [n] No",
                action,
                id.to_gid()
            );

            let block = Block::default()
                .borders(Borders::ALL)
                .border_style(theme.warning_style())
                .title(Line::from(" Confirm ").style(theme.warning_style()));

            let paragraph = Paragraph::new(text)
                .block(block)
                .style(theme.normal_style())
                .wrap(Wrap { trim: true });

            frame.render_widget(paragraph, area);
        }
        DialogState::Error { message } => {
            let area = centered_rect(50, 20, frame.area());
            frame.render_widget(Clear, area);

            let text = format!("{}\n\nPress any key to close", message);

            let block = Block::default()
                .borders(Borders::ALL)
                .border_style(theme.error_style())
                .title(Line::from(" Error ").style(theme.error_style()));

            let paragraph = Paragraph::new(text)
                .block(block)
                .style(theme.normal_style())
                .wrap(Wrap { trim: true });

            frame.render_widget(paragraph, area);
        }
    }
}

/// Create a centered rect
fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
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


