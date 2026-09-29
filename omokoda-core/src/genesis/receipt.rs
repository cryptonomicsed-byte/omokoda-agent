use serde::{Deserialize, Serialize};

/// Full birth certificate for an Ọmọ Kọ́dà agent.
/// Every field must be populated before birth is considered complete.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AgentGenesisReceipt {
    // ── Genesis ─────────────────────────────────────────────────────────
    pub agent_id: String,
    pub genesis_hash: String,             // SHA-256(all proofs concatenated)
    pub birth_entropy_commitment: String, // hex of SHA-256(entropy)

    // ── BIPỌ̀N39 ─────────────────────────────────────────────────────────
    pub harmonic_signature: String, // hex
    pub derivation_root_id: String, // hex, used to derive all child keys

    // ── CloakSeed ────────────────────────────────────────────────────────
    pub symbolic_address: String, // vanity display name
    pub sigil_hash: String,       // hex
    pub cloak_commitment: String, // hex — proves cloak was applied

    // ── Koodu ────────────────────────────────────────────────────────────
    pub born_at: u64, // Unix timestamp millis
    pub koodu_epoch: u64,
    pub koodu_cycle: u64,
    pub koodu_phase: u8,
    pub bitcoin_height: Option<u64>,
    pub bitcoin_anchor: Option<String>, // hex of block hash if available
    pub gregorian_fallback: bool,       // true if BTC unavailable at birth

    // ── If-Script / Soul ─────────────────────────────────────────────────
    pub primary_odu: u8,
    pub composed_odu: u16,
    pub temperament: String,
    pub orisha_alignment: String,
    pub destiny_threads: Vec<String>,

    // ── Minipae / Memory ─────────────────────────────────────────────────
    pub minipae_pubkey: String,     // hex
    pub memory_root: String,        // GlyphIndex merkle root at birth
    pub birth_memory_glyph: String, // GIX-FOLD-v1 glyph of genesis fact

    // ── IP-Layer ─────────────────────────────────────────────────────────
    pub ip_root_event: Option<String>, // Nostr event id (kind 31900)

    // ── Device / Agent-Phone ─────────────────────────────────────────────
    pub device_binding: Option<DeviceBinding>,

    // ── Governance ───────────────────────────────────────────────────────
    pub hermetic_fingerprint: String, // hex, derived from 7-gate scores

    // ── Federation ───────────────────────────────────────────────────────
    pub blockmesh_identity: Option<String>,

    // ── Economy ──────────────────────────────────────────────────────────
    pub vantage_identity: Option<String>,

    // ── Walrus cold archive ──────────────────────────────────────────────
    /// Walrus blob anchor written at birth (fail-open: None if Walrus unreachable).
    pub cold_archive_anchor: Option<WalrusAnchor>,

    // ── GPU compute (populated post-birth when agent first contributes) ──
    /// Total GPU-seconds contributed by this agent across all verified work.
    pub contributed_gpu_seconds: Option<f64>,
    /// Lease id of the agent's first verified GPU lease.
    pub first_lease_id: Option<String>,
    /// VerifiedGPUWork id of the agent's first completed GPU job.
    pub first_work_id: Option<String>,

    // ── Witness ──────────────────────────────────────────────────────────
    pub witness_receipt: Option<String>, // Zàngbétò receipt hash

    // ── Memory write status ──────────────────────────────────────────────
    /// Status of the minipae NIP-AE memory write at birth (fail-open).
    #[serde(default)]
    pub memory_write_status: MemoryWriteStatus,

    // ── Self-proof ───────────────────────────────────────────────────────
    pub genesis_signature: String, // Ed25519 signature over genesis_hash
    pub receipt_version: u8,       // currently 2
}

impl AgentGenesisReceipt {
    pub const CURRENT_VERSION: u8 = 2;

    /// Compute the canonical genesis hash from all constituent proofs.
    /// This is what gets signed to produce genesis_signature.
    pub fn compute_genesis_hash(
        agent_id: &str,
        harmonic_sig: &str,
        koodu_epoch: u64,
        primary_odu: u8,
        memory_root: &str,
        born_at: u64,
    ) -> String {
        use sha2::{Digest, Sha256};
        let mut h = Sha256::new();
        h.update(agent_id.as_bytes());
        h.update(harmonic_sig.as_bytes());
        h.update(koodu_epoch.to_le_bytes());
        h.update([primary_odu]);
        h.update(memory_root.as_bytes());
        h.update(born_at.to_le_bytes());
        hex::encode(h.finalize())
    }
}

/// Walrus cold-archive anchor — written at birth, proves genesis data persisted.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WalrusAnchor {
    /// Walrus blob id (hex).
    pub blob_id: String,
    /// Walrus epoch the blob was registered in.
    pub epoch: u64,
    /// Unix millis when the anchor was written.
    pub anchored_at: u64,
    /// Optional Walrus explorer URL for human-readable proof.
    pub explorer_url: Option<String>,
}

/// Status of the minipae memory write at birth.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub enum MemoryWriteStatus {
    #[default]
    Pending,
    Written {
        glyph_id: String,
    },
    Failed {
        reason: String,
    },
    Unavailable,
}

/// Proof returned by the BIPỌ̀N39 provider.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BiponProof {
    pub agent_id: String,
    pub harmonic_signature: String,
    pub derivation_root_id: String,
    pub symbolic_address: String,
    pub sigil_hash: String,
    pub cloak_commitment: String,
    pub entropy_commitment: String,
    /// The mnemonic is kept in-memory only; never logged or returned to callers.
    #[serde(skip)]
    pub mnemonic_sealed: String,
    #[serde(skip)]
    pub seed_bytes: Vec<u8>,
}

/// Proof returned by the Koodu temporal provider.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KooduTimeProof {
    pub born_at: u64,
    pub koodu_epoch: u64,
    pub koodu_cycle: u64,
    pub koodu_phase: u8,
    pub bitcoin_height: Option<u64>,
    pub bitcoin_anchor: Option<String>,
    pub gregorian_fallback: bool,
}

/// Proof returned by the Soul / If-Script provider.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SoulProof {
    pub primary_odu: u8,
    pub composed_odu: u16,
    pub temperament: String,
    pub orisha_alignment: String,
    pub destiny_threads: Vec<String>,
}

/// Proof returned by the Memory (Minipae) provider.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryProof {
    pub minipae_pubkey: String,
    pub memory_root: String,
    pub birth_memory_glyph: String,
}

/// Proof returned by the Network (IP-Layer) provider.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkProof {
    pub ip_root_event: Option<String>,
}

/// Optional device binding from the Agent-Phone provider.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DeviceBinding {
    pub device_id: String,
    pub device_kind: String, // "fold", "tablet", "server", "node", etc.
    pub binding_timestamp: u64,
}

/// Input to the birth ceremony.
#[derive(Debug, Clone)]
pub struct GenesisRequest {
    pub name: String,
    pub entropy: Vec<u8>, // 32 bytes, NIST-validated
    pub passphrase: Option<String>,
    pub sovereign: bool,
    pub device_id: Option<String>,
    pub device_kind: Option<String>,
}
