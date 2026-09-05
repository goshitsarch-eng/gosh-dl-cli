use anyhow::Result;
use crossterm::event::{self, Event as CrosstermEvent, KeyEvent, KeyEventKind};
use futures_util::StreamExt;
use gosh_dl::{DownloadEvent, RecursiveJobEvent};
use std::time::Duration;
use tokio::sync::broadcast;

/// Application events
#[allow(dead_code)]
pub enum AppEvent {
    /// Terminal input event (keyboard, mouse)
    Terminal(CrosstermEvent),
    /// Engine download event
    Engine(DownloadEvent),
    /// Recursive mirror job event
    RecursiveJob(RecursiveJobEvent),
    /// Periodic tick for UI refresh
    Tick,
    /// Full resync after missed events (broadcast lagged)
    Resync,
    /// Resize event (width, height - reserved for future use)
    Resize(u16, u16),
}

/// Returns false for key events that must be dropped before reaching the app.
///
/// Windows delivers both Press and Release events for every keystroke (and
/// Repeat for held keys); Unix terminals only deliver Press. Without this
/// filter every keypress on Windows is handled twice (GitHub issue #1).
/// Repeat is kept so held-key navigation still works on Windows.
pub(crate) fn should_process(event: &CrosstermEvent) -> bool {
    match event {
        CrosstermEvent::Key(key) => key.kind != KeyEventKind::Release,
        _ => true,
    }
}

/// Event handler that merges terminal and engine events
pub struct EventHandler {
    engine_events: broadcast::Receiver<DownloadEvent>,
    recursive_events: broadcast::Receiver<RecursiveJobEvent>,
    tick_rate: Duration,
    ticker: tokio::time::Interval,
    terminal_reader: crossterm::event::EventStream,
    /// Set when the corresponding stream has closed, so its select branch is
    /// disabled instead of busy-spinning on an immediately-ready error.
    engine_closed: bool,
    recursive_closed: bool,
    terminal_closed: bool,
}

impl EventHandler {
    pub fn new(
        engine_events: broadcast::Receiver<DownloadEvent>,
        recursive_events: broadcast::Receiver<RecursiveJobEvent>,
        tick_rate: Duration,
    ) -> Self {
        Self {
            engine_events,
            recursive_events,
            tick_rate,
            ticker: Self::ticker(tick_rate),
            terminal_reader: crossterm::event::EventStream::new(),
            engine_closed: false,
            recursive_closed: false,
            terminal_closed: false,
        }
    }

    fn ticker(period: Duration) -> tokio::time::Interval {
        let mut ticker = tokio::time::interval_at(tokio::time::Instant::now() + period, period);
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        ticker
    }

    pub fn set_tick_rate(&mut self, period: Duration) {
        if period != self.tick_rate {
            self.tick_rate = period;
            self.ticker = Self::ticker(period);
        }
    }

    /// Get the next event
    pub async fn next(&mut self) -> Result<AppEvent> {
        loop {
            tokio::select! {
                // Check for terminal events
                result = self.terminal_reader.next(), if !self.terminal_closed => {
                    match result {
                        Some(Ok(event)) => {
                            if !should_process(&event) {
                                // Swallow key Release events (Windows double input)
                                continue;
                            }
                            if let CrosstermEvent::Resize(w, h) = event {
                                return Ok(AppEvent::Resize(w, h));
                            }
                            return Ok(AppEvent::Terminal(event));
                        }
                        Some(Err(e)) => return Err(e.into()),
                        None => {
                            anyhow::bail!("Terminal input closed");
                        }
                    }
                }
                // Check for engine events
                result = self.engine_events.recv(), if !self.engine_closed => {
                    match result {
                        Ok(event) => return Ok(AppEvent::Engine(event)),
                        Err(broadcast::error::RecvError::Lagged(_)) => {
                            // Missed events — trigger full resync
                            return Ok(AppEvent::Resync);
                        }
                        Err(broadcast::error::RecvError::Closed) => {
                            // Engine shut down
                            self.engine_closed = true;
                            continue;
                        }
                    }
                }
                // Check for recursive mirror job events
                result = self.recursive_events.recv(), if !self.recursive_closed => {
                    match result {
                        Ok(event) => return Ok(AppEvent::RecursiveJob(event)),
                        Err(broadcast::error::RecvError::Lagged(_)) => {
                            return Ok(AppEvent::Resync);
                        }
                        Err(broadcast::error::RecvError::Closed) => {
                            self.recursive_closed = true;
                            continue;
                        }
                    }
                }
                // Tick for periodic refresh
                _ = self.ticker.tick() => {
                    return Ok(AppEvent::Tick);
                }
            }
        }
    }
}

