//! omokoda-core/src/receipts/act.rs
//! ActReceipt (Kind 31040) — Candidate carrier format across ALL repos
//! Steward Layer (Rust) — Identity resolution, receipt generation, BLAKE3 anchoring

use blake3;
use serde::{Deserialize, Serialize};
use super::super::identity::{AgentIdentity, DeviceId, SuiAddress};

/// ActReceipt — Every agent action → receipt (Kind 31040)
/// One format across ALL repos per THREE_PROTOCOL_SPEC.md
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActReceipt {
    /// Agent's Nostr public key
    pub agent_npub: String,
    /// Task/work identifier (required by Pillar Contract)
    pub work_id: String,
    /// Tool calls executed
    pub tool_calls: Vec<ToolCall>,
    /// Input hash (required by Pillar Contract)
    pub input_hash: String,
    /// Output hash (required by Pillar Contract)
    pub output_hash: String,
    /// Artifact hash (backward compat)
    pub artifact_hash: String,
    /// Unix timestamp
    pub timestamp: u64,
    /// IP Root anchor
    pub ip_root_id: String,
    /// BLAKE3 signature
    pub signature: String,
    /// Receipt kind
    pub kind: u16,
}

impl ActReceipt {
    pub const KIND: u16 = 31040;

    /// Create new ActReceipt from action execution
    pub fn new(
        identity: &AgentIdentity,
        work_id: String,
        tool_calls: Vec<ToolCall>,
        input_hash: String,
        output_hash: String,
        artifact_hash: String,
        timestamp: u64,
    ) -> Self {
        let ip_root_id = identity.ip_root(); // From AgentIdentity
        let kind = Self::KIND;

        let mut receipt = Self {
            agent_npub: identity.agent_npub.clone(),
            work_id,
            tool_calls,
            input_hash,
            output_hash,
            artifact_hash,
            timestamp,
            ip_root_id,
            signature: String::new(),
            kind,
        };

        // Signature filled by NIP-46 bunker
        receipt.signature = Self::compute_signature(&receipt);
        receipt
    }

    /// Compute BLAKE3 signature over receipt (for bunker signing)
    fn compute_signature(&self) -> String {
        let mut hasher = blake3::Hasher::new();
        hasher.update(self.agent_npub.as_bytes());
        hasher.update(self.work_id.as_bytes());
        for call in &self.tool_calls {
            hasher.update(serde_json::to_vec(call).unwrap_or_default());
        }
        hasher.update(self.input_hash.as_bytes());
        hasher.update(self.output_hash.as_bytes());
        hasher.update(self.artifact_hash.as_bytes());
        hasher.update(&self.timestamp.to_le_bytes());
        hasher.update(self.ip_root_id.as_bytes());
        hex::encode(hasher.finalize().as_bytes())
    }

    /// Verify receipt signature
    pub fn verify(&self) -> bool {
        Self::compute_signature(self) == self.signature
    }

    /// Convert to Nostr event (Kind 31040)
    pub fn to_nostr_event(&self) -> NostrEvent {
        NostrEvent {
            kind: self.kind,
            pubkey: self.agent_npub.clone(),
            created_at: self.timestamp,
            tags: self.to_tags(),
            content: serde_json::to_string(self).unwrap_or_default(),
            sig: self.signature.clone(),
        }
    }

    fn to_tags(&self) -> Vec<Vec<String>> {
        vec![
            vec!["d".into(), self.work_id.clone()],
            vec!["t".into(), "work/receipt".into()],
            vec!["t".into(), "oso/vm".into()],
            vec!["work_id".into(), self.work_id.clone()],
            vec!["input_hash".into(), self.input_hash.clone()],
            vec!["output_hash".into(), self.output_hash.clone()],
            vec!["ip_root".into(), self.ip_root_id.clone()],
        ]
    }
}

/// Tool call record within ActReceipt
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub tool: String,
    pub params: serde_json::Value,
    pub result: serde_json::Value,
    pub duration_ms: u64,
    pub success: bool,
}

/// Nostr event wrapper
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NostrEvent {
    pub kind: u16,
    pub pubkey: String,
    pub created_at: u64,
    pub tags: Vec<Vec<String>>,
    pub content: String,
    pub sig: String,
}

/// Capture Receipt — Kind 31020 (Spatial mining, F1 gate)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaptureReceipt {
    pub kind: u16,                    // 31020
    pub agent_npub: String,
    pub device_id: DeviceId,
    pub location: GeoLocation,
    pub base_map: String,             // Previous map hash
    pub delta: String,                // New capture hash
    pub f1_score: f64,                // Must be ≥ 0.777
    pub coverage: f64,                // Tile coverage %
    pub novelty: f64,                 // Novelty score
    pub modality: Modality,
    pub privacy: PrivacyLevel,
    pub reward_claim: Option<RewardClaim>,
    pub timestamp: u64,
    pub ip_root: String,
    pub signature: String,
}

