//! Pure sans-I/O synchronization engine for uclip.
//!
//! This module contains the state machine that manages logical clocks,
//! resolves sync conflicts (Last-Writer-Wins), and suppresses echoes.

use crate::DeviceId;
use serde::{Deserialize, Serialize};

/// Identifies an update in distributed logical time.
///
/// Implements [`Ord`] using Rust's field-declaration order:
/// 1. Higher `lamport` timestamp wins.
/// 2. If timestamps are tied, higher `origin` [`DeviceId`] wins (deterministic tie-breaker).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct UpdateId {
    pub lamport: u64,
    pub origin: DeviceId,
}

/// A monotonic Lamport logical clock.
///
/// Tracks the order of events in a distributed system without relying on wall-clock time.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct LamportClock {
    counter: u64,
}

impl LamportClock {
    /// Create a new logical clock starting at zero.
    pub fn new() -> Self {
        Self { counter: 0 }
    }

    /// Read the current logical timestamp without advancing the clock.
    pub fn get(&self) -> u64 {
        self.counter
    }

    /// Advance the clock for a local event (Rule 1).
    ///
    /// Increments the counter by 1 and returns the new value.
    pub fn tick(&mut self) -> u64 {
        self.counter += 1;
        self.counter
    }

    /// Update the clock upon receiving a remote event (Rule 2).
    ///
    /// Sets the counter to `max(local, remote) + 1` and returns the new value.
    pub fn witness(&mut self, remote_time: u64) -> u64 {
        self.counter = self.counter.max(remote_time) + 1;
        self.counter
    }
}

use crate::ClipContent;
use crate::proto::Message;
use uuid::Uuid;

/// Actions emitted by the sync engine for the outer I/O layer to execute.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    /// Broadcast a new clip update to all connected peers over the network.
    Broadcast(Message),
    /// Apply new content to the local operating system clipboard.
    ApplyToClipboard(ClipContent),
}

/// The pure sans-I/O state machine that coordinates clipboard synchronization.
pub struct Engine {
    device_id: DeviceId,
    clock: LamportClock,
    current_hash: Option<[u8; 32]>,
}

impl Engine {
    /// Create a new sync engine for the given local device ID.
    pub fn new(device_id: DeviceId) -> Self {
        Self {
            device_id,
            clock: LamportClock::new(),
            current_hash: None,
        }
    }

    /// Current logical clock counter.
    pub fn clock(&self) -> u64 {
        self.clock.get()
    }

    /// Handle a local clipboard change event.
    ///
    /// If the content is new:
    /// - Advances the Lamport clock.
    /// - Records the new hash.
    /// - Emits [`Action::Broadcast`] containing a `Message::ClipUpdate`.
    ///
    /// If the content matches the currently stored hash, it is ignored (deduplicated).
    pub fn on_local_change(&mut self, content: ClipContent, now_ms: u64) -> Vec<Action> {
        let hash = content.content_hash();

        // 1. Deduplication: if identical to current content, ignore.
        if Some(hash) == self.current_hash {
            return Vec::new();
        }

        // 2. Advance logical clock and record new state.
        let lamport = self.clock.tick();
        self.current_hash = Some(hash);

        // 3. Emit Broadcast action.
        let update_msg = Message::ClipUpdate {
            update_id: Uuid::new_v4(),
            origin: self.device_id,
            lamport,
            created_at_ms: now_ms,
            content,
        };

        vec![Action::Broadcast(update_msg)]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clock_ticks_monotonically() {
        let mut clock = LamportClock::new();
        assert_eq!(clock.get(), 0);
        assert_eq!(clock.tick(), 1);
        assert_eq!(clock.tick(), 2);
        assert_eq!(clock.get(), 2);
    }

    #[test]
    fn witness_advances_clock_past_remote_time() {
        let mut clock = LamportClock::new();
        clock.tick(); // local = 1

        // Witness a future remote timestamp (e.g. 10)
        assert_eq!(clock.witness(10), 11);
        assert_eq!(clock.get(), 11);

        // Witness an older remote timestamp (e.g. 5) -> clock still moves forward!
        assert_eq!(clock.witness(5), 12);
        assert_eq!(clock.get(), 12);
    }

    #[test]
    fn update_id_orders_by_lamport_then_origin() {
        let dev_a: DeviceId = "11111111-1111-1111-1111-111111111111".parse().unwrap();
        let dev_b: DeviceId = "22222222-2222-2222-2222-222222222222".parse().unwrap();

        let id1 = UpdateId {
            lamport: 1,
            origin: dev_a,
        };
        let id2 = UpdateId {
            lamport: 2,
            origin: dev_a,
        };
        let id3 = UpdateId {
            lamport: 1,
            origin: dev_b,
        };

        // Higher lamport clock wins (2 > 1)
        assert!(id2 > id1);

        // Same lamport clock (1 == 1) -> tie-broken by origin (dev_b > dev_a)
        assert!(id3 > id1);
    }

    #[test]
    fn on_local_change_emits_broadcast_for_new_content() {
        let dev_id: DeviceId = "11111111-1111-1111-1111-111111111111".parse().unwrap();
        let mut engine = Engine::new(dev_id);

        let content = ClipContent::Text {
            text: "hello world".to_string(),
        };
        let actions = engine.on_local_change(content.clone(), 1000);

        assert_eq!(actions.len(), 1);
        assert_eq!(engine.clock(), 1);

        match &actions[0] {
            Action::Broadcast(Message::ClipUpdate {
                origin,
                lamport,
                created_at_ms,
                content: msg_content,
                ..
            }) => {
                assert_eq!(*origin, dev_id);
                assert_eq!(*lamport, 1);
                assert_eq!(*created_at_ms, 1000);
                assert_eq!(msg_content, &content);
            }
            other => panic!("expected Action::Broadcast(ClipUpdate), got {:?}", other),
        }
    }

    #[test]
    fn on_local_change_deduplicates_identical_content() {
        let dev_id: DeviceId = "11111111-1111-1111-1111-111111111111".parse().unwrap();
        let mut engine = Engine::new(dev_id);

        let content = ClipContent::Text {
            text: "hello".to_string(),
        };
        let actions1 = engine.on_local_change(content.clone(), 1000);
        let actions2 = engine.on_local_change(content.clone(), 1050);

        assert_eq!(actions1.len(), 1);
        assert!(actions2.is_empty());
        assert_eq!(engine.clock(), 1);
    }
}
