# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.2] - 2026-01-24

### Changed

- Upgrade gosh-dl engine from 0.1.5 to 0.1.6
- Upgrade ratatui from 0.28 to 0.30
- Upgrade crossterm from 0.28 to 0.29
- Upgrade indicatif from 0.17 to 0.18
- Upgrade directories from 5 to 6
- Upgrade dirs from 5 to 6
- Upgrade toml from 0.8 to 0.9
- Upgrade anyhow to 1.0.100

### Fixed

- Replace deprecated `Block::title_style()` with styled `Line` titles for ratatui 0.29+ compatibility

## [0.1.1] - 2026-01-12

### Changed

- Upgrade gosh-dl engine from 0.1.3 to 0.1.5

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

[Unreleased]: https://github.com/goshitsarch-eng/gosh-dl-cli/compare/v0.1.2...HEAD
[0.1.2]: https://github.com/goshitsarch-eng/gosh-dl-cli/compare/v0.1.1...v0.1.2
[0.1.1]: https://github.com/goshitsarch-eng/gosh-dl-cli/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/goshitsarch-eng/gosh-dl-cli/releases/tag/v0.1.0
