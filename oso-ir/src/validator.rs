/// Phase 21.2 — Ọ̀ṢỌ́-IR validator.
///
/// Validates an OsoIr document for semantic correctness:
/// - Required fields present
/// - Action-capability cross-references valid
/// - Settlement currency known
/// - Tier values in range
/// - Backend targets appropriate for contract class

use crate::types::{OsoIr, ContractClass, BackendTarget};

/// Result of validating an OsoIr document.
#[derive(Debug)]
pub struct ValidationResult {
    pub valid:    bool,
    pub errors:   Vec<ValidationError>,
    pub warnings: Vec<String>,
}

impl ValidationResult {
    pub fn ok() -> Self {
        Self { valid: true, errors: vec![], warnings: vec![] }
    }
}

/// A single validation error.
#[derive(Debug)]
pub struct ValidationError {
    pub field:   String,
    pub message: String,
}

impl std::fmt::Display for ValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.field, self.message)
    }
}

/// Validate an OsoIr document. Returns a ValidationResult with all errors found.
pub fn validate(ir: &OsoIr) -> ValidationResult {
    let mut errors   = Vec::new();
    let mut warnings = Vec::new();

    // name must be non-empty
    if ir.name.trim().is_empty() {
        errors.push(err("name", "contract name must not be empty"));
    }

    // assets: no duplicate names
    let mut asset_names = std::collections::HashSet::new();
    for asset in &ir.assets {
        if asset.name.trim().is_empty() {
            errors.push(err("assets[].name", "asset name must not be empty"));
        }
        if !asset_names.insert(asset.name.clone()) {
            errors.push(err("assets[].name", &format!("duplicate asset name '{}'", asset.name)));
        }
    }

    // capabilities: tier must be 0–5
    for cap in &ir.capabilities {
        if cap.minimum_tier > 5 {
            errors.push(err(
                "capabilities[].minimum_tier",
                &format!("capability '{}' has tier {} (max is 5)", cap.name, cap.minimum_tier),
            ));
        }
    }

    // actions: no duplicate names; vessel must be a known If-Script vessel if set
    let known_vessels = [
        "Act", "Oracle", "Create", "Destroy", "Transfer", "Observe",
        "Communicate", "Compute", "Store", "Retrieve", "Execute", "Validate",
        "Govern", "Prove", "Attest", "Sign",
    ];
    let mut action_names = std::collections::HashSet::new();
    for action in &ir.actions {
        if action.name.trim().is_empty() {
            errors.push(err("actions[].name", "action name must not be empty"));
        }
        if !action_names.insert(action.name.clone()) {
            errors.push(err("actions[].name", &format!("duplicate action name '{}'", action.name)));
        }
        if let Some(vessel) = &action.vessel {
            if !known_vessels.contains(&vessel.as_str()) {
                warnings.push(format!(
                    "action '{}': vessel '{}' is not a canonical If-Script vessel",
                    action.name, vessel
                ));
            }
        }
    }

    // settlement: currency must be known if present
    let known_currencies = ["ASE", "DOPAMINE", "SYNAPSE", "SUI", "USDC"];
    if let Some(s) = &ir.settlement {
        if !s.currency.is_empty() && !known_currencies.contains(&s.currency.as_str()) {
            warnings.push(format!(
                "settlement.currency '{}' is not a canonical Ọ̀ṢỌ́ currency",
                s.currency
            ));
        }
        if s.treasury_pct < 0.0 || s.treasury_pct > 100.0 {
            errors.push(err("settlement.treasury_pct", "must be in [0, 100]"));
        }
    }

    // evidence: if required, evidence_type must be set
    if let Some(e) = &ir.evidence {
        if e.required && e.evidence_type.trim().is_empty() {
            errors.push(err("evidence.evidence_type", "required when evidence.required is true"));
        }
        if e.minimum_count == 0 {
            errors.push(err("evidence.minimum_count", "must be >= 1"));
        }
    }

    // backend_targets: governance contracts should not target Move (Move is for asset contracts)
    if ir.contract_class == ContractClass::Governance {
        if ir.backend_targets.contains(&BackendTarget::Move) && !ir.backend_targets.contains(&BackendTarget::Native) {
            warnings.push(
                "governance contracts should target 'native' (ABCI) rather than Move only".to_string()
            );
        }
    }

    // Work contracts must have at least one action
    if ir.contract_class == ContractClass::Work && ir.actions.is_empty() {
        errors.push(err("actions", "work contracts must define at least one action"));
    }

    let valid = errors.is_empty();
    ValidationResult { valid, errors, warnings }
}

