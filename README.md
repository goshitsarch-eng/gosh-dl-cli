# gosh

[![Crates.io](https://img.shields.io/crates/v/gosh-dl-cli.svg)](https://crates.io/crates/gosh-dl-cli)
[![Documentation](https://docs.rs/gosh-dl-cli/badge.svg)](https://docs.rs/gosh-dl-cli)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![CI](https://github.com/goshitsarch-eng/gosh-dl-cli/actions/workflows/ci.yml/badge.svg)](https://github.com/goshitsarch-eng/gosh-dl-cli/actions/workflows/ci.yml)

A fast, modern download manager CLI with HTTP/HTTPS multi-connection acceleration and full BitTorrent protocol support. Powered by [gosh-dl](https://github.com/goshitsarch-eng/gosh-dl).

## Features

- **HTTP/HTTPS Downloads**: Multi-connection segmented downloads (up to 16 parallel connections), automatic resume, checksum verification
- **BitTorrent**: Full protocol support with DHT, PEX, magnet links, encryption, and WebSeeds
- **Three Usage Modes**: Interactive TUI, direct aria2-style CLI, and scriptable commands
- **Cross-Platform**: Linux, macOS, and Windows support

## Installation

### From Source

```bash
git clone https://github.com/goshitsarch-eng/gosh-dl-cli
cd gosh-dl-cli
cargo build --release
sudo cp target/release/gosh /usr/local/bin/
```

### From Crates.io

```bash
cargo install gosh-dl-cli
```

### Pre-built Binaries

Download from [GitHub Releases](https://github.com/goshitsarch-eng/gosh-dl-cli/releases).

**Linux builds:** Statically linked with musl for maximum portability across all distributions (including Alpine).

## Usage Modes

gosh supports three distinct usage patterns:

### 1. Interactive TUI Mode

Launch without arguments to enter the full-screen terminal UI:

```bash
gosh
```

**TUI Keyboard Shortcuts:**

| Key | Action |
|-----|--------|
| `a` | Add new download |
| `p` | Pause selected |
| `r` | Resume selected |
| `c` | Cancel selected |
| `d` | Cancel and delete files |
| `1` | View all downloads |
| `2` | View active only |
| `3` | View completed only |
| `j`/`k` or arrows | Navigate |
| `?` | Show help |
| `q` or Ctrl+C | Quit |

### 2. Direct Download Mode (aria2-style)

Pass URLs directly to download immediately with progress bars:

```bash
# Single download
gosh https://example.com/file.zip

# Multiple parallel downloads
gosh https://example.com/file1.zip https://example.com/file2.zip

# With options
gosh -d /downloads -x 8 --max-speed 5M https://example.com/large-file.iso

# Torrent/Magnet
gosh magnet:?xt=urn:btih:...
gosh /path/to/file.torrent
```

### 3. Command Mode

Use subcommands for scriptable operations:

```bash
gosh add https://example.com/file.zip
gosh list
gosh status abc123
gosh pause abc123
gosh resume abc123
```

## Direct Download Options

When using direct download mode (`gosh URL`), the following options are available:

| Option | Description |
|--------|-------------|
| `-d, --dir <PATH>` | Output directory |
| `-o, --out <NAME>` | Output filename (single download only) |
| `-H, --header <HEADER>` | Custom header (format: "Name: Value") |
| `--user-agent <UA>` | User agent string |
| `--referer <URL>` | Referer URL |
| `--cookie <COOKIE>` | Cookie (format: "name=value") |
| `--checksum <HASH>` | Expected checksum (format: "md5:xxx" or "sha256:xxx") |
| `-x, --max-connections <N>` | Max connections per download |
| `--max-speed <SPEED>` | Max download speed (supports K/M/G suffixes) |
| `--sequential` | Sequential download mode (for torrents) |
| `--select-files <INDICES>` | Select specific files (comma-separated indices) |
| `--seed-ratio <RATIO>` | Seed ratio limit for torrents |

**Examples:**

```bash
# Download with custom headers and user agent
gosh -H "Authorization: Bearer token" --user-agent "MyApp/1.0" https://api.example.com/file

# Download with checksum verification
gosh --checksum sha256:abc123... https://example.com/important-file.iso

# Limit download speed to 5 MB/s with 8 connections
gosh -x 8 --max-speed 5M https://example.com/large-file.zip

# Download specific files from a torrent
gosh --select-files 0,2,5 /path/to/multi-file.torrent

# Sequential download for streaming
gosh --sequential magnet:?xt=urn:btih:...
```

## Commands Reference

### `gosh add`

Add a new download to the queue.

```bash
gosh add [OPTIONS] <URL>...

Options:
  -d, --dir <PATH>            Output directory
  -o, --out <NAME>            Output filename (single download only)
  -p, --priority <PRIORITY>   Priority [low, normal, high, critical]
  -w, --wait                  Wait for download to complete (show progress)
  -i, --input-file <FILE>     Read URLs from file (one per line)
  -H, --header <HEADER>       Custom header
      --user-agent <UA>       User agent string
      --referer <URL>         Referer URL
      --cookie <COOKIE>       Cookie
      --checksum <HASH>       Expected checksum
  -x, --max-connections <N>   Max connections
      --max-speed <SPEED>     Max download speed
      --sequential            Sequential mode (torrents)
      --select-files <IDX>    Select files (torrents)
      --seed-ratio <RATIO>    Seed ratio (torrents)
```

### `gosh list`

List all downloads.

```bash
gosh list [OPTIONS]

Options:
  -s, --state <STATE>   Filter by state [active, waiting, paused, completed, error]
      --ids-only        Show only download IDs
```

### `gosh status`

Show detailed status of a download.

```bash
gosh status [OPTIONS] <ID>

Options:
      --peers   Show peer information (torrents)
      --files   Show file list (torrents)
```

### `gosh pause`

Pause one or more downloads.

```bash
gosh pause <ID>...
gosh pause all    # Pause all active downloads
```

### `gosh resume`

Resume paused downloads.

```bash
gosh resume <ID>...
gosh resume all   # Resume all paused downloads
```

### `gosh cancel`

Cancel and optionally delete downloads.

```bash
gosh cancel [OPTIONS] <ID>...

Options:
      --delete   Also delete downloaded files
  -y, --yes      Skip confirmation prompt
```

### `gosh priority`

Set download priority.

```bash
gosh priority <ID> <PRIORITY>

Priorities: low, normal, high, critical
```

### `gosh stats`

Show global download/upload statistics.

```bash
gosh stats
```

Output:
```
Global Statistics
=================

Downloads:
  Active:   2
  Waiting:  5
  Stopped:  10
  Total:    17

Speed:
  Download: 5.2 MB/s
  Upload:   1.1 MB/s
```

### `gosh info`

Parse and display torrent file information.

```bash
gosh info <TORRENT_FILE>
```

### `gosh config`

Manage configuration.

```bash
gosh config show              # Show current config
gosh config path              # Show config file path
gosh config get <KEY>         # Get a config value
gosh config set <KEY> <VALUE> # Set a config value
```

## Global Options

These options can be used with any command:

| Option | Description |
|--------|-------------|
| `-c, --config <PATH>` | Config file path |
| `-v, --verbose` | Increase verbosity (-v, -vv, -vvv) |
| `-q, --quiet` | Suppress output except errors |
| `--output <FORMAT>` | Output format [table, json, json-pretty] |

## Configuration

Configuration file location: `~/.config/gosh/config.toml`

```toml
[general]
download_dir = "~/Downloads"
log_level = "warn"

[engine]
max_concurrent_downloads = 5
max_connections_per_download = 16
global_download_limit = 0      # 0 = unlimited
global_upload_limit = 0

# BitTorrent settings
enable_dht = true
enable_pex = true
enable_lpd = true
max_peers = 55
seed_ratio = 1.0

# HTTP settings
user_agent = "gosh/0.1"
connect_timeout = 30
read_timeout = 60

[tui]
refresh_rate_ms = 250
theme = "dark"                 # or "light"
show_speed_graph = true
```

## Exit Codes

| Code | Meaning |
|------|---------|
| 0 | Success (all downloads completed) |
| 1 | Partial failure (some downloads failed) |
| 2 | Total failure (all downloads failed) |
| 130 | Interrupted (Ctrl+C) |

## Comparison with aria2

gosh provides aria2-style command-line usage while offering a native Rust implementation:

| Feature | aria2c | gosh |
|---------|--------|------|
| Direct download | `aria2c URL` | `gosh URL` |
| Output directory | `-d /path` | `-d /path` |
| Max connections | `-x 8` | `-x 8` |
| Custom headers | `--header "..."` | `-H "..."` |
| Interactive mode | N/A | `gosh` (TUI) |
| Background daemon | `aria2c --enable-rpc` | Coming soon |

## Examples

### Download a File

```bash
# Simple download
gosh https://example.com/file.zip

# Download to specific directory
gosh -d ~/Downloads https://example.com/file.zip

# Download with custom filename
gosh -o myfile.zip https://example.com/file.zip
```

### Batch Downloads

```bash
# Multiple URLs
gosh https://example.com/file1.zip https://example.com/file2.zip

# From file
gosh add -i urls.txt

# Pipe from stdin
cat urls.txt | gosh add -
```

### BitTorrent

```bash
# Magnet link
gosh "magnet:?xt=urn:btih:..."

# Torrent file
gosh ubuntu-24.04.torrent

# Select specific files
gosh --select-files 0,1,2 multi-file.torrent

# Sequential for streaming
gosh --sequential movie.torrent
```

### Scripting

```bash
# JSON output for parsing
gosh list --output json | jq '.[] | .id'

# Check download status
if gosh status abc123 --output json | jq -e '.state == "Completed"'; then
  echo "Download finished!"
fi

# Monitor progress
gosh add -w https://example.com/large-file.iso
```

## Building from Source

```bash
# Development build
cargo build

# Release build
cargo build --release

# Run tests
cargo test

# Run with verbose logging
RUST_LOG=debug cargo run -- https://example.com/file.zip
```

## Requirements

- Rust 1.75+ (for async trait support)
- Linux, macOS, or Windows

## License

MIT License - see [LICENSE](LICENSE) for details.

## Related Projects

- [gosh-dl](https://github.com/goshitsarch-eng/gosh-dl) - The underlying download engine library
- [docs.rs/gosh-dl-cli](https://docs.rs/gosh-dl-cli) - API documentation
