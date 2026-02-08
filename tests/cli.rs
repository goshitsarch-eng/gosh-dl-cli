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
    gosh()
        .arg("--help")
        .env("NO_COLOR", "1")
        .assert()
        .success();
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
    gosh()
        .arg("not-a-url")
        .assert()
        .failure();
}
