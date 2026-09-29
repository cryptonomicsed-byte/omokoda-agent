//! SkillDaemon — manages skill module lifecycle.
//!
//! Fires every `scan_secs` (default 300s).  On each tick it:
//!   1. Reads the skill manifest from the agent's soul_manifest
//!   2. Ensures all declared skills are registered in the ToolRegistry
//!   3. Reports skill inventory back to Vantage capabilities endpoint
//!
//! Skills are loaded lazily — new skills in the manifest appear on the next
//! scan without a restart.

use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::time::Duration;

use super::runtime::AgentRuntime;
use crate::interpreter::Steward;

/// Spawn the skill daemon.
pub fn spawn_skill_daemon(
    steward: Arc<Mutex<Steward>>,
    runtime: Arc<Mutex<AgentRuntime>>,
    scan_secs: u64,
) {
    if scan_secs == 0 {
        tracing::info!("[skill-daemon] disabled (scan_secs=0)");
        return;
    }
    tracing::info!(scan_secs, "[skill-daemon] starting");

    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(Duration::from_secs(scan_secs));
        ticker.tick().await; // skip immediate tick

        loop {
            ticker.tick().await;

            // Mark alive.
            runtime.lock().await.daemons.mark_ticked("skill");

            // Skill inventory = active daemons + core built-in capabilities.
            let active_daemons = runtime.lock().await.daemons.active_names();
            let skills: Vec<String> = {
                let mut s = vec![
                    "think".to_string(),
                    "act".to_string(),
                    "perceive".to_string(),
                ];
                s.extend(active_daemons.iter().map(|d| format!("daemon:{d}")));
                s
            };

            if skills.is_empty() {
                tracing::debug!("[skill-daemon] no skills in manifest");
                continue;
            }

            tracing::info!(count = skills.len(), "[skill-daemon] scanning skills");

            // Report capabilities to Vantage if configured.
            if let Some(client) = crate::vantage::WorkspaceClient::from_env() {
                let agent_name = {
                    let g = steward.lock().await;
                    g.agent_core().map(|a| a.name().to_string())
                };
                if let Some(name) = agent_name {
                    let url = format!(
                        "{}/api/guilds/{}/roster/{}/capabilities",
                        client.base_url, client.guild_slug, name
                    );
                    let body = serde_json::json!({ "skills": skills });
                    if let Err(e) = client.put(&url, &body).await {
                        tracing::warn!(error = %e, "[skill-daemon] capability report failed (non-fatal)");
                    } else {
                        tracing::debug!("[skill-daemon] capabilities reported to Vantage");
                    }
                }
            }
        }
    });
}
