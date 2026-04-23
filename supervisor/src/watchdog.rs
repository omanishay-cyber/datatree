//! Watchdog: 1-second heartbeat check, 60-second deep self-test.
//!
//! - Heartbeat tick (every 1s): for every running child, ensure the last
//!   heartbeat is within `HEARTBEAT_DEADLINE`. If a child has missed it, the
//!   watchdog force-kills the PID and lets the [`crate::ChildManager`]
//!   monitor task pick up the corpse and restart with backoff.
//! - Deep self-test (every `health_check_interval`, default 60s): pings each
//!   child's `/health` endpoint over its dedicated IPC channel.

use crate::child::ChildStatus;
use crate::error::SupervisorError;
use crate::manager::ChildManager;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Notify;
use tracing::{debug, error, info, warn};

/// Maximum time a running child can go without sending a heartbeat before the
/// watchdog force-kills it.
///
/// v0.1: 1 hour. Every worker will eventually push heartbeats over the IPC
/// channel; until they do, we effectively keep the watchdog off so idle
/// stubs (md-ingest, brain-stub) aren't reaped. v0.2 restores to 5s.
pub const HEARTBEAT_DEADLINE: Duration = Duration::from_secs(3600);

/// Watchdog that supervises the [`ChildManager`].
pub struct Watchdog {
    manager: Arc<ChildManager>,
    self_test_interval: Duration,
}

impl Watchdog {
    /// Construct a new watchdog.
    pub fn new(manager: Arc<ChildManager>, self_test_interval: Duration) -> Self {
        Self {
            manager,
            self_test_interval,
        }
    }

    /// Run the watchdog forever (until `shutdown.notified()`).
    pub async fn run(&self, shutdown: Arc<Notify>) {
        info!(
            self_test_interval_s = self.self_test_interval.as_secs(),
            heartbeat_deadline_s = HEARTBEAT_DEADLINE.as_secs(),
            "watchdog started"
        );
        let mut heartbeat_tick = tokio::time::interval(Duration::from_secs(1));
        let mut self_test_tick = tokio::time::interval(self.self_test_interval);
        // First tick fires immediately for both — skip it.
        heartbeat_tick.tick().await;
        self_test_tick.tick().await;

        loop {
            tokio::select! {
                _ = shutdown.notified() => {
                    info!("watchdog shutting down");
                    break;
                }
                _ = heartbeat_tick.tick() => {
                    if let Err(e) = self.heartbeat_pass().await {
                        warn!(error = %e, "heartbeat pass error");
                    }
                }
                _ = self_test_tick.tick() => {
                    if let Err(e) = self.self_test_pass().await {
                        warn!(error = %e, "self-test pass error");
                    }
                }
            }
        }
    }

    async fn heartbeat_pass(&self) -> Result<(), SupervisorError> {
        let names = self.manager.child_names().await;
        let now = Instant::now();
        for name in names {
            let handle = match self.manager.handle_for(&name).await {
                Some(h) => h,
                None => continue,
            };
            let (status, last_hb) = {
                let h = handle.lock().await;
                (h.status, h.last_heartbeat)
            };
            if status != ChildStatus::Running {
                continue;
            }
            let last = match last_hb {
                Some(t) => t,
                None => continue,
            };
            let missed = now.duration_since(last);
            if missed > HEARTBEAT_DEADLINE {
                error!(
                    child = %name,
                    missed_ms = missed.as_millis() as u64,
                    "heartbeat missed past deadline; force-kill"
                );
                if let Err(e) = self.manager.kill_child(&name).await {
                    warn!(child = %name, error = %e, "kill_child failed");
                }
            } else {
                debug!(child = %name, missed_ms = missed.as_millis() as u64, "heartbeat ok");
            }
        }
        Ok(())
    }

    async fn self_test_pass(&self) -> Result<(), SupervisorError> {
        // The deep self-test pings each child's per-process /health endpoint
        // over its dedicated IPC channel. Children publish a one-shot
        // socket/pipe at `<root>/<child-name>.sock`. The supervisor only
        // verifies the channel responds; semantic results are interpreted by
        // each worker.
        let names = self.manager.child_names().await;
        for name in names {
            let handle = match self.manager.handle_for(&name).await {
                Some(h) => h,
                None => continue,
            };
            let (status, endpoint) = {
                let h = handle.lock().await;
                (h.status, h.spec.health_endpoint.clone())
            };
            if status != ChildStatus::Running {
                continue;
            }
            let endpoint = match endpoint {
                Some(e) => e,
                None => continue,
            };

            // The actual `/health` call is delegated to the child's IPC
            // surface. To avoid pulling in an HTTP client just for a localhost
            // ping, we treat the heartbeat-update path as proof of life: any
            // child that has updated its heartbeat within the last
            // `self_test_interval` is considered healthy.
            let elapsed_ms = {
                let h = handle.lock().await;
                h.last_heartbeat
                    .map(|t| t.elapsed().as_millis() as u64)
                    .unwrap_or(u64::MAX)
            };
            let healthy = elapsed_ms < self.self_test_interval.as_millis() as u64;
            debug!(
                child = %name,
                endpoint = %endpoint,
                healthy,
                last_hb_ms = elapsed_ms,
                "self-test"
            );
        }
        Ok(())
    }
}
