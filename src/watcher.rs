//! Clipboard watcher abstraction and event types.
//!
//! A watcher monitors a clipboard backend for changes and sends
//! [`ClipEvent`] messages over a channel to the rest of the application.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Receiver;
use std::thread::JoinHandle;

use crate::{ClipContent, ClipboardBackend, ClipboardError};

/// An event emitted by a clipboard watcher when something happens.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClipEvent {
    /// The clipboard content changed to new content.
    Changed(ClipContent),
}

/// A handle to a running background clipboard watcher.
///
/// Holding this handle keeps the background thread running.
/// You can pull events from the `rx` channel. When you want to stop
/// the watcher, call [`WatcherHandle::stop`].
pub struct WatcherHandle {
    /// The receiving end of the event channel.
    pub rx: Receiver<ClipEvent>,

    /// Flag used to signal the background thread to stop.
    pub(crate) stop_signal: Arc<AtomicBool>,

    /// Handle to the spawned background thread, used to wait for it to exit.
    pub(crate) thread_handle: Option<JoinHandle<()>>,
}

impl WatcherHandle {
    /// Create a new watcher handle.
    pub fn new(
        rx: Receiver<ClipEvent>,
        stop_signal: Arc<AtomicBool>,
        thread_handle: JoinHandle<()>,
    ) -> Self {
        Self {
            rx,
            stop_signal,
            thread_handle: Some(thread_handle),
        }
    }

    /// Signal the watcher thread to stop and wait for it to cleanly exit.
    pub fn stop(mut self) {
        // 1. Tell the background loop to stop
        self.stop_signal.store(true, Ordering::Relaxed);

        // 2. Wait for the thread to finish its current loop and terminate
        if let Some(handle) = self.thread_handle.take() {
            let _ = handle.join();
        }
    }
}

/// A watcher that detects changes by polling a [`ClipboardBackend`].
///
/// It holds no background threads or channels of its own — it is a pure
/// state-tracker. Each call to [`poll_once`] checks the backend and returns
/// an event if the content has changed since the last check.
pub struct PollingWatcher<B: ClipboardBackend> {
    backend: B,
    last_content: Option<ClipContent>,
}

impl<B: ClipboardBackend> PollingWatcher<B> {
    /// Create a new polling watcher wrapping a clipboard backend.
    ///
    /// It starts with no previous content recorded.
    pub fn new(backend: B) -> Self {
        Self {
            backend,
            last_content: None,
        }
    }

    /// Poll the clipboard once.
    ///
    /// - Returns `Ok(Some(ClipEvent::Changed(...)))` if the content changed.
    /// - Returns `Ok(None)` if the content is the same as the last poll.
    /// - Returns `Err(ClipboardError)` if reading the clipboard failed.
    pub fn poll_once(&mut self) -> Result<Option<ClipEvent>, ClipboardError> {
        let current = self.backend.get()?;

        if current != self.last_content {
            self.last_content = current.clone();
            if let Some(content) = current {
                return Ok(Some(ClipEvent::Changed(content)));
            }
        }

        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::MemoryClipboard;

    #[test]
    fn polling_watcher_detects_changes_and_ignores_duplicates() {
        let fake = MemoryClipboard::new();
        let mut watcher = PollingWatcher::new(fake);

        // 1. Starts empty — no change
        assert_eq!(watcher.poll_once().unwrap(), None);

        // 2. Add content — detects change
        let clip1 = ClipContent::Text("hello".to_string());
        watcher.backend.set(&clip1).unwrap();
        assert_eq!(
            watcher.poll_once().unwrap(),
            Some(ClipEvent::Changed(clip1))
        );

        // 3. Poll again with no clipboard changes — returns None (no duplicate!)
        assert_eq!(watcher.poll_once().unwrap(), None);

        // 4. Change content — detects new change
        let clip2 = ClipContent::Text("world".to_string());
        watcher.backend.set(&clip2).unwrap();
        assert_eq!(
            watcher.poll_once().unwrap(),
            Some(ClipEvent::Changed(clip2))
        );
    }
}
