//! CapabilityFabric — WHO + WHAT + HOW
//!
//! Every agent action in the sovereign ecosystem is authorized through this
//! three-part capability model:
//!   WHO  = verified agent identity (from genesis receipt)
//!   WHAT = authority scopes (what the agent may do in a given ecosystem)
//!   HOW  = adapter pattern (how the agent enters that ecosystem)
//!
//! Design: grants are issued at birth for default scopes; agents can acquire
//! additional grants via delegation or federation handshakes.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fmt;

// ─────────────────────────────────────────────────────────────────────────────
// Capability Scopes (WHAT)
// ─────────────────────────────────────────────────────────────────────────────

/// Bitmask constants for capability_flags in AgentCapsule.
pub mod flags {
    pub const READ: u32 = 1 << 0;
    pub const WRITE: u32 = 1 << 1;
    pub const DEPLOY: u32 = 1 << 2;
    pub const SIGN: u32 = 1 << 3;
    pub const PAY: u32 = 1 << 4;
    pub const PUBLISH: u32 = 1 << 5;
    pub const EXECUTE: u32 = 1 << 6;
    pub const ADMIN: u32 = 1 << 7;

    /// Default scopes granted at birth to every agent.
    pub const DEFAULT_BIRTH: u32 = READ | WRITE | SIGN | PUBLISH;
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityScope {
    Read,
    Write,
    Deploy,
    Sign,
    Pay,
    Publish,
    Execute,
    Admin,
}

impl CapabilityScope {
    pub fn to_flag(&self) -> u32 {
        match self {
            Self::Read => flags::READ,
            Self::Write => flags::WRITE,
            Self::Deploy => flags::DEPLOY,
            Self::Sign => flags::SIGN,
            Self::Pay => flags::PAY,
            Self::Publish => flags::PUBLISH,
            Self::Execute => flags::EXECUTE,
            Self::Admin => flags::ADMIN,
        }
    }

    pub fn from_flag(flag: u32) -> Vec<Self> {
        let mut scopes = Vec::new();
        if flag & flags::READ != 0 {
            scopes.push(Self::Read);
        }
        if flag & flags::WRITE != 0 {
            scopes.push(Self::Write);
        }
        if flag & flags::DEPLOY != 0 {
            scopes.push(Self::Deploy);
        }
        if flag & flags::SIGN != 0 {
            scopes.push(Self::Sign);
        }
        if flag & flags::PAY != 0 {
            scopes.push(Self::Pay);
        }
        if flag & flags::PUBLISH != 0 {
            scopes.push(Self::Publish);
        }
        if flag & flags::EXECUTE != 0 {
            scopes.push(Self::Execute);
        }
        if flag & flags::ADMIN != 0 {
            scopes.push(Self::Admin);
        }
        scopes
    }

    pub fn all_to_flags(scopes: &[CapabilityScope]) -> u32 {
        scopes.iter().fold(0u32, |acc, s| acc | s.to_flag())
    }
}

impl fmt::Display for CapabilityScope {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Self::Read => "read",
            Self::Write => "write",
            Self::Deploy => "deploy",
            Self::Sign => "sign",
            Self::Pay => "pay",
            Self::Publish => "publish",
            Self::Execute => "execute",
            Self::Admin => "admin",
        };
        write!(f, "{}", s)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Capability Grant (WHO + WHAT bound to an ecosystem)
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapabilityGrant {
    pub grant_id: String,
    pub agent_id: String,
    /// Ecosystem this grant applies to: "nostr", "freenet", "meshtastic",
    /// "zima", "sui", "ethereum", "bitcoin", "ip_layer", "vantage", "web2"
    pub ecosystem: String,
    pub scopes: Vec<CapabilityScope>,
    pub capability_flags: u32,
    pub granted_at: u64,
    pub expires_at: Option<u64>,
    /// SHA-256(agent_id || ecosystem || flags || granted_at) — integrity proof
    pub proof: String,
    /// If delegated from another agent, their agent_id
    pub delegated_by: Option<String>,
}

impl CapabilityGrant {
    pub fn new(
        agent_id: &str,
        ecosystem: &str,
        scopes: Vec<CapabilityScope>,
        granted_at: u64,
        expires_at: Option<u64>,
        delegated_by: Option<String>,
    ) -> Self {
        let flags = CapabilityScope::all_to_flags(&scopes);
        let proof = Self::compute_proof(agent_id, ecosystem, flags, granted_at);
        let grant_id = {
            let mut h = Sha256::new();
            h.update(agent_id.as_bytes());
            h.update(ecosystem.as_bytes());
            h.update(granted_at.to_le_bytes());
            hex::encode(&h.finalize()[..8])
        };
        Self {
            grant_id,
            agent_id: agent_id.to_string(),
            ecosystem: ecosystem.to_string(),
            scopes,
            capability_flags: flags,
            granted_at,
            expires_at,
            proof,
            delegated_by,
        }
    }

