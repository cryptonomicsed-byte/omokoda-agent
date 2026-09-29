//! Vantage registration bridge — registers (or re-registers) this agent
//! with Vantage and retries on transient heartbeat failures.
//!
//! Fail-open: registration failure never blocks birth. The retry loop is
//! best-effort and runs in a detached task; callers do not await completion.

use serde_json::{json, Value};

fn vantage_base() -> String {
    std::env::var("VANTAGE_API_URL")
        .unwrap_or_else(|_| "http://127.0.0.1:8000".to_string())
        .trim_end_matches('/')
        .to_string()
}

fn api_key() -> String {
    std::env::var("VANTAGE_API_KEY").unwrap_or_default()
}

fn http() -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(8))
        .build()
        .unwrap_or_default()
}

/// Register this agent with Vantage (/api/agents/register) and store the
/// returned API key.  Returns the Vantage API key on success.
///
/// This is called at birth; if Vantage already has this agent the server
/// returns the existing key (idempotent).
pub async fn register(
    agent_id: &str,
    agent_name: &str,
    genesis_receipt_id: &str,
    invite_token: Option<&str>,
) -> Option<String> {
    let mut body = json!({
        "agent_id":          agent_id,
        "name":              agent_name,
        "genesis_receipt_id": genesis_receipt_id,
    });
    if let Some(token) = invite_token {
        body["invite_token"] = json!(token);
    }

    let key = api_key();
    let mut req = http()
        .post(format!("{}/api/agents/register", vantage_base()))
        .json(&body);
    if !key.is_empty() {
        req = req.header("X-Agent-Key", &key);
    }

    let resp = req.send().await.ok()?;
    if !resp.status().is_success() {
        return None;
    }

    let val: Value = resp.json().await.ok()?;
    val["api_key"]
        .as_str()
        .map(str::to_string)
        .or_else(|| val["key"].as_str().map(str::to_string))
}

/// Send a simple presence heartbeat. Returns true on success.
pub async fn heartbeat(api_key_override: Option<&str>) -> bool {
    let key = api_key_override.map(str::to_string).unwrap_or_else(api_key);
    if key.is_empty() {
        return false;
    }

    http()
        .post(format!("{}/api/me/heartbeat", vantage_base()))
        .header("X-Agent-Key", &key)
        .send()
        .await
        .map(|r| r.status().is_success())
        .unwrap_or(false)
}

/// Heartbeat with exponential back-off retries.
/// Tries up to `max_attempts` with 1 s / 2 s / 4 s delay (capped at 16 s).
/// Runs to completion in the caller's async context — spawn in a detached
/// task if you do not want to wait.
pub async fn heartbeat_with_retry(api_key_override: Option<&str>, max_attempts: u32) -> bool {
    let mut delay_secs = 1u64;
    for attempt in 0..max_attempts {
        if heartbeat(api_key_override).await {
            return true;
        }
        if attempt + 1 < max_attempts {
            tokio::time::sleep(std::time::Duration::from_secs(delay_secs)).await;
            delay_secs = (delay_secs * 2).min(16);
        }
    }
    false
}
