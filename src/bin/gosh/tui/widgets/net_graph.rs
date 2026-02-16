use ratatui::prelude::*;

use super::btop_border::btop_block;
use crate::format::format_speed;
use crate::tui::app::TuiApp;

const BLOCKS: [char; 8] = [
    '\u{2581}', '\u{2582}', '\u{2583}', '\u{2584}', '\u{2585}', '\u{2586}', '\u{2587}', '\u{2588}',
];

pub fn render_net_graph(frame: &mut Frame, area: Rect, app: &TuiApp) {
    let theme = app.theme();
    let block = btop_block("Network", theme, false);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if inner.height < 3 || inner.width < 4 {
        return;
    }

    // Split inner area into download (top) and upload (bottom) sub-graphs
    let dl_height = (inner.height / 2).max(2);
    let ul_height = inner.height.saturating_sub(dl_height);

    let dl_area = Rect::new(inner.x, inner.y, inner.width, dl_height);
    let ul_area = Rect::new(inner.x, inner.y + dl_height, inner.width, ul_height);

    // Extract speed data
    let dl_data: Vec<u64> = app.speed_history.iter().map(|(d, _)| *d).collect();
    let ul_data: Vec<u64> = app.speed_history.iter().map(|(_, u)| *u).collect();

    let buf = frame.buffer_mut();

    render_sub_graph(buf, dl_area, &dl_data, true, theme);
    render_sub_graph(buf, ul_area, &ul_data, false, theme);
}

fn render_sub_graph(
    buf: &mut Buffer,
    area: Rect,
    data: &[u64],
    is_download: bool,
    theme: &crate::tui::theme::Theme,
) {
    if area.height == 0 || area.width == 0 {
        return;
    }

    let graph_width = area.width as usize;
    let graph_height = area.height as usize;

    // Take the last N samples that fit the width
    let visible: Vec<u64> = if data.len() > graph_width {
        data[data.len() - graph_width..].to_vec()
    } else {
        data.to_vec()
    };

    // Auto-scale: max value with 10% headroom, minimum 1024
    let max_val = visible.iter().copied().max().unwrap_or(0);
    let max_val = ((max_val as f64) * 1.1) as u64;
    let max_val = max_val.max(1024);

    let total_eighths = graph_height * 8;

    // Render label in top-right corner
    let arrow = if is_download { "\u{2193}" } else { "\u{2191}" };
    let label = format!("{} {}/s", arrow, format_speed(max_val));
    let label_len = label.len();
    if label_len < area.width as usize {
        let label_x = area.x + area.width - label_len as u16;
        let label_color = if is_download { theme.teal } else { theme.peach };
        for (i, ch) in label.chars().enumerate() {
            let cell = &mut buf[(label_x + i as u16, area.y)];
            cell.set_char(ch);
            cell.set_fg(label_color);
        }
    }

    // Render bars from right to left (newest data on the right)
    let offset = graph_width.saturating_sub(visible.len());

    for (i, &value) in visible.iter().enumerate() {
        let col = area.x + (offset + i) as u16;
        let height_eighths = if max_val > 0 {
            ((value as f64 / max_val as f64) * total_eighths as f64) as usize
        } else {
            0
        };

        for row in 0..graph_height {
            let y = area.y + area.height - 1 - row as u16;
            let row_bottom_eighth = row * 8;
            let row_top_eighth = row_bottom_eighth + 8;

            let cell = &mut buf[(col, y)];

            if height_eighths >= row_top_eighth {
                // Full block
                cell.set_char(BLOCKS[7]);
            } else if height_eighths > row_bottom_eighth {
                // Partial block
                let partial = height_eighths - row_bottom_eighth;
                cell.set_char(BLOCKS[partial - 1]);
            } else {
                // Empty
                cell.set_char(' ');
                continue;
            }

            // Color gradient: bottom rows = start, top rows = end
            let row_ratio = if graph_height > 1 {
                row as f64 / (graph_height - 1) as f64
            } else {
                0.5
            };

            let color = if is_download {
                theme.dl_graph_gradient(row_ratio)
            } else {
                theme.ul_graph_gradient(row_ratio)
            };
            cell.set_fg(color);
        }
    }
}
