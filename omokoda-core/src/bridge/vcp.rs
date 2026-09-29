use serde_json::{json, Value};

fn vcp_base() -> String {
    std::env::var("VCP_BROKER_URL").unwrap_or_else(|_| "http://127.0.0.1:7791".to_string())
}

/// Initiate a VCP session for a device.
/// Returns (session_id, challenge) or None if VCP is unreachable.
pub async fn initiate_session(device_id: &str, agent_id: &str) -> Option<(String, String)> {
    let client = reqwest::Client::new();
    let body = json!({
        "device_id": device_id,
        "agent_id":  agent_id,
    });
    let resp = client
        .post(format!("{}/api/vcp/sessions", vcp_base()))
        .json(&body)
        .timeout(std::time::Duration::from_secs(5))
        .send()
        .await
        .ok()?;

    if !resp.status().is_success() {
        return None;
    }
    let v: Value = resp.json().await.ok()?;
    let session_id = v.get("session_id").and_then(|s| s.as_str())?.to_string();
    let challenge = v
        .get("challenge")
        .and_then(|c| c.as_str())
        .unwrap_or("")
        .to_string();
    Some((session_id, challenge))
}

/// Authenticate an existing VCP session with a signed response.
pub async fn authenticate_session(session_id: &str, signed_response: &str) -> bool {
    let client = reqwest::Client::new();
    let body = json!({ "signed_response": signed_response });
    client
        .post(format!(
            "{}/api/vcp/sessions/{}/auth",
            vcp_base(),
            session_id
        ))
        .json(&body)
        .timeout(std::time::Duration::from_secs(5))
        .send()
        .await
        .map(|r| r.status().is_success())
        .unwrap_or(false)
}

/// Close a VCP session.
pub async fn close_session(session_id: &str) -> bool {
    let client = reqwest::Client::new();
    client
        .delete(format!("{}/api/vcp/sessions/{}", vcp_base(), session_id))
        .timeout(std::time::Duration::from_secs(5))
        .send()
        .await
        .map(|r| r.status().is_success())
        .unwrap_or(false)
}

/// Get the capability grant for an authenticated session.
pub async fn get_capability_grant(session_id: &str) -> Option<Value> {
    let client = reqwest::Client::new();
    client
        .get(format!(
            "{}/api/vcp/sessions/{}/grant",
            vcp_base(),
            session_id
        ))
        .timeout(std::time::Duration::from_secs(5))
        .send()
        .await
        .ok()?
        .json::<Value>()
        .await
        .ok()
}
