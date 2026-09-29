//! NetworkRouter — transport-agnostic message routing for Ọmọ Kọ́dà agents.
//!
//! An agent knows WHERE it wants to reach (agent_id or address) but doesn't
//! need to know HOW to get there. The NetworkRouter:
//!   1. Resolves a destination to one or more available transports
//!   2. Selects the best transport by preference order
//!   3. Delegates to the registered EcosystemAdapter for that transport
//!   4. Returns a RouteReceipt with proof of attempted delivery
//!
//! Transport preference order (sovereign-first):
//!   Meshtastic > Reticulum > Nostr > IpLayer > Freenet > Sui > Web2 > Fallback

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fmt;

// ─────────────────────────────────────────────────────────────────────────────
// Transport tier — sovereign/mesh preferred over Internet
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TransportTier {
    /// Physical mesh — fully sovereign, no Internet required
    Mesh = 0,
    /// Censorship-resistant overlay (Freenet / Reticulum)
    Overlay = 1,
    /// Decentralised pubkey infrastructure (Nostr / IP-Layer)
    Decentralised = 2,
    /// Blockchain-settled (Sui / Bitcoin)
    Onchain = 3,
    /// Conventional Internet (HTTPS / WebSocket)
    Web = 4,
}

impl fmt::Display for TransportTier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Self::Mesh => "mesh",
            Self::Overlay => "overlay",
            Self::Decentralised => "decentralised",
            Self::Onchain => "onchain",
            Self::Web => "web",
        };
        write!(f, "{}", s)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// RouteMessage — the envelope the router carries
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteMessage {
    pub from_agent_id: String,
    pub to_agent_id: String,
    /// Resolved destination address on the chosen transport (pubkey, relay URL, etc.)
    pub destination_address: Option<String>,
    pub payload: Vec<u8>,
    pub content_type: String,
    pub created_at: u64,
    /// Requested transport preference; None = let the router choose
    pub preferred_transport: Option<String>,
}

impl RouteMessage {
    pub fn new(from: &str, to: &str, payload: Vec<u8>, content_type: &str) -> Self {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        Self {
            from_agent_id: from.to_string(),
            to_agent_id: to.to_string(),
            destination_address: None,
            payload,
            content_type: content_type.to_string(),
            created_at: now,
            preferred_transport: None,
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// RouteReceipt — proof of attempted delivery
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteReceipt {
    pub receipt_id: String,
    pub from_agent_id: String,
    pub to_agent_id: String,
    pub transport_used: String,
    pub transport_tier: TransportTier,
    pub destination_address: String,
    pub sent_at: u64,
    pub status: RouteStatus,
    /// SHA-256(from || to || transport || sent_at || payload_len)
    pub integrity_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RouteStatus {
    Delivered,
    Attempted,
    Failed { reason: String },
}

// ─────────────────────────────────────────────────────────────────────────────
// Transport — a registered transport the router can use
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum RouterError {
    NoRoute(String),
    Transport(String),
    Resolution(String),
}

impl fmt::Display for RouterError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoRoute(s) => write!(f, "no route to: {s}"),
            Self::Transport(s) => write!(f, "transport error: {s}"),
            Self::Resolution(s) => write!(f, "address resolution: {s}"),
        }
    }
}

/// A transport registered with the router.
/// Implementors handle the actual send for a given tier of network.
#[async_trait]
pub trait Transport: Send + Sync {
    fn transport_id(&self) -> &str;
    fn tier(&self) -> TransportTier;

    /// Resolve agent_id → transport-specific address, if known.
    /// Returns None if this transport cannot reach that agent.
    async fn resolve(&self, agent_id: &str) -> Option<String>;

    /// Send payload to address. Returns delivery address on success.
    async fn send(&self, address: &str, payload: &[u8]) -> Result<String, RouterError>;

