# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.2.6] - 2026-02-14

### Added

- Two-column dashboard layout with responsive auto-detection (TwoColumn ≥100x24, SingleColumn 80–99, Minimal <80x20)
- btop-style borders with embedded titles on all panels and dialogs
- Gradient progress bars with three-stop color interpolation (red→orange→green) via direct buffer writes
- Network activity graph using Unicode block elements (▁▂▃▄▅▆▇█) with auto-scaling Y-axis and gradient coloring
- Chunk map widget showing parallel download progress per chunk with superpixel downsampling
- Activity log panel with timestamped events, level icons, and elapsed-time display (toggle with `L`)
- Search/filter bar with vim-style `/` trigger, real-time filtering, and scope cycling (`Ctrl+S`: All/Name/URL/State)
- In-TUI settings panel with 5 tabs (General, Network, BitTorrent, Interface, Schedule), boolean toggles, and inline text editing
- Batch import dialog with two phases: multi-line URL input and review/confirm with per-entry validation
- Download queue reordering with `J`/`K` (Shift) keys mapped to engine priority
- Brand bar showing version, download/upload speeds, and download count
- ASCII logo widget in two-column left header
- Standalone tab bar widget with per-tab download counts
- Peak download/upload speed tracking
- Theme gradient functions: `lerp_color`, `progress_gradient`, `dl_graph_gradient`, `ul_graph_gradient`
- `is_tab`, `is_shift_key`, `is_upper_key` key detection helpers

### Changed

- Split monolithic `ui.rs` (741 LOC) into 16 focused widget modules under `tui/widgets/`
- `ui.rs` rewritten as slim layout dispatcher (~230 LOC) with three responsive render paths
- Right column dynamically allocates space between net graph, details/activity log, and chunk map
- Status bar updated with new keybinding hints (S settings, A batch, / search, L log)
- Details panel and download list now use btop-style block borders
- Progress bars replaced from LineGauge to custom gradient bar renderer

## [0.2.5] - 2026-02-14

### Added

- Catppuccin color theme system with Mocha (dark), Macchiato (alt dark), and Latte (light) palettes
- Animated braille spinners for downloading/connecting states (throbber-widgets-tui)
- Toast notifications for download completion and failure events (auto-dismiss after 4s)
- Startup fade-in animation via tachyonfx
- Dimmed background behind modal dialogs and help overlay
- Connection quality indicator bars in details panel (peer-count based)
- Sparkline speed graphs in details panel (download and upload history)
- Scrollbar widget on download list
- Tabs widget for view mode switching (All / Active / Completed)
- Multi-line download items with LineGauge progress bars colored by completion percentage
- Rounded borders on all panels and dialogs
- Unicode state icons (✓ completed, ✗ error, ⏸ paused, ◷ queued, ↑ seeding)
- Styled key badges in status bar
- Config value validation (max_concurrent_downloads, max_connections, refresh_rate_ms, seed_ratio, schedule hours)
- Warning on unrecognized schedule day names in config
- Header validation for `--header` flag (rejects missing colon)
- Resync event on broadcast lag to catch missed completion events

### Changed

- Upgrade gosh-dl engine from 0.2.2 to 0.2.5
- Theme system rewritten from 13-field role-based to 25-field palette-based design
- Cursor tracking in AddUrl dialog uses character indices (UTF-8 safe)
- Page up/down uses actual visible height instead of hardcoded 10
- Help dialog closes on any key press
- Resumed downloads use engine StateChanged events instead of hardcoding Downloading state
- Broadcast Lagged errors trigger full resync instead of being treated as channel-closed
- `format_duration(0)` returns "0:00" instead of "--"
- URL auto-detection improved: rejects common file extensions, requires www. prefix for bare domains
- Deduplicated `parse_speed` and `parse_checksum` into `util.rs`
- `config set` respects `--config` path for both load and save

### Fixed