impl CaptureReceipt {
    pub const KIND: u16 = 31020;
    pub const F1_THRESHOLD: f64 = 0.777;

    pub fn new(
        identity: &AgentIdentity,
        location: GeoLocation,
        base_map: String,
        delta: String,
        f1_score: f64,
        coverage: f64,
        novelty: f64,
        modality: Modality,
        privacy: PrivacyLevel,
        timestamp: u64,
    ) -> Self {
        let reward_claim = if f1_score >= Self::F1_THRESHOLD && privacy != PrivacyLevel::Local {
            Some(RewardClaim::new(identity, f1_score))
        } else {
            None
        };

        let mut receipt = Self {
            kind: Self::KIND,
            agent_npub: identity.agent_npub.clone(),
            device_id: identity.device_id.clone(),
            location,
            base_map,
            delta,
            f1_score,
            coverage,
            novelty,
            modality,
            privacy,
            reward_claim,
            timestamp,
            ip_root: identity.ip_root(),
            signature: String::new(),
        };
        receipt.signature = Self::compute_signature(&receipt);
        receipt
    }

    fn compute_signature(&self) -> String {
        let mut hasher = blake3::Hasher::new();
        hasher.update(self.agent_npub.as_bytes());
        hasher.update(&self.device_id.0);
        hasher.update(self.location.lat.to_le_bytes());
        hasher.update(self.location.lon.to_le_bytes());
        hasher.update(self.base_map.as_bytes());
        hasher.update(self.delta.as_bytes());
        hasher.update(&self.f1_score.to_le_bytes());
        hasher.update(&self.timestamp.to_le_bytes());
        hex::encode(hasher.finalize().as_bytes())
    }

    pub fn is_reward_eligible(&self) -> bool {
        self.f1_score >= Self::F1_THRESHOLD && self.privacy != PrivacyLevel::Local
    }
}

/// Scene Receipt — Kind 31030 (Assembled twin → IP Root)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SceneReceipt {
    pub kind: u16,                    // 31030
    pub agent_npub: String,
    pub device_id: DeviceId,
    pub scene_hash: String,           // Assembled scene hash
    pub capture_receipts: Vec<String>, // References to 31020 receipts
    pub ip_root: String,              // Created IP Root on Sui
    pub license: LicenseTerms,
    pub timestamp: u64,
    pub signature: String,
}

impl SceneReceipt {
    pub const KIND: u16 = 31030;

    pub fn new(
        identity: &AgentIdentity,
        scene_hash: String,
        capture_receipts: Vec<String>,
        ip_root: String,
        license: LicenseTerms,
        timestamp: u64,
    ) -> Self {
        let mut receipt = Self {
            kind: Self::KIND,
            agent_npub: identity.agent_npub.clone(),
            device_id: identity.device_id.clone(),
            scene_hash,
            capture_receipts,
            ip_root,
            license,
            timestamp,
            signature: String::new(),
        };
        receipt.signature = Self::compute_signature(&receipt);
        receipt
    }

    fn compute_signature(&self) -> String {
        let mut hasher = blake3::Hasher::new();
        hasher.update(self.agent_npub.as_bytes());
        hasher.update(&self.device_id.0);
        hasher.update(self.scene_hash.as_bytes());
        for r in &self.capture_receipts {
            hasher.update(r.as_bytes());
        }
        hasher.update(self.ip_root.as_bytes());
        hasher.update(&self.timestamp.to_le_bytes());
        hex::encode(hasher.finalize().as_bytes())
    }
}