    fn is_available(&self) -> bool {
        true
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// NetworkRouter
// ─────────────────────────────────────────────────────────────────────────────

pub struct NetworkRouter {
    /// transport_id → Box<dyn Transport>, ordered by preference
    transports: Vec<(String, Box<dyn Transport>)>,
}

impl NetworkRouter {
    pub fn new() -> Self {
        Self {
            transports: Vec::new(),
        }
    }

    /// Register a transport. Lower-tier (sovereign) transports should be
    /// registered first to maintain preference order.
    pub fn register(&mut self, transport: Box<dyn Transport>) {
        let id = transport.transport_id().to_string();
        self.transports.push((id, transport));
        // Keep sorted by tier (ascending = sovereign first)
        self.transports.sort_by_key(|(_, t)| t.tier().clone());
    }

    /// Route a message using the best available transport.
    pub async fn route(&self, msg: &RouteMessage) -> Result<RouteReceipt, RouterError> {
        let sent_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        // If caller specified a transport, try that one first
        if let Some(ref preferred) = msg.preferred_transport {
            if let Some((_, transport)) = self.transports.iter().find(|(id, _)| id == preferred) {
                if transport.is_available() {
                    if let Some(addr) = transport.resolve(&msg.to_agent_id).await {
                        match transport.send(&addr, &msg.payload).await {
                            Ok(final_addr) => {
                                return Ok(self.build_receipt(
                                    msg,
                                    transport.as_ref(),
                                    &final_addr,
                                    sent_at,
                                    RouteStatus::Delivered,
                                ));
                            }
                            Err(e) => {
                                return Ok(self.build_receipt(
                                    msg,
                                    transport.as_ref(),
                                    &addr,
                                    sent_at,
                                    RouteStatus::Failed {
                                        reason: e.to_string(),
                                    },
                                ));
                            }
                        }
                    }
                }
            }
        }

        // Try all transports in preference order (sovereign first)
        for (_, transport) in &self.transports {
            if !transport.is_available() {
                continue;
            }
            // Use provided destination_address or resolve from agent_id
            let addr = match &msg.destination_address {
                Some(a) => Some(a.clone()),
                None => transport.resolve(&msg.to_agent_id).await,
            };
            if let Some(addr) = addr {
                match transport.send(&addr, &msg.payload).await {
                    Ok(final_addr) => {
                        return Ok(self.build_receipt(
                            msg,
                            transport.as_ref(),
                            &final_addr,
                            sent_at,
                            RouteStatus::Delivered,
                        ));
                    }
                    Err(_) => continue,
                }
            }
        }

        Err(RouterError::NoRoute(msg.to_agent_id.clone()))
    }

    fn build_receipt(
        &self,
        msg: &RouteMessage,
        transport: &dyn Transport,
        address: &str,
        sent_at: u64,
        status: RouteStatus,
    ) -> RouteReceipt {
        let mut h = Sha256::new();
        h.update(msg.from_agent_id.as_bytes());
        h.update(msg.to_agent_id.as_bytes());
        h.update(transport.transport_id().as_bytes());
        h.update(sent_at.to_le_bytes());
        h.update((msg.payload.len() as u64).to_le_bytes());
        let integrity_hash = hex::encode(h.finalize());

        let receipt_id = {
            let mut h2 = Sha256::new();
            h2.update(integrity_hash.as_bytes());
            h2.update(b"route-receipt-v1");
            hex::encode(&h2.finalize()[..8])
        };

        RouteReceipt {
            receipt_id,
            from_agent_id: msg.from_agent_id.clone(),
            to_agent_id: msg.to_agent_id.clone(),
            transport_used: transport.transport_id().to_string(),
            transport_tier: transport.tier(),
            destination_address: address.to_string(),
            sent_at,
            status,
            integrity_hash,
        }
    }

    pub fn available_transports(&self) -> Vec<&str> {
        self.transports
            .iter()
            .filter(|(_, t)| t.is_available())
            .map(|(id, _)| id.as_str())
            .collect()
    }
}

impl Default for NetworkRouter {
    fn default() -> Self {
        Self::new()
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// UAIL — Universal Agent Identity Layer
//
// Answers: "Given agent_id X, what addresses do I have for them on each
// transport?" Maintains a local resolution cache populated from:
//   - AgentManifest bindings (own agent or received over network)
//   - IP-Layer Nostr events (kind 31900)
//   - Received AgentCapsules
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UailRecord {
    pub agent_id: String,
    /// transport_id → address
    pub addresses: HashMap<String, String>,
    pub last_seen: u64,
    pub verified: bool,
}

#[derive(Debug, Default)]
pub struct UniversalAgentIdentityLayer {
    records: HashMap<String, UailRecord>,
}

impl UniversalAgentIdentityLayer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn upsert(&mut self, agent_id: &str, transport: &str, address: &str) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        let record = self
            .records
            .entry(agent_id.to_string())
            .or_insert_with(|| UailRecord {
                agent_id: agent_id.to_string(),
                addresses: HashMap::new(),
                last_seen: now,
                verified: false,
            });
        record
            .addresses
            .insert(transport.to_string(), address.to_string());
        record.last_seen = now;
    }

    /// Load all bindings from a manifest into the UAIL cache.
    pub fn ingest_manifest(&mut self, manifest: &crate::genesis::manifest::AgentManifest) {
        let id = &manifest.identity.agent_id;
        if let Some(ref pk) = manifest.network.nostr_pubkey {
            self.upsert(id, "nostr", pk);
        }
        if let Some(ref addr) = manifest.network.sui_address {
            self.upsert(id, "sui", addr);
        }
        if let Some(ref addr) = manifest.network.btc_address {
            self.upsert(id, "bitcoin", addr);
        }
        if let Some(ref addr) = manifest.network.eth_address {
            self.upsert(id, "ethereum", addr);
        }
        if let Some(ref ev) = manifest.network.ip_root_event {
            self.upsert(id, "ip_layer", ev);
        }
    }

    /// Load bindings from an AgentCapsule into the UAIL cache.
    pub fn ingest_capsule(&mut self, capsule: &crate::genesis::capsule::AgentCapsule) {
        for binding in &capsule.network_bindings {
            let transport = format!("{:?}", binding.transport).to_lowercase();
            self.upsert(&capsule.agent_id, &transport, &binding.address);
        }
    }

    pub fn resolve(&self, agent_id: &str, transport: &str) -> Option<&str> {
        self.records
            .get(agent_id)
            .and_then(|r| r.addresses.get(transport))
            .map(|s| s.as_str())
    }

    pub fn record(&self, agent_id: &str) -> Option<&UailRecord> {
        self.records.get(agent_id)
    }

    pub fn all_records(&self) -> impl Iterator<Item = &UailRecord> {
        self.records.values()
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Stub transports for offline/test use
// ─────────────────────────────────────────────────────────────────────────────

/// A loopback transport: resolves nothing from the network, sends to a
/// local in-memory sink. Used in tests and offline genesis ceremony.
pub struct LoopbackTransport {
    id: String,
    tier: TransportTier,
}

impl LoopbackTransport {
    pub fn new(id: &str, tier: TransportTier) -> Self {
        Self {
            id: id.to_string(),
            tier,
        }
    }
}

#[async_trait]
impl Transport for LoopbackTransport {
    fn transport_id(&self) -> &str {
        &self.id
    }

    fn tier(&self) -> TransportTier {
        self.tier.clone()
    }

    async fn resolve(&self, _agent_id: &str) -> Option<String> {
        None
    }

    async fn send(&self, address: &str, _payload: &[u8]) -> Result<String, RouterError> {
        Ok(address.to_string())
    }

    fn is_available(&self) -> bool {
        true
    }
}

/// Build a NetworkRouter with loopback stubs for all 5 transport tiers.
pub fn default_loopback_router() -> NetworkRouter {
    let mut router = NetworkRouter::new();
    router.register(Box::new(LoopbackTransport::new(
        "meshtastic",
        TransportTier::Mesh,
    )));
    router.register(Box::new(LoopbackTransport::new(
        "freenet",
        TransportTier::Overlay,
    )));
    router.register(Box::new(LoopbackTransport::new(
        "nostr",
        TransportTier::Decentralised,
    )));
    router.register(Box::new(LoopbackTransport::new(
        "sui",
        TransportTier::Onchain,
    )));
    router.register(Box::new(LoopbackTransport::new(
        "https",
        TransportTier::Web,
    )));
    router
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_transport_tier_ordering() {
        let mut tiers = vec![
            TransportTier::Web,
            TransportTier::Mesh,
            TransportTier::Onchain,
            TransportTier::Overlay,
            TransportTier::Decentralised,
        ];
        tiers.sort();
        assert_eq!(tiers[0], TransportTier::Mesh);
        assert_eq!(tiers[4], TransportTier::Web);
    }

    #[tokio::test]
    async fn test_router_loopback_message() {
        let router = default_loopback_router();
        let msg = RouteMessage::new(
            "agent-sender",
            "agent-receiver",
            b"hello sovereign world".to_vec(),
            "text/plain",
        );
        // Loopback transport returns NoRoute because resolve returns None
        // (no UAIL records) but with destination_address set it should succeed
        let mut msg_with_addr = msg.clone();
        msg_with_addr.destination_address = Some("mesh-addr-xyz".to_string());
        msg_with_addr.preferred_transport = Some("meshtastic".to_string());
        let receipt = router.route(&msg_with_addr).await.unwrap();
        assert_eq!(receipt.transport_used, "meshtastic");
        assert_eq!(receipt.transport_tier, TransportTier::Mesh);
        assert_eq!(receipt.status, RouteStatus::Delivered);
    }

    #[test]
    fn test_uail_upsert_resolve() {
        let mut uail = UniversalAgentIdentityLayer::new();
        uail.upsert("agent-abc", "nostr", "npub1abc123");
        uail.upsert("agent-abc", "sui", "0xdeadbeef");
        assert_eq!(uail.resolve("agent-abc", "nostr"), Some("npub1abc123"));
        assert_eq!(uail.resolve("agent-abc", "sui"), Some("0xdeadbeef"));
        assert_eq!(uail.resolve("agent-abc", "meshtastic"), None);
    }

    #[test]
    fn test_route_receipt_integrity() {
        let router = default_loopback_router();
        let msg = RouteMessage::new("a", "b", vec![1, 2, 3], "application/octet-stream");
        let transport = LoopbackTransport::new("nostr", TransportTier::Decentralised);
        let receipt = router.build_receipt(
            &msg,
            &transport,
            "npub1xyz",
            1_700_000_000_000,
            RouteStatus::Delivered,
        );
        assert!(!receipt.integrity_hash.is_empty());
        assert!(!receipt.receipt_id.is_empty());
    }
}
