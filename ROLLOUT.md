# gosh-dl-cli 0.6.2 rollout audit

Baseline: CLI 0.5.0 at `8f17ced`, using gosh-dl 0.5.0. This update uses the
published gosh-dl 0.6.2 engine. The binary is still named `gosh`.

## Defects addressed

| Area | Observed failure | Resolution |
| --- | --- | --- |
| Engine dependency | CLI missed the 0.6.x HTTP, streaming, torrent lifecycle, and uTP fixes. | Upgrade the registry dependency and lockfile to 0.6.2. |
| Waiting downloads | `add --wait` returned success after an HTTP 404. | Track failed outcomes and return 1/2 for partial/total failure. |
| Resume | `resume` started workers and immediately shut them down. | Wait in the foreground, including `resume-all`. |
| Interrupts | Direct downloads and mirrors cancelled records, losing normal resume access. `add --wait` had no graceful Ctrl+C branch. | Pause unfinished work, preserve state, and exit 130. |
| Queue-only operations | Detached mirrors claimed background work even though the process exited; in-memory queue entries disappeared on exit. | Save explicitly paused work; prefer `--enqueue` and retain `--detach` as an alias. Reject queue-only operations without storage. |
| JSON output | Empty `cancel-all` and foreground mirrors printed plain text; log messages could reach stdout. | Serialize result objects and route logging to stderr. |
| Local commands | Config and torrent-info commands opened the download database unnecessarily; `config path` ignored `--config`. | Route local commands before engine creation and honor the selected path. |
| Validation | Speed multiplication overflowed; schedule typos could become unrestricted rules; zero HTTP attempts prevented discovery. | Checked arithmetic and explicit configuration validation. |
| Rust support | Manifest claimed Rust 1.85 despite TUI dependencies requiring 1.88. | Set and test Rust 1.88; modernize existing Clippy match patterns. |
| TUI event loop | Progress floods reset ticks; errors could leave raw mode active. | Persistent ticker, coalesced rendering, guaranteed terminal cleanup. |
| TUI controls | Uppercase shortcuts failed on some terminals; invalid add input exited; dialogs clipped at 80×24. | Portable key handling, contained errors, usable dialog sizes, scrollable settings/import review. |
| TUI settings | Custom paths ignored; failed saves and invalid numeric edits appeared successful; UI toggles did nothing. | Validate edits, retain failed drafts, honor paths, apply UI settings and document restart requirements. |
| TUI integrity | New engine verification/repair unavailable in interface. | Background `v` checks and confirmed `V` repair, persistent activity-log results. |
| Startup/logging | Missing download folders prevented first launch; logs could overwrite TUI. | Create download directory and honor log file/level settings with TUI console suppression. |
| Releases | Tag-triggered binary builds had no test gate or crates.io publication. | Gate Trusted Publishing and six binary archives on reusable CI; attach checksums. |

Seven targeted regressions reproduced existing failures before their fixes.
Additional cases cover successful nested output paths, queue
persistence, invalid schedule/attempt limits, and process interruption. HTTP
fixtures run on loopback; they do not depend on external download servers.

## Validation

- CLI integration tests cover HTTP success/failure, JSON, queue/resume,
  configuration, mirrors, and Unix SIGINT recovery.
- Unit tests cover parsers, configuration, batch results, and TUI behavior.
  TUI regressions include 80×24/small-terminal rendering, Unicode input,
  uppercase keys, settings save failures, refresh deadlines, and a real HTTP
  download verified, corrupted on disk, then repaired through the interface.
- Default and TUI-free tests run on Linux, macOS, and Windows.
- Formatting, Clippy, Rust 1.88, and verified Cargo packaging are checked.
- Binary builds target Linux musl, macOS, and Windows on x86_64 and ARM64.
- Registry publication uses the official pinned OIDC action. A rerun accepts
  an existing crate only when the checksum matches and it is not yanked.

## Remaining scope

- There is no daemon, RPC, or inter-process coordination. Use one process per
  storage location. Separate commands do not control a running TUI. A future
  daemon/locking design is needed for concurrent process management.
- Metalink import and streaming readers are not exposed. Verification/repair
  are available in the TUI; equivalent scriptable subcommands are follow-up work.
- The progress map is an estimate from completed bytes, not per-piece telemetry.
  Visual list reordering does not change engine priority (use `gosh priority`).
- TUI network/storage/logging edits require a restart; UI, concurrency, and
  bandwidth settings apply immediately. Avoid overlapping processes.
- Real-swarm interoperability and long-duration network testing still matter;
  engine limitations are documented in the
  [gosh-dl rollout audit](https://github.com/goshitsarch-eng/gosh-dl/blob/v0.6.2/ROLLOUT.md).
- AUR packaging is updated separately from the crate and GitHub release.
- The crate owner must configure Trusted Publishing for **gosh-dl-cli**
  before merging the release PR. The engine's publisher does not cover it.
  See the exact values in [README.md](README.md#releasing).
