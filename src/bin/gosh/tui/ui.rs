use gosh_dl::types::{DownloadState, DownloadStatus};
use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Wrap},
};

use super::app::{DialogState, TuiApp, ViewMode};

/// Main render function
pub fn render(frame: &mut Frame, app: &TuiApp) {
    let _theme = app.theme();

    // Main layout
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // Header
            Constraint::Fill(1),   // Download list - takes remaining space
            Constraint::Length(7), // Details panel
            Constraint::Length(1), // Status bar
        ])
        .split(frame.area());

    // Render header
    render_header(frame, chunks[0], app);

    // Render download list
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

fn render_download_list(frame: &mut Frame, area: Rect, app: &TuiApp) {
    let theme = app.theme();

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(theme.border_style())
        .title(" Downloads ")
        .title_style(theme.title_style());

    let inner = block.inner(area);
    frame.render_widget(block, area);

    if app.downloads.is_empty() {
        let empty_msg = Paragraph::new("No downloads. Press 'a' to add one.")
            .style(theme.muted_style())
            .alignment(Alignment::Center);
        frame.render_widget(empty_msg, inner);
        return;
    }

    // Create list items
    let items: Vec<ListItem> = app
        .downloads
        .iter()
        .enumerate()
        .map(|(i, dl)| {
            let is_selected = i == app.selected;
            create_download_item(dl, is_selected, theme)
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

    let name = truncate(&dl.metadata.name, 35);
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
        .title(" Details ")
        .title_style(theme.title_style());

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

        let details = format!(
            "Name: {}\n\
             State: {}  │  Progress: {:.1}%  │  Size: {} / {}\n\
             Speed: {} ↓  {} ↑  │  Connections: {}  │  ETA: {}\n\
             Path: {}",
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
            dl.metadata.save_dir.display()
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
        .title(" Help ")
        .title_style(theme.title_style());

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
                .title(" Add Download ")
                .title_style(theme.title_style());

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
                .title(" Confirm ")
                .title_style(theme.warning_style());

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
                .title(" Error ")
                .title_style(theme.error_style());

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

// Helper functions

fn format_speed(bytes_per_sec: u64) -> String {
    if bytes_per_sec == 0 {
        "0 B".to_string()
    } else if bytes_per_sec < 1024 {
        format!("{} B", bytes_per_sec)
    } else if bytes_per_sec < 1024 * 1024 {
        format!("{:.1} KB", bytes_per_sec as f64 / 1024.0)
    } else if bytes_per_sec < 1024 * 1024 * 1024 {
        format!("{:.1} MB", bytes_per_sec as f64 / (1024.0 * 1024.0))
    } else {
        format!("{:.2} GB", bytes_per_sec as f64 / (1024.0 * 1024.0 * 1024.0))
    }
}

fn format_size(bytes: u64) -> String {
    if bytes == 0 {
        "0 B".to_string()
    } else if bytes < 1024 {
        format!("{} B", bytes)
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else if bytes < 1024 * 1024 * 1024 {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    } else {
        format!("{:.2} GB", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
    }
}

fn format_duration(seconds: u64) -> String {
    if seconds == 0 {
        return "--".to_string();
    }

    let hours = seconds / 3600;
    let minutes = (seconds % 3600) / 60;
    let secs = seconds % 60;

    if hours > 0 {
        format!("{}:{:02}:{:02}", hours, minutes, secs)
    } else {
        format!("{}:{:02}", minutes, secs)
    }
}

fn format_state(state: &DownloadState) -> String {
    match state {
        DownloadState::Queued => "Queued".to_string(),
        DownloadState::Connecting => "Connecting".to_string(),
        DownloadState::Downloading => "Downloading".to_string(),
        DownloadState::Seeding => "Seeding".to_string(),
        DownloadState::Paused => "Paused".to_string(),
        DownloadState::Completed => "Completed".to_string(),
        DownloadState::Error { kind, .. } => format!("Error: {}", kind),
    }
}

fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len - 3])
    }
}