    pub fn birth_grant(agent_id: &str, ecosystem: &str, born_at: u64) -> Self {
        let scopes = CapabilityScope::from_flag(flags::DEFAULT_BIRTH);
        Self::new(agent_id, ecosystem, scopes, born_at, None, None)
    }

    pub fn is_valid_at(&self, now_ms: u64) -> bool {
        match self.expires_at {
            Some(exp) => now_ms < exp,
            None => true,
        }
    }

    pub fn has_scope(&self, scope: &CapabilityScope) -> bool {
        self.capability_flags & scope.to_flag() != 0
    }

    pub fn verify_proof(&self) -> bool {
        let expected = Self::compute_proof(
            &self.agent_id,
            &self.ecosystem,
            self.capability_flags,
            self.granted_at,
        );
        expected == self.proof
    }

    fn compute_proof(agent_id: &str, ecosystem: &str, flags: u32, granted_at: u64) -> String {
        let mut h = Sha256::new();
        h.update(agent_id.as_bytes());
        h.update(ecosystem.as_bytes());
        h.update(flags.to_le_bytes());
        h.update(granted_at.to_le_bytes());
        h.update(b"capability-grant-v1");
        hex::encode(h.finalize())
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Capability Registry
// ─────────────────────────────────────────────────────────────────────────────

/// In-memory grant store keyed by (agent_id, ecosystem).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CapabilityRegistry {
    /// agent_id → Vec<CapabilityGrant>
    grants: HashMap<String, Vec<CapabilityGrant>>,
}

impl CapabilityRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn issue(&mut self, grant: CapabilityGrant) {
        self.grants
            .entry(grant.agent_id.clone())
            .or_default()
            .push(grant);
    }

    pub fn grants_for(&self, agent_id: &str) -> &[CapabilityGrant] {
        self.grants
            .get(agent_id)
            .map(|v| v.as_slice())
            .unwrap_or(&[])
    }

    pub fn grants_for_ecosystem<'a>(
        &'a self,
        agent_id: &str,
        ecosystem: &str,
    ) -> Vec<&'a CapabilityGrant> {
        self.grants_for(agent_id)
            .iter()
            .filter(|g| g.ecosystem == ecosystem)
            .collect()
    }

    pub fn has_capability(
        &self,
        agent_id: &str,
        ecosystem: &str,
        scope: &CapabilityScope,
        now_ms: u64,
    ) -> bool {
        self.grants_for_ecosystem(agent_id, ecosystem)
            .iter()
            .any(|g| g.is_valid_at(now_ms) && g.has_scope(scope))
    }

    /// Compute the combined capability_flags for an agent across ALL ecosystems.
    /// Used to populate AgentCapsule.capability_flags.
    pub fn aggregate_flags(&self, agent_id: &str, now_ms: u64) -> u32 {
        self.grants_for(agent_id)
            .iter()
            .filter(|g| g.is_valid_at(now_ms))
            .fold(0u32, |acc, g| acc | g.capability_flags)
    }

    /// Issue default birth grants across all core ecosystems for a new agent.
    pub fn issue_birth_grants(&mut self, agent_id: &str, born_at: u64) {
        for eco in &[
            "nostr",
            "ip_layer",
            "vantage",
            "meshtastic",
            "freenet",
            "sui",
            "bitcoin",
            "web2",
        ] {
            self.issue(CapabilityGrant::birth_grant(agent_id, eco, born_at));
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// EcosystemAdapter trait (HOW)
// ─────────────────────────────────────────────────────────────────────────────

/// A binding proof returned by an EcosystemAdapter after provisioning identity.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EcosystemBinding {
    pub ecosystem_id: String,
    pub address: String,       // ecosystem-specific address/pubkey/object-id
    pub binding_proof: String, // SHA-256 commitment tying agent to ecosystem
    pub bound_at: u64,
    pub metadata: HashMap<String, String>,
}

#[derive(Debug)]
pub enum AdapterError {
    Provisioning(String),
    Verification(String),
    Unsupported(String),
    Offline(String),
}

impl fmt::Display for AdapterError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Provisioning(s) => write!(f, "provisioning error: {s}"),
            Self::Verification(s) => write!(f, "verification error: {s}"),
            Self::Unsupported(s) => write!(f, "unsupported: {s}"),
            Self::Offline(s) => write!(f, "offline: {s}"),
        }
    }
}

