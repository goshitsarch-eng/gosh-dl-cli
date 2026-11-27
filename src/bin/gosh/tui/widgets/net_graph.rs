//! Network history: download bars rise from the axis, upload bars hang
//! below it, each scaled to its own peak in the visible window.

use ratatui::{prelude::*, widgets::Paragraph};

use super::{panel, panel_title};
use crate::format::format_speed;
use crate::tui::app::TuiApp;

/// Lower one-eighth to full block.
const BLOCKS: [&str; 9] = [
    " ", "\u{2581}", "\u{2582}", "\u{2583}", "\u{2584}", "\u{2585}", "\u{2586}", "\u{2587}",
    "\u{2588}",
];

pub fn render(frame: &mut Frame, area: Rect, app: &TuiApp) {
    let theme = app.theme();
    // Inside the border, with one cell of padding left and right.
    let inner = Rect::new(
        area.x + 2,
        area.y + 1,
        area.width.saturating_sub(4),
        area.height.saturating_sub(2),
    );
    if inner.width < 8 || inner.height < 3 {
        frame.render_widget(
            panel(theme, panel_title(theme, "Network", String::new()), false),
            area,
        );
        return;
    }

    // One column per sample, newest on the right.
    let columns = inner.width as usize;
    let skip = app.speed_history.len().saturating_sub(columns);
    let samples: Vec<(u64, u64)> = app.speed_history.iter().skip(skip).copied().collect();
    let window = columns as f64 * app.config().tui.refresh_rate_ms as f64 / 1000.0;
    frame.render_widget(
        panel(
            theme,
            panel_title(theme, "Network", format!("last {:.0} s", window.max(1.0))),
            false,
        ),
        area,
    );

    let label = |arrow: &str, now: u64, peak: u64, color: Color| {
        Line::from(vec![
            Span::styled(
                format!("{arrow} {}/s", format_speed(now)),
                Style::default().fg(color).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("   peak {}/s", format_speed(peak)),
                Style::default().fg(theme.overlay1),
            ),
        ])
    };
    frame.render_widget(
        Paragraph::new(label(
            "\u{2193}",
            app.download_speed,
            app.peak_download_speed,
            theme.teal,
        )),
        Rect::new(inner.x, inner.y, inner.width, 1),
    );
    frame.render_widget(
        Paragraph::new(label(
            "\u{2191}",
            app.upload_speed,
            app.peak_upload_speed,
            theme.peach,
        )),
        Rect::new(inner.x, inner.y + inner.height - 1, inner.width, 1),
    );

    // Rows between the two labels: download above the axis, upload below.
    let rows = inner.height - 2;
    if rows == 0 {
        return;
    }
    let (down_rows, axis, up_rows) = match rows {
        1 => (1, 0, 0),
        2 => (1, 0, 1),
        3 => (2, 0, 1),
        n => {
            let up = ((n - 1) / 3).max(1);
            (n - 1 - up, 1, up)
        }
    };
    let top = inner.y + 1;
    let x0 = inner.x + (columns - samples.len()) as u16;
    let buf = frame.buffer_mut();

    if axis == 1 {
        let y = top + down_rows;
        for x in inner.x..inner.x + inner.width {
            buf[(x, y)].set_symbol("\u{2500}").set_fg(theme.surface1);
        }
    }

    // A floor keeps an idle link from being drawn as a wall of noise.
    let down_max = samples.iter().map(|s| s.0).max().unwrap_or(0).max(1024) as f64;
    let up_max = samples.iter().map(|s| s.1).max().unwrap_or(0).max(1024) as f64;
    let plain = matches!(theme.bg, Color::Reset);

    for (i, (down, up)) in samples.iter().enumerate() {
        let x = x0 + i as u16;

        // Download: eighths of a cell, growing upward from the axis.
        let eighths = level(*down, down_max, down_rows);
        for row in 0..down_rows {
            let filled = eighths.saturating_sub(usize::from(row) * 8).min(8);
            if filled == 0 {
                break;
            }
            let y = top + down_rows - 1 - row;
            let t = f64::from(row) / f64::from(down_rows.max(2) - 1);
            buf[(x, y)]
                .set_symbol(BLOCKS[filled])
                .set_fg(theme.dl_graph_gradient(t));
        }

        // Upload: mirrored, growing downward. A partial cell is drawn as
        // the *inverse* of a lower block, which fills the top of the cell.
        let eighths = level(*up, up_max, up_rows);
        for row in 0..up_rows {
            let filled = eighths.saturating_sub(usize::from(row) * 8).min(8);
            if filled == 0 {
                break;
            }
            let y = top + down_rows + axis + row;
            let t = f64::from(row) / f64::from(up_rows.max(2) - 1);
            let color = theme.ul_graph_gradient(t);
            let cell = &mut buf[(x, y)];
            if filled == 8 {
                cell.set_symbol(BLOCKS[8]).set_fg(color);
            } else if plain {
                // Without colours there is no inverse; use the half block.
                cell.set_symbol(if filled >= 4 { "\u{2580}" } else { "\u{2594}" });
            } else {
                cell.set_symbol(BLOCKS[8 - filled])
                    .set_fg(theme.bg)
                    .set_bg(color);
            }
        }
    }
}

/// Bar height in eighths of a cell. Any traffic at all shows at least one.
fn level(value: u64, max: f64, rows: u16) -> usize {
    if value == 0 || rows == 0 {
        return 0;
    }
    let cells = usize::from(rows) * 8;
    (((value as f64 / max) * cells as f64).round() as usize).clamp(1, cells)
}

#[cfg(test)]
mod tests {
    use super::level;

    #[test]
    fn bar_levels_are_clamped_and_never_hide_traffic() {
        assert_eq!(level(0, 1024.0, 4), 0);
        assert_eq!(level(1, 1_000_000.0, 4), 1, "tiny traffic is still visible");
        assert_eq!(level(500, 1000.0, 4), 16);
        assert_eq!(level(5000, 1000.0, 4), 32, "never taller than the graph");
        assert_eq!(level(100, 1000.0, 0), 0);
    }
}
