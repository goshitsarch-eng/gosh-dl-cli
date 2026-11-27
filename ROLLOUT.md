# gosh-dl-cli 0.7.0 rollout audit

Baseline: CLI 0.6.3 at `6505472`, using gosh-dl 0.6.3. This update uses the
published gosh-dl 0.7.0 engine, raises the minimum Rust version to 1.89, adds
the engine's new features to the CLI and the TUI, and rebuilds the TUI. The
binary is still named `gosh`.

## Engine features now exposed

| Engine 0.7.0 | CLI | TUI |
| --- | --- | --- |
| File conflict policy (`overwrite`, `rename`, `fail`) | `--on-conflict`, `general.file_conflict` | Settings, General tab; applied immediately |
| `verify` / `repair` | `gosh verify <ID>`, `gosh repair <ID>` | `v`, `V`, with a result dialog |
| Metalink | `.meta4` / `.metalink` as an input | Add dialog and batch import |
| HTTP mirrors | `--alt-url` | Shown in the details panel |
| Recursive jobs | `gosh mirror` (unchanged) | Directory URL in the add dialog; one row per job |
| Paused creation | `gosh add` (unchanged) | F3 / Shift+Enter in the add dialog |
| `HEAD` to ranged `GET` probe fallback, single-stream retries | Automatic | Automatic |

## Defects addressed

| Area | Observed failure | Resolution |
| --- | --- | --- |
| Retry setting | `--max-retries 0` was rejected, although the engine defines the value as retries after the first attempt. | Accept 0 as "try once"; a download and a mirror crawl with 0 are tested. |
| Torrent info | `gosh info` divided the piece count by 20 (it treated the hash list as raw bytes). | Report the number of piece hashes. |
| Size parsing | `config set` multiplied sizes without an overflow check. | One checked parser for sizes and speeds. |
| Input detection | A missing local file with a common extension became an `https://` URL. | Recognise more extensions as file names; report the missing file. |
| Error labels | Error kinds were cut to ten characters of debug output. | Show the error category, and the status code for HTTP failures. |
| Seed ratio | `NaN` and infinity were accepted. | Rejected with a message. |
| TUI keys | Control chords triggered the plain-key bindings. | Ctrl and Alt chords are not bindings. |
| TUI mirrors | Every mirrored file was its own row, log line and notification. | One row and one notification per job; children are listed in the details panel. |
| TUI settings | Settings were a hand-written switch with its own parsing; invalid edits could be partially applied; sizes were raw byte counts. | Table of config keys using the `gosh config` parser; whole-config validation before an edit is accepted. |
| TUI paste | Pasted text arrived as key presses; a newline submitted the dialog. | Bracketed paste, inserted as one edit. |
| TUI responsiveness | Mirror discovery would have blocked the event loop. | Background task, collected on the next tick. |
| TUI honesty | The "progress map" drew invented segments from a byte count. | Removed; the details panel shows only what the engine reports. |
| Packaging | The AUR `PKGBUILD` was unmaintained; the Homebrew formula pointed at a placeholder account. | `PKGBUILD` removed; formula points at this repository and documents the two values to fill in. |
| Automation | The repository had no CI after the workflows were removed. | New `ci.yml`, `build.yml`, and a tag-driven `release.yml` without a stored registry token. |

## Validation

- 94 unit tests (one of them the screenshot generator, ignored by default)
  and 40 CLI integration tests; all HTTP fixtures run on loopback.
- New CLI tests: conflict policy (flag, config key, override), verify and
  repair on a file damaged on disk, Metalink with a dead first mirror,
  `--alt-url` failover and validation, zero retries, torrent piece count, and
  bare file names.
- New TUI tests drive the real key handler against an in-process engine: a
  directory URL becoming a single mirror row that is cancelled as a job; save
  paused then resume; verify, damage, repair and re-verify of a real download;
  settings edited by keyboard including rejected values; schedule rules
  toggled and saved; filter, views and reordering; control chords; paste.
- Every screen and dialog is rendered at nine sizes from 1x1 to 200x60, empty
  and populated, to catch layout panics.
- `cargo fmt --check`, Clippy with `-D warnings` with and without the `tui`
  feature, `cargo check` on Rust 1.89, and `cargo audit` (no vulnerabilities;
  one "unmaintained" notice for `chrono`).

## Remaining scope

- There is no daemon, RPC, or inter-process coordination. Use one process per
  storage location. Separate commands do not control a running TUI.
- The engine reports no per-connection, per-segment or per-piece telemetry,
  and no list of connected peers, so the details panel shows totals. The
  torrent piece figure is derived from completed bytes and marked with `≈`.
- A whole mirror cannot be verified in one step; verify its files one at a
  time with `gosh verify <ID>`.
- Schedule rules are toggled in the TUI but added and edited in the config
  file.
- The conflict policy applies to HTTP downloads. The final name of a download
  that uses a server-supplied `Content-Disposition` name is only known when it
  completes.
- Visual list reordering does not change engine priority (use `gosh priority`).
- The README pictures are SVGs produced from the rendering code with sample
  data, not captures of a terminal emulator.
- Publishing to crates.io is a manual `cargo publish`; the release workflow
  only creates the GitHub release.
- Engine limitations are documented in the
  [gosh-dl rollout audit](https://github.com/goshitsarch-eng/gosh-dl/blob/v0.7.0/ROLLOUT.md).
