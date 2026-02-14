use gosh_dl::{DownloadState, DownloadStatus};
use ratatui::{
    prelude::*,
    text::Line,
    widgets::{
        Block, BorderType, Borders, Clear, LineGauge, Paragraph, Scrollbar, ScrollbarOrientation,
        ScrollbarState, Sparkline, Tabs, Wrap,
    },
};

use super::app::{DialogState, ToastLevel, TuiApp, ViewMode};
use crate::format::{format_duration, format_size, format_speed, format_state};
use crate::util::truncate_str;

/// Main render function
pub fn render(frame: &mut Frame, app: &mut TuiApp) {
    let theme = app.theme();

    // Fill background
    frame.render_widget(Block::default().style(Style::default().bg(theme.bg)), frame.area());

    // Main layout
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Header with tabs
            Constraint::Fill(1),  // Download list
            Constraint::Length(9), // Details panel
            Constraint::Length(1), // Status bar
        ])
        .split(frame.area());

    render_header(frame, chunks[0], app);
    render_download_list(frame, chunks[1], app);
    render_details(frame, chunks[2], app);
    render_status_bar(frame, chunks[3], app);

    // Dim background behind overlays
    if app.show_help || app.dialog.is_some() {
        dim_background(frame);
    }

    // Overlays
    if app.show_help {
        render_help_dialog(frame, app);
    }
    if let Some(ref dialog) = app.dialog {
        render_dialog(frame, dialog, app);
    }

    // Toast notifications (always on top)
    render_toasts(frame, app);

    // Startup fade effect (queued once on first frame)
    if !app.startup_effects_added {
        app.startup_effects_added = true;
        let fade = tachyonfx::fx::fade_from(
            Color::Rgb(17, 17, 27), // Crust (darkest bg)
            Color::Rgb(17, 17, 27),
            (500, tachyonfx::Interpolation::CubicOut),
        );
        app.effect_manager.add_effect(fade);
    }

    // Process tachyonfx effects (must be last)
    let elapsed = app.last_frame.elapsed();
    app.last_frame = std::time::Instant::now();
    let screen_area = frame.area();
    app.effect_manager.process_effects(
        elapsed.into(),
        frame.buffer_mut(),
        screen_area,
    );
}

fn render_header(frame: &mut Frame, area: Rect, app: &TuiApp) {
    let theme = app.theme();

    let speed_str = format!(
        " ↓ {}  ↑ {}  │  {} downloads ",
        format_speed(app.download_speed),
        format_speed(app.upload_speed),
        app.downloads.len()
    );

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(theme.border_style())
        .title(
            Line::from(format!(" gosh v{} ", env!("CARGO_PKG_VERSION")))
                .style(theme.title_style()),
        )
        .title_bottom(Line::from(speed_str).right_aligned().style(Style::default().fg(theme.teal)));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    // Tabs widget for view modes
    let tab_titles = vec!["All", "Active", "Completed"];
    let selected_tab = match app.mode {
        ViewMode::All => 0,
        ViewMode::Active => 1,
        ViewMode::Completed => 2,
    };

    let tabs = Tabs::new(tab_titles)
        .select(selected_tab)
        .highlight_style(Style::default().fg(theme.accent).add_modifier(Modifier::BOLD))
        .style(Style::default().fg(theme.overlay1))
        .divider("│")
        .padding(" ", " ");

    frame.render_widget(tabs, inner);
}

