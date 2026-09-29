//! JobDaemon — polls Vantage for assigned tasks and executes them.
//!
//! Fires every `poll_secs` (default 60s).  On each tick it:
//!   1. GETs /guilds/{slug}/tasks?status=assigned&assigned_to={agent_id}
//!   2. For each task, dispatches `Statement::Act` via the Steward
//!   3. PATCHes the task to completed/failed based on the result
//!
//! The daemon marks itself in the DaemonRegistry so the heartbeat chain
//! reflects it as an active daemon.

use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::time::Duration;

use super::runtime::AgentRuntime;
use crate::interpreter::Steward;
use crate::parser::Statement;

/// Spawn the job daemon.  Fire-and-forget; a crash logs and restarts.
pub fn spawn_job_daemon(
    steward: Arc<Mutex<Steward>>,
    runtime: Arc<Mutex<AgentRuntime>>,
    poll_secs: u64,
) {
    if poll_secs == 0 {
        tracing::info!("[job-daemon] disabled (poll_secs=0)");
        return;
    }
    tracing::info!(poll_secs, "[job-daemon] starting");

    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(Duration::from_secs(poll_secs));
        ticker.tick().await; // skip immediate tick

        loop {
            ticker.tick().await;

            // Mark daemon as alive in the registry.
            runtime.lock().await.daemons.mark_ticked("job");

            // Pull agent identity and Vantage client; skip if not configured.
            let (agent_id, agent_name) = {
                let g = steward.lock().await;
                let id = g.agent_core().map(|a| a.id().as_str().to_string());
                let name = g.agent_core().map(|a| a.name().to_string());
                (id, name)
            };
            let (Some(agent_id), Some(agent_name)) = (agent_id, agent_name) else {
                tracing::debug!("[job-daemon] no agent yet, skipping");
                continue;
            };
            let Some(client) = crate::vantage::WorkspaceClient::from_env() else {
                tracing::debug!("[job-daemon] no VANTAGE_URL, skipping");
                continue;
            };

            // Fetch assigned tasks.
            let url = format!(
                "{}/api/guilds/{}/tasks?status=assigned&assigned_to={}",
                client.base_url, client.guild_slug, agent_id
            );
            let tasks = match client.get(&url).await {
                Ok(v) => v,
                Err(e) => {
                    tracing::warn!(error = %e, "[job-daemon] fetch tasks failed");
                    continue;
                }
            };

            let task_list = tasks.as_array().cloned().unwrap_or_default();
            if task_list.is_empty() {
                tracing::debug!("[job-daemon] no assigned tasks");
                continue;
            }

            tracing::info!(count = task_list.len(), "[job-daemon] executing tasks");

            for task in task_list {
                let task_id = task["id"].as_u64().unwrap_or(0);
                let task_desc = task["description"].as_str().unwrap_or("").to_string();
                let task_url = format!(
                    "{}/api/guilds/{}/tasks/{}",
                    client.base_url, client.guild_slug, task_id
                );

                // Build a think prompt from the task description.
                let prompt = format!(
                    "You have been assigned task #{task_id}: {task_desc}\n\
                     Complete this task and report back what you did."
                );

                let result = {
                    let mut g = steward.lock().await;
                    g.dispatch(Statement::Think {
                        prompt,
                        private: false,
                        modifiers: Default::default(),
                    })
                    .await
                };

                let (new_status, result_text) = match result {
                    Ok(resp) => (
                        "completed",
                        resp.tool_output
                            .unwrap_or_default()
                            .chars()
                            .take(1000)
                            .collect::<String>(),
                    ),
                    Err(e) => ("failed", format!("Error: {e}")),
                };

                // Patch task status back to Vantage.
                let patch_body = serde_json::json!({
                    "status": new_status,
                    "result": result_text,
                    "completed_by": agent_name,
                });
                if let Err(e) = client.patch(&task_url, &patch_body).await {
                    tracing::warn!(task_id, error = %e, "[job-daemon] patch task failed");
                } else {
                    tracing::info!(task_id, status = new_status, "[job-daemon] task done");
                }
            }
        }
    });
}
