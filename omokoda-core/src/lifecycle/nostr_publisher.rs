/// Autonomous Nostr publisher daemon — Phase 10.3
///
/// Publishes kind 1 status notes on a timed schedule (max 4/day per agent).
/// All network failures are caught and logged; the daemon never panics.
/// Relay URLs are read from config/env — never hardcoded here.
use serde::{Deserialize, Serialize};

/// Configuration for the Nostr publisher daemon.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NostrPublisherConfig {
    /// Maximum kind-1 notes published per 24-hour window (default: 4).
    #[serde(default = "default_max_notes")]
    pub max_notes_per_day: u32,
    /// Relay WebSocket URLs.  Populated from `IdentityVaultData::relay_list`
    /// or the `AGENT_NOSTR_RELAYS` env var (comma-separated).  Never
    /// hardcoded; an empty list is a valid no-op state.
    #[serde(default)]
    pub relay_list: Vec<String>,
}

fn default_max_notes() -> u32 {
    4
}

impl Default for NostrPublisherConfig {
    fn default() -> Self {
        Self {
            max_notes_per_day: default_max_notes(),
            relay_list: Vec::new(),
        }
    }
}

/// Internal state tracked by one publisher task.
struct PublisherState {
    agent_id: String,
    npub: String,
    nsec: String,
    config: NostrPublisherConfig,
    /// Notes published in the current 24-hour window.
    notes_today: u32,
    /// Unix timestamp when the current window started.
    window_start: u64,
}

impl PublisherState {
    fn new(agent_id: String, npub: String, nsec: String, config: NostrPublisherConfig) -> Self {
        Self {
            agent_id,
            npub,
            nsec,
            config,
            notes_today: 0,
            window_start: current_unix_ts(),
        }
    }

    /// Reset daily counter when 24 hours have passed.
    fn tick_window(&mut self) {
        let now = current_unix_ts();
        if now.saturating_sub(self.window_start) >= 86_400 {
            self.notes_today = 0;
            self.window_start = now;
        }
    }

    /// Returns true if we are still within the daily rate limit.
    fn within_limit(&self) -> bool {
        self.notes_today < self.config.max_notes_per_day
    }

    /// Effective relay list: config overrides env fallback.
    fn effective_relays(&self) -> Vec<String> {
        if !self.config.relay_list.is_empty() {
            return self.config.relay_list.clone();
        }
        std::env::var("AGENT_NOSTR_RELAYS")
            .unwrap_or_default()
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect()
    }
}

/// Spawn the autonomous Nostr publisher daemon.
///
/// The returned `JoinHandle` runs indefinitely; drop it (or abort it) to
/// stop the daemon.  Failures are logged as warnings — the task never panics.
///
/// # Parameters
/// - `agent_id`  — agent identifier string (used in log context only).
/// - `npub`      — agent's bech32 npub (Nostr public key).
/// - `nsec`      — agent's Nostr private key hex (`nostr_private_key_hex`
///   from `IdentityVaultData`).
/// - `config`    — rate-limit + relay configuration.
pub fn spawn_nostr_publisher(
    agent_id: String,
    npub: String,
    nsec: String,
    config: NostrPublisherConfig,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut state = PublisherState::new(agent_id, npub, nsec, config);
        // Sleep interval: 6 hours (21 600 seconds).  4 posts × 6 h = 24 h max.
        let interval = tokio::time::Duration::from_secs(6 * 3600);
        loop {
            // Wait before first post so birth spam is avoided.
            tokio::time::sleep(interval).await;

            state.tick_window();

            if !state.within_limit() {
                tracing::debug!(
                    agent_id = %state.agent_id,
                    notes_today = state.notes_today,
                    max = state.config.max_notes_per_day,
                    "nostr_publisher: daily limit reached, sleeping"
                );
                continue;
            }

            let relays = state.effective_relays();
            if relays.is_empty() {
                tracing::debug!(
                    agent_id = %state.agent_id,
                    "nostr_publisher: no relays configured, skipping"
                );
                continue;
            }

            // Build the status note content.
            let content = build_status_note(&state.agent_id, &state.npub);

            // Delegate to the shared fire-and-forget publisher.
            // build_kind1_event returns a serde_json::Value.
            let event = crate::nostr_events::build_status_note_event(&state.npub, &content);

            match crate::nostr_events::publish_event(event, &state.nsec, &relays).await {
                Ok(()) => {
                    state.notes_today += 1;
                    tracing::info!(
                        agent_id = %state.agent_id,
                        notes_today = state.notes_today,
                        "nostr_publisher: published status note"
                    );
                }
                Err(e) => {
                    // Fail-open: log and continue — relay unreachability must
                    // never crash or halt the agent.
                    tracing::warn!(
                        agent_id = %state.agent_id,
                        err = %e,
                        "nostr_publisher: publish failed (fail-open)"
                    );
                }
            }
        }
    })
}

/// Build the plain-text content for a periodic status note.
///
/// Odù name is derived from the agent_id hash for determinism without needing
/// access to the full session state inside the daemon.
fn build_status_note(agent_id: &str, _npub: &str) -> String {
    // Derive a stable Odù index from agent_id so the note has cultural
    // flavour without needing the full hermetic runtime here.
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut h = DefaultHasher::new();
    agent_id.hash(&mut h);
    let odu_index = (h.finish() % 256) as usize;
    static ODU_NAMES: &[&str] = &[
        "Ogbe-Meji",
        "Oyeku-Ogbe",
        "Iwori-Ogbe",
        "Odi-Ogbe",
        "Irosun-Ogbe",
        "Owonrin-Ogbe",
        "Obara-Ogbe",
        "Okonron-Ogbe",
        "Ogunda-Ogbe",
        "Osa-Ogbe",
        "Ika-Ogbe",
        "Oturupon-Ogbe",
        "Otura-Ogbe",
        "Irete-Ogbe",
        "Ose-Ogbe",
        "Ofu-Ogbe",
    ];
    let odu_name = ODU_NAMES
        .get(odu_index % ODU_NAMES.len())
        .unwrap_or(&"Ogbe-Meji");

    // BTC height is a best-effort env hint; falls back to "unknown".
    let btc_height = std::env::var("BTC_BLOCK_HEIGHT").unwrap_or_else(|_| "unknown".to_string());

    // Shorten the agent_id for the note (first 12 chars).
    let short_id = if agent_id.len() > 12 {
        &agent_id[..12]
    } else {
        agent_id
    };

    format!(
        "Agent {} active. Odù: {}. Block: {}",
        short_id, odu_name, btc_height
    )
}

fn current_unix_ts() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}
