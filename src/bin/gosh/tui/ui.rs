use ratatui::{
    prelude::*,
    widgets::{Block, Paragraph},
};

use super::app::{LayoutMode, SearchState, TuiApp};
use super::widgets::{
    activity_log, batch_import, chunk_map, details_panel, dialogs, download_list, header, logo,
    net_graph, settings, status_bar, tab_bar, toasts,
};

/// Main render function
pub fn render(frame: &mut Frame, app: &mut TuiApp) {
    let theme = app.theme();

    // Fill background
    frame.render_widget(
        Block::default().style(Style::default().bg(theme.bg)),
        frame.area(),
    );

    match app.layout_mode {
        LayoutMode::TwoColumn => render_two_column(frame, app),
        LayoutMode::SingleColumn => render_single_column(frame, app),
        LayoutMode::Minimal => render_minimal(frame, app),
    }

    // Overlays (same for all modes)
    if app.show_help || app.dialog.is_some() {
        dialogs::dim_background(frame);
    }
    if app.show_help {
        dialogs::render_help_dialog(frame, app);
    }
    if let Some(ref dialog) = app.dialog {
        dialogs::render_dialog(frame, dialog, app);
        settings::render_settings(frame, dialog, app);
        batch_import::render_batch_import(frame, dialog, app);
    }

    // Render search bar overlay if active
    if let Some(ref search) = app.search {
        render_search_bar(frame, app, search);
    }

    // Toast notifications (always on top)
    toasts::render_toasts(frame, app);

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
    app.effect_manager
        .process_effects(elapsed.into(), frame.buffer_mut(), screen_area);
}

fn render_two_column(frame: &mut Frame, app: &mut TuiApp) {
    let chunks = Layout::vertical([
        Constraint::Length(1),
        Constraint::Fill(1),
        Constraint::Length(1),
    ])
    .split(frame.area());

    // Brand bar (top line)
    render_brand_bar(frame, chunks[0], app);

    let main_cols = Layout::horizontal([Constraint::Percentage(58), Constraint::Percentage(42)])
        .split(chunks[1]);

    // Left column
    let left = Layout::vertical([
        Constraint::Length(4),
        Constraint::Length(3),
        Constraint::Fill(1),
    ])
    .split(main_cols[0]);

    logo::render_logo(frame, left[0], app);
    tab_bar::render_tab_bar(frame, left[1], app);
    download_list::render_download_list(frame, left[2], app);

    // Right column: net graph, details/activity log, optional chunk map
    let has_chunks = !app.chunk_states.is_empty();
    let chunk_height = if has_chunks {
        let rows_needed =
            (app.chunk_count as u16).div_ceil(main_cols[1].width.saturating_sub(4).max(1)) + 2;
        rows_needed.min(8)
    } else {
        0
    };

    let right = Layout::vertical([
        Constraint::Length(12),
        Constraint::Fill(1),
        Constraint::Length(chunk_height),
    ])
    .split(main_cols[1]);

    // Net graph
    net_graph::render_net_graph(frame, right[0], app);
    // Details or Activity log
    if app.show_activity_log {
        activity_log::render_activity_log(frame, right[1], app);
    } else {
        details_panel::render_details(frame, right[1], app);
    }
    // Chunk map (only when chunks exist)
    if has_chunks && chunk_height > 0 {
        chunk_map::render_chunk_map(frame, right[2], app);
    }

    // Status bar
    status_bar::render_status_bar(frame, chunks[2], app);
}

fn render_single_column(frame: &mut Frame, app: &mut TuiApp) {
    let chunks = Layout::vertical([
        Constraint::Length(3),
        Constraint::Fill(1),
        Constraint::Length(9),
        Constraint::Length(1),
    ])
    .split(frame.area());

    header::render_header(frame, chunks[0], app);
    download_list::render_download_list(frame, chunks[1], app);
    details_panel::render_details(frame, chunks[2], app);
    status_bar::render_status_bar(frame, chunks[3], app);
}

fn render_minimal(frame: &mut Frame, app: &mut TuiApp) {
    let chunks = Layout::vertical([Constraint::Fill(1), Constraint::Length(1)]).split(frame.area());

    download_list::render_download_list(frame, chunks[0], app);
    status_bar::render_status_bar(frame, chunks[1], app);
}

fn render_brand_bar(frame: &mut Frame, area: Rect, app: &TuiApp) {
    let theme = app.theme();
    let brand = Line::from(vec![
        Span::styled(
            format!(" gosh v{} ", env!("CARGO_PKG_VERSION")),
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled("\u{2502} ", Style::default().fg(theme.surface2)),
        Span::styled(
            format!(
                "\u{2193} {}/s  ",
                crate::format::format_speed(app.download_speed)
            ),
            Style::default().fg(theme.teal),
        ),
        Span::styled(
            format!(
                "\u{2191} {}/s  ",
                crate::format::format_speed(app.upload_speed)
            ),
            Style::default().fg(theme.peach),
        ),
        Span::styled(
            format!("\u{2502} {} downloads", app.downloads.len()),
            Style::default().fg(theme.subtext0),
        ),
    ]);
    frame.render_widget(
        Paragraph::new(brand).style(Style::default().bg(theme.bg_dim)),
        area,
    );
}

fn render_search_bar(frame: &mut Frame, app: &TuiApp, search: &SearchState) {
    let theme = app.theme();
    let screen = frame.area();

    // Render at top of screen, 1 line tall
    let bar_area = Rect::new(screen.x, screen.y, screen.width, 1);

    // Background
    frame.render_widget(
        Block::default().style(Style::default().bg(theme.surface0)),
        bar_area,
    );

    let scope_label = search.scope.label();
    let line = Line::from(vec![
        Span::styled(
            " / ",
            Style::default()
                .fg(theme.bg_deep)
                .bg(theme.accent)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!(" [{}] ", scope_label),
            Style::default().fg(theme.accent).bg(theme.surface0),
        ),
        Span::styled(
            search.query.to_string(),
            Style::default().fg(theme.text).bg(theme.surface0),
        ),
        Span::styled(
            if search.query.is_empty() {
                " Type to search... (Ctrl+S: scope, Esc: cancel)"
            } else {
                ""
            },
            Style::default().fg(theme.overlay0).bg(theme.surface0),
        ),
    ]);

    frame.render_widget(Paragraph::new(line), bar_area);

    // Position cursor
    let cursor_x = bar_area.x + 3 + scope_label.len() as u16 + 4 + search.cursor as u16;
    if cursor_x < bar_area.x + bar_area.width {
        frame.set_cursor_position(Position::new(cursor_x, bar_area.y));
    }
}
