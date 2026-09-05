# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.6.3] - 2026-09-05

### Added
- TUI `v` verification and confirmed `V` repair through gosh-dl 0.6.3, running in
  the background with results retained in the activity log.
- Portable F2 batch-import review shortcut and accurate partial import reporting.

### Changed
- Upgrade the gosh-dl engine from 0.5.0 to 0.6.3 and align the CLI version.
- Correct the minimum Rust version to 1.88, required by the existing TUI dependencies.
- `resume` and `resume-all` now run in the foreground until downloads finish.
- `add` without `--wait` saves downloads paused. Mirror `--enqueue` (with the
  retained `--detach` alias) saves a paused job instead of claiming background
  execution. These operations require persistent storage.
- Require at least one HTTP attempt for `--max-retries`; zero attempts cannot
  perform recursive discovery in the engine.

### Fixed
- Queue-only add and mirror operations use the engine’s atomic paused creation
  API, preventing fast downloads from starting or finishing before a later pause.
- `add --wait` returns failed/partial exit codes instead of reporting success
  after failed downloads. Closed event streams and removed downloads cannot
  silently count as success. Foreground torrent downloads continue seeding
  until the requested positive ratio is reached; zero means unlimited seeding.
- Ctrl+C preserves paused resume records in direct downloads, `add --wait`,
  resumed downloads, and foreground mirrors instead of cancelling/removing them.
- Foreground and enqueued mirrors return valid JSON when requested. Empty
  `cancel-all` also returns JSON, and logs go to stderr instead of corrupting stdout.
- Configuration and torrent-info commands no longer initialize the download
  database. `config path` respects the selected config file, and relative
  database paths no longer attempt to create an empty parent directory.
- Speed overflow returns a validation error instead of panicking or wrapping.
  Invalid schedule days/speeds and non-finite seed ratios are rejected.
- TUI refresh ticks survive progress-event floods and redraws are coalesced.
  All error paths restore terminal mode/cursor; Ctrl+C works inside dialogs.
- Invalid add input and unreadable torrent files show errors without exiting.
  Uppercase shortcuts work with Unix and Windows modifier conventions.
- Add/help dialogs are usable at 80×24; settings and batch review scroll selected
  rows into view. Completed view includes seeding downloads.
- Settings save to the selected config path, report failures, retain unsaved
  drafts, reject numeric typos, and apply refresh/theme/graph/peer changes.
- Create missing download directories on startup. Honor configured log files
  and levels; TUI console logs no longer overwrite the interface.
- Label the estimated progress map accurately; show priority and error details.
  Preserve visual list order across refreshes without claiming queue priority changes.

### Release engineering and documentation
- Explicitly use blocking accepted sockets in the loopback HTTP test fixture,
  preventing intermittent Windows mirror-test connection resets.
- Add crates.io Trusted Publishing gated on tests, Rust compatibility, linting,
  source-package verification, and all six platform binary builds.
- Test both default and TUI-free builds on Linux, macOS, and Windows with the
  committed lockfile. Pin the cross-build tool and registry authentication action.
- Attach source/binary packages and SHA-256 checksums to GitHub releases;
  verify matching registry checksums before skipping an already-published version.
- Update install, lifecycle, exit-code, queue, and publishing guidance. Add the
  CLI rollout audit and distinguish archived engine/planning documents.

## [0.5.0] - 2026-06-09

### Added

- **Recursive HTTP mirroring**: new `gosh mirror <URL>` command (wget -r style) built on gosh-dl 0.5.0's recursive engine — flags for `--depth`, `--include`/`--exclude` globs, `--prefix`, `--span-hosts`, `--flatten`, `--overwrite`, `--fail-fast`, `--discovery-concurrency`, plus `--dry-run` (discover-only preview) and `--detach` (fire and forget)
- Mirror job management: `gosh mirror list|status|cancel|remove` with UUID-prefix job resolution
- **Batch commands**: `gosh pause-all`, `gosh resume-all`, and `gosh cancel-all [--delete-files] [-y]` using the engine's new batch APIs, with per-download outcome reporting (succeeded/skipped/failed) in table and JSON formats
- TUI batch keybindings: `Shift+P` pause all, `Shift+R` resume all, `Shift+C` cancel all (with confirmation dialog); outcomes shown as toasts
- TUI: active mirror jobs shown as a compact counter in the top bar, with activity-log entries on completion/failure
- TUI: activity log can now be scrolled with `[` / `]`
- TUI: Tab now visibly highlights the focused right panel
- `general.storage_backend` config option: `sqlite` (default), `file` (one JSON sidecar per download, aria2-style), or `none` (no persistence)

### Changed

- Upgrade gosh-dl engine from 0.3.2 to 0.5.0
- Align gosh-dl-cli crate version to 0.5.0
- **Pausing now also holds queued downloads** (gosh-dl 0.5.0 behavior change): `gosh pause all` / `pause-all` freezes the entire queue instead of letting waiting downloads get promoted into freed slots
- `gosh pause/resume/cancel all` now delegate to the engine's atomic batch APIs
- Subcommands now shut the engine down cleanly before exiting (previously only direct mode and the TUI did)
- Batch and mirror command failures map to documented exit codes (0/1/2/130)
- TUI redraws are coalesced: engine progress events mark state dirty and the screen repaints at most once per refresh interval, instead of once per event — significantly lower CPU with active downloads
- TUI search (`/`) now actually filters the download list (by name, URL, or state, with Ctrl+S scope cycling); Enter commits the filter and releases the keyboard, Esc clears it
- TUI help overlay and status bar now document the full keymap

