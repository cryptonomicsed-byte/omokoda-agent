use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

/// Full OSO-IR document — spec v1.0 (sovereign-eco-blueprint/specs/oso-ir-spec.md).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OsoIR {
    pub oso_ir_version: String,
    pub contract_class: String,
    pub contract_name: String,
    #[serde(default)]
    pub assets: Vec<AssetSpec>,
    #[serde(default)]
    pub capabilities: Vec<String>,
    #[serde(default)]
    pub minimum_tier: u8,
    /// GIX-native evidence requirements (spec §evidence).
    #[serde(default)]
    pub evidence: EvidenceSpec,
    /// External witness quorum policy (spec §witness_policy).
    #[serde(default)]
    pub witness_policy: Option<WitnessPolicySpec>,
    pub settlement: SettlementSpec,
    /// Freeform behavioural policy map (spec §policy).
    /// Common keys: esu_tithe (f64), fork_allowed (bool), deadline_enforcement (bool),
    /// quality_threshold (f64), private_execution (bool), escalation_path (string).
    #[serde(default)]
    pub policy: BTreeMap<String, Value>,
    #[serde(default)]
    pub lifecycle: Vec<String>,
}

// ── Asset ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetSpec {
    pub name: String,
    pub fields: Vec<String>,
}

// ── Evidence ──────────────────────────────────────────────────────────────────

/// Evidence requirement for a contract.
///
/// When `required = false`, the `kind` and `fields` fields are ignored.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct EvidenceSpec {
    #[serde(default)]
    pub required: bool,
    /// Serialised as `"type"` to match the OSO-IR spec wire format.
    #[serde(rename = "type", default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<EvidenceKind>,
    #[serde(default)]
    pub fields: Vec<String>,
}

/// Canonical evidence kinds from the OSO-IR spec.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum EvidenceKind {
    ComputeReceipt,
    DeviceAttestation,
    ZangbetoReceipt,
    WitnessBundle,
    AgentLifecycle,
    WorkCompletion,
    GovernanceVote,
}

// ── Witness Policy ────────────────────────────────────────────────────────────

/// External witness quorum requirement.
///
/// Omit (None) for contracts that do not require external witnessing.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct WitnessPolicySpec {
    /// Minimum witness signatures before settlement proceeds.
    #[serde(default)]
    pub quorum: u8,
    /// Accepted witness types.
    #[serde(default)]
    pub types: Vec<WitnessType>,
}

/// Who may serve as a witness for a contract.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WitnessType {
    Peer,
    Device,
    Agent,
    Node,
}

// ── Settlement ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SettlementSpec {
    /// "ASE" | "SUI" | "USDC"
    #[serde(default)]
    pub currency: Option<String>,
    /// "6-pool" | "direct" | "dao-pool"
    #[serde(default)]
    pub fee_routing: Option<String>,
    /// Creator's share of fees (0.0–1.0)
    #[serde(default)]
    pub creator_share: Option<f64>,
    /// Burn share (0.0–1.0)
    #[serde(default)]
    pub burn_share: Option<f64>,
    /// Provider/ecosystem pool share (0.0–1.0)
    #[serde(default)]
    pub provider_share: Option<f64>,
    /// Legacy: fixed settlement amount in token units (kept for WASM codegen)
    #[serde(default)]
    pub amount: Option<u64>,
    /// Legacy: Èṣù tithe rate override (preferred: use policy.esu_tithe)
    #[serde(default)]
    pub tithe_rate: Option<f64>,
}