fn render_download_list(frame: &mut Frame, area: Rect, app: &mut TuiApp) {
    let theme = app.theme().clone();

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(theme.border_style())
        .title(Line::from(" Downloads ").style(theme.title_style()));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    if app.downloads.is_empty() {
        let empty = vec![
            Line::from(""),
            Line::from(Span::styled("No downloads yet", Style::default().fg(theme.overlay0))),
            Line::from(""),
            Line::from(vec![
                Span::styled("Press ", Style::default().fg(theme.overlay0)),
                Span::styled(" a ", Style::default().fg(theme.accent).add_modifier(Modifier::BOLD)),
                Span::styled("to add a download", Style::default().fg(theme.overlay0)),
            ]),
        ];
        let paragraph = Paragraph::new(empty).alignment(Alignment::Center);
        frame.render_widget(paragraph, inner);
        return;
    }

    // Each download takes 2 lines
    let lines_per_item = 2;
    let visible_items = (inner.height as usize) / lines_per_item;
    app.last_visible_height = visible_items;
    app.adjust_scroll(visible_items);

    let end = (app.scroll_offset + visible_items).min(app.downloads.len());

    // Get current spinner symbol for animated states
    let spinner = spinner_symbol(&app.throbber_state);

    // Render each download item as 2-line block
    for (i, dl) in app.downloads[app.scroll_offset..end].iter().enumerate() {
        let global_idx = i + app.scroll_offset;
        let is_selected = global_idx == app.selected;
        let y = inner.y + (i * lines_per_item) as u16;

        if y + 1 >= inner.y + inner.height {
            break;
        }

        let item_area = Rect::new(inner.x, y, inner.width, lines_per_item as u16);
        render_download_item(frame, item_area, dl, is_selected, &theme, spinner);
    }

    // Scrollbar
    if app.downloads.len() > visible_items {
        let mut scrollbar_state = ScrollbarState::new(app.downloads.len())
            .position(app.selected);
        let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .style(Style::default().fg(theme.surface2));
        frame.render_stateful_widget(scrollbar, area, &mut scrollbar_state);
    }
}

/// Extract current spinner symbol from throbber state
fn spinner_symbol(state: &throbber_widgets_tui::ThrobberState) -> &'static str {
    let set = &throbber_widgets_tui::BRAILLE_SIX;
    let len = set.symbols.len() as i8;
    let idx = ((state.index() % len) + len) % len;
    set.symbols[idx as usize]
}

