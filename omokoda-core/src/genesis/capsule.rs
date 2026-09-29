use super::capability::flags;
use serde::{Deserialize, Serialize};

/// The transportable authenticated identity + state commitment for an Ọmọ Kọ́dà agent.
///
/// An AgentCapsule can travel across:
///   HTTPS | Nostr | WebRTC | Freenet | Reticulum | Meshtastic | Sui | Walrus
///
/// The receiving end reconstructs/verifies the agent's identity from this
/// compact proof WITHOUT needing the full agent state. Full state is fetched
/// separately from Walrus/Seal/local store once identity is verified.
///
/// Invariant: AgentID = SAME regardless of which transport carries this capsule.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentCapsule {
    // ── Identity ──────────────────────────────────────────────────────────
    pub agent_id: String,
    pub genesis_id: String,          // genesis_hash from birth receipt
    pub identity_commitment: String, // harmonic_signature hex

    // ── Temporal state ────────────────────────────────────────────────────
    pub koodu_epoch: u64,
    pub koodu_cycle: u64,
    pub koodu_phase: u8,
    pub btc_anchor: Option<String>,
    pub capsule_timestamp: u64, // Unix ms when capsule was sealed

    // ── Capability manifest ───────────────────────────────────────────────
    pub capability_root: String, // derivation root id
    pub capability_flags: u32,   // bitmask of active capabilities

    // ── State commitment ──────────────────────────────────────────────────
    pub memory_root: String,       // current Minipae/GlyphIndex root
    pub glyph_root: String,        // birth glyph (immutable anchor)
    pub twin_root: Option<String>, // 1:1 twin merkle root if bound
    pub reputation_root: f64,      // current reputation score

    // ── Network bindings summary ──────────────────────────────────────────
    pub network_bindings: Vec<NetworkBinding>,

    // ── Device binding ────────────────────────────────────────────────────
    pub device_binding: Option<CapsuleDeviceBinding>,

    // ── Previous state link (forms a verifiable chain) ────────────────────
    pub previous_receipt: Option<String>,
    pub previous_capsule_hash: Option<String>,

    // ── Authorization ─────────────────────────────────────────────────────
    pub authorized_by: Option<String>, // delegating agent_id, if any
    pub authorization_proof: Option<String>,

    // ── Self-proof ────────────────────────────────────────────────────────
    pub capsule_hash: String, // SHA-256 over all fields above
    pub signature: String,    // Ed25519 signature over capsule_hash

    pub capsule_version: u8,
}

