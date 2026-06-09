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
