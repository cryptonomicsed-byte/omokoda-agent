/// Ọ̀ṢỌ́ Intermediate Representation (OSO-IR) — Rust validator.
///
/// Validates that a JSON IR document conforms to the OSO-IR v1.0 specification
/// (`sovereign-eco-blueprint/specs/oso-ir-spec.md`).
///
/// Design:
/// - All deserialization is serde-driven; unknown fields are allowed at the
///   top level (backends may add extension keys).
/// - Class-specific validation rules are encoded as match arms in `validate()`.
/// - Returns `Ok(())` or `Err(Vec<String>)` with all violations collected
///   in one pass (not fail-fast), so the compiler can surface all errors.
///
/// Phase 21.2 — OSO-IR validation gate.
use serde::{Deserialize, Serialize};

// ── Top-level document ────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OsoIR {
    pub oso_ir_version: String,
    pub contract_class: ContractClass,
    pub contract_name: String,
    pub assets: Vec<AssetSpec>,
    #[serde(default)]
    pub capabilities: Vec<String>,
    #[serde(default)]
    pub minimum_tier: u8,
    #[serde(default)]
    pub evidence: EvidenceSpec,
    #[serde(default)]
    pub witness_policy: Option<WitnessPolicySpec>,
    pub settlement: SettlementSpec,
    pub policy: PolicySpec,
    pub lifecycle: Vec<String>,
}

// ── Enum: ContractClass ───────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ContractClass {
    Financial,
    Agent,
    Work,
    Device,
    Evidence,
    Governance,
}

impl ContractClass {
    pub fn as_str(&self) -> &'static str {
        match self {
            ContractClass::Financial => "financial",
            ContractClass::Agent => "agent",
            ContractClass::Work => "work",
            ContractClass::Device => "device",
            ContractClass::Evidence => "evidence",
            ContractClass::Governance => "governance",
        }
    }
}

// ── AssetSpec ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetSpec {
    pub name: String,
    pub fields: Vec<String>,
}

// ── EvidenceSpec ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct EvidenceSpec {
    #[serde(default)]
    pub required: bool,
    #[serde(default)]
    pub r#type: Option<String>,
    #[serde(default)]
    pub fields: Vec<String>,
}

// ── WitnessPolicySpec ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WitnessPolicySpec {
    pub quorum: u8,
    pub types: Vec<String>,
}

// ── SettlementSpec ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SettlementSpec {
    pub currency: String,
    pub fee_routing: String,
    #[serde(default)]
    pub creator_share: f64,
    #[serde(default)]
    pub burn_share: f64,
    #[serde(default)]
    pub provider_share: f64,
}

// ── PolicySpec ────────────────────────────────────────────────────────────────

/// Freeform policy map. The validator checks well-known keys when present
/// but allows arbitrary extension keys for backend-specific configuration.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PolicySpec {
    #[serde(default)]
    pub deadline_enforcement: bool,
    #[serde(default)]
    pub quality_threshold: f64,
    #[serde(default)]
    pub esu_tithe: f64,
    #[serde(default)]
    pub fork_allowed: bool,
    #[serde(default)]
    pub private_execution: bool,
    #[serde(default)]
    pub escalation_path: Option<String>,
    /// Catch-all for extension keys not enumerated above.
    #[serde(flatten)]
    pub extra: std::collections::HashMap<String, serde_json::Value>,
}

// ── Validation ────────────────────────────────────────────────────────────────

impl OsoIR {
    /// Parse an OSO-IR JSON document and validate it.
    ///
    /// Returns `Ok(OsoIR)` when the document is valid, or
    /// `Err(Vec<String>)` containing all validation violations found.
    pub fn parse_and_validate(json: &str) -> Result<Self, Vec<String>> {
        let ir: OsoIR = match serde_json::from_str(json) {
            Ok(v) => v,
            Err(e) => return Err(vec![format!("JSON parse error: {}", e)]),
        };
        ir.validate().map(|()| ir)
    }

