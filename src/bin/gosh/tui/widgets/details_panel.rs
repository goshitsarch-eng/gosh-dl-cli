use ratatui::{
    prelude::*,
    widgets::{Paragraph, Sparkline},
};

use super::btop_border::btop_block;
use super::download_item::connection_quality;
use crate::format::{format_duration, format_size, format_speed, format_state};
use crate::tui::app::TuiApp;
use crate::util::truncate_str;

pub fn render_details(frame: &mut Frame, area: Rect, app: &TuiApp) {
    let theme = app.theme();

    let block = btop_block("Details", theme, false);

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
                Span::styled("  \u{2502}  ", Style::default().fg(theme.surface2)),
                Span::styled(
                    format!("{:.1}%", dl.progress.percentage()),
                    Style::default().fg(theme.text),
                ),
                Span::styled("  \u{2502}  ", Style::default().fg(theme.surface2)),
                Span::styled(
                    format!("{} / {}", completed, total),
                    Style::default().fg(theme.subtext0),
                ),
            ]),
            Line::from(vec![
                Span::styled(" Speed: ", Style::default().fg(theme.overlay1)),
                Span::styled(
                    format!("{} \u{2193}", format_speed(dl.progress.download_speed)),
                    Style::default().fg(theme.teal),
                ),
                Span::styled("  ", Style::default()),
                Span::styled(
                    format!("{} \u{2191}", format_speed(dl.progress.upload_speed)),
                    Style::default().fg(theme.peach),
                ),
                Span::styled("  \u{2502}  ", Style::default().fg(theme.surface2)),
                Span::styled(
                    format!(
                        "ETA: {}",
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
                Span::styled(
                    format!("{}", dl.progress.connections),
                    Style::default().fg(theme.text),
                ),
                Span::styled(" ", Style::default()),
                Span::styled(quality.0, Style::default().fg(quality.1)),
                Span::styled("  \u{2502}  ", Style::default().fg(theme.surface2)),
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
        let dl_label = Line::from(vec![Span::styled(
            " Speed \u{2193} ",
            Style::default().fg(theme.teal).add_modifier(Modifier::BOLD),
        )]);
        frame.render_widget(Paragraph::new(dl_label), spark_chunks[0]);

        let dl_data: Vec<u64> = app.speed_history.iter().map(|(d, _)| *d).collect();
        let dl_sparkline = Sparkline::default()
            .data(&dl_data)
            .style(Style::default().fg(theme.teal).bg(theme.bg_dim));
        frame.render_widget(dl_sparkline, spark_chunks[1]);

        // Upload speed sparkline
        let ul_label = Line::from(vec![Span::styled(
            " Speed \u{2191} ",
            Style::default()
                .fg(theme.peach)
                .add_modifier(Modifier::BOLD),
        )]);
        frame.render_widget(Paragraph::new(ul_label), spark_chunks[2]);

        let ul_data: Vec<u64> = app.speed_history.iter().map(|(_, u)| *u).collect();
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
