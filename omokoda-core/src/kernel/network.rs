//! Omo-Koda2 Kernel — Protocol Router
//!
//! Routes outbound messages from the OS kernel to the appropriate transport:
//!   • Nostr (relays) — primary pub/sub bus for agent events
//!   • DIP broker   — cross-ecosystem federated messages (port 7792)
//!   • Mesh         — LoRa/Meshtastic local broadcast
//!   • Vantage HTTP — direct REST calls to the Vantage backend
//!
//! All paths are fail-open: a transport error logs and returns an error
//! without crashing the caller.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

// ── Transport kinds ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TransportKind {
    Nostr,
    Dip,
    Mesh,
    Vantage,
}

// ── Outbound message ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkMessage {
    /// Which transport to use.
    pub transport: TransportKind,
    /// Destination: relay URL (Nostr), DIP route (Dip), device_id (Mesh), endpoint (Vantage).
    pub destination: String,
    /// Event kind (Nostr NIP kind, DIP verb, Vantage HTTP method, etc.)
    pub kind: u32,
    /// Payload serialised as JSON.
    pub payload: Value,
    /// Optional correlation / reply-to ID.
    pub correlation_id: Option<String>,
}

// ── Router ────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct NetworkConfig {
    /// Primary Nostr relay WebSocket URLs.
    pub nostr_relays: Vec<String>,
    /// Nostr agent private key (hex, 32 bytes). Empty = read-only mode.
    pub nostr_privkey: String,
    /// DIP broker base URL (default http://localhost:7792).
    pub dip_url: String,
    /// Vantage base URL (e.g. https://omokoda.duckdns.org).
    pub vantage_url: String,
    /// Vantage API key / bearer token.
    pub vantage_key: String,
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self {
            nostr_relays: vec!["wss://relay.damus.io".to_string()],
            nostr_privkey: String::new(),
            dip_url: std::env::var("DIP_URL").unwrap_or_else(|_| "http://localhost:7792".into()),
            vantage_url: std::env::var("VANTAGE_URL").unwrap_or_default(),
            vantage_key: std::env::var("VANTAGE_KEY").unwrap_or_default(),
        }
    }
}

impl NetworkConfig {
    pub fn from_env() -> Self {
        Self {
            nostr_relays: std::env::var("NOSTR_RELAYS")
                .map(|s| s.split(',').map(str::to_string).collect())
                .unwrap_or_else(|_| vec!["wss://relay.damus.io".into()]),
            nostr_privkey: std::env::var("NOSTR_PRIVKEY").unwrap_or_default(),
            dip_url: std::env::var("DIP_URL").unwrap_or_else(|_| "http://localhost:7792".into()),
            vantage_url: std::env::var("VANTAGE_URL").unwrap_or_default(),
            vantage_key: std::env::var("VANTAGE_KEY").unwrap_or_default(),
        }
    }
}

/// In-flight message counters per transport (for observability).
#[derive(Debug, Default, Clone)]
pub struct NetworkStats {
    pub sent: HashMap<String, u64>,
    pub errors: HashMap<String, u64>,
}

pub struct ProtocolRouter {
    config: NetworkConfig,
    client: reqwest::Client,
    stats: Arc<Mutex<NetworkStats>>,
}

impl ProtocolRouter {
    pub fn new(config: NetworkConfig) -> Self {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .unwrap_or_default();
        Self {
            config,
            client,
            stats: Arc::new(Mutex::new(NetworkStats::default())),
        }
    }

    pub fn with_env() -> Self {
        Self::new(NetworkConfig::from_env())
    }

    /// Route a message to the appropriate transport.
    /// Returns `Ok(())` if dispatched, `Err(description)` on failure.
    /// Callers should treat errors as advisory — the kernel continues.
    pub async fn send(&self, msg: NetworkMessage) -> Result<(), String> {
        let result = match msg.transport {
            TransportKind::Nostr => self.send_nostr(&msg).await,
            TransportKind::Dip => self.send_dip(&msg).await,
            TransportKind::Mesh => self.send_mesh(&msg).await,
            TransportKind::Vantage => self.send_vantage(&msg).await,
        };
        let key = format!("{:?}", msg.transport);
        let mut stats = self.stats.lock().await;
        if result.is_ok() {
            *stats.sent.entry(key).or_insert(0) += 1;
        } else {
            *stats.errors.entry(key).or_insert(0) += 1;
        }
        result
    }

    pub async fn stats(&self) -> NetworkStats {
        self.stats.lock().await.clone()
    }

    // ── Nostr transport ───────────────────────────────────────────────────────

