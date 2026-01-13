# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0] - 2026-01-09

### Added

- Initial release of gosh-dl-cli
- Three usage modes:
  - **TUI Mode**: Interactive terminal UI with download list, progress bars, and speed graph
  - **Direct Mode**: aria2-style command-line downloads with progress bars
  - **Command Mode**: Scriptable subcommands (add, list, status, pause, resume, cancel, priority, stats, info, config)
- HTTP/HTTPS download support via gosh-dl engine:
  - Multi-connection segmented downloads (up to 16 parallel connections)
  - Automatic resume with ETag/Last-Modified validation
  - Checksum verification (MD5, SHA256)
  - Custom headers, user agent, referer, and cookies
  - Speed limiting
- BitTorrent support via gosh-dl engine:
  - Torrent files and magnet links
  - DHT, PEX, and Local Peer Discovery
  - WebSeeds (BEP 17/19)
  - Sequential download mode for streaming
  - Selective file downloading
  - Seed ratio limiting
- Configuration file support (TOML format)
- JSON output format for scripting
- Cross-platform support (Linux, macOS, Windows)
- GitHub Actions CI/CD workflows
- Pre-built binaries (statically linked musl for Linux)

[Unreleased]: https://github.com/goshitsarch-eng/gosh-dl-cli/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/goshitsarch-eng/gosh-dl-cli/releases/tag/v0.1.0
