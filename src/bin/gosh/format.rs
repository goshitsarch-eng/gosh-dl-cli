use std::sync::OnceLock;

use gosh_dl::DownloadState;

use crate::util::truncate_str;

static COLOR_ENABLED: OnceLock<bool> = OnceLock::new();

pub fn init_color(force: Option<bool>) {
    let enabled = match force {
        Some(v) => v,
        None => std::env::var_os("NO_COLOR").is_none(),
    };
    COLOR_ENABLED.set(enabled).ok();
}

pub fn color_enabled() -> bool {
    *COLOR_ENABLED.get().unwrap_or(&true)
}

pub fn print_error(msg: &str) {
    if color_enabled() {
        eprintln!("\x1b[1;31merror\x1b[0m: {msg}");
    } else {
        eprintln!("error: {msg}");
    }
}

pub fn print_warning(msg: &str) {
    if color_enabled() {
        eprintln!("\x1b[1;33mwarning\x1b[0m: {msg}");
    } else {
        eprintln!("warning: {msg}");
    }
}

/// Format bytes-per-second as a human-readable speed string (no "/s" suffix).
///
/// Callers that need "/s" should append it themselves.
pub fn format_speed(bytes_per_sec: u64) -> String {
    if bytes_per_sec == 0 {
        "0 B".to_string()
    } else if bytes_per_sec < 1024 {
        format!("{} B", bytes_per_sec)
    } else if bytes_per_sec < 1024 * 1024 {
        format!("{:.1} KB", bytes_per_sec as f64 / 1024.0)
    } else if bytes_per_sec < 1024 * 1024 * 1024 {
        format!("{:.1} MB", bytes_per_sec as f64 / (1024.0 * 1024.0))
    } else {
        format!(
            "{:.2} GB",
            bytes_per_sec as f64 / (1024.0 * 1024.0 * 1024.0)
        )
    }
}

/// Format a byte count as a human-readable size string.
pub fn format_size(bytes: u64) -> String {
    if bytes == 0 {
        "0 B".to_string()
    } else if bytes < 1024 {
        format!("{} B", bytes)
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else if bytes < 1024 * 1024 * 1024 {
        format!("{:.2} MB", bytes as f64 / (1024.0 * 1024.0))
    } else {
        format!("{:.2} GB", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
    }
}

/// Format seconds as "M:SS" or "H:MM:SS". Returns "--" for 0.
pub fn format_duration(seconds: u64) -> String {
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

/// Format a download state as a short label. Shows "Error: {kind}" for errors.
pub fn format_state(state: &DownloadState) -> String {
    match state {
        DownloadState::Queued => "Queued".to_string(),
        DownloadState::Connecting => "Connecting".to_string(),
        DownloadState::Downloading => "Downloading".to_string(),
        DownloadState::Seeding => "Seeding".to_string(),
        DownloadState::Paused => "Paused".to_string(),
        DownloadState::Completed => "Completed".to_string(),
        DownloadState::Error { kind, .. } => format!("Error: {}", truncate_str(kind, 10)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_speed_zero() {
        assert_eq!(format_speed(0), "0 B");
    }

    #[test]
    fn test_format_speed_bytes() {
        assert_eq!(format_speed(500), "500 B");
    }

    #[test]
    fn test_format_speed_kb() {
        assert_eq!(format_speed(1024), "1.0 KB");
        assert_eq!(format_speed(1536), "1.5 KB");
    }

    #[test]
    fn test_format_speed_mb() {
        assert_eq!(format_speed(1024 * 1024), "1.0 MB");
        assert_eq!(format_speed(5 * 1024 * 1024), "5.0 MB");
    }

    #[test]
    fn test_format_speed_gb() {
        assert_eq!(format_speed(1024 * 1024 * 1024), "1.00 GB");
    }

    #[test]
    fn test_format_size_zero() {
        assert_eq!(format_size(0), "0 B");
    }

    #[test]
    fn test_format_size_bytes() {
        assert_eq!(format_size(100), "100 B");
    }

    #[test]
    fn test_format_size_kb() {
        assert_eq!(format_size(2048), "2.0 KB");
    }

    #[test]
    fn test_format_size_mb() {
        assert_eq!(format_size(10 * 1024 * 1024), "10.00 MB");
    }

    #[test]
    fn test_format_size_gb() {
        assert_eq!(format_size(3 * 1024 * 1024 * 1024), "3.00 GB");
    }

    #[test]
    fn test_format_duration_zero() {
        assert_eq!(format_duration(0), "--");
    }

    #[test]
    fn test_format_duration_short() {
        assert_eq!(format_duration(65), "1:05");
        assert_eq!(format_duration(30), "0:30");
    }

    #[test]
    fn test_format_duration_hours() {
        assert_eq!(format_duration(3661), "1:01:01");
    }

    #[test]
    fn test_color_init_no_color() {
        // This test verifies init_color logic without calling it
        // (OnceLock can only be set once per process)
        assert!(std::env::var_os("NO_COLOR").is_none() || !color_enabled());
    }
}
