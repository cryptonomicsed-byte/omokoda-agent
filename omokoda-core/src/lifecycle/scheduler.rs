//! Multi-interval agent scheduler — the circadian rhythm of the sovereign OS.
//!
//! Each tier runs on its own cadence and calls a distinct function class.
//! The 5-minute cognitive cycle is handled by `spawn_heartbeat()` in server.rs;
//! this scheduler handles everything else.
//!
//! Interval tiers:
//!   30s   — SecurityTick: identity integrity + anomaly check
//!   60s   — PresenceTick: Vantage last_seen_at refresh (keeps agent non-stale between cognitives)
//!   30min — LearningTick: short-term memory consolidation
//!   1hr   — StrategicTick: goal review + opportunity evaluation
//!   6hr   — MemoryTick: deep archive + compaction
//!   24hr  — EconomicTick: Synapse accounting + tier re-evaluation

use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tracing::{debug, info, warn};

use crate::interpreter::Steward;

/// Interval configuration (seconds). Set any to 0 to disable that tier.
#[derive(Debug, Clone)]
pub struct SchedulerConfig {
    pub security_secs: u64,  // default 30
    pub presence_secs: u64,  // default 60
    pub learning_secs: u64,  // default 1800  (30min)
    pub strategic_secs: u64, // default 3600  (1hr)
    pub memory_secs: u64,    // default 21600 (6hr)
    pub economic_secs: u64,  // default 86400 (24hr)
}

impl Default for SchedulerConfig {
    fn default() -> Self {
        Self {
            security_secs: 30,
            presence_secs: 60,
            learning_secs: 1800,
            strategic_secs: 3600,
            memory_secs: 21600,
            economic_secs: 86400,
        }
    }
}

impl SchedulerConfig {
    /// Build from environment variables. Falls back to defaults.
    pub fn from_env() -> Self {
        fn env_u64(key: &str, default: u64) -> u64 {
            std::env::var(key)
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(default)
        }
        Self {
            security_secs: env_u64("SCHEDULER_SECURITY_SECS", 30),
            presence_secs: env_u64("SCHEDULER_PRESENCE_SECS", 60),
            learning_secs: env_u64("SCHEDULER_LEARNING_SECS", 1800),
            strategic_secs: env_u64("SCHEDULER_STRATEGIC_SECS", 3600),
            memory_secs: env_u64("SCHEDULER_MEMORY_SECS", 21600),
            economic_secs: env_u64("SCHEDULER_ECONOMIC_SECS", 86400),
        }
    }
}

/// Spawns all background scheduler tasks. Returns immediately; tasks run forever.
pub fn spawn_scheduler(steward: Arc<Mutex<Steward>>, config: SchedulerConfig) {
    if config.security_secs > 0 {
        spawn_security_tick(config.security_secs);
    }
    if config.presence_secs > 0 {
        spawn_presence_tick(config.presence_secs);
    }
    if config.learning_secs > 0 {
        spawn_learning_tick(steward.clone(), config.learning_secs);
    }
    if config.strategic_secs > 0 {
        spawn_strategic_tick(steward.clone(), config.strategic_secs);
    }
    if config.memory_secs > 0 {
        spawn_memory_tick(steward.clone(), config.memory_secs);
    }
    if config.economic_secs > 0 {
        spawn_economic_tick(steward.clone(), config.economic_secs);
    }
    info!(
        security_secs = config.security_secs,
        presence_secs = config.presence_secs,
        learning_secs = config.learning_secs,
        strategic_secs = config.strategic_secs,
        memory_secs = config.memory_secs,
        economic_secs = config.economic_secs,
        "AgentScheduler started"
    );
}

// ── 30s — SecurityTick ────────────────────────────────────────────────────────

fn spawn_security_tick(interval_secs: u64) {
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(Duration::from_secs(interval_secs));
        ticker.tick().await; // skip first immediate fire
        loop {
            ticker.tick().await;
            debug!("[scheduler:security] integrity check");
            // Identity hash check — verify the agent's on-disk key hasn't drifted.
            // Expand here when NodeIdentity is wired to omokoda-core.
        }
    });
}

// ── 60s — PresenceTick ────────────────────────────────────────────────────────

