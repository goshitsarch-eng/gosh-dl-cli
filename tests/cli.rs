use assert_cmd::Command;
use predicates::prelude::*;

fn gosh() -> Command {
    #[allow(deprecated)]
    Command::cargo_bin("gosh").unwrap()
}

#[test]
fn test_version() {
    gosh()
        .arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains(env!("CARGO_PKG_VERSION")));
}

#[test]
fn test_help() {
    gosh()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("download manager"));
}

#[test]
fn test_completions_bash() {
    gosh()
        .args(["completions", "bash"])
        .assert()
        .success()
        .stdout(predicate::str::is_empty().not());
}

#[test]
fn test_completions_zsh() {
    gosh()
        .args(["completions", "zsh"])
        .assert()
        .success()
        .stdout(predicate::str::is_empty().not());
}

#[test]
fn test_completions_fish() {
    gosh()
        .args(["completions", "fish"])
        .assert()
        .success()
        .stdout(predicate::str::is_empty().not());
}

#[test]
fn test_info_missing_file() {
    gosh()
        .args(["info", "nonexistent.torrent"])
        .assert()
        .failure();
}

#[test]
fn test_no_color_env() {
    gosh().arg("--help").env("NO_COLOR", "1").assert().success();
}

#[test]
fn test_color_never_flag() {
    gosh()
        .args(["--color", "never", "--help"])
        .assert()
        .success();
}

#[test]
fn test_invalid_url() {
    gosh().arg("not-a-url").assert().failure();
}

/// Write an isolated config so tests never touch the user's real database.
/// TOML literal strings (single quotes) keep Windows backslash paths intact —
/// double-quoted strings would treat them as escape sequences.
fn temp_config(dir: &tempfile::TempDir) -> std::path::PathBuf {
    let config_path = dir.path().join("config.toml");
    let contents = format!(
        "[general]\ndownload_dir = '{}'\ndatabase_path = '{}'\n",
        dir.path().display(),
        dir.path().join("gosh.db").display()
    );
    std::fs::write(&config_path, contents).unwrap();
    config_path
}