    /// Validate a deserialized OsoIR document.
    ///
    /// Collects all errors rather than short-circuiting on the first.
    pub fn validate(&self) -> Result<(), Vec<String>> {
        let mut errors: Vec<String> = Vec::new();

        // ── Global rules ────────────────────────────────────────────────────

        if self.oso_ir_version != "1.0" {
            errors.push(format!(
                "unknown oso_ir_version: {} (expected 1.0)",
                self.oso_ir_version
            ));
        }

        if !is_pascal_case(&self.contract_name) {
            errors.push(format!(
                "contract_name '{}' fails PascalCase rule (must match [A-Z][A-Za-z0-9]{{2,63}})",
                self.contract_name
            ));
        }

        if self.assets.is_empty() {
            errors.push("assets cannot be empty".to_string());
        }

        // Validate each asset
        for (i, asset) in self.assets.iter().enumerate() {
            if asset.name.is_empty() {
                errors.push(format!("assets[{}].name is empty", i));
            }
            if asset.fields.is_empty() {
                errors.push(format!(
                    "assets[{}] '{}': fields cannot be empty",
                    i, asset.name
                ));
            }
        }

        if self.minimum_tier > 5 {
            errors.push(format!(
                "minimum_tier {} out of range (0–5)",
                self.minimum_tier
            ));
        }

        if self.lifecycle.is_empty() {
            errors.push("lifecycle cannot be empty".to_string());
        }

        // Settlement share sum must not exceed 1.0
        let share_sum = self.settlement.creator_share
            + self.settlement.burn_share
            + self.settlement.provider_share;
        if share_sum > 1.0 + f64::EPSILON {
            errors.push(format!(
                "settlement shares sum to {:.4} — must be ≤ 1.0",
                share_sum
            ));
        }

        // Known currencies
        if !["ASE", "SUI", "USDC"].contains(&self.settlement.currency.as_str()) {
            errors.push(format!(
                "settlement.currency '{}' is not a known currency (ASE, SUI, USDC)",
                self.settlement.currency
            ));
        }

        // Known fee_routing values
        if !["6-pool", "direct", "dao-pool"].contains(&self.settlement.fee_routing.as_str()) {
            errors.push(format!(
                "settlement.fee_routing '{}' is not recognised (6-pool, direct, dao-pool)",
                self.settlement.fee_routing
            ));
        }

        // Esu tithe sanity check
        if self.policy.esu_tithe > 1.0 {
            errors.push(format!(
                "policy.esu_tithe {} > 1.0 — must be a fraction",
                self.policy.esu_tithe
            ));
        }

        // ── Class-specific rules ─────────────────────────────────────────────

        match self.contract_class {
            ContractClass::Work => {
                if self.capabilities.is_empty() {
                    errors.push("work contracts must declare at least one capability".to_string());
                }
                if !self.evidence.required {
                    errors.push(
                        "work contracts require evidence (evidence.required must be true)"
                            .to_string(),
                    );
                }
                if self.settlement.provider_share <= 0.5 {
                    errors.push(format!(
                        "work provider_share must exceed 0.5 (got {:.3})",
                        self.settlement.provider_share
                    ));
                }
                self.require_lifecycle_states(
                    &["ASSIGNED", "EXECUTED", "VERIFIED", "SETTLED"],
                    &mut errors,
                );
            }

            ContractClass::Agent => {
                if !self.assets_have_field("agent_id") {
                    errors
                        .push("agent contracts require an asset with field 'agent_id'".to_string());
                }
                self.require_lifecycle_states(&["REGISTERED", "DEREGISTERED"], &mut errors);
            }

            ContractClass::Financial => {
                if self.settlement.currency != "ASE" {
                    errors.push(format!(
                        "financial contracts should use currency ASE (got {})",
                        self.settlement.currency
                    ));
                }
                if !self.assets_have_any_field(&["balance", "pool"]) {
                    errors.push(
                        "financial contracts require an asset with 'balance' or 'pool' field"
                            .to_string(),
                    );
                }
            }

            ContractClass::Device => {
                if !self.assets_have_field("device_id") {
                    errors.push(
                        "device contracts require an asset with field 'device_id'".to_string(),
                    );
                }
                if self.evidence.required
                    && self.evidence.r#type.as_deref() != Some("DeviceAttestation")
                {
                    errors.push(
                        "device contracts with required evidence must use type DeviceAttestation"
                            .to_string(),
                    );
                }
                self.require_lifecycle_states(&["BOUND", "UNBOUND"], &mut errors);
            }

            ContractClass::Evidence => {
                if !self.assets_have_field("receipt_hash") {
                    errors.push(
                        "evidence contracts require an asset with field 'receipt_hash'".to_string(),
                    );
                }
                if self.settlement.fee_routing != "direct" {
                    errors.push(format!(
                        "evidence contracts should use fee_routing 'direct' (got {})",
                        self.settlement.fee_routing
                    ));
                }
                self.require_lifecycle_states(&["SUBMITTED", "VERIFIED"], &mut errors);
            }

            ContractClass::Governance => {
                if !self.assets_have_field("proposal_id") || !self.assets_have_field("proposer") {
                    errors.push(
                        "governance contracts require assets with 'proposal_id' and 'proposer' fields"
                            .to_string(),
                    );
                }
                if let Some(ref wp) = self.witness_policy {
                    if wp.quorum < 7 {
                        errors.push(format!(
                            "governance quorum must be >= 7 (got {})",
                            wp.quorum
                        ));
                    }
                } else {
                    errors.push("governance contracts require a witness_policy".to_string());
                }
                self.require_lifecycle_states(
                    &["PROPOSED", "VOTING", "ENACTED", "REJECTED"],
                    &mut errors,
                );
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    // ── Internal helpers ──────────────────────────────────────────────────────

    fn assets_have_field(&self, field: &str) -> bool {
        self.assets
            .iter()
            .any(|a| a.fields.iter().any(|f| f == field))
    }

    fn assets_have_any_field(&self, fields: &[&str]) -> bool {
        self.assets
            .iter()
            .any(|a| a.fields.iter().any(|f| fields.contains(&f.as_str())))
    }

    fn require_lifecycle_states(&self, required: &[&str], errors: &mut Vec<String>) {
        for state in required {
            if !self.lifecycle.iter().any(|s| s == state) {
                errors.push(format!(
                    "lifecycle for {} contract must include '{}' state",
                    self.contract_class.as_str(),
                    state
                ));
            }
        }
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Check that a string matches `[A-Z][A-Za-z0-9]{2,63}` (PascalCase, 3–64 chars).
fn is_pascal_case(s: &str) -> bool {
    if s.len() < 3 || s.len() > 64 {
        return false;
    }
    let mut chars = s.chars();
    match chars.next() {
        Some(c) if c.is_ascii_uppercase() => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_alphanumeric())
}

// ── Public convenience ────────────────────────────────────────────────────────

/// Validate a raw JSON string as an OSO-IR document.
///
/// Returns a human-readable summary string:
/// - `"valid: ContractName (work)"` on success
/// - `"invalid: [error1, error2, ...]"` on failure
pub fn validate_json(json: &str) -> String {
    match OsoIR::parse_and_validate(json) {
        Ok(ir) => format!(
            "valid: {} ({})",
            ir.contract_name,
            ir.contract_class.as_str()
        ),
        Err(errs) => format!("invalid: [{}]", errs.join("; ")),
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn minimal_work_ir() -> &'static str {
        r#"{
            "oso_ir_version": "1.0",
            "contract_class": "work",
            "contract_name": "MyJob",
            "assets": [{"name": "Job", "fields": ["job_id", "creator"]}],
            "capabilities": ["GPU_COMPUTE"],
            "minimum_tier": 2,
            "evidence": {"required": true, "type": "ComputeReceipt", "fields": ["gpu_seconds"]},
            "settlement": {
                "currency": "ASE",
                "fee_routing": "6-pool",
                "creator_share": 0.10,
                "burn_share": 0.05,
                "provider_share": 0.85
            },
            "policy": {"esu_tithe": 0.0369},
            "lifecycle": ["CREATED", "ASSIGNED", "EXECUTED", "VERIFIED", "SETTLED"]
        }"#
    }

    #[test]
    fn valid_work_contract_passes() {
        let ir = OsoIR::parse_and_validate(minimal_work_ir());
        assert!(ir.is_ok(), "expected Ok, got: {:?}", ir);
    }

    #[test]
    fn wrong_version_fails() {
        let json = minimal_work_ir().replace("\"1.0\"", "\"2.0\"");
        let result = OsoIR::parse_and_validate(&json);
        let errs = result.unwrap_err();
        assert!(errs.iter().any(|e| e.contains("unknown oso_ir_version")));
    }

    #[test]
    fn empty_assets_fails() {
        let json = minimal_work_ir().replace(
            r#"[{"name": "Job", "fields": ["job_id", "creator"]}]"#,
            "[]",
        );
        let result = OsoIR::parse_and_validate(&json);
        let errs = result.unwrap_err();
        assert!(errs.iter().any(|e| e.contains("assets cannot be empty")));
    }

    #[test]
    fn work_without_capabilities_fails() {
        let json = minimal_work_ir().replace(
            r#""capabilities": ["GPU_COMPUTE"]"#,
            r#""capabilities": []"#,
        );
        let result = OsoIR::parse_and_validate(&json);
        let errs = result.unwrap_err();
        assert!(errs
            .iter()
            .any(|e| e.contains("work contracts must declare at least one capability")));
    }

    #[test]
    fn work_with_low_provider_share_fails() {
        let json = minimal_work_ir().replace("0.85", "0.40");
        let result = OsoIR::parse_and_validate(&json);
        let errs = result.unwrap_err();
        assert!(errs
            .iter()
            .any(|e| e.contains("provider_share must exceed 0.5")));
    }

    #[test]
    fn settlement_shares_overflow_fails() {
        let json = minimal_work_ir()
            .replace("\"creator_share\": 0.10", "\"creator_share\": 0.50")
            .replace("\"burn_share\": 0.05", "\"burn_share\": 0.50")
            .replace("\"provider_share\": 0.85", "\"provider_share\": 0.85");
        let result = OsoIR::parse_and_validate(&json);
        let errs = result.unwrap_err();
        assert!(errs.iter().any(|e| e.contains("settlement shares sum")));
    }

    #[test]
    fn is_pascal_case_rules() {
        assert!(is_pascal_case("MyContract"));
        assert!(is_pascal_case("GPUMarket"));
        assert!(!is_pascal_case("myContract")); // lowercase first
        assert!(!is_pascal_case("A")); // too short
        assert!(!is_pascal_case("My_Contract")); // underscore
    }

    #[test]
    fn governance_requires_quorum_7() {
        let gov = r#"{
            "oso_ir_version": "1.0",
            "contract_class": "governance",
            "contract_name": "CouncilDao",
            "assets": [
                {"name": "Proposal", "fields": ["proposal_id", "proposer"]},
                {"name": "Vote", "fields": ["vote_id", "voter_agent_id"]}
            ],
            "capabilities": ["PROPOSAL_CREATE"],
            "minimum_tier": 3,
            "evidence": {"required": true, "type": "GovernanceVote", "fields": ["proposal_id"]},
            "witness_policy": {"quorum": 3, "types": ["agent"]},
            "settlement": {"currency": "ASE", "fee_routing": "dao-pool", "creator_share": 0.0, "burn_share": 0.03, "provider_share": 0.0},
            "policy": {"esu_tithe": 0.0369},
            "lifecycle": ["PROPOSED", "VOTING", "ENACTED", "REJECTED"]
        }"#;
        let result = OsoIR::parse_and_validate(gov);
        let errs = result.unwrap_err();
        assert!(errs
            .iter()
            .any(|e| e.contains("governance quorum must be >= 7")));
    }

    #[test]
    fn validate_json_convenience_fn() {
        let result = validate_json(minimal_work_ir());
        assert!(
            result.starts_with("valid:"),
            "expected valid, got: {}",
            result
        );
    }
}
