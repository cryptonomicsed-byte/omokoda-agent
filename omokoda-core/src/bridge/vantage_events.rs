use serde::{Deserialize, Serialize};
use serde_json::Value;

fn vantage_base() -> String {
    std::env::var("VANTAGE_API_URL").unwrap_or_else(|_| "http://127.0.0.1:8000".to_string())
}

fn api_key() -> String {
    std::env::var("VANTAGE_API_KEY").unwrap_or_default()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum VantageEventKind {
    TradeExecuted,
    AgentJoined,
    AgentLeft,
    ProposalCreated,
    ProposalResolved,
    Other { raw_kind: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VantageEvent {
    pub event_id: String,
    pub kind: VantageEventKind,
    pub agent_id: Option<String>,
    pub payload: Value,
    pub timestamp: i64,
}

/// Fetch the latest inbound events from Vantage for this agent.
/// Fail-open: unreachable Vantage returns empty vec.
pub async fn poll_events(agent_id: &str, since_ts: Option<i64>) -> Vec<VantageEvent> {
    let client = reqwest::Client::new();
    let key = api_key();
    let mut url = format!("{}/api/events?agent_id={}", vantage_base(), agent_id);
    if let Some(ts) = since_ts {
        url.push_str(&format!("&since={}", ts));
    }
    let mut req = client.get(&url).timeout(std::time::Duration::from_secs(5));
    if !key.is_empty() {
        req = req.header("X-Agent-Key", &key);
    }
    match req.send().await {
        Ok(r) => r.json::<Vec<VantageEvent>>().await.unwrap_or_default(),
        Err(_) => Vec::new(),
    }
}

/// Normalized event — all event kinds collapsed to a common envelope.
#[derive(Debug, Clone)]
pub struct NormalizedEvent {
    pub event_id: String,
    pub category: EventCategory,
    pub agent_id: Option<String>,
    pub payload: Value,
    pub timestamp: i64,
}

#[derive(Debug, Clone, PartialEq)]
pub enum EventCategory {
    Economic,
    Social,
    Governance,
    Other,
}

/// Fetch and normalize in one call.
pub async fn fetch_normalized(agent_id: &str, since_ts: Option<i64>) -> Vec<NormalizedEvent> {
    poll_events(agent_id, since_ts)
        .await
        .into_iter()
        .map(normalize)
        .collect()
}

fn normalize(ev: VantageEvent) -> NormalizedEvent {
    let category = match &ev.kind {
        VantageEventKind::TradeExecuted => EventCategory::Economic,
        VantageEventKind::AgentJoined | VantageEventKind::AgentLeft => EventCategory::Social,
        VantageEventKind::ProposalCreated | VantageEventKind::ProposalResolved => {
            EventCategory::Governance
        }
        VantageEventKind::Other { .. } => EventCategory::Other,
    };
    NormalizedEvent {
        event_id: ev.event_id,
        category,
        agent_id: ev.agent_id,
        payload: ev.payload,
        timestamp: ev.timestamp,
    }
}
