use std::sync::atomic::{AtomicU8, Ordering};

use gosh_dl::DownloadState;

use crate::util::truncate_str;

const COLOR_AUTO: u8 = 0;
const COLOR_ENABLED: u8 = 1;
const COLOR_DISABLED: u8 = 2;

static COLOR_MODE: AtomicU8 = AtomicU8::new(COLOR_AUTO);

pub fn init_color(force: Option<bool>) {
    let mode = match force {
        Some(true) => COLOR_ENABLED,
        Some(false) => COLOR_DISABLED,
        None => COLOR_AUTO,
    };
    COLOR_MODE.store(mode, Ordering::Relaxed);
}

pub fn color_enabled() -> bool {
    match COLOR_MODE.load(Ordering::Relaxed) {
        COLOR_ENABLED => true,
        COLOR_DISABLED => false,
        _ => std::env::var_os("NO_COLOR").is_none(),
    }
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

/// Format seconds as "M:SS" or "H:MM:SS".
pub fn format_duration(seconds: u64) -> String {
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
        DownloadState::Error { kind, .. } => format!("Error: {}", error_kind_label(kind)),
    }
}

/// A short category for an engine error.
///
/// The engine stores the error's `Debug` rendering as its "kind", e.g.
/// `Network { kind: HttpStatus(404), message: "...", retryable: false }`.
/// Cutting that to ten characters produced labels like `Network { `; this
/// keeps the variant name and, for HTTP failures, the status code.
pub fn error_kind_label(kind: &str) -> String {
    if let Some(rest) = kind.split("HttpStatus(").nth(1) {
        let code: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
        if !code.is_empty() {
            return format!("HTTP {code}");
        }
    }
    let variant: String = kind
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric())
        .collect();
    if variant.is_empty() {
        truncate_str(kind, 16)
    } else {
        variant
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Mutex, OnceLock};

    fn env_lock() -> std::sync::MutexGuard<'static, ()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(())).lock().unwrap()
    }

    #[test]
    fn error_labels_are_readable() {
        assert_eq!(
            error_kind_label(
                "Network { kind: HttpStatus(404), message: \"HTTP error: 404\", retryable: false }"
            ),
            "HTTP 404"
        );
        assert_eq!(
            error_kind_label("Storage { kind: AlreadyExists, path: \"/x\", message: \"..\" }"),
            "Storage"
        );
        assert_eq!(error_kind_label("RecursiveFailFast"), "RecursiveFailFast");
        assert_eq!(error_kind_label(""), "");
    }

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
        assert_eq!(format_duration(0), "0:00");
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
    fn test_color_forced_modes() {
        let _guard = env_lock();
        unsafe {
            std::env::remove_var("NO_COLOR");
        }

        init_color(Some(true));
        assert!(color_enabled());

        init_color(Some(false));
        assert!(!color_enabled());
    }

    #[test]
    fn test_color_auto_honors_no_color() {
        let _guard = env_lock();
        unsafe {
            std::env::set_var("NO_COLOR", "1");
        }
        init_color(None);
        assert!(!color_enabled());

        unsafe {
            std::env::remove_var("NO_COLOR");
        }
        init_color(None);
        assert!(color_enabled());
    }
}
