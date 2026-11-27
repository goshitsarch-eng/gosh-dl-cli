//! Screen layout. Three sizes:
//!
//! * wide (100x24 and up): list and activity on the left, network graph and
//!   details on the right
//! * compact (60x12 and up): one-line rows above a short details box
//! * minimal: just the list

use ratatui::{
    prelude::*,
    widgets::{Block, Paragraph},
};

use super::app::{LayoutMode, TuiApp};
use super::widgets::{
    activity_log, batch_import, details, dialogs, dim_background,
    download_list::{self, ListStyle},
    help, net_graph, settings, status, toasts,
};

/// Main render function
pub fn render(frame: &mut Frame, app: &mut TuiApp) {
    let theme = app.theme();
    frame.render_widget(
        Block::default().style(Style::default().bg(theme.bg).fg(theme.text)),
        frame.area(),
    );

    match app.layout_mode {
        LayoutMode::Wide => render_wide(frame, app),
        LayoutMode::Compact => render_compact(frame, app),
        LayoutMode::Minimal => render_minimal(frame, app),
    }

    // Overlays (same for all sizes)
    if app.show_help || app.dialog.is_some() {
        dim_background(frame);
    }
    if app.show_help {
        help::render(frame, app);
    }
    if let Some(dialog) = &app.dialog {
        dialogs::render(frame, dialog, app);
        settings::render(frame, dialog, app);
        batch_import::render(frame, dialog, app);
    }
    toasts::render(frame, app);

    // Startup fade (queued once, on the first frame)
    if !app.startup_effects_added {
        app.startup_effects_added = true;
        if let Color::Rgb(r, g, b) = app.theme().bg_deep {
            let from = Color::Rgb(r, g, b);
            app.effect_manager.add_effect(tachyonfx::fx::fade_from(
                from,
                from,
                (400, tachyonfx::Interpolation::CubicOut),
            ));
        }
    }
    let elapsed = app.last_frame.elapsed();
    app.last_frame = std::time::Instant::now();
    let screen = frame.area();
    app.effect_manager
        .process_effects(elapsed.into(), frame.buffer_mut(), screen);
}

/// Tell the app how many rows the list shows, so paging and scrolling match
/// what is on screen, then draw it.
fn draw_list(frame: &mut Frame, area: Rect, app: &mut TuiApp, style: ListStyle) {
    let visible = download_list::visible_rows(area, style, app.search.is_some());
    app.last_visible_height = visible.max(1);
    app.adjust_scroll(visible);
    download_list::render(frame, area, app, style);
}

fn render_wide(frame: &mut Frame, app: &mut TuiApp) {
    let [top, main, bottom] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Fill(1),
        Constraint::Length(1),
    ])
    .areas(frame.area());
    status::render_status_line(frame, top, app);
    status::render_key_bar(frame, bottom, app);

    let [left, right] =
        Layout::horizontal([Constraint::Percentage(58), Constraint::Percentage(42)]).areas(main);

    // Left: downloads, with the activity panel underneath when it is on.
    if app.show_activity_log && left.height >= 18 {
        let [list, log] =
            Layout::vertical([Constraint::Fill(1), Constraint::Length(9)]).areas(left);
        draw_list(frame, list, app, ListStyle::Wide);
        activity_log::render(frame, log, app);
    } else {
        draw_list(frame, left, app, ListStyle::Wide);
    }

    // Right: network graph above the details.
    if app.show_speed_graph() && right.height >= 20 {
        let graph_height = (right.height / 3).clamp(8, 13);
        let [graph, info] =
            Layout::vertical([Constraint::Length(graph_height), Constraint::Fill(1)]).areas(right);
        net_graph::render(frame, graph, app);
        details::render(frame, info, app);
    } else {
        details::render(frame, right, app);
    }
}

fn render_compact(frame: &mut Frame, app: &mut TuiApp) {
    let area = frame.area();
    // The details box needs room for a few list rows above it.
    let details_height = if area.height >= 18 { 7 } else { 0 };
    let [top, tabs, list, info, bottom] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Fill(1),
        Constraint::Length(details_height),
        Constraint::Length(1),
    ])
    .areas(area);

    status::render_status_line(frame, top, app);
    render_tabs(frame, tabs, app);
    draw_list(frame, list, app, ListStyle::Compact);
    if details_height > 0 {
        details::render_compact(frame, info, app);
    }
    status::render_key_bar(frame, bottom, app);
}

fn render_minimal(frame: &mut Frame, app: &mut TuiApp) {
    let area = frame.area();
    if area.height >= 4 {
        let [top, tabs, list] = Layout::vertical([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Fill(1),
        ])
        .areas(area);
        status::render_status_line(frame, top, app);
        render_tabs(frame, tabs, app);
        draw_list(frame, list, app, ListStyle::Compact);
    } else {
        draw_list(frame, area, app, ListStyle::Compact);
    }
}

fn render_tabs(frame: &mut Frame, area: Rect, app: &TuiApp) {
    let mut line = download_list::tabs_line(app, true);
    line.spans.insert(0, Span::raw(" "));
    frame.render_widget(
        Paragraph::new(line).style(Style::default().bg(app.theme().bg)),
        area,
    );
}
