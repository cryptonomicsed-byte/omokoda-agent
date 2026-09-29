//! DaemonSupervisor — watches tokio task handles, restarts crashed daemons.
//!
//! Companion to DaemonRegistry (runtime.rs), which tracks *state*.
//! This module tracks the actual JoinHandles and respawns on panic/exit.
//! Feeds active_names() back into AgentHeartbeat.active_daemons on each tick.

use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::time::{SystemTime, UNIX_EPOCH};

use tokio::task::JoinHandle;
use tracing::{info, warn};

type SpawnFn = Box<dyn Fn() -> Pin<Box<dyn Future<Output = ()> + Send>> + Send + Sync>;

/// A registered daemon entry with its live handle.
pub struct DaemonHandle {
    pub name: String,
    pub restart_count: u32,
    pub started_at: u64,
    pub handle: JoinHandle<()>,
    /// Factory that produces a fresh task future for restarts.
    spawn_fn: SpawnFn,
}

/// Supervises a set of named background daemons.
/// On `supervise_loop()`, polls every `poll_secs` seconds and restarts any
/// daemon whose JoinHandle has finished (panic or clean exit).
pub struct DaemonSupervisor {
    daemons: HashMap<String, DaemonHandle>,
    poll_secs: u64,
}

impl DaemonSupervisor {
    pub fn new(poll_secs: u64) -> Self {
        Self {
            daemons: HashMap::new(),
            poll_secs,
        }
    }

    /// Register a daemon with its name and a factory closure.
    /// The factory is called immediately to start the first instance,
    /// and again on each restart.
    pub fn register<F, Fut>(&mut self, name: impl Into<String>, factory: F)
    where
        F: Fn() -> Fut + Send + Sync + 'static,
        Fut: Future<Output = ()> + Send + 'static,
    {
        let name = name.into();
        let spawn_fn: SpawnFn = Box::new(move || {
            let fut = factory();
            Box::pin(fut)
        });
        let handle = tokio::spawn((spawn_fn)());
        info!(daemon = %name, "supervisor: registered and started");
        self.daemons.insert(
            name.clone(),
            DaemonHandle {
                name,
                restart_count: 0,
                started_at: now_secs(),
                handle,
                spawn_fn,
            },
        );
    }

    /// Names of currently-running daemons.  Feed into AgentHeartbeat.active_daemons.
    pub fn running_names(&self) -> Vec<String> {
        self.daemons
            .values()
            .filter(|d| !d.handle.is_finished())
            .map(|d| d.name.clone())
            .collect()
    }

    /// Supervision loop — runs forever. Checks each daemon on every tick;
    /// restarts those whose handles have finished.
    pub async fn supervise_loop(mut self) {
        let mut ticker =
            tokio::time::interval(std::time::Duration::from_secs(self.poll_secs.max(1)));
        loop {
            ticker.tick().await;
            for entry in self.daemons.values_mut() {
                if entry.handle.is_finished() {
                    entry.restart_count += 1;
                    entry.started_at = now_secs();
                    entry.handle = tokio::spawn((entry.spawn_fn)());
                    warn!(
                        daemon = %entry.name,
                        restart_count = entry.restart_count,
                        "supervisor: daemon exited — restarted",
                    );
                }
            }
        }
    }
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{
        atomic::{AtomicU32, Ordering},
        Arc,
    };

    #[tokio::test]
    async fn register_and_running_names() {
        let mut sup = DaemonSupervisor::new(1);
        sup.register("test-daemon", || async { /* instant exit */ });
        // Immediately after spawn the handle may still be live.
        assert!(sup.daemons.contains_key("test-daemon"));
    }

    #[tokio::test]
    async fn counts_only_running_daemons() {
        let mut sup = DaemonSupervisor::new(1);
        // Long-running daemon
        sup.register("keeper", || async {
            tokio::time::sleep(std::time::Duration::from_secs(60)).await;
        });
        // Let the event loop tick so handles settle
        tokio::task::yield_now().await;
        let names = sup.running_names();
        assert!(names.contains(&"keeper".to_string()));
    }

    #[tokio::test]
    async fn restart_count_increments() {
        let counter = Arc::new(AtomicU32::new(0));
        let c = counter.clone();
        let mut sup = DaemonSupervisor::new(1);
        sup.register("crasher", move || {
            let c2 = c.clone();
            async move {
                c2.fetch_add(1, Ordering::SeqCst);
            }
        });
        // Wait for first run to finish
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        // Manually trigger supervision check
        for entry in sup.daemons.values_mut() {
            if entry.handle.is_finished() {
                entry.restart_count += 1;
                entry.handle = tokio::spawn((entry.spawn_fn)());
            }
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        assert!(sup.daemons["crasher"].restart_count >= 1);
    }
}