/// Helper to check if a key event matches.
///
/// For non-alphanumeric keys (e.g. `?`), SHIFT is also accepted because some
/// platforms (notably Windows) report shifted punctuation with the SHIFT
/// modifier set. Alphanumeric keys require exact NONE so that e.g. `p` and
/// Shift+`P` stay distinct bindings.
pub fn is_key(event: &CrosstermEvent, key: char) -> bool {
    matches!(event, CrosstermEvent::Key(KeyEvent {
        code: event::KeyCode::Char(c),
        modifiers,
        ..
    }) if *c == key
        && (*modifiers == event::KeyModifiers::NONE
            || (!key.is_ascii_alphanumeric() && *modifiers == event::KeyModifiers::SHIFT)))
}

/// Helper to check for Enter key
pub fn is_enter(event: &CrosstermEvent) -> bool {
    matches!(
        event,
        CrosstermEvent::Key(KeyEvent {
            code: event::KeyCode::Enter,
            ..
        })
    )
}

/// Helper to check for Escape key
pub fn is_escape(event: &CrosstermEvent) -> bool {
    matches!(
        event,
        CrosstermEvent::Key(KeyEvent {
            code: event::KeyCode::Esc,
            ..
        })
    )
}

/// Helper to check for arrow keys
pub fn is_up(event: &CrosstermEvent) -> bool {
    matches!(
        event,
        CrosstermEvent::Key(KeyEvent {
            code: event::KeyCode::Up,
            ..
        })
    )
}

pub fn is_down(event: &CrosstermEvent) -> bool {
    matches!(
        event,
        CrosstermEvent::Key(KeyEvent {
            code: event::KeyCode::Down,
            ..
        })
    )
}

pub fn is_page_up(event: &CrosstermEvent) -> bool {
    matches!(
        event,
        CrosstermEvent::Key(KeyEvent {
            code: event::KeyCode::PageUp,
            ..
        })
    )
}

pub fn is_page_down(event: &CrosstermEvent) -> bool {
    matches!(
        event,
        CrosstermEvent::Key(KeyEvent {
            code: event::KeyCode::PageDown,
            ..
        })
    )
}

/// Helper to check for a shifted (uppercase) key
pub fn is_shift_key(event: &CrosstermEvent, key: char) -> bool {
    matches!(event, CrosstermEvent::Key(KeyEvent {
        code: event::KeyCode::Char(c),
        modifiers: event::KeyModifiers::SHIFT,
        ..
    }) if *c == key)
}

/// Alias for is_shift_key (uppercase letter check)
pub fn is_upper_key(event: &CrosstermEvent, key: char) -> bool {
    is_shift_key(event, key) || is_key(event, key)
}

/// Helper to check for Tab key
pub fn is_tab(event: &CrosstermEvent) -> bool {
    matches!(
        event,
        CrosstermEvent::Key(KeyEvent {
            code: event::KeyCode::Tab,
            ..
        })
    )
}