fn render_download_item(
    frame: &mut Frame,
    area: Rect,
    dl: &DownloadStatus,
    is_selected: bool,
    theme: &super::theme::Theme,
    spinner: &str,
) {
    // Use animated spinner for active states, static icons for rest
    let state_icon = match &dl.state {
        DownloadState::Downloading => spinner,
        DownloadState::Connecting => spinner,
        DownloadState::Seeding => "↑",
        DownloadState::Paused => "⏸",
        DownloadState::Queued => "◷",
        DownloadState::Completed => "✓",
        DownloadState::Error { .. } => "✗",
    };

    let state_color = theme.state_color(&dl.state);
    let name = truncate_str(&dl.metadata.name, area.width.saturating_sub(20) as usize);
    let state_label = format_state(&dl.state);

    let selector = if is_selected { "▶" } else { " " };
    let bg = if is_selected { theme.surface0 } else { theme.bg };

    // Line 1: selector + icon + name + state
    let line1 = Line::from(vec![
        Span::styled(format!(" {} ", selector), Style::default().fg(theme.lavender).bg(bg)),
        Span::styled(format!("{} ", state_icon), Style::default().fg(state_color).bg(bg)),
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
        let progress_color = theme.progress_color(progress);

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
        let gauge_left = 5_u16; // "  ▶ ↓ " prefix width
        let label_width = label.len() as u16 + 1;
        let gauge_width = area.width.saturating_sub(gauge_left + label_width);

        if gauge_width > 4 {
            let gauge_area = Rect::new(area.x + gauge_left, area.y + 1, gauge_width, 1);
            let gauge = LineGauge::default()
                .ratio(progress / 100.0)
                .filled_style(Style::default().fg(progress_color).bg(bg))
                .unfilled_style(Style::default().fg(theme.surface1).bg(bg))
                .filled_symbol("━")
                .unfilled_symbol("━");
            frame.render_widget(gauge, gauge_area);

            let label_area = Rect::new(
                area.x + gauge_left + gauge_width,
                area.y + 1,
                label_width,
                1,
            );
            let label_widget =
                Paragraph::new(Span::styled(format!(" {}", label), Style::default().fg(theme.subtext0).bg(bg)));
            frame.render_widget(label_widget, label_area);
        }
    }
}

fn render_details(frame: &mut Frame, area: Rect, app: &TuiApp) {
    let theme = app.theme();

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(theme.border_style())
        .title(Line::from(" Details ").style(theme.title_style()));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    if let Some(dl) = app.selected_download() {
        // Split details: left metadata, right sparkline
        let detail_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Fill(1), Constraint::Length(34)])
            .split(inner);

        // Left: metadata
        let total = dl
            .progress
            .total_size
            .map(format_size)
            .unwrap_or_else(|| "Unknown".to_string());
        let completed = format_size(dl.progress.completed_size);
        let state = format_state(&dl.state);
        let state_color = theme.state_color(&dl.state);

        // Connection quality indicator
        let quality = connection_quality(dl.progress.connections);

        let meta_lines = vec![
            Line::from(vec![
                Span::styled("  Name: ", Style::default().fg(theme.overlay1)),
                Span::styled(&dl.metadata.name, Style::default().fg(theme.text)),
            ]),
            Line::from(vec![
                Span::styled(" State: ", Style::default().fg(theme.overlay1)),
                Span::styled(state, Style::default().fg(state_color)),
                Span::styled("  │  ", Style::default().fg(theme.surface2)),
                Span::styled(format!("{:.1}%", dl.progress.percentage()), Style::default().fg(theme.text)),
                Span::styled("  │  ", Style::default().fg(theme.surface2)),
                Span::styled(format!("{} / {}", completed, total), Style::default().fg(theme.subtext0)),
            ]),
            Line::from(vec![
                Span::styled(" Speed: ", Style::default().fg(theme.overlay1)),
                Span::styled(
                    format!("{} ↓", format_speed(dl.progress.download_speed)),
                    Style::default().fg(theme.teal),
                ),
                Span::styled("  ", Style::default()),
                Span::styled(
                    format!("{} ↑", format_speed(dl.progress.upload_speed)),
                    Style::default().fg(theme.peach),
                ),
                Span::styled("  │  ", Style::default().fg(theme.surface2)),
                Span::styled(
                    format!("ETA: {}",
                        dl.progress
                            .eta_seconds
                            .map(format_duration)
                            .unwrap_or_else(|| "--".to_string())
                    ),
                    Style::default().fg(theme.subtext0),
                ),
            ]),
            Line::from(vec![
                Span::styled(" Peers: ", Style::default().fg(theme.overlay1)),
                Span::styled(format!("{}", dl.progress.connections), Style::default().fg(theme.text)),
                Span::styled(" ", Style::default()),
                Span::styled(quality.0, Style::default().fg(quality.1)),
                Span::styled("  │  ", Style::default().fg(theme.surface2)),
                Span::styled("Path: ", Style::default().fg(theme.overlay1)),
                Span::styled(
                    truncate_str(&dl.metadata.save_dir.display().to_string(), 40),
                    Style::default().fg(theme.overlay0),
                ),
            ]),
        ];

        frame.render_widget(Paragraph::new(meta_lines), detail_chunks[0]);

        // Right: sparkline graphs
        let spark_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1), // Download label
                Constraint::Length(3), // Download sparkline
                Constraint::Length(1), // Upload label
                Constraint::Length(2), // Upload sparkline
            ])
            .split(detail_chunks[1]);

        // Download speed sparkline
        let dl_label = Line::from(vec![
            Span::styled(" Speed ↓ ", Style::default().fg(theme.teal).add_modifier(Modifier::BOLD)),
        ]);
        frame.render_widget(Paragraph::new(dl_label), spark_chunks[0]);

        let dl_data: Vec<u64> = app
            .speed_history
            .iter()
            .map(|(d, _)| *d)
            .collect();
        let dl_sparkline = Sparkline::default()
            .data(&dl_data)
            .style(Style::default().fg(theme.teal).bg(theme.bg_dim));
        frame.render_widget(dl_sparkline, spark_chunks[1]);

        // Upload speed sparkline
        let ul_label = Line::from(vec![
            Span::styled(" Speed ↑ ", Style::default().fg(theme.peach).add_modifier(Modifier::BOLD)),
        ]);
        frame.render_widget(Paragraph::new(ul_label), spark_chunks[2]);

        let ul_data: Vec<u64> = app
            .speed_history
            .iter()
            .map(|(_, u)| *u)
            .collect();
        let ul_sparkline = Sparkline::default()
            .data(&ul_data)
            .style(Style::default().fg(theme.peach).bg(theme.bg_dim));
        frame.render_widget(ul_sparkline, spark_chunks[3]);
    } else {
        let msg = Paragraph::new("Select a download to view details")
            .style(theme.muted_style())
            .alignment(Alignment::Center);
        frame.render_widget(msg, inner);
    }
}

/// Connection quality bar based on peer count
fn connection_quality(connections: u32) -> (&'static str, Color) {
    if connections > 50 {
        ("▰▰▰▰▰", Color::Rgb(166, 227, 161)) // success green
    } else if connections > 20 {
        ("▰▰▰▰▱", Color::Rgb(137, 180, 250)) // accent blue
    } else if connections > 5 {
        ("▰▰▰▱▱", Color::Rgb(148, 226, 213)) // teal
    } else if connections > 1 {
        ("▰▰▱▱▱", Color::Rgb(249, 226, 175)) // warning yellow
    } else if connections > 0 {
        ("▰▱▱▱▱", Color::Rgb(250, 179, 135)) // peach
    } else {
        ("▱▱▱▱▱", Color::Rgb(108, 112, 134)) // muted
    }
}

