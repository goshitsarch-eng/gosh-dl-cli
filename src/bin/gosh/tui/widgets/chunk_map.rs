use ratatui::prelude::*;

use super::btop_border::btop_block;
use crate::tui::app::{ChunkState, TuiApp};

pub fn render_chunk_map(frame: &mut Frame, area: Rect, app: &TuiApp) {
    let theme = app.theme();
    let block = btop_block(&format!("Chunks ({})", app.chunk_count), theme, false);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if app.chunk_states.is_empty() {
        return;
    }

    let cols = inner.width as usize;
    let rows = inner.height as usize;
    let cells = cols * rows;

    if cells == 0 {
        return;
    }

    // Superpixel downsampling
    let chunks_per_cell = app.chunk_count.div_ceil(cells);

    let buf = frame.buffer_mut();
    for row in 0..rows {
        for col in 0..cols {
            let cell_idx = row * cols + col;
            let chunk_start = cell_idx * chunks_per_cell;
            let chunk_end = ((cell_idx + 1) * chunks_per_cell).min(app.chunk_count);

            if chunk_start >= app.chunk_count {
                break;
            }

            let state = majority_state(&app.chunk_states[chunk_start..chunk_end]);
            let (symbol, color) = match state {
                ChunkState::Pending => ("\u{2591}", theme.surface1),
                ChunkState::Downloading => ("\u{2588}", theme.teal),
                ChunkState::Complete => ("\u{2588}", theme.success),
                ChunkState::Failed => ("\u{2588}", theme.error),
            };

            let x = inner.x + col as u16;
            let y = inner.y + row as u16;
            if x < inner.x + inner.width && y < inner.y + inner.height {
                let cell = &mut buf[(x, y)];
                cell.set_symbol(symbol);
                cell.set_fg(color);
            }
        }
    }
}

fn majority_state(states: &[ChunkState]) -> ChunkState {
    if states.is_empty() {
        return ChunkState::Pending;
    }
    // Priority: Failed > Downloading > Complete > Pending
    if states.contains(&ChunkState::Failed) {
        return ChunkState::Failed;
    }
    if states.contains(&ChunkState::Downloading) {
        return ChunkState::Downloading;
    }
    let complete = states.iter().filter(|s| **s == ChunkState::Complete).count();
    if complete > states.len() / 2 {
        ChunkState::Complete
    } else {
        ChunkState::Pending
    }
}