### Fixed

- **TUI double input on Windows** (#1): key Release events are now filtered out; Windows delivers both Press and Release for every keystroke, so every key previously acted twice
- TUI: `?` (help) now works on platforms that report shifted punctuation with the SHIFT modifier (Windows)
- `gosh add --wait` could hang forever if a small download finished before the event subscription was set up; events are now subscribed before adding, with a periodic reconcile fallback
- Direct mode had the same missed-event race, and could stall after a lagged event stream; both paths now reconcile against engine state every 2 seconds
- Direct mode Ctrl+C no longer tries to cancel downloads that already finished
- TUI: selection no longer jumps to a different download when the list refreshes (selection is preserved by ID)
- TUI: "Download removed" events were logged as "Download added"
- TUI: Max Peers and Seed Ratio settings rows were impossible to edit (misclassified as toggles)
- TUI: settings navigation could scroll past the last row onto phantom rows
- TUI: Esc in settings now reports validation errors as a toast instead of silently discarding edits
- TUI: batch-import dialog could panic when truncating URLs containing multibyte characters
- TUI: Add-download dialog could underflow and panic in very small terminals
- TUI: event loop no longer busy-spins at 100% CPU if stdin closes or the engine event channel shuts down

## [0.3.1] - 2026-03-08

### Changed

- Upgrade gosh-dl engine from 0.3.0 to 0.3.1
- Align gosh-dl-cli crate version to 0.3.1

## [0.3.0] - 2026-03-08

### Changed

- Upgrade gosh-dl engine from 0.2.9 to 0.3.0
- Align gosh-dl-cli crate version to 0.3.0

## [0.2.9] - 2026-03-08

### Changed

- Upgrade gosh-dl engine from 0.2.8 to 0.2.9
- Align gosh-dl-cli crate version to 0.2.9
- Validate runtime configuration after environment and CLI overrides are applied
- `gosh list` table summary now separates filtered rows from global totals
- Correct the documented default config path in CLI help to `~/.config/gosh-dl/config.toml`

### Fixed

- Make `NO_COLOR` and `--color auto|always|never` deterministic by replacing one-shot color state initialization
- Reject invalid `--max-connections 0`, malformed `--select-files`, and negative `--seed-ratio` values in both direct mode and `gosh add`
- Reject existing non-torrent files passed as positional inputs instead of misclassifying them as torrent files
- Tighten implicit URL detection so obvious local paths like `./foo.bar` and `/tmp/foo.bar` are not treated as URLs
- Add `config get/set` support for documented scalar keys: `general.log_file`, `engine.proxy_url`, `engine.connect_timeout`, `engine.read_timeout`, `engine.max_retries`, and `engine.accept_invalid_certs`
- Support `unset` for optional config values in `config set`

## [0.2.8] - 2026-03-07

### Changed

- Upgrade gosh-dl engine from 0.2.7 to 0.2.8
- Upgrade tachyonfx from 0.22 to 0.25 (fixes CubicOut easing calculation)
- Upgrade throbber-widgets-tui from 0.10 to 0.11
- Upgrade toml from 0.9 to 1.0
- Tighten clap version constraint from 4 to 4.5

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

[Unreleased]: https://github.com/goshitsarch-eng/gosh-dl-cli/compare/v0.6.3...HEAD
[0.6.3]: https://github.com/goshitsarch-eng/gosh-dl-cli/compare/v0.5.0...v0.6.3
[0.5.0]: https://github.com/goshitsarch-eng/gosh-dl-cli/compare/v0.3.2...v0.5.0
[0.3.2]: https://github.com/goshitsarch-eng/gosh-dl-cli/compare/v0.3.1...v0.3.2
[0.3.1]: https://github.com/goshitsarch-eng/gosh-dl-cli/compare/v0.3.0...v0.3.1
[0.3.0]: https://github.com/goshitsarch-eng/gosh-dl-cli/compare/v0.2.9...v0.3.0
[0.2.9]: https://github.com/goshitsarch-eng/gosh-dl-cli/compare/v0.2.8...v0.2.9
[0.2.8]: https://github.com/goshitsarch-eng/gosh-dl-cli/compare/v0.2.6...v0.2.8
[0.2.6]: https://github.com/goshitsarch-eng/gosh-dl-cli/compare/v0.2.5...v0.2.6
[0.2.5]: https://github.com/goshitsarch-eng/gosh-dl-cli/compare/v0.2.2...v0.2.5
[0.2.2]: https://github.com/goshitsarch-eng/gosh-dl-cli/compare/v0.1.2...v0.2.2
[0.1.2]: https://github.com/goshitsarch-eng/gosh-dl-cli/compare/v0.1.1...v0.1.2
[0.1.1]: https://github.com/goshitsarch-eng/gosh-dl-cli/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/goshitsarch-eng/gosh-dl-cli/releases/tag/v0.1.0