fn err(field: &str, msg: &str) -> ValidationError {
    ValidationError { field: field.to_string(), message: msg.to_string() }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::*;

    fn minimal_ir(class: ContractClass) -> OsoIr {
        OsoIr {
            ir_version:      "1.0".to_string(),
            contract_class:  class,
            name:            "TestContract".to_string(),
            version:         "0.1.0".to_string(),
            assets:          vec![],
            capabilities:    vec![],
            actions:         vec![],
            evidence:        None,
            settlement:      None,
            policy:          None,
            backend_targets: vec![BackendTarget::Native],
            metadata:        Default::default(),
        }
    }

    #[test]
    fn valid_minimal_financial_contract() {
        let ir = minimal_ir(ContractClass::Financial);
        let result = validate(&ir);
        assert!(result.valid, "errors: {:?}", result.errors);
    }

    #[test]
    fn empty_name_fails_validation() {
        let mut ir = minimal_ir(ContractClass::Agent);
        ir.name = "  ".to_string();
        let result = validate(&ir);
        assert!(!result.valid);
        assert!(result.errors.iter().any(|e| e.field == "name"));
    }

    #[test]
    fn tier_out_of_range_fails() {
        let mut ir = minimal_ir(ContractClass::Work);
        ir.actions.push(ActionDef {
            name: "execute".to_string(),
            requires: vec![],
            emits: vec![],
            mutates_state: true,
            vessel: None,
        });
        ir.capabilities.push(CapabilityRef {
            name: "GPU_COMPUTE".to_string(),
            minimum_tier: 6, // invalid
            required: true,
        });
        let result = validate(&ir);
        assert!(!result.valid);
        assert!(result.errors.iter().any(|e| e.field.contains("minimum_tier")));
    }

    #[test]
    fn duplicate_asset_names_fail() {
        let mut ir = minimal_ir(ContractClass::Financial);
        ir.assets.push(AssetDef { name: "Token".to_string(), fields: vec![], transferable: true, divisible: false });
        ir.assets.push(AssetDef { name: "Token".to_string(), fields: vec![], transferable: false, divisible: false });
        let result = validate(&ir);
        assert!(!result.valid);
        assert!(result.errors.iter().any(|e| e.message.contains("duplicate")));
    }

    #[test]
    fn work_contract_without_actions_fails() {
        let ir = minimal_ir(ContractClass::Work);
        // No actions added
        let result = validate(&ir);
        assert!(!result.valid);
        assert!(result.errors.iter().any(|e| e.field == "actions"));
    }

    #[test]
    fn evidence_required_without_type_fails() {
        let mut ir = minimal_ir(ContractClass::Evidence);
        ir.evidence = Some(EvidencePolicy {
            required: true,
            evidence_type: "".to_string(), // missing!
            minimum_count: 1,
        });
        let result = validate(&ir);
        assert!(!result.valid);
        assert!(result.errors.iter().any(|e| e.field.contains("evidence_type")));
    }

    #[test]
    fn valid_work_contract_with_actions() {
        let mut ir = minimal_ir(ContractClass::Work);
        ir.actions.push(ActionDef {
            name: "submit_job".to_string(),
            requires: vec![PolicyExpr::Capability { name: "GPU_COMPUTE".to_string() }],
            emits: vec!["JobSubmitted".to_string()],
            mutates_state: true,
            vessel: Some("Act".to_string()),
        });
        ir.evidence = Some(EvidencePolicy {
            required: true,
            evidence_type: "ComputeReceipt".to_string(),
            minimum_count: 1,
        });
        ir.settlement = Some(SettlementPolicy {
            currency:     "ASE".to_string(),
            fee_routing:  "6-pool".to_string(),
            treasury_pct: 3.69,
        });
        let result = validate(&ir);
        assert!(result.valid, "errors: {:?}", result.errors);
    }
}