fn render_status_bar(frame: &mut Frame, area: Rect, app: &TuiApp) {
    let theme = app.theme();

    let keys = vec![
        ("a", "add"),
        ("p", "pause"),
        ("r", "resume"),
        ("c", "cancel"),
        ("d", "delete"),
        ("1-3", "views"),
        ("?", "help"),
        ("q", "quit"),
    ];

    let mut spans = Vec::new();
    for (i, (key, desc)) in keys.iter().enumerate() {
        if i > 0 {
            spans.push(Span::styled("  ", Style::default().bg(theme.bg_deep)));
        }
        spans.push(Span::styled(
            format!(" {} ", key),
            Style::default().fg(theme.bg_deep).bg(theme.accent),
        ));
        spans.push(Span::styled(
            format!(" {} ", desc),
            Style::default().fg(theme.subtext0).bg(theme.bg_deep),
        ));
    }

    let status = Paragraph::new(Line::from(spans)).style(Style::default().bg(theme.bg_deep));
    frame.render_widget(status, area);
}

fn render_help_dialog(frame: &mut Frame, app: &TuiApp) {
    let theme = app.theme();

    let area = centered_rect(60, 70, frame.area());
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
        .border_type(BorderType::Rounded)
        .border_style(theme.border_focused_style())
        .title(Line::from(" Help ").style(theme.title_style()));

    let paragraph = Paragraph::new(help_text)
        .block(block)
        .style(Style::default().fg(theme.text).bg(theme.bg))
        .wrap(Wrap { trim: true });

    frame.render_widget(paragraph, area);
}

fn render_dialog(frame: &mut Frame, dialog: &DialogState, app: &TuiApp) {
    let theme = app.theme();

    match dialog {
        DialogState::AddUrl { input, cursor } => {
            let area = centered_rect(65, 20, frame.area());
            frame.render_widget(Clear, area);

            let block = Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(theme.border_focused_style())
                .title(Line::from(" Add Download ").style(theme.title_style()))
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

            let block = Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(theme.warning))
                .title(Line::from(" Confirm ").style(Style::default().fg(theme.warning).add_modifier(Modifier::BOLD)))
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

            let block = Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(theme.error))
                .title(Line::from(" Error ").style(Style::default().fg(theme.error).add_modifier(Modifier::BOLD)))
                .style(Style::default().bg(theme.bg));

            let paragraph = Paragraph::new(content).block(block);
            frame.render_widget(paragraph, area);
        }
    }
}

/// Render toast notifications in top-right corner
fn render_toasts(frame: &mut Frame, app: &TuiApp) {
    let theme = app.theme();
    let area = frame.area();

    if app.toasts.is_empty() || area.width < 30 {
        return;
    }

    let toast_width = 44_u16.min(area.width - 2);

    for (i, toast) in app.toasts.iter().rev().enumerate() {
        let y = area.y + 1 + (i as u16 * 3);
        if y + 2 >= area.height {
            break;
        }

        let toast_area = Rect::new(area.width - toast_width - 1, y, toast_width, 3);

        // Fade based on age (dim after 3 seconds)
        let age = toast.created.elapsed().as_secs_f32();
        let fading = age > 3.0;

        let (icon, border_color) = match toast.level {
            ToastLevel::Success => ("✓ ", theme.success),
            ToastLevel::Error => ("✗ ", theme.error),
        };

        let fg = if fading { theme.overlay0 } else { theme.text };
        let border_fg = if fading { theme.surface1 } else { border_color };

        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(border_fg))
            .style(Style::default().bg(theme.bg_dim));

        let content = Line::from(vec![
            Span::styled(icon, Style::default().fg(border_color)),
            Span::styled(
                truncate_str(&toast.message, (toast_width - 6) as usize),
                Style::default().fg(fg),
            ),
        ]);

        frame.render_widget(Clear, toast_area);
        frame.render_widget(
            Paragraph::new(content).block(block),
            toast_area,
        );
    }
}

/// Apply DIM modifier to all cells (darkens background behind overlays)
fn dim_background(frame: &mut Frame) {
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
