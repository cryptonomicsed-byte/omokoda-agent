//! DIP bridge — sends DipEnvelope messages to the local DIP server (port 7792).
//!
//! Callers supply NetworkRepr entries; this module never hard-codes a transport.
//! Fail-open: unreachable DIP server never blocks the calling operation.
//!
//! `DipEnvelope` is re-exported from the canonical `dip-types` crate.
//! The local `NetworkRepr` is a lightweight DTO used by `habitat::types` and
//! other internal callers that only need `network` + `address`.  It is NOT the
//! same as `dip_types::NetworkRepr` (which carries `public_key` + `metadata`).

pub use dip_types::DipEnvelope;
use dip_types::{DipMessage, DipMessageKind};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

/// Lightweight network endpoint DTO used internally by Omo-Koda2.
///
/// Network-agnostic: never hardcode `network = "nostr"`.
/// The caller picks the transport; this struct carries it through.
///
/// Note: this is a simpler subset of `dip_types::NetworkRepr`.  Use
/// `dip_types::NetworkRepr` when constructing full DIP identity manifests.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkRepr {
    /// "nostr" | "meshtastic" | "a2a" | "mcp" | "libp2p" | "freenet" | "habitat"
    pub network: String,
    /// Network-specific address / pubkey / node-id.
    pub address: String,
}

/// High-level client over the raw `post_outbound` helpers below.
/// Construct with `DipBridge::new(router_url)` or let it use the default port.
pub struct DipBridge {
    pub dip_router_url: String,
}

impl DipBridge {
    /// Create a bridge pointing at a specific DIP router URL.
    pub fn new(dip_router_url: &str) -> Self {
        Self { dip_router_url: dip_router_url.trim_end_matches('/').to_string() }
    }

    /// Build an identity DipEnvelope (does not send — call `send_via_router`).
    pub fn agent_to_dip_identity(agent_id: &str, network: &NetworkRepr) -> DipEnvelope {
        let from = format!("agent:{agent_id}");
        DipEnvelope::new(
            DipMessageKind::IdentityClaim,
            &from,
            "*",
            DipMessage::IdentityClaim {
                did:      format!("did:omokoda:{agent_id}"),
                proof:    String::new(),
                networks: vec![dip_types::NetworkBinding {
                    network: network.network.clone(),
                    address: network.address.clone(),
                }],
            },
        )
    }

    /// Wrap an arbitrary action into a DipEnvelope.
    pub fn wrap_action(
        &self,
        agent_id: &str,
        action_kind: &str,
        payload: Value,
        network: &NetworkRepr,
    ) -> DipEnvelope {
        let from = format!("agent:{agent_id}");
        let to   = network.address.clone();
        DipEnvelope::new(
            DipMessageKind::AgentDelegate,
            &from,
            &to,
            DipMessage::AgentDelegate {
                task:                   action_kind.to_string(),
                params:                 payload,
                capabilities_required:  vec![],
                deadline_secs:          None,
            },
        )
    }

    /// POST the envelope to the DIP router's /api/route endpoint.
    /// Returns Ok(()) on 2xx; Err(String) on transport or HTTP error.
    pub async fn send_via_router(&self, envelope: &DipEnvelope) -> Result<(), String> {
        let client = reqwest::Client::new();
        let resp = client
            .post(format!("{}/api/route", self.dip_router_url))
            .json(envelope)
            .timeout(std::time::Duration::from_secs(10))
            .send()
            .await
            .map_err(|e| e.to_string())?;
        if resp.status().is_success() {
            Ok(())
        } else {
            Err(format!("DIP router error: {}", resp.status()))
        }
    }
}

const DIP_PORT_DEFAULT: u16 = 7792;

fn dip_base() -> String {
    let port: u16 = std::env::var("DIP_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(DIP_PORT_DEFAULT);
    format!("http://127.0.0.1:{port}")
}

fn build_envelope(
    kind: &str,
    from: &str,
    to: &str,
    payload_kind: &str,
    payload: Value,
) -> Value {
    json!({
        "envelope_id":  uuid_v4(),
        "kind":         kind,
        "from":         from,
        "to":           to,
        "payload": {
            "kind":    payload_kind,
            "content": payload,
        },
        "ttl_secs":  300,
        "created_at": chrono::Utc::now().to_rfc3339(),
        "signature": "",
    })
}

fn uuid_v4() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let t = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .subsec_nanos();
    format!("dip-{:016x}", t as u64 ^ (rand_u64()))
}

fn rand_u64() -> u64 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut h = DefaultHasher::new();
    std::thread::current().id().hash(&mut h);
    h.finish()
}

async fn post_outbound(envelope: Value) -> bool {
    let client = reqwest::Client::new();
    client
        .post(format!("{}/api/dip/outbound", dip_base()))
        .json(&envelope)
        .timeout(std::time::Duration::from_secs(3))
        .send()
        .await
        .map(|r| r.status().is_success())
        .unwrap_or(false)
}

/// Announce agent identity across requested networks.
/// `networks` is a slice of `{network, address, public_key, metadata}` objects
/// matching DIP's NetworkRepr shape (caller constructs these from IdentityVault).
/// Returns true if DIP accepted the announcement; false otherwise (fail-open).
pub async fn announce_identity(
    agent_id: &str,
    agent_did: &str,
    networks: &[Value],
) -> bool {
    let from = format!("agent:{agent_id}");
    let envelope = build_envelope(
        "identity",
        &from,
        "*",
        "identity_announce",
        json!({
            "agent_id":  agent_id,
            "did":       agent_did,
            "kind":      "agent",
            "networks":  networks,
        }),
    );
    post_outbound(envelope).await
}

/// Send a capability advertisement — lets other agents discover what this
/// agent can do (UCX tools, IfáScript tools, compute contributions, etc.).
pub async fn advertise_capability(
    agent_id: &str,
    capability_kind: &str,
    capability_meta: Value,
) -> bool {
    let from = format!("agent:{agent_id}");
    let envelope = build_envelope(
        "capability",
        &from,
        "*",
        "capability_ad",
        json!({
            "agent_id":        agent_id,
            "capability_kind": capability_kind,
            "meta":            capability_meta,
        }),
    );
    post_outbound(envelope).await
}

/// Forward a raw inbound DIP envelope received by an external adapter
/// to the DIP server's inbound endpoint for routing to Vantage/Omo-Koda2.
pub async fn forward_inbound(raw_envelope: Value) -> bool {
    let client = reqwest::Client::new();
    client
        .post(format!("{}/api/dip/inbound", dip_base()))
        .json(&raw_envelope)
        .timeout(std::time::Duration::from_secs(3))
        .send()
        .await
        .map(|r| r.status().is_success())
        .unwrap_or(false)
}