/// Helper to check for Ctrl+C
pub fn is_ctrl_c(event: &CrosstermEvent) -> bool {
    matches!(
        event,
        CrosstermEvent::Key(KeyEvent {
            code: event::KeyCode::Char('c'),
            modifiers: event::KeyModifiers::CONTROL,
            ..
        })
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyCode, KeyEventState, KeyModifiers};

    fn key_event(code: KeyCode, modifiers: KeyModifiers, kind: KeyEventKind) -> CrosstermEvent {
        CrosstermEvent::Key(KeyEvent {
            code,
            modifiers,
            kind,
            state: KeyEventState::NONE,
        })
    }

    // Regression tests for GitHub issue #1: TUI double input on Windows.
    // Windows delivers Press AND Release for every keystroke; only Press
    // (and Repeat, for held keys) may reach the app.
    #[test]
    fn release_events_are_filtered() {
        let release = key_event(
            KeyCode::Char('q'),
            KeyModifiers::NONE,
            KeyEventKind::Release,
        );
        assert!(!should_process(&release));
    }

    #[test]
    fn press_and_repeat_events_pass() {
        let press = key_event(KeyCode::Char('q'), KeyModifiers::NONE, KeyEventKind::Press);
        let repeat = key_event(KeyCode::Down, KeyModifiers::NONE, KeyEventKind::Repeat);
        assert!(should_process(&press));
        assert!(should_process(&repeat));
    }

    #[test]
    fn non_key_events_pass() {
        assert!(should_process(&CrosstermEvent::Resize(80, 24)));
        assert!(should_process(&CrosstermEvent::FocusGained));
    }

    // The key helpers are deliberately kind-agnostic: filtering MUST happen
    // upstream in EventHandler::next() via should_process().
    #[test]
    fn helpers_match_release_events_filtering_is_upstream() {
        let release = key_event(
            KeyCode::Char('q'),
            KeyModifiers::NONE,
            KeyEventKind::Release,
        );
        assert!(is_key(&release, 'q'));
    }

    // Windows reports shifted punctuation (e.g. `?`) with SHIFT set.
    #[test]
    fn shifted_punctuation_matches_is_key() {
        let question = key_event(KeyCode::Char('?'), KeyModifiers::SHIFT, KeyEventKind::Press);
        assert!(is_key(&question, '?'));
    }

    #[test]
    fn shifted_letter_does_not_match_is_key() {
        let upper_p = key_event(KeyCode::Char('P'), KeyModifiers::SHIFT, KeyEventKind::Press);
        assert!(!is_key(&upper_p, 'P'));
        assert!(is_shift_key(&upper_p, 'P'));
    }

    #[test]
    fn plain_key_matches_is_key() {
        let p = key_event(KeyCode::Char('p'), KeyModifiers::NONE, KeyEventKind::Press);
        assert!(is_key(&p, 'p'));
        assert!(!is_key(&p, 'q'));
    }
}

#[cfg(test)]
mod refresh_tests {
    use super::*;

    #[test]
    fn uppercase_shortcuts_work_on_unix_and_windows() {
        for letter in ['A', 'P', 'R', 'C', 'S', 'L', 'J', 'K', 'V'] {
            for modifier in [event::KeyModifiers::NONE, event::KeyModifiers::SHIFT] {
                let key =
                    CrosstermEvent::Key(KeyEvent::new(event::KeyCode::Char(letter), modifier));
                assert!(is_upper_key(&key, letter));
            }
        }
    }

    #[tokio::test]
    async fn tick_deadline_survives_cancelled_selects() {
        let mut ticker = EventHandler::ticker(Duration::from_millis(20));
        let mut ticks = 0;
        let work = async {
            for _ in 0..100 {
                tokio::select! {
                    _ = ticker.tick() => ticks += 1,
                    _ = tokio::time::sleep(Duration::from_millis(1)) => {},
                }
            }
        };
        tokio::time::timeout(Duration::from_secs(3), work)
            .await
            .unwrap();
        assert!(ticks > 0, "progress events must not starve UI ticks");
    }
}