#[test]
fn test_mirror_help_mentions_flags() {
    gosh()
        .args(["mirror", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("--dry-run"))
        .stdout(predicate::str::contains("--depth"))
        .stdout(predicate::str::contains("--detach"));
}

#[test]
fn test_mirror_requires_url() {
    let dir = tempfile::tempdir().unwrap();
    let config = temp_config(&dir);
    gosh()
        .args(["-c", config.to_str().unwrap(), "mirror"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("URL is required"));
}

#[test]
fn test_mirror_list_empty() {
    let dir = tempfile::tempdir().unwrap();
    let config = temp_config(&dir);
    gosh()
        .args(["-c", config.to_str().unwrap(), "mirror", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("No mirror jobs"));
}

#[test]
fn test_pause_all_empty() {
    let dir = tempfile::tempdir().unwrap();
    let config = temp_config(&dir);
    gosh()
        .args(["-c", config.to_str().unwrap(), "pause-all"])
        .assert()
        .success()
        .stdout(predicate::str::contains("No downloads to pause"));
}

#[test]
fn test_resume_all_empty() {
    let dir = tempfile::tempdir().unwrap();
    let config = temp_config(&dir);
    gosh()
        .args(["-c", config.to_str().unwrap(), "resume-all"])
        .assert()
        .success()
        .stdout(predicate::str::contains("No downloads to resume"));
}

#[test]
fn test_cancel_all_empty() {
    let dir = tempfile::tempdir().unwrap();
    let config = temp_config(&dir);
    gosh()
        .args(["-c", config.to_str().unwrap(), "cancel-all", "-y"])
        .assert()
        .success()
        .stdout(predicate::str::contains("No downloads to cancel"));
}

#[test]
fn test_pause_all_json_output() {
    let dir = tempfile::tempdir().unwrap();
    let config = temp_config(&dir);
    gosh()
        .args([
            "-c",
            config.to_str().unwrap(),
            "--output",
            "json",
            "pause-all",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"succeeded\":[]"));
}

#[test]
fn test_config_storage_backend_round_trip() {
    let dir = tempfile::tempdir().unwrap();
    let config = temp_config(&dir);
    gosh()
        .args([
            "-c",
            config.to_str().unwrap(),
            "config",
            "set",
            "general.storage_backend",
            "file",
        ])
        .assert()
        .success();
    gosh()
        .args([
            "-c",
            config.to_str().unwrap(),
            "config",
            "get",
            "general.storage_backend",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("file"));
}

#[test]
fn test_config_storage_backend_rejects_invalid() {
    let dir = tempfile::tempdir().unwrap();
    let config = temp_config(&dir);
    gosh()
        .args([
            "-c",
            config.to_str().unwrap(),
            "config",
            "set",
            "general.storage_backend",
            "redis",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Invalid storage backend"));
}

struct HttpFixture {
    url: String,
    stop: std::sync::Arc<std::sync::atomic::AtomicBool>,
    started: std::sync::Arc<std::sync::atomic::AtomicBool>,
    file_requests: std::sync::Arc<std::sync::atomic::AtomicUsize>,
    worker: Option<std::thread::JoinHandle<()>>,
}

impl HttpFixture {
    fn new() -> Self {
        use std::io::{Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        listener.set_nonblocking(true).unwrap();
        let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let done = stop.clone();
        let started = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let request_started = started.clone();
        let file_requests = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let observed_files = file_requests.clone();
        let worker = std::thread::spawn(move || {
            while !done.load(std::sync::atomic::Ordering::Relaxed) {
                let (mut stream, _) = match listener.accept() {
                    Ok(connection) => connection,
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        std::thread::sleep(std::time::Duration::from_millis(5));
                        continue;
                    }
                    Err(e) => panic!("fixture accept: {e}"),
                };
                stream
                    .set_read_timeout(Some(std::time::Duration::from_secs(2)))
                    .unwrap();
                let mut request = Vec::new();
                let mut buf = [0; 1024];
                while !request.windows(4).any(|s| s == b"\r\n\r\n") {
                    match stream.read(&mut buf) {
                        Ok(0) | Err(_) => break,
                        Ok(n) => request.extend_from_slice(&buf[..n]),
                    }
                }
                let request = String::from_utf8_lossy(&request);
                let path = request.split_whitespace().nth(1).unwrap_or("");
                if path != "/files/" {
                    observed_files.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                }
                if path == "/slow.bin" {
                    let _ = stream.write_all(
                        b"HTTP/1.1 200 OK\r\nContent-Length: 65536\r\nConnection: close\r\n\r\n",
                    );
                    if !request.starts_with("HEAD ") {
                        request_started.store(true, std::sync::atomic::Ordering::Relaxed);
                        for _ in 0..64 {
                            if done.load(std::sync::atomic::Ordering::Relaxed)
                                || stream.write_all(&[b'x'; 1024]).is_err()
                            {
                                break;
                            }
                            std::thread::sleep(std::time::Duration::from_millis(20));
                        }
                    }
                    continue;
                }
                let (status, body) = match path {
                    "/missing" => ("404 Not Found", "missing"),
                    "/files/" => ("200 OK", "<html><a href=\"file.bin\">file</a></html>"),
                    _ => ("200 OK", "verified CLI download\n"),
                };
                let content_type = if path == "/files/" {
                    "text/html"
                } else {
                    "application/octet-stream"
                };
                let header = format!("HTTP/1.1 {status}\r\nContent-Length: {}\r\nContent-Type: {content_type}\r\nConnection: close\r\n\r\n", body.len());
                let _ = stream.write_all(header.as_bytes());
                if !request.starts_with("HEAD ") {
                    let _ = stream.write_all(body.as_bytes());
                }
            }
        });
        Self {
            url,
            stop,
            started,
            file_requests,
            worker: Some(worker),
        }
    }
}

impl Drop for HttpFixture {
    fn drop(&mut self) {
        self.stop.store(true, std::sync::atomic::Ordering::Relaxed);
        self.worker.take().unwrap().join().unwrap();
    }
}

fn isolated_gosh(config: &std::path::Path) -> Command {
    let mut cmd = gosh();
    cmd.args([
        "-c",
        config.to_str().unwrap(),
        "--color",
        "never",
        "--max-retries",
        "1",
    ])
    .timeout(std::time::Duration::from_secs(20));
    for key in [
        "HTTP_PROXY",
        "HTTPS_PROXY",
        "ALL_PROXY",
        "http_proxy",
        "https_proxy",
        "all_proxy",
    ] {
        cmd.env_remove(key);
    }
    cmd.env("NO_PROXY", "127.0.0.1");
    cmd
}

#[test]
fn regression_wait_reports_download_failure() {
    let fixture = HttpFixture::new();
    let dir = tempfile::tempdir().unwrap();
    let config = temp_config(&dir);
    isolated_gosh(&config)
        .args(["add", "--wait", &format!("{}/missing", fixture.url)])
        .assert()
        .code(2);
}

#[test]
fn regression_add_wait_downloads_nested_output() {
    let fixture = HttpFixture::new();
    let dir = tempfile::tempdir().unwrap();
    let config = temp_config(&dir);
    let result = isolated_gosh(&config)
        .args([
            "--output",
            "json",
            "add",
            "--wait",
            "-o",
            "nested/file.bin",
            &format!("{}/file.bin", fixture.url),
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    assert!(serde_json::from_slice::<serde_json::Value>(&result)
        .unwrap()
        .is_array());
    assert_eq!(
        std::fs::read(dir.path().join("nested/file.bin")).unwrap(),
        b"verified CLI download\n"
    );
}

#[test]
fn regression_resume_waits_for_completion() {
    let fixture = HttpFixture::new();
    let dir = tempfile::tempdir().unwrap();
    let config = temp_config(&dir);
    isolated_gosh(&config)
        .args(["add", &format!("{}/file.bin", fixture.url)])
        .assert()
        .success();
    isolated_gosh(&config)
        .args(["resume", "all"])
        .assert()
        .success();
    let output = isolated_gosh(&config)
        .args(["--output", "json", "list"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let entries: serde_json::Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(
        entries[0]["state"]["state"], "completed",
        "resume exited before completion: {entries}"
    );
    assert_eq!(
        std::fs::read(dir.path().join("file.bin")).unwrap(),
        b"verified CLI download\n"
    );
}

#[test]
fn regression_empty_cancel_all_is_json() {
    let dir = tempfile::tempdir().unwrap();
    let config = temp_config(&dir);
    let output = isolated_gosh(&config)
        .args(["--output", "json", "cancel-all", "-y"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    assert!(serde_json::from_slice::<serde_json::Value>(&output).unwrap()["succeeded"].is_array());
}

#[test]
fn regression_config_path_honors_selected_file() {
    let dir = tempfile::tempdir().unwrap();
    let config = temp_config(&dir);
    isolated_gosh(&config)
        .args(["config", "path"])
        .assert()
        .success()
        .stdout(predicate::str::contains(config.to_str().unwrap()));
}

#[test]
fn regression_config_does_not_initialize_database() {
    let dir = tempfile::tempdir().unwrap();
    let config = temp_config(&dir);
    let db = dir.path().join("gosh.db");
    std::fs::create_dir(&db).unwrap(); // an invalid SQLite path must not affect config commands
    isolated_gosh(&config)
        .args(["config", "get", "general.storage_backend"])
        .assert()
        .success();
}

#[test]
fn regression_speed_overflow_is_error_not_panic() {
    let dir = tempfile::tempdir().unwrap();
    let config = temp_config(&dir);
    isolated_gosh(&config)
        .args([
            "--max-speed",
            "18446744073709551615G",
            "http://127.0.0.1/file",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("panicked").not())
        .stderr(predicate::str::contains("too large"));
}

#[test]
fn regression_mirror_foreground_is_json() {
    let fixture = HttpFixture::new();
    let dir = tempfile::tempdir().unwrap();
    let config = temp_config(&dir);
    let output = isolated_gosh(&config)
        .args([
            "--output",
            "json",
            "mirror",
            &format!("{}/files/", fixture.url),
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    assert!(serde_json::from_slice::<serde_json::Value>(&output)
        .unwrap()
        .is_object());
}

#[test]
fn regression_enqueue_requires_persistence() {
    let dir = tempfile::tempdir().unwrap();
    let config = temp_config(&dir);
    std::fs::write(&config, "[general]\nstorage_backend = 'none'\n").unwrap();
    isolated_gosh(&config)
        .args(["add", "http://127.0.0.1/file"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("requires persistent storage"));
    isolated_gosh(&config)
        .args(["mirror", "--enqueue", "http://127.0.0.1/files/"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("requires persistent storage"));
}

#[test]
fn regression_mirror_enqueue_saves_paused_children() {
    let fixture = HttpFixture::new();
    let dir = tempfile::tempdir().unwrap();
    let config = temp_config(&dir);
    let output = isolated_gosh(&config)
        .args([
            "--output",
            "json",
            "mirror",
            "--enqueue",
            &format!("{}/files/", fixture.url),
        ])
        .assert()
        .success()
        .stderr(predicate::str::contains("no background process"))
        .get_output()
        .stdout
        .clone();
    let result: serde_json::Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(result["status"]["progress"]["paused_children"], 1);
    assert_eq!(
        fixture
            .file_requests
            .load(std::sync::atomic::Ordering::Relaxed),
        0,
        "queued mirror must not probe or transfer child files"
    );
    isolated_gosh(&config)
        .args(["resume-all"])
        .assert()
        .success();
    assert_eq!(
        std::fs::read(dir.path().join("file.bin")).unwrap(),
        b"verified CLI download\n"
    );
}

#[test]
fn regression_invalid_schedule_is_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let config = temp_config(&dir);
    for rule in ["days = 'funday'", "days = 'all'\ndownload_limit = 'oops'"] {
        std::fs::write(
            &config,
            format!("[[schedule.rules]]\nstart_hour = 1\nend_hour = 2\n{rule}\n"),
        )
        .unwrap();
        isolated_gosh(&config).args(["list"]).assert().failure();
    }
}

#[test]
fn regression_zero_attempts_is_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let config = temp_config(&dir);
    gosh()
        .args(["-c", config.to_str().unwrap(), "--max-retries", "0", "list"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("max_retries must be between 1"));
}

#[cfg(unix)]
fn interrupt_preserves_download(mode: &[&str]) {
    use std::time::{Duration, Instant};
    let fixture = HttpFixture::new();
    let dir = tempfile::tempdir().unwrap();
    let config = temp_config(&dir);
    let mut cmd = std::process::Command::new(env!("CARGO_BIN_EXE_gosh"));
    cmd.args([
        "-c",
        config.to_str().unwrap(),
        "--color",
        "never",
        "--max-retries",
        "1",
    ])
    .args(mode)
    .arg(format!("{}/slow.bin", fixture.url))
    .stdout(std::process::Stdio::null())
    .stderr(std::process::Stdio::null());
    for key in [
        "HTTP_PROXY",
        "HTTPS_PROXY",
        "ALL_PROXY",
        "http_proxy",
        "https_proxy",
        "all_proxy",
    ] {
        cmd.env_remove(key);
    }
    let mut child = cmd.spawn().unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    while !fixture.started.load(std::sync::atomic::Ordering::Relaxed) {
        if Instant::now() > deadline {
            child.kill().ok();
            child.wait().ok();
            panic!("download did not start");
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    std::thread::sleep(Duration::from_millis(100));
    assert!(std::process::Command::new("kill")
        .args(["-INT", &child.id().to_string()])
        .status()
        .unwrap()
        .success());
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break status;
        }
        if Instant::now() > deadline {
            child.kill().ok();
            child.wait().ok();
            panic!("interrupt did not stop the command");
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    assert_eq!(status.code(), Some(130));
    let output = isolated_gosh(&config)
        .args(["--output", "json", "list"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let result: serde_json::Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(result[0]["state"]["state"], "paused");
}

#[test]
#[cfg(unix)]
fn regression_direct_interrupt_preserves_resume_record() {
    interrupt_preserves_download(&[]);
}

#[test]
#[cfg(unix)]
fn regression_add_wait_interrupt_preserves_resume_record() {
    interrupt_preserves_download(&["add", "--wait"]);
}

#[test]
fn regression_queue_add_does_not_start_any_fast_file() {
    let fixture = HttpFixture::new();
    let dir = tempfile::tempdir().unwrap();
    let config = temp_config(&dir);
    let mut command = isolated_gosh(&config);
    command.arg("add");
    for i in 0..30 {
        command.arg(format!("{}/file-{i}.bin", fixture.url));
    }
    command.assert().success();
    assert_eq!(
        fixture
            .file_requests
            .load(std::sync::atomic::Ordering::Relaxed),
        0
    );
    let output = isolated_gosh(&config)
        .args(["--output", "json", "list"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let entries: serde_json::Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(entries.as_array().unwrap().len(), 30);
    assert!(entries
        .as_array()
        .unwrap()
        .iter()
        .all(|d| d["state"]["state"] == "paused"));
}

#[cfg(unix)]
#[test]
fn regression_direct_torrent_waits_for_requested_seed_ratio() {
    use std::process::{Command as ProcessCommand, Stdio};
    let dir = tempfile::tempdir().unwrap();
    let config = temp_config(&dir);
    std::fs::write(dir.path().join("seed.bin"), b"test").unwrap();
    let mut torrent =
        b"d4:infod6:lengthi4e4:name8:seed.bin12:piece lengthi16384e6:pieces20:".to_vec();
    torrent.extend(hex::decode("a94a8fe5ccb19ba61c4c0873d391e987982fbbd3").unwrap());
    torrent.extend(b"ee");
    let torrent_path = dir.path().join("seed.torrent");
    std::fs::write(&torrent_path, torrent).unwrap();
    let mut command = ProcessCommand::new(env!("CARGO_BIN_EXE_gosh"));
    command
        .arg("--config")
        .arg(&config)
        .args([
            "--color",
            "never",
            "--no-dht",
            "--no-pex",
            "--no-lpd",
            "--seed-ratio",
            "1",
        ])
        .arg(&torrent_path)
        .stdout(Stdio::null())
        .stderr(Stdio::piped());
    let mut child = command.spawn().unwrap();
    std::thread::sleep(std::time::Duration::from_secs(3));
    assert!(
        child.try_wait().unwrap().is_none(),
        "must keep seeding until ratio is reached"
    );
    assert!(ProcessCommand::new("kill")
        .args(["-INT", &child.id().to_string()])
        .stdout(Stdio::null())
        .status()
        .unwrap()
        .success());
    let output = child.wait_with_output().unwrap();
    assert_eq!(output.status.code(), Some(130));
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("Verified 1 existing pieces"),
        "fixture must have reached seeding: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}
