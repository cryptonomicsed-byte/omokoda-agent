/// Phase 23.1 — Bridge from canonical `oso-ir` types to `oso-move` internal `OsoIR`.
///
/// The canonical `oso_ir::OsoIr` uses the Phase 21 type hierarchy (actions, PolicyExpr, etc.).
/// The existing `oso-move` codegen was written against the Phase 21 spec draft (lifecycle array,
/// asset fields as Vec<String>). This bridge translates between the two so that:
///
///   oso_ir::OsoIr → bridge::to_move_ir() → OsoIR (internal) → MoveCodegen::compile() → Move source
use crate::ir::{AssetSpec, EvidenceSpec, OsoIR, SettlementSpec};
use std::collections::HashMap;

/// Translate a canonical `oso_ir::OsoIr` document to the internal `OsoIR` used by `MoveCodegen`.
pub fn to_move_ir(ir: &oso_ir::OsoIr) -> OsoIR {
    let assets: Vec<AssetSpec> = ir.assets.iter().map(|a| AssetSpec {
        name:   a.name.clone(),
        fields: a.fields.iter().map(|f| f.name.clone()).collect(),
    }).collect();

    let capabilities: Vec<String> = ir.capabilities.iter().map(|c| c.name.clone()).collect();

    let evidence = if let Some(e) = &ir.evidence {
        EvidenceSpec {
            required: e.required,
            kind:     if e.evidence_type.is_empty() { None } else { Some(e.evidence_type.clone()) },
            fields:   vec![],
        }
    } else {
        EvidenceSpec { required: false, kind: None, fields: vec![] }
    };

    let settlement = if let Some(s) = &ir.settlement {
        SettlementSpec {
            currency:       s.currency.clone(),
            fee_routing:    s.fee_routing.clone(),
            creator_share:  0.0,
            burn_share:     0.0,
            provider_share: 1.0 - (s.treasury_pct / 100.0),
        }
    } else {
        SettlementSpec {
            currency:       "ASE".to_string(),
            fee_routing:    "6-pool".to_string(),
            creator_share:  0.0,
            burn_share:     0.0,
            provider_share: 0.9631,
        }
    };

    // Build lifecycle from action names (since canonical OsoIr uses actions, not lifecycle array)
    let lifecycle = derive_lifecycle(&ir.contract_class, &ir.actions);

    // Build policy map
    let mut policy: HashMap<String, serde_json::Value> = HashMap::new();
    if let Some(s) = &ir.settlement {
        policy.insert("esu_tithe".to_string(), serde_json::Value::Number(
            serde_json::Number::from_f64(s.treasury_pct / 100.0).unwrap_or(serde_json::Number::from(0))
        ));
    }
    if let Some(p) = &ir.policy {
        policy.insert("witness_quorum".to_string(), serde_json::Value::Number(serde_json::Number::from(p.witness_quorum)));
    }

    let minimum_tier = ir.capabilities.iter().map(|c| c.minimum_tier).max().unwrap_or(0);

    OsoIR {
        oso_ir_version: ir.ir_version.clone(),
        contract_class:  contract_class_to_str(&ir.contract_class),
        contract_name:   ir.name.clone(),
        assets,
        capabilities,
        minimum_tier,
        evidence,
        witness_policy:  None,
        settlement,
        policy,
        lifecycle,
    }
}

fn contract_class_to_str(c: &oso_ir::ContractClass) -> String {
    match c {
        oso_ir::ContractClass::Financial  => "financial".to_string(),
        oso_ir::ContractClass::Agent      => "agent".to_string(),
        oso_ir::ContractClass::Work       => "work".to_string(),
        oso_ir::ContractClass::Device     => "device".to_string(),
        oso_ir::ContractClass::Evidence   => "evidence".to_string(),
        oso_ir::ContractClass::Governance => "governance".to_string(),
    }
}

