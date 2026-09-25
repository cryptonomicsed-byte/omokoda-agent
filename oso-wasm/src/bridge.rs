/// Phase 24.1/24.2 — Bridge from canonical `oso-ir` types to `oso-wasm` internal `OsoIR`.
///
/// Same adapter pattern as `oso-move/src/bridge.rs`: canonical OsoIr → internal OsoIR →
/// WasmCodegen::compile() → WAT source string.
use crate::ir::{AssetSpec, EvidenceSpec, OsoIR, SettlementSpec};

/// Translate canonical `oso_ir::OsoIr` to the internal `OsoIR` used by `WasmCodegen`.
pub fn to_wasm_ir(ir: &oso_ir::OsoIr) -> OsoIR {
    let assets: Vec<AssetSpec> = ir.assets.iter().map(|a| AssetSpec {
        name:   a.name.clone(),
        fields: a.fields.iter().map(|f| f.name.clone()).collect(),
    }).collect();

    let capabilities: Vec<String> = ir.capabilities.iter().map(|c| c.name.clone()).collect();

    let evidence = if let Some(e) = &ir.evidence {
        EvidenceSpec {
            required: e.required,
            kind:     None, // WasmCodegen uses the raw string from policy; kind enum not required
            fields:   vec![],
        }
    } else {
        EvidenceSpec { required: false, kind: None, fields: vec![] }
    };

    let settlement = if let Some(s) = &ir.settlement {
        SettlementSpec {
            currency:       Some(s.currency.clone()),
            fee_routing:    Some(s.fee_routing.clone()),
            creator_share:  Some(0.0),
            burn_share:     Some(0.0),
            provider_share: Some(1.0 - (s.treasury_pct / 100.0)),
            amount:         None,
            tithe_rate:     Some(s.treasury_pct / 100.0),
        }
    } else {
        SettlementSpec {
            currency:      Some("ASE".to_string()),
            fee_routing:   Some("6-pool".to_string()),
            creator_share: None, burn_share: None, provider_share: None,
            amount: None, tithe_rate: Some(0.0369),
        }
    };

    let minimum_tier = ir.capabilities.iter().map(|c| c.minimum_tier).max().unwrap_or(0);

    let lifecycle = derive_lifecycle(&ir.contract_class);

    OsoIR {
        oso_ir_version:  ir.ir_version.clone(),
        contract_class:  contract_class_to_str(&ir.contract_class),
        contract_name:   ir.name.clone(),
        assets,
        capabilities,
        minimum_tier,
        evidence,
        witness_policy:  None,
        settlement,
        policy:          Default::default(),
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

fn derive_lifecycle(class: &oso_ir::ContractClass) -> Vec<String> {
    match class {
        oso_ir::ContractClass::Financial  =>
            vec!["OPEN","FUNDED","ACTIVE","SETTLING","CLOSED"],
        oso_ir::ContractClass::Agent      =>
            vec!["CREATED","REGISTERED","ACTIVE","SUSPENDED","DEREGISTERED"],
        oso_ir::ContractClass::Work       =>
            vec!["CREATED","ASSIGNED","ACCEPTED","STARTED","EXECUTED","VERIFIED","SETTLED"],
        oso_ir::ContractClass::Device     =>
            vec!["DISCOVERED","MANIFESTED","BOUND","ACTIVE","SUSPENDED","UNBOUND"],
        oso_ir::ContractClass::Evidence   =>
            vec!["SUBMITTED","PENDING_WITNESS","VERIFIED","ARCHIVED"],
        oso_ir::ContractClass::Governance =>
            vec!["PROPOSED","SECONDED","VOTING","TALLYING","ENACTED","REJECTED"],
    }.into_iter().map(str::to_string).collect()
}

/// High-level: compile canonical OsoIr directly to WAT source.
pub fn compile_to_wasm(ir: &oso_ir::OsoIr) -> Result<String, crate::codegen::CompileError> {
    let internal = to_wasm_ir(ir);
    crate::codegen::WasmCodegen::compile(&internal)
}

#[cfg(test)]
mod tests {
    use super::*;
    use oso_ir::types::*;

    fn minimal_financial_ir() -> OsoIr {
        OsoIr {
            ir_version:     "1.0".to_string(),
            contract_class:  ContractClass::Financial,
            name:            "AsePool".to_string(),
            version:         "0.1.0".to_string(),
            assets:          vec![AssetDef { name: "Pool".to_string(), fields: vec![], transferable: false, divisible: false }],
            capabilities:    vec![],
            actions:         vec![],
            evidence:        Some(EvidencePolicy { required: true, evidence_type: "ZangbetoReceipt".to_string(), minimum_count: 1 }),
            settlement:      Some(SettlementPolicy { currency: "ASE".to_string(), fee_routing: "6-pool".to_string(), treasury_pct: 3.69 }),
            policy:          None,
            backend_targets: vec![BackendTarget::Wasm],
            metadata:        Default::default(),
        }
    }

    #[test]
    fn bridge_financial_compiles_to_wat() {
        let oso = minimal_financial_ir();
        let result = compile_to_wasm(&oso);
        assert!(result.is_ok(), "bridge compile failed: {:?}", result.err());
        let wat = result.unwrap();
        assert!(wat.contains("(module"), "missing WAT module");
    }

    #[test]
    fn bridge_lifecycle_financial() {
        let oso = minimal_financial_ir();
        let internal = to_wasm_ir(&oso);
        assert_eq!(internal.lifecycle[0], "OPEN");
        assert_eq!(*internal.lifecycle.last().unwrap(), "CLOSED");
    }

    #[test]
    fn bridge_tithe_rate_preserved() {
        let oso = minimal_financial_ir();
        let internal = to_wasm_ir(&oso);
        let rate = internal.settlement.tithe_rate.unwrap_or(0.0);
        assert!((rate - 0.0369).abs() < 0.001);
    }
}
