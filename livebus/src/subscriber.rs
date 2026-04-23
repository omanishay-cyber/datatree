//! Subscriber registry and slow-consumer eviction.
//!
//! A [`Subscriber`] is the in-process representation of an SSE or WebSocket
//! client. It owns:
//!
//! 1. A bounded MPSC sender to the transport task that writes to the wire.
//! 2. A list of topic patterns it registered for.
//! 3. A monotonic counter of how many consecutive events it has failed to
//!    accept (the "lag window"). When the lag exceeds [`BACKPRESSURE_WINDOW`]
//!    the [`SubscriberManager`] evicts it and emits a `system.degraded_mode`
//!    event so other subscribers can see the drop.
//!
//! The registry is kept in a single `RwLock<HashMap>` keyed by subscriber id.
//! This is contended only on subscribe/unsubscribe; the per-event fast path
//! does NOT touch the registry — each subscriber is driven by its own
//! independent broadcast `Receiver`.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, RwLock};

use tokio::sync::mpsc;
use tracing::{info, warn};

use crate::bus::{topic_matches_any, validate_topic, EventBus};
use crate::error::LivebusError;
use crate::event::{DegradedMode, Event, EventPayload};

/// Per-subscriber lag budget. If the subscriber falls this many events behind
/// it is evicted and a `system.degraded_mode` event is published.
pub const BACKPRESSURE_WINDOW: usize = 50;

/// Snapshot of subscriber-related stats for `/health`.
#[derive(Debug, Clone, serde::Serialize)]
pub struct SubscriberStats {
    pub active_subscribers: usize,
    pub evicted_subscribers: u64,
}

/// In-process handle a transport task uses to push events to a single client.
#[derive(Debug)]
pub struct SubscriberHandle {
    pub id: String,
    pub rx: mpsc::Receiver<Event>,
}

/// Registry entry tracking a single live subscriber.
#[derive(Debug)]
pub struct Subscriber {
    pub id: String,
    pub patterns: Vec<String>,
    pub tx: mpsc::Sender<Event>,
    /// Monotonic count of consecutive failed `try_send` calls — reset on a
    /// successful send.
    pub lag: AtomicU64,
    /// Total events delivered to this subscriber.
    pub delivered: AtomicU64,
}

impl Subscriber {
    fn new(id: String, patterns: Vec<String>, tx: mpsc::Sender<Event>) -> Self {
        Self {
            id,
            patterns,
            tx,
            lag: AtomicU64::new(0),
            delivered: AtomicU64::new(0),
        }
    }

    /// True if this subscriber wants to see `topic`.
    pub fn matches(&self, topic: &str) -> bool {
        topic_matches_any(&self.patterns, topic)
    }
}

/// Registry of all live subscribers.
#[derive(Debug, Clone)]
pub struct SubscriberManager {
    inner: Arc<ManagerInner>,
}

#[derive(Debug)]
struct ManagerInner {
    subscribers: RwLock<HashMap<String, Arc<Subscriber>>>,
    bus: EventBus,
    next_id: AtomicU64,
    evicted: AtomicU64,
}

impl SubscriberManager {
    pub fn new(bus: EventBus) -> Self {
        Self {
            inner: Arc::new(ManagerInner {
                subscribers: RwLock::new(HashMap::new()),
                bus,
                next_id: AtomicU64::new(1),
                evicted: AtomicU64::new(0),
            }),
        }
    }

    /// Register a new subscriber with the given topic patterns. Returns a
    /// [`SubscriberHandle`] whose `rx` the transport task should drain to the
    /// wire.
    pub fn register(
        &self,
        patterns: Vec<String>,
    ) -> Result<SubscriberHandle, LivebusError> {
        for p in &patterns {
            validate_topic(p)?;
        }
        let n = self.inner.next_id.fetch_add(1, Ordering::Relaxed);
        let id = format!("sub-{n}");
        let (tx, rx) = mpsc::channel::<Event>(BACKPRESSURE_WINDOW);
        let sub = Arc::new(Subscriber::new(id.clone(), patterns, tx));
        self.write_registry().insert(id.clone(), sub);
        info!(subscriber = %id, "subscriber registered");
        Ok(SubscriberHandle { id, rx })
    }

    /// Replace the topic patterns of an existing subscriber. Used by the
    /// WebSocket `subscribe` / `unsubscribe` control messages.
    pub fn update_patterns(
        &self,
        id: &str,
        patterns: Vec<String>,
    ) -> Result<(), LivebusError> {
        for p in &patterns {
            validate_topic(p)?;
        }
        let mut guard = self.write_registry();
        let Some(existing) = guard.get(id).cloned() else {
            return Err(LivebusError::SubscriberEvicted(
                id.into(),
                "subscriber not found".into(),
            ));
        };
        let replaced = Arc::new(Subscriber {
            id: existing.id.clone(),
            patterns,
            tx: existing.tx.clone(),
            lag: AtomicU64::new(existing.lag.load(Ordering::Relaxed)),
            delivered: AtomicU64::new(existing.delivered.load(Ordering::Relaxed)),
        });
        guard.insert(id.to_string(), replaced);
        Ok(())
    }