fn spawn_presence_tick(interval_secs: u64) {
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(Duration::from_secs(interval_secs));
        ticker.tick().await;
        loop {
            ticker.tick().await;
            // Lightweight ping — just refreshes last_seen_at without a full cognitive cycle.
            // The cognitive heartbeat (spawn_heartbeat in server.rs) also calls this at 5min;
            // this 60s tick fills the gaps so the agent never appears stale to Vantage.
            if let Some(client) = crate::vantage::WorkspaceClient::from_env() {
                match client.heartbeat().await {
                    Ok(_) => debug!("[scheduler:presence] last_seen_at refreshed"),
                    Err(e) => {
                        warn!(error = %e, "[scheduler:presence] heartbeat failed (non-fatal)")
                    }
                }
                match client.mesh_heartbeat().await {
                    Ok(_) => debug!("[scheduler:presence] mesh last_seen_at refreshed"),
                    Err(e) => {
                        warn!(error = %e, "[scheduler:presence] mesh heartbeat failed (non-fatal)")
                    }
                }
            }
        }
    });
}

// ── 30min — LearningTick ─────────────────────────────────────────────────────

fn spawn_learning_tick(steward: Arc<Mutex<Steward>>, interval_secs: u64) {
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(Duration::from_secs(interval_secs));
        ticker.tick().await;
        loop {
            ticker.tick().await;
            info!("[scheduler:learning] short-term memory consolidation tick");
            let guard = steward.lock().await;
            let agent_id = guard.agent_core().map(|a| a.id().as_str().to_string());
            drop(guard);
            if let Some(id) = agent_id {
                debug!(agent_id = %id, "[scheduler:learning] consolidation started");
                // Memory consolidation logic will be added when memory module is extended.
                // Hook: call steward.dispatch(Statement::Think { prompt: consolidation_prompt })
            }
        }
    });
}

// ── 1hr — StrategicTick ──────────────────────────────────────────────────────

fn spawn_strategic_tick(steward: Arc<Mutex<Steward>>, interval_secs: u64) {
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(Duration::from_secs(interval_secs));
        ticker.tick().await;
        loop {
            ticker.tick().await;
            info!("[scheduler:strategic] goal review + opportunity evaluation");
            let guard = steward.lock().await;
            let has_agent = guard.agent_core().is_some();
            drop(guard);
            if !has_agent {
                debug!("[scheduler:strategic] no agent born yet — skipping");
            }
        }
    });
}

// ── 6hr — MemoryTick ─────────────────────────────────────────────────────────

fn spawn_memory_tick(steward: Arc<Mutex<Steward>>, interval_secs: u64) {
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(Duration::from_secs(interval_secs));
        ticker.tick().await;
        loop {
            ticker.tick().await;
            info!("[scheduler:memory] deep archive + compaction");
            let guard = steward.lock().await;
            let has_agent = guard.agent_core().is_some();
            drop(guard);
            if has_agent {
                // Trigger context compaction and memory archiving.
                // Hook: send LoopEvent::Compact via LoopHandle when wired.
                debug!("[scheduler:memory] compaction deferred to LoopHandle integration");
            }
        }
    });
}

// ── 24hr — EconomicTick ──────────────────────────────────────────────────────

fn spawn_economic_tick(steward: Arc<Mutex<Steward>>, interval_secs: u64) {
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(Duration::from_secs(interval_secs));
        ticker.tick().await;
        loop {
            ticker.tick().await;
            info!("[scheduler:economic] Synapse accounting + tier re-evaluation");
            let guard = steward.lock().await;
            let agent_id = guard.agent_core().map(|a| a.id().as_str().to_string());
            drop(guard);
            if let Some(id) = agent_id {
                debug!(agent_id = %id, "[scheduler:economic] daily accounting tick");
                // Apply 1%/day Synapse decay; re-evaluate reputation tier.
                // Hook: call economics module when Synapse decay is wired.
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_has_canonical_intervals() {
        let cfg = SchedulerConfig::default();
        assert_eq!(cfg.security_secs, 30);
        assert_eq!(cfg.presence_secs, 60);
        assert_eq!(cfg.learning_secs, 1800);
        assert_eq!(cfg.strategic_secs, 3600);
        assert_eq!(cfg.memory_secs, 21600);
        assert_eq!(cfg.economic_secs, 86400);
    }

    #[test]
    fn zero_disables_tier() {
        let cfg = SchedulerConfig {
            security_secs: 0,
            ..Default::default()
        };
        assert_eq!(cfg.security_secs, 0);
    }
}
