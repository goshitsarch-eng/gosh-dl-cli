use anyhow::Result;
use crossterm::event::{self, Event as CrosstermEvent, KeyEvent};
use futures_util::StreamExt;
use gosh_dl::DownloadEvent;
use std::time::Duration;
use tokio::sync::broadcast;

/// Application events
#[allow(dead_code)]
pub enum AppEvent {
    /// Terminal input event (keyboard, mouse)
    Terminal(CrosstermEvent),
    /// Engine download event
    Engine(DownloadEvent),
    /// Periodic tick for UI refresh
    Tick,
    /// Resize event (width, height - reserved for future use)
    Resize(u16, u16),
}

/// Event handler that merges terminal and engine events
pub struct EventHandler {
    engine_events: broadcast::Receiver<DownloadEvent>,
    tick_rate: Duration,
    terminal_reader: crossterm::event::EventStream,
}

impl EventHandler {
    pub fn new(engine_events: broadcast::Receiver<DownloadEvent>, tick_rate: Duration) -> Self {
        Self {
            engine_events,
            tick_rate,
            terminal_reader: crossterm::event::EventStream::new(),
        }
    }

    /// Get the next event
    pub async fn next(&mut self) -> Result<AppEvent> {
        let tick = tokio::time::sleep(self.tick_rate);

        tokio::select! {
            // Check for terminal events
            result = self.terminal_reader.next() => {
                match result {
                    Some(Ok(event)) => {
                        if let CrosstermEvent::Resize(w, h) = event {
                            Ok(AppEvent::Resize(w, h))
                        } else {
                            Ok(AppEvent::Terminal(event))
                        }
                    }
                    Some(Err(e)) => Err(e.into()),
                    None => Ok(AppEvent::Tick),
                }
            }
            // Check for engine events
            result = self.engine_events.recv() => {
                match result {
                    Ok(event) => Ok(AppEvent::Engine(event)),
                    Err(broadcast::error::RecvError::Lagged(_)) => {
                        // Missed some events, continue
                        Ok(AppEvent::Tick)
                    }
                    Err(broadcast::error::RecvError::Closed) => {
                        // Engine shut down
                        Ok(AppEvent::Tick)
                    }
                }
            }
            // Tick for periodic refresh
            _ = tick => {
                Ok(AppEvent::Tick)
            }
        }
    }
}

/// Helper to check if a key event matches
pub fn is_key(event: &CrosstermEvent, key: char) -> bool {
    matches!(event, CrosstermEvent::Key(KeyEvent {
        code: event::KeyCode::Char(c),
        modifiers: event::KeyModifiers::NONE,
        ..
    }) if *c == key)
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