/// Every ecosystem the agent can enter implements this trait.
/// Each adapter is sovereign — it knows how to speak its own protocol.
/// The CapabilityFabric orchestrates which adapters are invoked and when.
#[async_trait]
pub trait EcosystemAdapter: Send + Sync {
    /// Short lowercase identifier: "nostr", "sui", "meshtastic", etc.
    fn ecosystem_id(&self) -> &str;

    /// Deterministically derive an ecosystem-specific identity from the agent's
    /// seed bytes and provision it on the target network.
    async fn provision_identity(
        &self,
        agent_id: &str,
        seed: &[u8],
    ) -> Result<EcosystemBinding, AdapterError>;

    /// Verify that a CapabilityGrant is still valid on this ecosystem.
    async fn verify_capability(&self, grant: &CapabilityGrant) -> Result<bool, AdapterError>;

    /// Optional: true if this adapter can operate without network access.
    fn is_offline_capable(&self) -> bool {
        false
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// CapabilityFabric — the orchestrating layer that wires WHO+WHAT+HOW
// ─────────────────────────────────────────────────────────────────────────────

/// The CapabilityFabric holds the registry and the adapter set.
/// At birth it provisions identity on all registered adapters and issues grants.
/// At runtime it answers: "can agent X do Y on ecosystem Z?"
pub struct CapabilityFabric {
    pub registry: CapabilityRegistry,
    adapters: HashMap<String, Box<dyn EcosystemAdapter>>,
}

impl CapabilityFabric {
    pub fn new() -> Self {
        Self {
            registry: CapabilityRegistry::new(),
            adapters: HashMap::new(),
        }
    }

    pub fn register_adapter(&mut self, adapter: Box<dyn EcosystemAdapter>) {
        self.adapters
            .insert(adapter.ecosystem_id().to_string(), adapter);
    }

    /// Provision identity on all registered adapters and issue birth grants.
    pub async fn birth_ceremony(
        &mut self,
        agent_id: &str,
        seed: &[u8],
        born_at: u64,
    ) -> Vec<EcosystemBinding> {
        self.registry.issue_birth_grants(agent_id, born_at);

        let adapter_ids: Vec<String> = self.adapters.keys().cloned().collect();
        let mut bindings = Vec::new();
        for id in adapter_ids {
            if let Some(adapter) = self.adapters.get(&id) {
                match adapter.provision_identity(agent_id, seed).await {
                    Ok(binding) => bindings.push(binding),
                    Err(e) => {
                        // fail-open: log and continue
                        eprintln!(
                            "[CapabilityFabric] adapter '{}' provision failed: {}",
                            id, e
                        );
                    }
                }
            }
        }
        bindings
    }

    pub fn can(
        &self,
        agent_id: &str,
        ecosystem: &str,
        scope: &CapabilityScope,
        now_ms: u64,
    ) -> bool {
        self.registry
            .has_capability(agent_id, ecosystem, scope, now_ms)
    }

    pub fn aggregate_flags(&self, agent_id: &str, now_ms: u64) -> u32 {
        self.registry.aggregate_flags(agent_id, now_ms)
    }

    pub fn adapter(&self, ecosystem_id: &str) -> Option<&dyn EcosystemAdapter> {
        self.adapters.get(ecosystem_id).map(|a| a.as_ref())
    }
}

impl Default for CapabilityFabric {
    fn default() -> Self {
        Self::new()
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Offline-capable default adapters (stub implementations for offline birth)
// ─────────────────────────────────────────────────────────────────────────────

/// Stub adapter that derives a deterministic binding from seed without network.
pub struct OfflineAdapter {
    id: String,
}

impl OfflineAdapter {
    pub fn new(id: &str) -> Self {
        Self { id: id.to_string() }
    }
}

#[async_trait]
impl EcosystemAdapter for OfflineAdapter {
    fn ecosystem_id(&self) -> &str {
        &self.id
    }

    async fn provision_identity(
        &self,
        agent_id: &str,
        seed: &[u8],
    ) -> Result<EcosystemBinding, AdapterError> {
        let mut h = Sha256::new();
        h.update(seed);
        h.update(agent_id.as_bytes());
        h.update(self.id.as_bytes());
        h.update(b"offline-adapter-v1");
        let digest = h.finalize();
        let address = hex::encode(&digest[..16]);
        let binding_proof = hex::encode(digest);
        let bound_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        Ok(EcosystemBinding {
            ecosystem_id: self.id.clone(),
            address,
            binding_proof,
            bound_at,
            metadata: HashMap::new(),
        })
    }

    async fn verify_capability(&self, grant: &CapabilityGrant) -> Result<bool, AdapterError> {
        Ok(grant.verify_proof())
    }

    fn is_offline_capable(&self) -> bool {
        true
    }
}

/// Build a CapabilityFabric with offline stubs for all 8 core ecosystems.
/// Used at birth when network adapters are not yet wired in.
pub fn default_offline_fabric() -> CapabilityFabric {
    let mut fabric = CapabilityFabric::new();
    for id in &[
        "nostr",
        "ip_layer",
        "vantage",
        "meshtastic",
        "freenet",
        "sui",
        "bitcoin",
        "web2",
    ] {
        fabric.register_adapter(Box::new(OfflineAdapter::new(id)));
    }
    fabric
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scope_flags_roundtrip() {
        let scopes = vec![
            CapabilityScope::Read,
            CapabilityScope::Sign,
            CapabilityScope::Publish,
        ];
        let flags = CapabilityScope::all_to_flags(&scopes);
        let recovered = CapabilityScope::from_flag(flags);
        assert!(recovered.contains(&CapabilityScope::Read));
        assert!(recovered.contains(&CapabilityScope::Sign));
        assert!(recovered.contains(&CapabilityScope::Publish));
        assert!(!recovered.contains(&CapabilityScope::Admin));
    }

    #[test]
    fn test_grant_proof_verifies() {
        let grant = CapabilityGrant::birth_grant("agent-abc123", "nostr", 1_700_000_000_000);
        assert!(grant.verify_proof());
    }

    #[test]
    fn test_grant_has_default_scopes() {
        let grant = CapabilityGrant::birth_grant("agent-xyz", "sui", 1_700_000_000_000);
        assert!(grant.has_scope(&CapabilityScope::Read));
        assert!(grant.has_scope(&CapabilityScope::Write));
        assert!(grant.has_scope(&CapabilityScope::Sign));
        assert!(grant.has_scope(&CapabilityScope::Publish));
        assert!(!grant.has_scope(&CapabilityScope::Admin));
        assert!(!grant.has_scope(&CapabilityScope::Deploy));
    }

    #[test]
    fn test_registry_birth_grants() {
        let mut registry = CapabilityRegistry::new();
        registry.issue_birth_grants("agent-test", 1_700_000_000_000);
        assert!(registry.has_capability(
            "agent-test",
            "nostr",
            &CapabilityScope::Sign,
            1_700_000_000_001
        ));
        assert!(registry.has_capability(
            "agent-test",
            "sui",
            &CapabilityScope::Read,
            1_700_000_000_001
        ));
        assert!(!registry.has_capability(
            "agent-test",
            "sui",
            &CapabilityScope::Admin,
            1_700_000_000_001
        ));
    }

    #[test]
    fn test_grant_expiry() {
        let mut grant = CapabilityGrant::birth_grant("agent-exp", "nostr", 1_000_000);
        grant.expires_at = Some(2_000_000);
        assert!(grant.is_valid_at(1_500_000));
        assert!(!grant.is_valid_at(2_000_001));
    }

    #[tokio::test]
    async fn test_offline_adapter_provisions() {
        let adapter = OfflineAdapter::new("nostr");
        let binding = adapter
            .provision_identity("agent-abc", &[1u8; 32])
            .await
            .unwrap();
        assert_eq!(binding.ecosystem_id, "nostr");
        assert!(!binding.address.is_empty());
    }

    #[tokio::test]
    async fn test_fabric_birth_ceremony() {
        let mut fabric = default_offline_fabric();
        let bindings = fabric
            .birth_ceremony("agent-fabric-test", &[42u8; 32], 1_700_000_000_000)
            .await;
        assert_eq!(bindings.len(), 8);
        assert!(fabric.can(
            "agent-fabric-test",
            "nostr",
            &CapabilityScope::Sign,
            1_700_000_000_001
        ));
    }
}