fn derive_lifecycle(class: &oso_ir::ContractClass, actions: &[oso_ir::ActionDef]) -> Vec<String> {
    // If actions specify a clear progression, derive lifecycle from canonical class pattern.
    // Each contract class has a canonical lifecycle per the spec.
    match class {
        oso_ir::ContractClass::Financial  =>
            vec!["OPEN","FUNDED","ACTIVE","SETTLING","CLOSED"].into_iter().map(str::to_string).collect(),
        oso_ir::ContractClass::Agent      =>
            vec!["CREATED","REGISTERED","ACTIVE","SUSPENDED","DEREGISTERED"].into_iter().map(str::to_string).collect(),
        oso_ir::ContractClass::Work       =>
            vec!["CREATED","ASSIGNED","ACCEPTED","STARTED","EXECUTED","VERIFIED","SETTLED"].into_iter().map(str::to_string).collect(),
        oso_ir::ContractClass::Device     =>
            vec!["DISCOVERED","MANIFESTED","BOUND","ACTIVE","SUSPENDED","UNBOUND"].into_iter().map(str::to_string).collect(),
        oso_ir::ContractClass::Evidence   =>
            vec!["SUBMITTED","PENDING_WITNESS","VERIFIED","ARCHIVED"].into_iter().map(str::to_string).collect(),
        oso_ir::ContractClass::Governance =>
            vec!["PROPOSED","SECONDED","VOTING","TALLYING","ENACTED","REJECTED"].into_iter().map(str::to_string).collect(),
    }
}

/// High-level: compile canonical OsoIr directly to a Move module source string.
pub fn compile_to_move(ir: &oso_ir::OsoIr) -> Result<String, crate::codegen::CompileError> {
    let internal = to_move_ir(ir);
    crate::codegen::MoveCodegen::compile(&internal)
}

#[cfg(test)]
mod tests {
    use super::*;
    use oso_ir::types::*;

    fn minimal_work_ir() -> oso_ir::OsoIr {
        OsoIr {
            ir_version:     "1.0".to_string(),
            contract_class:  ContractClass::Work,
            name:            "GpuMarketplace".to_string(),
            version:         "0.1.0".to_string(),
            assets:          vec![AssetDef { name: "ComputeJob".to_string(), fields: vec![], transferable: false, divisible: false }],
            capabilities:    vec![CapabilityRef { name: "GPU_COMPUTE".to_string(), minimum_tier: 2, required: true }],
            actions:         vec![ActionDef { name: "submit_job".to_string(), requires: vec![], emits: vec![], mutates_state: true, vessel: Some("Create".to_string()) }],
            evidence:        Some(EvidencePolicy { required: true, evidence_type: "ComputeReceipt".to_string(), minimum_count: 1 }),
            settlement:      Some(SettlementPolicy { currency: "ASE".to_string(), fee_routing: "6-pool".to_string(), treasury_pct: 3.69 }),
            policy:          None,
            backend_targets: vec![BackendTarget::Move],
            metadata:        Default::default(),
        }
    }

    #[test]
    fn bridge_work_contract_compiles() {
        let oso = minimal_work_ir();
        let result = compile_to_move(&oso);
        assert!(result.is_ok(), "bridge compile failed: {:?}", result.err());
        let src = result.unwrap();
        assert!(src.contains("module oso_contracts::gpu_marketplace"));
        assert!(src.contains("struct ComputeJob"));
        assert!(src.contains("GPU_COMPUTE"));
    }

    #[test]
    fn bridge_lifecycle_derived_correctly() {
        let oso = minimal_work_ir();
        let internal = to_move_ir(&oso);
        assert_eq!(internal.lifecycle[0], "CREATED");
        assert_eq!(*internal.lifecycle.last().unwrap(), "SETTLED");
    }

    #[test]
    fn bridge_settlement_currency_preserved() {
        let oso = minimal_work_ir();
        let internal = to_move_ir(&oso);
        assert_eq!(internal.settlement.currency, "ASE");
        assert_eq!(internal.settlement.fee_routing, "6-pool");
    }
}
