# gosh-dl-cli

[![Crates.io](https://img.shields.io/crates/v/gosh-dl-cli)](https://crates.io/crates/gosh-dl-cli)
[![docs.rs](https://img.shields.io/docsrs/gosh-dl-cli)](https://docs.rs/gosh-dl-cli)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

A download manager for the terminal. HTTP/HTTPS with multi-connection acceleration, full BitTorrent support, and an optional TUI. Built on [gosh-dl](https://github.com/goshitsarch-eng/gosh-dl).

## Features

- **HTTP/HTTPS downloads** with multi-connection acceleration, resume, retries, mirrors, checksum verification, and speed limits
- **Full BitTorrent support** -- torrents and magnet links with DHT, PEX, LPD, sequential mode, file selection, and seed ratio control
- **Recursive HTTP mirroring** (`gosh mirror`) -- crawl a directory listing and download everything under it, wget -r style, with depth limits, include/exclude globs, dry-run previews, and detached background jobs
- **Three usage modes** -- aria2-style direct downloads with progress bars, a full-screen TUI, and scriptable subcommands with JSON output
- **Batch operations** -- `pause-all`, `resume-all`, and `cancel-all` (CLI and TUI) with per-download outcome reporting
- **Interactive TUI** -- live speed graphs, chunk visualization, search/filtering, activity log, settings editor, batch URL import, and mirror job tracking
- **Persistent queue** -- downloads survive restarts via SQLite (default), aria2-style JSON sidecar files, or no persistence at all (`storage_backend`)
- **Bandwidth scheduling** -- time-of-day and day-of-week speed limit rules
- Cross-platform: Linux, macOS, and Windows

### What's new in 0.5.0

Built on gosh-dl 0.5.0: the new `gosh mirror` command with job management, batch pause/resume/cancel commands and TUI keybindings (`P`/`R`/`C`), pluggable storage backends, a working TUI search filter, coalesced TUI redraws (much lower CPU), and a fix for doubled keystrokes in the TUI on Windows. Pausing now also holds queued downloads. See the [CHANGELOG](CHANGELOG.md) for the full list.

## Screenshots

![Screenshot 1](screenshots/img1.png)

## Install

From source (requires Rust 1.85+):

```bash
git clone https://github.com/goshitsarch-eng/gosh-dl-cli
cd gosh-dl-cli
cargo build --release
cp target/release/gosh ~/.local/bin/   # or /usr/local/bin/
```

From crates.io:

```bash
cargo install gosh-dl-cli
```

Arch Linux (AUR):

```bash
yay -S gosh-dl-cli
```

Without the TUI (smaller binary, fewer dependencies):

```bash
cargo install gosh-dl-cli --no-default-features
```

Pre-built binaries are available on [GitHub Releases](https://github.com/goshitsarch-eng/gosh-dl-cli/releases). Linux builds are statically linked with musl.

## Quick start

Download a file:

```bash
gosh https://example.com/file.zip
```

Download to a specific directory with a custom filename:

```bash
gosh -d ~/Downloads -o archive.zip https://example.com/file.zip
```

Multiple files at once:

```bash
gosh https://example.com/a.zip https://example.com/b.zip
```

Torrents and magnet links work the same way:

```bash
gosh magnet:?xt=urn:btih:...
gosh ./ubuntu.torrent
```

Mirror an HTTP directory listing recursively (wget -r style):

```bash
gosh mirror https://ftp.gnu.org/gnu/hello/
```

Launch the interactive TUI by running `gosh` with no arguments.

## Usage modes

gosh has three modes:

**Direct mode** -- pass URLs as arguments and downloads start immediately with progress bars. This is the aria2-style workflow most people want.

**TUI mode** -- run `gosh` with no arguments for a full-screen terminal interface. You can add, pause, resume, and monitor downloads interactively.

**Command mode** -- use subcommands (`gosh add`, `gosh list`, etc.) for scripting and automation.

## CLI reference

### Global options

These work with any mode or subcommand:

| Flag | Description |
|------|-------------|
| `-c, --config <PATH>` | Config file path (env: `GOSH_CONFIG`) |
| `-v, --verbose` | Increase log verbosity (`-v`, `-vv`, `-vvv`) |
| `-q, --quiet` | Suppress output except errors |
| `--output <FORMAT>` | Output format: `table`, `json`, `json-pretty` |
| `--color <WHEN>` | Color output: `auto`, `always`, `never` |
| `--proxy <URL>` | Proxy URL (`http://`, `https://`, `socks5://`) |
| `--max-retries <N>` | Max retry attempts for failed downloads |

### Direct mode options

Used when passing URLs directly (`gosh [OPTIONS] <URL>...`):

| Flag | Description |
|------|-------------|
| `-d, --dir <PATH>` | Output directory |
| `-o, --out <NAME>` | Output filename (single download only) |
| `-x, --max-connections <N>` | Connections per download (default: 8) |
| `--max-speed <SPEED>` | Speed limit (supports `K`/`M`/`G` suffixes) |
| `-H, --header <HEADER>` | Custom header (`"Name: Value"`) |
| `--user-agent <UA>` | User agent string |
| `--referer <URL>` | Referer URL |
| `--cookie <COOKIE>` | Cookie (`"name=value"`) |
| `--checksum <HASH>` | Verify checksum (`md5:...` or `sha256:...`) |
| `--sequential` | Download pieces in order (torrents) |
| `--select-files <IDX>` | Download specific files (comma-separated, torrents) |
| `--seed-ratio <RATIO>` | Stop seeding after this ratio (torrents) |
| `--no-dht` | Disable DHT |
| `--no-pex` | Disable Peer Exchange |
| `--no-lpd` | Disable Local Peer Discovery |
| `--max-peers <N>` | Max peers per torrent |

### Subcommands

**`gosh add <URL>...`** -- Add downloads to the queue.

Accepts all the direct mode options above, plus:

| Flag | Description |
|------|-------------|
| `-p, --priority <LEVEL>` | `low`, `normal`, `high`, `critical` |
| `-w, --wait` | Block until download completes |
| `-i, --input-file <FILE>` | Read URLs from a file (one per line) |

**`gosh list`** -- List all downloads.

| Flag | Description |
|------|-------------|
| `-s, --state <STATE>` | Filter: `active`, `waiting`, `paused`, `completed`, `error` |
| `--ids-only` | Print only download IDs |

**`gosh status <ID>`** -- Show detailed status of a download.

| Flag | Description |
|------|-------------|
| `--peers` | Show peer info (torrents) |
| `--files` | Show file list (torrents) |

**`gosh pause <ID>...`** -- Pause downloads. Use `all` to pause everything.

**`gosh resume <ID>...`** -- Resume paused downloads. Use `all` to resume everything.

**`gosh cancel <ID>...`** -- Cancel downloads.

| Flag | Description |
|------|-------------|
| `--delete` | Also delete downloaded files |
| `-y, --yes` | Skip confirmation |

**`gosh pause-all`** / **`gosh resume-all`** / **`gosh cancel-all`** -- Batch operations across every download, reporting per-download outcomes (succeeded / skipped / failed).

| Flag (`cancel-all`) | Description |
|------|-------------|
| `--delete-files` | Also delete downloaded files |
| `-y, --yes` | Skip confirmation |

> **Note:** as of gosh-dl 0.5.0, pausing also holds *queued* downloads, so `pause all` / `pause-all` freezes the whole queue instead of letting waiting downloads get promoted into freed slots.

**`gosh mirror <URL>`** -- Recursively mirror an HTTP/HTTPS directory listing (like `wget -r`). See [Mirroring](#mirroring-recursive-http) below.

**`gosh priority <ID> <LEVEL>`** -- Set download priority (`low`, `normal`, `high`, `critical`).

**`gosh stats`** -- Show global download/upload statistics.

**`gosh info <FILE>`** -- Parse and display torrent file metadata.

**`gosh config <ACTION>`** -- Manage configuration: `show`, `path`, `get <KEY>`, `set <KEY> <VALUE>`.

**`gosh completions <SHELL>`** -- Generate shell completions for `bash`, `zsh`, `fish`, `elvish`, or `powershell`. Pipe the output to the appropriate completions directory for your shell.

## Mirroring (recursive HTTP)

`gosh mirror` crawls a directory-listing page and downloads every file it discovers, preserving the remote directory structure locally:

```bash
# Mirror a directory tree
gosh mirror https://ftp.gnu.org/gnu/hello/

# Preview what would be downloaded without downloading anything
gosh mirror --dry-run --depth 2 https://ftp.gnu.org/gnu/hello/

# Only .iso and .sig files, two levels deep, into ~/mirrors
gosh mirror -d ~/mirrors --depth 2 --include '*.iso' --include '*.sig' \
    https://example.com/releases/

# Start the mirror and return immediately; manage it later
gosh mirror --detach https://example.com/files/
gosh mirror list
gosh mirror status <ID>
gosh mirror cancel <ID>
gosh mirror remove <ID> --delete-files
```

| Flag | Description |
|------|-------------|
| `-d, --dir <PATH>` | Output directory (root of the mirrored tree) |
| `--depth <N>` | Maximum traversal depth (default: 16) |
| `--include <GLOB>` | Only download matching files (repeatable) |
| `--exclude <GLOB>` | Skip matching files (repeatable) |
| `--prefix <PREFIX>` | Restrict discovered URLs to a path prefix |
| `--span-hosts` | Follow links to other hosts (off by default) |
| `--flatten` | Put all files in one directory instead of preserving paths |
| `--overwrite` | Overwrite existing local files |
| `--fail-fast` | Abort remaining files after the first failure |
| `--discovery-concurrency <N>` | Concurrent page-fetch requests (default: 4) |
| `--dry-run` | Discover and list files without downloading |
| `--detach` | Add the job and exit without waiting |

HTTP options from direct mode (`-H`, `--user-agent`, `--referer`, `--cookie`, `-x`, `--max-speed`) also apply to each mirrored file. Overall download parallelism is governed by `engine.max_concurrent_downloads` in the config; `--discovery-concurrency` only affects the page crawl.

Mirror exit codes follow the standard table below: `0` all files completed, `1` some failed, `2` all failed, `130` interrupted.

## TUI keyboard shortcuts

| Key | Action |
|-----|--------|
| `a` | Add new download |
| `A` | Batch import URLs |
| `p` | Pause selected |
| `r` | Resume selected |
| `c` | Cancel selected |
| `d` | Cancel and delete files |
| `P` | Pause ALL downloads (including queued) |
| `R` | Resume ALL paused downloads |
| `C` | Cancel ALL downloads (with confirmation) |
| `/` | Search/filter the list (Ctrl+S cycles scope, Enter commits, Esc clears) |
| `S` | Open settings |
| `L` | Toggle activity log |
| `[` / `]` | Scroll activity log |
| Tab | Cycle right-panel focus |
| `1` / `2` / `3` | View all / active / completed |
| `j`/`k` or arrows | Navigate |
| PgUp / PgDn | Scroll page |
| `?` | Toggle help overlay |
| `q` or Ctrl+C | Quit |

The details panel at the bottom shows a speed graph sparkline for the selected download. Active mirror jobs appear as a compact counter in the top bar.

## Configuration

Config file location: `~/.config/gosh-dl/config.toml`

Override the path with `-c <PATH>` or the `GOSH_CONFIG` environment variable.

```toml
[general]
download_dir = "~/Downloads"
log_level = "info"                      # trace, debug, info, warn, error
storage_backend = "sqlite"              # sqlite (default), file (JSON sidecars), none

[engine]
max_concurrent_downloads = 5
max_connections_per_download = 8
global_download_limit = 0               # bytes/sec, 0 = unlimited
global_upload_limit = 0
max_retries = 3
connect_timeout = 30                    # seconds
read_timeout = 60

# BitTorrent
enable_dht = true
enable_pex = true
enable_lpd = true
max_peers = 55
seed_ratio = 1.0

# Proxy (overridden by --proxy flag or env vars)
# proxy_url = "socks5://127.0.0.1:1080"

# TLS (dangerous -- prefer --insecure flag for one-off use)
# accept_invalid_certs = false

[tui]
refresh_rate_ms = 250
theme = "dark"                          # or "light"
show_speed_graph = true
show_peers = true

# Bandwidth scheduling -- rules are evaluated in order, first match wins
# [[schedule.rules]]
# start_hour = 9
# end_hour = 17
# days = "weekdays"                     # "all", "weekdays", "weekends", or "mon,tue,..."
# download_limit = "2M"                 # K/M/G suffixes
# upload_limit = "512K"
```

## Environment variables

| Variable | Description |
|----------|-------------|
| `GOSH_CONFIG` | Custom config file path |
| `NO_COLOR` | Disable colored output (any value) |
| `HTTPS_PROXY` | HTTPS proxy URL |
| `HTTP_PROXY` | HTTP proxy URL |
| `ALL_PROXY` | Fallback proxy URL |
| `RUST_LOG` | Override log level filter |

Proxy precedence: `--proxy` flag > config file > `HTTPS_PROXY` > `HTTP_PROXY` > `ALL_PROXY`.

## Exit codes

| Code | Meaning |
|------|---------|
| 0 | All downloads completed |
| 1 | Some downloads failed |
| 2 | All downloads failed |
| 130 | Interrupted (Ctrl+C) |

## Building from source

```bash
cargo build                     # debug build
cargo build --release           # optimized (LTO + stripped)
cargo test                      # run tests
cargo build --no-default-features --release   # without TUI
```

Release builds use thin LTO, symbol stripping, and single codegen unit for smaller binaries.

## License

MIT -- see [LICENSE](LICENSE).
