//! ARP bridge — builds and submits AgentLifecycle ActionReceipts for the
//! three canonical lifecycle events: birth, think, and act.
//!
//! Uses the Vantage /api/arp/receipts endpoint. Fail-open: Vantage being
//! unreachable never blocks birth/think/act.
//!
//! Uses canonical arp-types::ActionReceipt (E-21 fix: replaced hand-rolled
//! JSON with the canonical ARP v1 envelope from the arp-types crate).
//!
//! Every receipt now carries a `Gix1` wire envelope (Phase 1 Step 7 of the
//! GIX spec). The `gix1_for_receipt` hand-roll has been replaced with
//! canonical `gix_types::Gix1::new`.

use arp_types::{
    ActionReceipt, ActionSpec, ReceiptKind,
    Principal, PrincipalKind,
};
use gix_types::{Gix1, GixKind, GixNamespace, RoutingHints};
use serde_json::{json, Value};
use uuid::Uuid;

fn vantage_base() -> String {
    std::env::var("VANTAGE_API_URL")
        .unwrap_or_else(|_| "http://127.0.0.1:8000".to_string())
        .trim_end_matches('/')
        .to_string()
}

fn api_key() -> String {
    std::env::var("VANTAGE_API_KEY").unwrap_or_default()
}

fn now_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

fn gix1_for_receipt(receipt_id: &str) -> Value {
    let created_at_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;

    let env = Gix1::new(
        GixKind::Receipt,
        GixNamespace::ArpReceipt,
        receipt_id.as_bytes(),
        None,
        created_at_ms,
        RoutingHints::default(),
    );

    json!({
        "canonical_id": hex::encode(env.canonical_id),
        "glyph":        env.glyph.to_string(),
        "kind":         "receipt",
        "odu_base":     env.odu_base,
        "odu_composed": env.odu_composed,
        "namespace":    "arp_receipt",
        "version":      env.version,
        "envelope_hash": hex::encode(env.integrity.envelope_hash),
    })
}

fn make_principal(agent_id: &str) -> Principal {
    Principal {
        principal_id: agent_id.to_string(),
        kind: PrincipalKind::Agent,
        agent_id: agent_id.to_string(),
        session_id: None,
        agent_tier: None,
        capabilities: vec![],
    }
}

fn make_action(action_kind: &str, target: &str, outcome: &str, params: Value) -> ActionSpec {
    ActionSpec {
        kind: action_kind.to_string(),
        target: target.to_string(),
        outcome: outcome.to_string(),
        params,
    }
}

/// Build a canonical ActionReceipt and attach a GIX1 envelope as extra JSON.
/// Returns the serialised receipt ready for POST to Vantage.
fn make_receipt_json(
    agent_id: &str,
    action_kind: &str,
    target: &str,
    outcome: &str,
    payload: Value,
    previous_hash: Option<&str>,
) -> Value {
    let receipt_id = Uuid::new_v4();
    let receipt = ActionReceipt {
        receipt_id,
        kind: ReceiptKind::AgentLifecycle,
        kind_ext: Some(action_kind.to_string()),
        principal: make_principal(agent_id),
        action: make_action(action_kind, target, outcome, payload),
        evidence_ids: vec![],
        witness_attestations: vec![],
        throne_evaluations: vec![],
        consensus_receipt: None,
        physical_attestation: None,
        zangbeto_anchor: None,
        nostr_event_id: None,
        timestamp: now_secs(),
        execution_id: None,
        previous_hash: previous_hash.map(str::to_string),
        signature: String::new(),
        gix1_canonical_id: None,
    };

    // Serialise the canonical receipt then inject the GIX1 envelope field.
    let mut v = serde_json::to_value(&receipt).unwrap_or_else(|_| json!({}));
    let gix1 = gix1_for_receipt(&receipt_id.to_string());
    if let Some(obj) = v.as_object_mut() {
        obj.insert("gix1".to_string(), gix1);
    }
    v
}

async fn post_receipt(receipt: Value) -> bool {
    let client = reqwest::Client::new();
    let key = api_key();
    let mut req = client
        .post(format!("{}/api/arp/receipts", vantage_base()))
        .json(&receipt)
        .timeout(std::time::Duration::from_secs(4));
    if !key.is_empty() {
        req = req.header("X-Agent-Key", &key);
    }
    req.send().await.map(|r| r.status().is_success()).unwrap_or(false)
}

/// Birth receipt — emitted once at agent creation.
pub async fn receipt_birth(
    agent_id: &str,
    genesis_receipt_id: &str,
    agent_name: &str,
) -> bool {
    let receipt = make_receipt_json(
        agent_id,
        "birth",
        genesis_receipt_id,
        "success",
        json!({ "agent_name": agent_name, "genesis_receipt_id": genesis_receipt_id }),
        None,
    );
    post_receipt(receipt).await
}

/// Think receipt — emitted after each THINK execution.
pub async fn receipt_think(
    agent_id: &str,
    think_id: &str,
    prompt_summary: &str,
    previous_hash: Option<&str>,
) -> bool {
    let receipt = make_receipt_json(
        agent_id,
        "think",
        think_id,
        "success",
        json!({ "prompt_summary": prompt_summary }),
        previous_hash,
    );
    post_receipt(receipt).await
}

/// Act receipt — emitted after each ACT/tool execution.
pub async fn receipt_act(
    agent_id: &str,
    act_id: &str,
    tool_name: &str,
    outcome: &str,
    previous_hash: Option<&str>,
) -> bool {
    let receipt = make_receipt_json(
        agent_id,
        "act",
        act_id,
        outcome,
        json!({ "tool_name": tool_name }),
        previous_hash,
    );
    post_receipt(receipt).await
}

/// Lifecycle transition receipt — emitted on Born/Migrate/Fork/Hibernate/Wake/Terminate.
pub async fn receipt_lifecycle_transition(
    agent_id: &str,
    transition_kind: &str,
    from_stage: &str,
    to_stage: &str,
    node_pubkey: &str,
    previous_hash: Option<&str>,
) -> bool {
    let receipt = make_receipt_json(
        agent_id,
        transition_kind,
        agent_id,
        "success",
        json!({
            "from_stage":  from_stage,
            "to_stage":    to_stage,
            "node_pubkey": node_pubkey,
        }),
        previous_hash,
    );
    post_receipt(receipt).await
}