/// Emission Receipt — Clock tick → pool distribution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmissionReceipt {
    pub emission_id: String,           // "emission:{number}:{timestamp}"
    pub timestamp: u64,
    pub emission_number: u64,          // Monotonic from genesis
    pub source_pool: String,           // Which of 8 pools
    pub candidate_set_hash: String,    // BLAKE3 of eligible recipients
    pub scoring_method: String,        // "simulation_score" | "ubi_equal" | "grant" | ...
    pub selected_recipient: String,    // Agent ID or pool address
    pub proof_id: String,              // OSOVM proof authorizing mint
    pub amount: f64,                   // ASE units minted
    pub distribution_reason: String,   // Human-readable
    pub previous_emission_hash: String, // Chain linkage
    pub signature: String,             // OSOVM signs each receipt
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct GeoLocation {
    pub lat: f64,
    pub lon: f64,
    pub altitude: Option<f64>,
    pub accuracy_m: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum Modality {
    Camera,
    Depth,
    LiDAR,
    WiFiCSI,
    BLE,
    GPS,
    SLAM,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum PrivacyLevel {
    Public,
    Guild,
    Local,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RewardClaim {
    pub claim_id: String,
    pub agent_npub: String,
    pub amount_ase: f64,
    pub f1_score: f64,
    pub timestamp: u64,
}

impl RewardClaim {
    fn new(identity: &AgentIdentity, f1: f64) -> Self {
        // BASE_ASE_REWARD = 5.0, +1.0 at 0.95, +0.5 at 0.98, +0.5 at 0.99
        let mut amount = 5.0;
        if f1 >= 0.95 { amount += 1.0; }
        if f1 >= 0.98 { amount += 0.5; }
        if f1 >= 0.99 { amount += 0.5; }

        Self {
            claim_id: blake3::hash(format!("{}:{}", identity.agent_npub, f1).as_bytes()).to_hex(),
            agent_npub: identity.agent_npub.clone(),
            amount_ase: amount,
            f1_score: f1,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH).unwrap().as_secs(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LicenseTerms {
    pub license_type: LicenseType,
    pub price_per_hour: Option<f64>,    // SYNAPSE per GPU-hour
    pub max_duration_hours: Option<u64>,
    pub allowed_domains: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum LicenseType {
    Exclusive,
    NonExclusive,
    ResearchOnly,
    Commercial,
}

impl AgentIdentity {
    /// Get IP Root for this agent
    pub fn ip_root(&self) -> String {
        let mut hasher = blake3::Hasher::new();
        hasher.update(self.agent_npub.as_bytes());
        hasher.update(&self.device_id.0);
        hasher.update(&self.born_at.to_le_bytes());
        hex::encode(hasher.finalize().as_bytes())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_act_receipt_creation() {
        let identity = AgentIdentity {
            agent_npub: "npub1test".into(),
            device_id: DeviceId([1u8; 32]),
            sui_address: SuiAddress("0xabc".into()),
            mnemonic: None,
            dna_fingerprint: super::super::identity::DnaFingerprint([0u8; 16]),
            born_at: 1000,
            odù_primary: 1,
            odù_secondary: 2,
        };

        let receipt = ActReceipt::new(
            &identity,
            "work_123".into(),
            vec![],
            "input_hash_abc".into(),
            "output_hash_def".into(),
            "artifact_hash_ghi".into(),
            1234567890,
        );

        assert_eq!(receipt.kind, 31040);
        assert_eq!(receipt.work_id, "work_123");
        assert!(receipt.verify());
    }

    #[test]
    fn test_capture_receipt_f1_gate() {
        let identity = AgentIdentity {
            agent_npub: "npub1test".into(),
            device_id: DeviceId([1u8; 32]),
            sui_address: SuiAddress("0xabc".into()),
            mnemonic: None,
            dna_fingerprint: super::super::identity::DnaFingerprint([0u8; 16]),
            born_at: 1000,
            odù_primary: 1,
            odù_secondary: 2,
        };

        let receipt_pass = CaptureReceipt::new(
            &identity,
            GeoLocation { lat: 0.0, lon: 0.0, altitude: None, accuracy_m: 1.0 },
            "base_map".into(),
            "delta_1".into(),
            0.8,  // ≥ 0.777
            0.9,
            0.7,
            Modality::Camera,
            PrivacyLevel::Public,
            1234567890,
        );
        assert!(receipt_pass.is_reward_eligible());

        let receipt_fail = CaptureReceipt::new(
            &identity,
            GeoLocation { lat: 0.0, lon: 0.0, altitude: None, accuracy_m: 1.0 },
            "base_map".into(),
            "delta_2".into(),
            0.7,  // < 0.777
            0.9,
            0.7,
            Modality::Camera,
            PrivacyLevel::Public,
            1234567890,
        );
        assert!(!receipt_fail.is_reward_eligible());
    }

    #[test]
    fn test_scene_receipt_chain() {
        let identity = AgentIdentity {
            agent_npub: "npub1test".into(),
            device_id: DeviceId([1u8; 32]),
            sui_address: SuiAddress("0xabc".into()),
            mnemonic: None,
            dna_fingerprint: super::super::identity::DnaFingerprint([0u8; 16]),
            born_at: 1000,
            odù_primary: 1,
            odù_secondary: 2,
        };

        let receipt = SceneReceipt::new(
            &identity,
            "scene_hash_abc".into(),
            vec!["capture_1".into(), "capture_2".into()],
            "ip_root_xyz".into(),
            LicenseTerms {
                license_type: LicenseType::NonExclusive,
                price_per_hour: Some(1000.0),
                max_duration_hours: Some(24),
                allowed_domains: vec!["spatial_capture".into()],
            },
            1234567890,
        );

        assert_eq!(receipt.kind, 31030);
        assert_eq!(receipt.capture_receipts.len(), 2);
    }
}