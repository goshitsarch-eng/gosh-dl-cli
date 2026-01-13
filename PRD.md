# Product Requirements Document: gosh

## Product Overview

gosh-dl-cli is a fast, modern download manager CLI with HTTP/HTTPS multi-connection acceleration and full BitTorrent protocol support. It provides three distinct usage modes: an interactive terminal UI, direct aria2-style downloads, and scriptable commands for automation.

Built on the [gosh-dl](https://github.com/goshitsarch-eng/gosh-dl) engine library, gosh-dl-cli benefits from a library-first architecture that eliminates RPC/IPC overhead typical of external process managers like aria2. The engine runs in-process with compile-time type safety and native async/await support via Tokio.

### Objectives

1. Provide a native Rust alternative to aria2c with modern async architecture
2. Unify HTTP and BitTorrent downloading in a single tool
3. Support both interactive and scriptable workflows
4. Maximize download speeds through parallel connections and protocol optimizations
5. Leverage in-process engine for minimal latency and zero serialization overhead

## Target Users

### Primary Users

- **Power users**: Technical users who want fast, reliable downloads from the command line
- **System administrators**: Need scriptable download automation for servers
- **Developers**: Integrating downloads into build pipelines or automation scripts
- **Home lab enthusiasts**: Managing media downloads with BitTorrent support

### User Personas

**CLI Power User**
- Comfortable with terminal workflows
- Values speed and reliability over GUI convenience
- Uses downloads as part of larger workflows (scripts, cron jobs)
- Expects aria2-compatible command-line options

**Interactive User**
- Wants to monitor multiple downloads at once
- Prefers visual feedback (progress bars, speed graphs)
- Manages downloads manually rather than through scripts

## Use Cases

### UC1: Direct File Download
User downloads a file directly with progress feedback:
```bash
gosh https://example.com/large-file.iso
```

### UC2: Batch Downloads
User downloads multiple files from a list:
```bash
gosh add -i urls.txt
```

### UC3: Interactive Management
User launches TUI to monitor and manage ongoing downloads:
```bash
gosh
```

### UC4: Scripted Automation
User integrates gosh into scripts with JSON output:
```bash
gosh list --output json | jq '.[] | select(.state == "Completed")'
```

### UC5: BitTorrent Download
User downloads via magnet link with seeding limit:
```bash
gosh --seed-ratio 1.0 "magnet:?xt=urn:btih:..."
```

### UC6: Resume Interrupted Download
User resumes a partially completed download:
```bash
gosh resume <download-id>
```

## Functional Requirements

### FR1: HTTP/HTTPS Downloads
- Multi-connection segmented downloads (configurable, up to 16 connections)
- Automatic resume detection using ETag/Last-Modified validation
- Mirror/fallback URLs with automatic failover
- Checksum verification (MD5, SHA256)
- Custom headers, user agent, referer, and cookies
- Proxy support (HTTP, HTTPS, SOCKS5)
- Speed limiting (global and per-download)

### FR2: BitTorrent Support
- Parse and download from .torrent files
- Parse and download from magnet URIs
- HTTP and UDP tracker support
- DHT for trackerless peer discovery
- PEX (Peer Exchange) for peer sharing
- LPD (Local Peer Discovery) for LAN peers
- WebSeeds for HTTP-based piece downloading (BEP 17/19)
- Protocol encryption (MSE/PE)
- uTP transport with LEDBAT congestion control
- Sequential download mode for media streaming
- Selective file downloading
- Configurable seed ratio

### FR3: TUI Mode
- Full-screen terminal interface
- Download list with status indicators
- Real-time speed graph
- Keyboard navigation (vim-style j/k)
- Quick actions: add, pause, resume, cancel
- Filter views: all, active, completed
- Help overlay

### FR4: Direct Mode
- aria2c-compatible command-line options
- Progress bars (indicatif)
- Multiple URL arguments
- Exit codes indicating success/failure

### FR5: Command Mode
- `add`: Queue new downloads
- `list`: Show all downloads (filterable by state)
- `status`: Detailed download information
- `pause`/`resume`: Control download state
- `cancel`: Remove downloads (optionally delete files)
- `priority`: Set download priority
- `stats`: Global statistics
- `info`: Parse torrent file information
- `config`: View and modify configuration

### FR6: Configuration
- TOML configuration file
- Configurable download directory
- Engine tuning (connections, segments, timeouts)
- TUI preferences (theme, refresh rate)

### FR7: Persistence
- SQLite database for download state
- Crash recovery (resume interrupted downloads)
- Segment-level progress tracking

## Non-Functional Requirements

### NFR1: Performance
- Support downloads up to 10+ Gbps on capable hardware
- Minimal CPU usage during idle periods
- Memory-efficient large file handling (streaming, not buffering)

### NFR2: Reliability
- Graceful handling of network interruptions
- Automatic retry with exponential backoff
- Data integrity verification

### NFR3: Portability
- Linux, macOS, and Windows support
- Static linking option for Linux (musl)
- No runtime dependencies beyond system libraries

### NFR4: Usability
- Intuitive command-line interface
- Comprehensive `--help` output
- Consistent error messages
- Exit codes suitable for scripting

### NFR5: Maintainability
- Rust 1.75+ (async traits)
- Modular architecture separating engine from UI
- Comprehensive type safety

## Success Metrics

1. **Download Speed**: Achieve speeds comparable to or exceeding aria2c on equivalent hardware
2. **Reliability**: Successfully resume interrupted downloads in 99%+ of cases where server supports range requests
3. **Compatibility**: Parse and download from 99%+ of valid torrent files and magnet URIs
4. **User Adoption**: Positive community feedback and growing usage

## Constraints

1. **Dependency on gosh-dl**: Core download functionality is provided by the [gosh-dl](https://github.com/goshitsarch-eng/gosh-dl) engine library. This CLI is a frontend; protocol implementations live in gosh-dl.
2. **Terminal-only**: No GUI planned; TUI is the most visual interface
3. **No daemon mode**: Background operation requires external tools (tmux, systemd)

## Assumptions

1. Users have basic familiarity with command-line interfaces
2. Network connectivity is available (no offline-first features)
3. Users can install Rust toolchain or use pre-built binaries
4. Target systems have standard filesystem permissions

## Future Considerations

- RPC daemon mode (aria2c JSON-RPC compatible)
- Web UI for daemon mode
- Bandwidth scheduling rules
- Download queuing with priorities
- Plugin system for protocol extensions