    async fn send_nostr(&self, msg: &NetworkMessage) -> Result<(), String> {
        if self.config.nostr_privkey.is_empty() {
            return Err("no Nostr private key configured".into());
        }
        // Publish via DIP broker's Nostr adapter (POST /api/nostr/publish)
        // to avoid embedding a full relay client in the kernel.
        // DIP handles relay connection pooling and signature creation.
        let body = serde_json::json!({
            "kind":    msg.kind,
            "content": msg.payload.to_string(),
            "tags":    [],
            "relays":  self.config.nostr_relays,
            "privkey": self.config.nostr_privkey,
        });
        let url = format!("{}/api/nostr/publish", self.config.dip_url);
        self.post_json(&url, &body, None).await
    }

    // ── DIP transport ─────────────────────────────────────────────────────────

    async fn send_dip(&self, msg: &NetworkMessage) -> Result<(), String> {
        if self.config.dip_url.is_empty() {
            return Err("DIP_URL not configured".into());
        }
        let body = serde_json::json!({
            "route":   msg.destination,
            "kind":    msg.kind,
            "payload": msg.payload,
            "correlation_id": msg.correlation_id,
        });
        let url = format!("{}/api/route", self.config.dip_url);
        self.post_json(&url, &body, None).await
    }

    // ── Mesh transport ────────────────────────────────────────────────────────

    async fn send_mesh(&self, msg: &NetworkMessage) -> Result<(), String> {
        // Mesh send goes through the DIP broker's Meshtastic adapter.
        // The destination is the device's node ID or broadcast address.
        let body = serde_json::json!({
            "device_id": msg.destination,
            "kind":      msg.kind,
            "payload":   msg.payload,
        });
        let url = format!("{}/api/mesh/send", self.config.dip_url);
        self.post_json(&url, &body, None).await
    }

    // ── Vantage HTTP transport ────────────────────────────────────────────────

    async fn send_vantage(&self, msg: &NetworkMessage) -> Result<(), String> {
        if self.config.vantage_url.is_empty() {
            return Err("VANTAGE_URL not configured".into());
        }
        let auth = if self.config.vantage_key.is_empty() {
            None
        } else {
            Some(format!("Bearer {}", self.config.vantage_key))
        };
        let url = format!("{}{}", self.config.vantage_url, msg.destination);
        self.post_json(&url, &msg.payload, auth.as_deref()).await
    }

    // ── shared HTTP helper ────────────────────────────────────────────────────

    async fn post_json(&self, url: &str, body: &Value, auth: Option<&str>) -> Result<(), String> {
        let mut req = self
            .client
            .post(url)
            .header("Content-Type", "application/json")
            .json(body);
        if let Some(token) = auth {
            req = req.header("Authorization", token);
        }
        req.send()
            .await
            .map_err(|e| format!("network error: {e}"))?
            .error_for_status()
            .map_err(|e| format!("HTTP error: {e}"))?;
        Ok(())
    }
}

// ── Convenience constructors for common message types ────────────────────────

impl NetworkMessage {
    /// Nostr agent event (kind 30000–30006 Synapse range, or any NIP kind).
    pub fn nostr_event(relay: impl Into<String>, kind: u32, payload: Value) -> Self {
        Self {
            transport: TransportKind::Nostr,
            destination: relay.into(),
            kind,
            payload,
            correlation_id: None,
        }
    }

    /// DIP routed message to a named adapter (e.g. "a2a", "mcp", "freenet").
    pub fn dip(route: impl Into<String>, kind: u32, payload: Value) -> Self {
        Self {
            transport: TransportKind::Dip,
            destination: route.into(),
            kind,
            payload,
            correlation_id: None,
        }
    }

    /// Vantage REST endpoint (e.g. "/api/agents/heartbeat").
    pub fn vantage(endpoint: impl Into<String>, payload: Value) -> Self {
        Self {
            transport: TransportKind::Vantage,
            destination: endpoint.into(),
            kind: 0,
            payload,
            correlation_id: None,
        }
    }

    /// Meshtastic broadcast to a node ID or "broadcast".
    pub fn mesh(device_id: impl Into<String>, kind: u32, payload: Value) -> Self {
        Self {
            transport: TransportKind::Mesh,
            destination: device_id.into(),
            kind,
            payload,
            correlation_id: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn message_constructors() {
        let m = NetworkMessage::nostr_event("wss://relay", 30000, serde_json::json!({"k": "v"}));
        assert_eq!(m.transport, TransportKind::Nostr);
        assert_eq!(m.kind, 30000);

        let m2 = NetworkMessage::vantage("/api/agents/heartbeat", serde_json::json!({}));
        assert_eq!(m2.transport, TransportKind::Vantage);
        assert_eq!(m2.destination, "/api/agents/heartbeat");
    }

    #[test]
    fn config_defaults() {
        let c = NetworkConfig::default();
        assert!(!c.nostr_relays.is_empty());
        assert_eq!(c.dip_url, "http://localhost:7792");
    }
}
