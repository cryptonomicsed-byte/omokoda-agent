/// Phase 21.1 — Ọ̀ṢỌ́-IR type definitions.
///
/// JSON schema:
/// {
///   "contract_class": "work_marketplace",
///   "assets": [{ "name": "ComputeJob", ... }],
///   "capabilities": [{ "name": "GPU_COMPUTE", "minimum_tier": 2 }],
///   "actions": [{ "name": "submit_job", "requires": [...], "emits": [...] }],
///   "evidence": { "required": true, "type": "ComputeReceipt" },
///   "settlement": { "currency": "ASE", "fee_routing": "6-pool" },
///   "policy": { "witness_quorum": 2, "quality_threshold": 90 },
///   "backend_targets": ["move", "native"]
/// }

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Top-level Ọ̀ṢỌ́-IR document.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OsoIr {
    /// Semantic version of this IR schema.
    #[serde(default = "default_ir_version")]
    pub ir_version: String,
    /// Which of the 6 canonical contract classes this belongs to.
    pub contract_class: ContractClass,
    /// Human-readable contract name.
    pub name: String,
    /// Contract version string.
    #[serde(default)]
    pub version: String,
    /// Asset types owned by this contract.
    #[serde(default)]
    pub assets: Vec<AssetDef>,
    /// Capabilities required for contract operation.
    #[serde(default)]
    pub capabilities: Vec<CapabilityRef>,
    /// Actions this contract exposes.
    #[serde(default)]
    pub actions: Vec<ActionDef>,
    /// Evidence requirements for action outcomes.
    #[serde(default)]
    pub evidence: Option<EvidencePolicy>,
    /// Settlement parameters (currency, fee routing).
    #[serde(default)]
    pub settlement: Option<SettlementPolicy>,
    /// Witness / governance policy.
    #[serde(default)]
    pub policy: Option<WitnessPolicy>,
    /// Which backends this IR should be compiled to.
    #[serde(default)]
    pub backend_targets: Vec<BackendTarget>,
    /// Arbitrary metadata (author, license, description, etc.).
    #[serde(default)]
    pub metadata: BTreeMap<String, serde_json::Value>,
}

fn default_ir_version() -> String { "1.0".to_string() }

impl OsoIr {
    /// BLAKE3 content hash of the canonical JSON representation.
    pub fn content_hash(&self) -> String {
        let json = serde_json::to_string(self).unwrap_or_default();
        hex::encode(blake3::hash(json.as_bytes()).as_bytes())
    }
}

/// The 6 canonical Ọ̀ṢỌ́ contract classes.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ContractClass {
    /// Financial contracts: AsePool, Payment, Escrow, Treasury, Exchange, Staking.
    Financial,
    /// Agent contracts: Registry, DAO, Marketplace, Hiring, Delegation, Reputation.
    Agent,
    /// Work contracts: JobContract with full 13-step lifecycle.
    Work,
    /// Device contracts: DeviceRegistry (first-class hardware objects).
    Device,
    /// Evidence contracts: Zàngbétò-native proof contracts.
    Evidence,
    /// Governance contracts: Council/DAO, 24-sector voting, constitutional.
    Governance,
}

/// An asset type owned and managed by this contract.
///
/// Assets are the on-chain objects this contract creates, transfers, and destroys.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetDef {
    /// Asset type name (PascalCase, e.g. "ComputeJob").
    pub name: String,
    /// Fields of this asset type.
    #[serde(default)]
    pub fields: Vec<FieldDef>,
    /// Whether this asset is transferable between principals.
    #[serde(default)]
    pub transferable: bool,
    /// Whether this asset can be split (fractional ownership).
    #[serde(default)]
    pub divisible: bool,
}

/// A field in an asset definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldDef {
    pub name:      String,
    pub field_type: String,
    #[serde(default)]
    pub required:  bool,
}