    /// Remove a subscriber by id (no-op if already gone).
    pub fn unregister(&self, id: &str) {
        if self.write_registry().remove(id).is_some() {
            info!(subscriber = %id, "subscriber unregistered");
        }
    }

    fn read_registry(
        &self,
    ) -> std::sync::RwLockReadGuard<'_, HashMap<String, Arc<Subscriber>>> {
        self.inner
            .subscribers
            .read()
            .expect("livebus subscriber registry poisoned")
    }

    fn write_registry(
        &self,
    ) -> std::sync::RwLockWriteGuard<'_, HashMap<String, Arc<Subscriber>>> {
        self.inner
            .subscribers
            .write()
            .expect("livebus subscriber registry poisoned")
    }

    /// Fan-out an event to every matching subscriber, evicting any that
    /// exceed the backpressure window.
    pub fn dispatch(&self, event: &Event) {
        // Snapshot the subscribers (cheap Arc clones) so we don't hold the
        // lock across `.try_send`.
        let snapshot: Vec<Arc<Subscriber>> =
            self.read_registry().values().cloned().collect();

        let mut to_evict: Vec<(String, String)> = Vec::new();
        for sub in snapshot {
            if !sub.matches(&event.topic) {
                continue;
            }
            match sub.tx.try_send(event.clone()) {
                Ok(()) => {
                    sub.lag.store(0, Ordering::Relaxed);
                    sub.delivered.fetch_add(1, Ordering::Relaxed);
                }
                Err(mpsc::error::TrySendError::Full(_)) => {
                    let lag = sub.lag.fetch_add(1, Ordering::Relaxed) + 1;
                    if lag as usize >= BACKPRESSURE_WINDOW {
                        to_evict.push((
                            sub.id.clone(),
                            format!(
                                "lag {lag} >= backpressure window {BACKPRESSURE_WINDOW}"
                            ),
                        ));
                    }
                }
                Err(mpsc::error::TrySendError::Closed(_)) => {
                    to_evict.push((sub.id.clone(), "channel closed".into()));
                }
            }
        }

        for (id, reason) in to_evict {
            self.evict(&id, &reason);
        }
    }

    /// Forcibly remove a subscriber and emit a `system.degraded_mode` warning.
    pub fn evict(&self, id: &str, reason: &str) {
        let removed = self.write_registry().remove(id).is_some();
        if !removed {
            return;
        }
        self.inner.evicted.fetch_add(1, Ordering::Relaxed);
        self.inner.bus.record_drop();
        warn!(subscriber = %id, %reason, "subscriber evicted (slow consumer)");

        // Post a degraded-mode notice so the rest of the world knows.
        let payload = EventPayload::DegradedMode(DegradedMode {
            reason: reason.into(),
            subscriber_id: Some(id.into()),
            dropped_count: Some(self.inner.evicted.load(Ordering::Relaxed)),
        });
        let ev = Event::from_typed("system.degraded_mode", None, None, payload);
        // Best-effort; if the bus is closed there's nothing we can do.
        let _ = self.inner.bus.publish(ev);
    }

    /// Current registry size and lifetime eviction count.
    pub fn stats(&self) -> SubscriberStats {
        SubscriberStats {
            active_subscribers: self.read_registry().len(),
            evicted_subscribers: self.inner.evicted.load(Ordering::Relaxed),
        }
    }

    /// Borrow the underlying bus.
    pub fn bus(&self) -> EventBus {
        self.inner.bus.clone()
    }
}

#[cfg(test)]
mod sub_tests {
    use super::*;

    #[tokio::test]
    async fn register_and_dispatch_matches() {
        let bus = EventBus::new();
        let mgr = SubscriberManager::new(bus.clone());
        let mut h = mgr
            .register(vec!["project.*.file_changed".into()])
            .unwrap();
        let ev = Event::from_json(
            "project.abc.file_changed",
            None,
            Some("abc".into()),
            serde_json::json!({"x": 1}),
        );
        mgr.dispatch(&ev);
        let got = h.rx.recv().await.unwrap();
        assert_eq!(got.topic, "project.abc.file_changed");
    }

    #[tokio::test]
    async fn non_matching_does_not_deliver() {
        let bus = EventBus::new();
        let mgr = SubscriberManager::new(bus.clone());
        let mut h = mgr.register(vec!["system.health".into()]).unwrap();
        mgr.dispatch(&Event::from_json(
            "project.abc.file_changed",
            None,
            None,
            serde_json::Value::Null,
        ));
        assert!(h.rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn slow_subscriber_is_evicted() {
        let bus = EventBus::new();
        let mgr = SubscriberManager::new(bus.clone());
        // Register but never drain the receiver.
        let _h = mgr.register(vec!["system.health".into()]).unwrap();
        for i in 0..(BACKPRESSURE_WINDOW * 4) {
            mgr.dispatch(&Event::from_json(
                "system.health",
                None,
                None,
                serde_json::json!({"i": i}),
            ));
        }
        assert_eq!(mgr.stats().active_subscribers, 0);
        assert!(mgr.stats().evicted_subscribers >= 1);
    }
}