- TUI panic hook installed before terminal setup (prevents bricked terminal on crash)
- UTF-8 cursor panic in AddUrl dialog on multi-byte input
- Broadcast `RecvError::Lagged` no longer breaks direct mode event loop
- `truncate_str` with `max_len < 3` no longer returns string longer than max_len
- `unreachable!()` replaced with `Ok(())` in command dispatch fallthrough

### Removed

- Dead widget modules: `download_list.rs`, `help_dialog.rs`, `progress_bar.rs`, `speed_graph.rs`
- Duplicate `parse_checksum` in `direct.rs` and `commands/add.rs`
- Duplicate `parse_speed` in `commands/add.rs`

## [0.2.2] - 2026-02-08

### Added

- Shell completions via `gosh completions <shell>` (bash, zsh, fish, elvish, powershell)
- TUI speed graph sparkline in the details panel
- TUI scrolling with PgUp/PgDn for long download lists
- `--color auto|always|never` flag and `NO_COLOR` environment variable support
- `--no-dht`, `--no-pex`, `--no-lpd`, `--max-peers` flags for BitTorrent control
- `--insecure` / `-k` flag to accept invalid TLS certificates (hidden, prints warning)
- `--max-retries` flag to configure retry attempts
- `--proxy` flag and `HTTPS_PROXY`/`HTTP_PROXY`/`ALL_PROXY` environment variable support
- Bandwidth scheduling via `[[schedule.rules]]` in config
- `[tui] show_peers` config option
- TUI feature flag (`default = ["tui"]`) -- build without TUI via `--no-default-features`
- Colored error and warning output (`print_error`, `print_warning`)
- Path traversal sanitization on `--out` filenames
- Test suite with unit and integration tests (assert_cmd + predicates)
- Packaging templates for Homebrew and AUR
- Release profile with thin LTO, symbol stripping, and single codegen unit

### Changed

- Upgrade gosh-dl engine from 0.1.6 to 0.2.2
- MSRV raised to Rust 1.85
- Default max connections per download reduced from 16 to 8
- Consolidated formatting into `format.rs` (format_speed, format_size, format_duration, format_state)
- Switched from `color-eyre` to `anyhow` for error handling
- `direct.rs` returns `Result<i32>` instead of calling `process::exit()` directly
- Store `EventStream` in `EventHandler` struct instead of recreating per call

### Fixed

- UTF-8 truncation panics -- replaced 5 inline truncation sites with safe `truncate_str` in `util.rs`
- Double Ctrl+C race condition -- removed AtomicBool signal handler, kept `tokio::select!`

### Removed

- Unused dependencies: `humansize`, `tokio-stream`, `humantime`, `color-eyre`
- Dead code: `output/json.rs`, `output/progress.rs`, `input/file_reader.rs`
- `types.rs` module (types re-exported at gosh-dl crate root in 0.2.2)

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

- Initial release
- Three usage modes: interactive TUI, direct download (aria2-style), and scriptable subcommands
- HTTP/HTTPS multi-connection segmented downloads with resume and checksum verification
- BitTorrent support: torrent files, magnet links, DHT, PEX, LPD, WebSeeds, encryption, uTP
- TOML configuration file
- JSON output format for scripting
- Cross-platform support (Linux, macOS, Windows)
- Pre-built binaries with musl static linking for Linux

[Unreleased]: https://github.com/goshitsarch-eng/gosh-dl-cli/compare/v0.2.6...HEAD
[0.2.6]: https://github.com/goshitsarch-eng/gosh-dl-cli/compare/v0.2.5...v0.2.6
[0.2.5]: https://github.com/goshitsarch-eng/gosh-dl-cli/compare/v0.2.2...v0.2.5
[0.2.2]: https://github.com/goshitsarch-eng/gosh-dl-cli/compare/v0.1.2...v0.2.2
[0.1.2]: https://github.com/goshitsarch-eng/gosh-dl-cli/compare/v0.1.1...v0.1.2
[0.1.1]: https://github.com/goshitsarch-eng/gosh-dl-cli/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/goshitsarch-eng/gosh-dl-cli/releases/tag/v0.1.0