/// A capability referenced by this contract.
///
/// Capabilities are OS-level grants verified by VCP before execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapabilityRef {
    /// Capability identifier (e.g. "GPU_COMPUTE", "SIGN_TX", "STORAGE_WRITE").
    pub name: String,
    /// Minimum agent tier required to hold this capability.
    #[serde(default)]
    pub minimum_tier: u8,
    /// Whether this capability is mandatory (contract fails without it)
    /// or optional (degrades gracefully).
    #[serde(default = "default_true")]
    pub required: bool,
}

fn default_true() -> bool { true }

/// An action exposed by this contract.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionDef {
    /// Action name (snake_case, e.g. "submit_job").
    pub name: String,
    /// Preconditions that must be satisfied for this action to proceed.
    #[serde(default)]
    pub requires: Vec<PolicyExpr>,
    /// Events or receipts emitted on successful execution.
    #[serde(default)]
    pub emits: Vec<String>,
    /// Whether this action modifies on-chain state.
    #[serde(default = "default_true")]
    pub mutates_state: bool,
    /// If-Script vessel this action maps to (e.g. "Act", "Oracle", "Create").
    #[serde(default)]
    pub vessel: Option<String>,
}

/// A policy expression — a precondition or constraint.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PolicyExpr {
    /// Caller must have this capability.
    Capability { name: String },
    /// Caller must be the named principal role.
    Principal { role: String },
    /// Numeric constraint (e.g. "budget >= minimum_budget").
    Numeric { field: String, op: String, value: serde_json::Value },
    /// A proof or evidence item must be present.
    Proof { proof_type: String },
    /// Boolean expression combining sub-expressions.
    And { exprs: Vec<PolicyExpr> },
    Or { exprs: Vec<PolicyExpr> },
    Not { expr: Box<PolicyExpr> },
}

/// Evidence requirements for action outcomes.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct EvidencePolicy {
    /// Whether evidence is required for settlement.
    pub required: bool,
    /// Type of evidence (e.g. "ComputeReceipt", "ZangbetoReceipt").
    #[serde(default)]
    pub evidence_type: String,
    /// Minimum number of evidence items required.
    #[serde(default = "default_one")]
    pub minimum_count: u32,
}

fn default_one() -> u32 { 1 }

/// Settlement parameters.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SettlementPolicy {
    /// Currency for payment (e.g. "ASE", "DOPAMINE", "SYNAPSE").
    pub currency: String,
    /// Fee routing model (e.g. "6-pool", "direct", "escrow").
    #[serde(default)]
    pub fee_routing: String,
    /// Percentage of payment routed to the treasury (Èṣù tithe).
    #[serde(default)]
    pub treasury_pct: f64,
}

/// Witness and governance policy.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct WitnessPolicy {
    /// Number of witness nodes required to confirm an action.
    #[serde(default)]
    pub witness_quorum: u32,
    /// Minimum quality score (0–100) for work acceptance.
    #[serde(default)]
    pub quality_threshold: u32,
    /// Whether this contract can be upgraded after deployment.
    #[serde(default)]
    pub upgradeable: bool,
    /// Council voting required for constitutional changes.
    #[serde(default)]
    pub requires_council: bool,
}

/// Compilation target backends.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BackendTarget {
    /// Ọ̀ṢỌ́ Move (agent-native Move with host environment).
    Move,
    /// WASM (CosmWasm model with Ọ̀ṢỌ́ host interfaces).
    Wasm,
    /// Native ABCI transaction (direct OSOVM integration).
    Native,
}

/// Error type for IR operations.
#[derive(Debug)]
pub enum IrError {
    InvalidJson(String),
    ValidationFailed(Vec<String>),
}

impl std::fmt::Display for IrError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            IrError::InvalidJson(e) => write!(f, "invalid JSON: {e}"),
            IrError::ValidationFailed(errs) => write!(f, "validation failed: {}", errs.join(", ")),
        }
    }
}

/// Parse an OsoIr document from a JSON string.
pub fn from_json(json: &str) -> Result<OsoIr, IrError> {
    serde_json::from_str(json).map_err(|e| IrError::InvalidJson(e.to_string()))
}

/// Serialize an OsoIr document to a JSON string.
pub fn to_json(ir: &OsoIr) -> String {
    serde_json::to_string_pretty(ir).unwrap_or_default()
}
