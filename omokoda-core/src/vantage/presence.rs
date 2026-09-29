use super::client::WorkspaceClient;
use std::sync::Arc;
use tokio::time::{sleep, Duration};

/// Vantage presence states. Must match backend STATES list in presence.py.
#[derive(Debug, Clone, PartialEq)]
pub enum PresenceState {
    Available,
    Thinking,
    Working,
    NeedsReview,
    Blocked,
    Offline,
}

impl PresenceState {
    pub fn as_str(&self) -> &'static str {
        match self {
            PresenceState::Available => "available",
            PresenceState::Thinking => "thinking",
            PresenceState::Working => "working",
            PresenceState::NeedsReview => "needs_review",
            PresenceState::Blocked => "blocked",
            PresenceState::Offline => "offline",
        }
    }
}

impl WorkspaceClient {
    /// Update presence for `agent_name` via PATCH /roster/{agent_name}/presence.
    pub async fn update_presence(
        &self,
        agent_name: &str,
        state: PresenceState,
    ) -> Result<(), String> {
        let url = self.guild_path(&format!("/roster/{agent_name}/presence"));
        self.put_form(&url, &[("state", state.as_str())]).await?;
        Ok(())
    }
}

/// Spawn a background presence heartbeat that fires every `interval_secs`.
/// Updates presence to the state returned by `state_fn` on each tick.
pub fn spawn_presence_heartbeat<F>(
    client: Arc<WorkspaceClient>,
    agent_name: String,
    interval_secs: u64,
    state_fn: F,
) -> tokio::task::JoinHandle<()>
where
    F: Fn() -> PresenceState + Send + Sync + 'static,
{
    tokio::spawn(async move {
        loop {
            let state = state_fn();
            let _ = client.update_presence(&agent_name, state).await;
            sleep(Duration::from_secs(interval_secs)).await;
        }
    })
}