/// Which network this binding address belongs to.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkBinding {
    pub transport: TransportKind,
    pub address: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum TransportKind {
    Nostr,
    Sui,
    Bitcoin,
    Ethereum,
    Meshtastic,
    Reticulum,
    Freenet,
    IpLayer, // Nostr kind 31900 IP Root
    Web2 { kind: String },
    Other(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapsuleDeviceBinding {
    pub device_id: String,
    pub device_kind: String,
}

impl AgentCapsule {
    pub const CURRENT_VERSION: u8 = 1;

    /// Compute the capsule hash from all fields (excluding capsule_hash + signature).
    pub fn compute_hash(
        agent_id: &str,
        genesis_id: &str,
        identity_commitment: &str,
        koodu_epoch: u64,
        memory_root: &str,
        capsule_timestamp: u64,
    ) -> String {
        use sha2::{Digest, Sha256};
        let mut h = Sha256::new();
        h.update(agent_id.as_bytes());
        h.update(genesis_id.as_bytes());
        h.update(identity_commitment.as_bytes());
        h.update(koodu_epoch.to_le_bytes());
        h.update(memory_root.as_bytes());
        h.update(capsule_timestamp.to_le_bytes());
        h.update(b"omokoda-capsule-v1");
        hex::encode(h.finalize())
    }

    /// Build an unsigned capsule from an AgentManifest + current state.
    pub fn from_manifest(
        manifest: &super::manifest::AgentManifest,
        reputation: f64,
        previous_receipt: Option<String>,
    ) -> Self {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        let network_bindings: Vec<NetworkBinding> = {
            let mut b = vec![];
            if let Some(ref pk) = manifest.network.nostr_pubkey {
                b.push(NetworkBinding {
                    transport: TransportKind::Nostr,
                    address: pk.clone(),
                });
            }
            if let Some(ref addr) = manifest.network.sui_address {
                b.push(NetworkBinding {
                    transport: TransportKind::Sui,
                    address: addr.clone(),
                });
            }
            if let Some(ref addr) = manifest.network.btc_address {
                b.push(NetworkBinding {
                    transport: TransportKind::Bitcoin,
                    address: addr.clone(),
                });
            }
            if let Some(ref addr) = manifest.network.eth_address {
                b.push(NetworkBinding {
                    transport: TransportKind::Ethereum,
                    address: addr.clone(),
                });
            }
            if let Some(ref ev) = manifest.network.ip_root_event {
                b.push(NetworkBinding {
                    transport: TransportKind::IpLayer,
                    address: ev.clone(),
                });
            }
            b
        };

        let capsule_hash = Self::compute_hash(
            &manifest.identity.agent_id,
            &manifest.identity.genesis_id,
            &manifest.identity.identity_commitment,
            manifest.temporal.koodu_epoch,
            &manifest.memory.memory_root,
            now,
        );

        Self {
            agent_id: manifest.identity.agent_id.clone(),
            genesis_id: manifest.identity.genesis_id.clone(),
            identity_commitment: manifest.identity.identity_commitment.clone(),
            koodu_epoch: manifest.temporal.koodu_epoch,
            koodu_cycle: manifest.temporal.koodu_cycle,
            koodu_phase: manifest.temporal.koodu_phase,
            btc_anchor: manifest.temporal.btc_anchor.clone(),
            capsule_timestamp: now,
            capability_root: manifest.identity.capability_root.clone(),
            capability_flags: flags::DEFAULT_BIRTH,
            memory_root: manifest.memory.memory_root.clone(),
            glyph_root: manifest.memory.glyph_root.clone(),
            twin_root: manifest.world.twin_root.clone(),
            reputation_root: reputation,
            network_bindings,
            device_binding: manifest
                .body
                .device_binding
                .as_ref()
                .map(|d| CapsuleDeviceBinding {
                    device_id: d.device_id.clone(),
                    device_kind: d.device_kind.clone(),
                }),
            previous_receipt,
            previous_capsule_hash: None,
            authorized_by: manifest.identity.owner.clone(),
            authorization_proof: None,
            capsule_hash,
            signature: String::new(), // populated after sealing with Ed25519
            capsule_version: Self::CURRENT_VERSION,
        }
    }

    /// Verify the capsule hash matches its contents (signature not checked here).
    pub fn verify_hash(&self) -> bool {
        let expected = Self::compute_hash(
            &self.agent_id,
            &self.genesis_id,
            &self.identity_commitment,
            self.koodu_epoch,
            &self.memory_root,
            self.capsule_timestamp,
        );
        self.capsule_hash == expected
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_capsule_hash_deterministic() {
        let h1 = AgentCapsule::compute_hash("agent-abc", "gen-1", "sig-x", 3, "mem-r", 1_000_000);
        let h2 = AgentCapsule::compute_hash("agent-abc", "gen-1", "sig-x", 3, "mem-r", 1_000_000);
        assert_eq!(h1, h2);
        assert_eq!(h1.len(), 64); // hex SHA-256
    }

    #[test]
    fn test_capsule_hash_differs_on_mutation() {
        let h1 = AgentCapsule::compute_hash("agent-abc", "gen-1", "sig-x", 3, "mem-r", 1_000_000);
        let h2 = AgentCapsule::compute_hash("agent-xyz", "gen-1", "sig-x", 3, "mem-r", 1_000_000);
        assert_ne!(h1, h2);
    }
}
