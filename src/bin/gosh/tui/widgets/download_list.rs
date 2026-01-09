// Download list widget - currently integrated in ui.rs
// This module can be expanded for more complex list functionality

use gosh_dl::types::DownloadStatus;

pub struct DownloadListState {
    pub selected: usize,
    pub offset: usize,
}

impl DownloadListState {
    pub fn new() -> Self {
        Self {
            selected: 0,
            offset: 0,
        }
    }

    pub fn select_next(&mut self, count: usize) {
        if count > 0 {
            self.selected = (self.selected + 1).min(count - 1);
        }
    }

    pub fn select_prev(&mut self) {
        if self.selected > 0 {
            self.selected -= 1;
        }
    }

    pub fn adjust_scroll(&mut self, visible_height: usize, total_count: usize) {
        // Ensure selected item is visible
        if self.selected < self.offset {
            self.offset = self.selected;
        } else if self.selected >= self.offset + visible_height {
            self.offset = self.selected - visible_height + 1;
        }

        // Ensure offset doesn't go beyond content
        if total_count <= visible_height {
            self.offset = 0;
        } else if self.offset > total_count - visible_height {
            self.offset = total_count - visible_height;
        }
    }
}

impl Default for DownloadListState {
    fn default() -> Self {
        Self::new()
    }
}
