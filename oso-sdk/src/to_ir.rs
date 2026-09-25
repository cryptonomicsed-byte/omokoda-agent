/// Phase 25.1 — Bridge from SDK `ContractClass` to canonical `OsoIr`.
///
/// Each contract class has a canonical OsoIr template. Callers can use these
/// as starting points, then customize (add assets, capabilities, settlement params)
/// before compiling to Move/WASM/Native.
use crate::contract::ContractClass;
use oso_ir::types::*;

/// Return the canonical `OsoIr` template for a given `ContractClass`.
///
/// The returned document is valid and passes `oso_ir::validator::validate()`.
/// Customize fields (name, assets, capabilities, settlement) as needed before compiling.
pub fn canonical_ir(class: ContractClass, name: impl Into<String>) -> OsoIr {
    let name = name.into();
    match class {
        ContractClass::Financial  => financial_template(name),
        ContractClass::Agent      => agent_template(name),
        ContractClass::Work       => work_template(name),
        ContractClass::Device     => device_template(name),
        ContractClass::Evidence   => evidence_template(name),
        ContractClass::Governance => governance_template(name),
    }
}

fn sdk_class_to_ir(class: ContractClass) -> oso_ir::ContractClass {
    match class {
        ContractClass::Financial  => oso_ir::ContractClass::Financial,
        ContractClass::Agent      => oso_ir::ContractClass::Agent,
        ContractClass::Work       => oso_ir::ContractClass::Work,
        ContractClass::Device     => oso_ir::ContractClass::Device,
        ContractClass::Evidence   => oso_ir::ContractClass::Evidence,
        ContractClass::Governance => oso_ir::ContractClass::Governance,
    }
}

fn base(class: ContractClass, name: String) -> OsoIr {
    OsoIr {
        ir_version:      "1.0".to_string(),
        contract_class:  sdk_class_to_ir(class),
        name,
        version:         "0.1.0".to_string(),
        assets:          vec![],
        capabilities:    vec![],
        actions:         vec![],
        evidence:        None,
        settlement:      Some(SettlementPolicy {
            currency:     "ASE".to_string(),
            fee_routing:  "6-pool".to_string(),
            treasury_pct: 3.69,
        }),
        policy:          None,
        backend_targets: vec![BackendTarget::Native],
        metadata:        Default::default(),
    }
}

fn financial_template(name: String) -> OsoIr {
    let mut ir = base(ContractClass::Financial, name);
    ir.assets = vec![AssetDef {
        name: "Pool".to_string(),
        fields: vec![
            FieldDef { name: "pool_id".to_string(),  field_type: "string".to_string(),  required: true },
            FieldDef { name: "balance".to_string(),  field_type: "u64".to_string(),     required: true },
            FieldDef { name: "owner".to_string(),    field_type: "address".to_string(), required: true },
        ],
        transferable: true,
        divisible:    true,
    }];
    ir.actions = vec![
        mk_action("deposit",  "Store",    vec![PolicyExpr::Principal { role: "authorized".to_string() }]),
        mk_action("withdraw", "Transfer", vec![PolicyExpr::Principal { role: "owner".to_string() }]),
        mk_action("settle",   "Attest",   vec![]),
    ];
    ir.evidence = Some(EvidencePolicy { required: true, evidence_type: "ZangbetoReceipt".to_string(), minimum_count: 1 });
    ir
}

fn agent_template(name: String) -> OsoIr {
    let mut ir = base(ContractClass::Agent, name);
    ir.assets = vec![AssetDef {
        name: "AgentRecord".to_string(),
        fields: vec![
            FieldDef { name: "agent_id".to_string(),  field_type: "string".to_string(), required: true },
            FieldDef { name: "nostr_pub".to_string(), field_type: "string".to_string(), required: true },
            FieldDef { name: "tier".to_string(),      field_type: "u32".to_string(),    required: true },
            FieldDef { name: "born_at".to_string(),   field_type: "u64".to_string(),    required: true },
        ],
        transferable: false, divisible: false,
    }];
    ir.actions = vec![
        mk_action("register",    "Create",  vec![PolicyExpr::Principal { role: "authorized".to_string() }]),
        mk_action("deregister",  "Destroy", vec![PolicyExpr::Principal { role: "owner".to_string() }]),
        mk_action("update_tier", "Govern",  vec![PolicyExpr::Principal { role: "authorized".to_string() }]),
    ];
    ir
}

fn work_template(name: String) -> OsoIr {
    let mut ir = base(ContractClass::Work, name);
    ir.assets = vec![AssetDef {
        name: "ComputeJob".to_string(),
        fields: vec![
            FieldDef { name: "job_id".to_string(),  field_type: "string".to_string(),  required: true },
            FieldDef { name: "creator".to_string(), field_type: "address".to_string(), required: true },
            FieldDef { name: "budget".to_string(),  field_type: "u64".to_string(),     required: true },
        ],
        transferable: false, divisible: false,
    }];
    ir.capabilities = vec![
        CapabilityRef { name: "GPU_COMPUTE".to_string(), minimum_tier: 2, required: true },
    ];
    ir.actions = vec![
        mk_action("submit_job",    "Create",   vec![PolicyExpr::Principal { role: "authorized".to_string() }]),
        mk_action("accept_job",    "Act",      vec![PolicyExpr::Capability { name: "GPU_COMPUTE".to_string() }]),
        mk_action("submit_result", "Compute",  vec![PolicyExpr::Capability { name: "GPU_COMPUTE".to_string() }]),
        mk_action("settle",        "Transfer", vec![PolicyExpr::Proof { proof_type: "ComputeReceipt".to_string() }]),
    ];
    ir.evidence = Some(EvidencePolicy { required: true, evidence_type: "ComputeReceipt".to_string(), minimum_count: 1 });
    ir.policy = Some(WitnessPolicy { witness_quorum: 2, quality_threshold: 77, upgradeable: false, requires_council: false });
    ir
}

fn device_template(name: String) -> OsoIr {
    let mut ir = base(ContractClass::Device, name);
    ir.assets = vec![AssetDef {
        name: "DeviceRecord".to_string(),
        fields: vec![
            FieldDef { name: "device_id".to_string(), field_type: "string".to_string(), required: true },
            FieldDef { name: "agent_id".to_string(),  field_type: "string".to_string(), required: true },
            FieldDef { name: "bound_at".to_string(),  field_type: "u64".to_string(),    required: true },
        ],
        transferable: false, divisible: false,
    }];
    ir.capabilities = vec![
        CapabilityRef { name: "DEVICE_INHABIT".to_string(), minimum_tier: 1, required: true },
    ];
    ir.actions = vec![
        mk_action("bind",   "Attest",   vec![PolicyExpr::Capability { name: "DEVICE_INHABIT".to_string() }]),
        mk_action("unbind", "Validate", vec![PolicyExpr::Principal { role: "owner".to_string() }]),
    ];
    ir.evidence = Some(EvidencePolicy { required: true, evidence_type: "DeviceAttestation".to_string(), minimum_count: 1 });
    ir
}

fn evidence_template(name: String) -> OsoIr {
    let mut ir = base(ContractClass::Evidence, name);
    ir.assets = vec![AssetDef {
        name: "Receipt".to_string(),
        fields: vec![
            FieldDef { name: "receipt_hash".to_string(), field_type: "hash".to_string(),   required: true },
            FieldDef { name: "agent_id".to_string(),     field_type: "string".to_string(), required: true },
            FieldDef { name: "timestamp".to_string(),    field_type: "u64".to_string(),    required: true },
        ],
        transferable: false, divisible: false,
    }];
    ir.actions = vec![
        mk_action("submit",  "Attest", vec![PolicyExpr::Proof { proof_type: "generic".to_string() }]),
        mk_action("verify",  "Prove",  vec![]),
        mk_action("archive", "Store",  vec![]),
    ];
    ir.evidence = Some(EvidencePolicy { required: true, evidence_type: "ZangbetoReceipt".to_string(), minimum_count: 1 });
    ir.settlement = Some(SettlementPolicy { currency: "ASE".to_string(), fee_routing: "direct".to_string(), treasury_pct: 3.69 });
    ir
}

fn governance_template(name: String) -> OsoIr {
    let mut ir = base(ContractClass::Governance, name);
    ir.assets = vec![
        AssetDef {
            name: "Proposal".to_string(),
            fields: vec![
                FieldDef { name: "proposal_id".to_string(), field_type: "string".to_string(),  required: true },
                FieldDef { name: "proposer".to_string(),    field_type: "address".to_string(), required: true },
                FieldDef { name: "sector".to_string(),      field_type: "u32".to_string(),     required: true },
            ],
            transferable: false, divisible: false,
        },
        AssetDef {
            name: "Vote".to_string(),
            fields: vec![
                FieldDef { name: "proposal_id".to_string(), field_type: "string".to_string(),  required: true },
                FieldDef { name: "voter".to_string(),       field_type: "address".to_string(), required: true },
                FieldDef { name: "in_favor".to_string(),    field_type: "bool".to_string(),    required: true },
            ],
            transferable: false, divisible: false,
        },
    ];
    ir.capabilities = vec![
        CapabilityRef { name: "PROPOSAL_CREATE".to_string(), minimum_tier: 3, required: true },
    ];
    ir.actions = vec![
        mk_action("create_proposal", "Govern",   vec![PolicyExpr::Capability { name: "PROPOSAL_CREATE".to_string() }]),
        mk_action("vote",            "Validate", vec![PolicyExpr::Principal { role: "authorized".to_string() }]),
        mk_action("enact",           "Execute",  vec![PolicyExpr::Proof { proof_type: "WitnessBundle".to_string() }]),
        mk_action("reject",          "Govern",   vec![]),
    ];
    ir.evidence = Some(EvidencePolicy { required: true, evidence_type: "WitnessBundle".to_string(), minimum_count: 7 });
    ir.policy = Some(WitnessPolicy { witness_quorum: 7, quality_threshold: 0, upgradeable: false, requires_council: true });
    ir.settlement = Some(SettlementPolicy { currency: "ASE".to_string(), fee_routing: "dao-pool".to_string(), treasury_pct: 3.69 });
    ir
}

fn mk_action(name: &str, vessel: &str, requires: Vec<PolicyExpr>) -> ActionDef {
    ActionDef {
        name:          name.to_string(),
        requires,
        emits:         vec![format!("{}Completed", to_pascal(name))],
        mutates_state: true,
        vessel:        Some(vessel.to_string()),
    }
}

fn to_pascal(s: &str) -> String {
    s.split('_').map(|w| {
        let mut c = w.chars();
        match c.next() {
            None    => String::new(),
            Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        }
    }).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use oso_ir::validator::validate;

    #[test]
    fn all_six_templates_are_valid() {
        for class in [
            ContractClass::Financial,
            ContractClass::Agent,
            ContractClass::Work,
            ContractClass::Device,
            ContractClass::Evidence,
            ContractClass::Governance,
        ] {
            let ir = canonical_ir(class, format!("Test{:?}", class));
            let result = validate(&ir);
            assert!(result.valid, "class {:?} invalid: {:?}", class, result.errors);
        }
    }

    #[test]
    fn work_template_has_capability() {
        let ir = canonical_ir(ContractClass::Work, "GpuMarket");
        assert!(!ir.capabilities.is_empty());
        assert_eq!(ir.capabilities[0].name, "GPU_COMPUTE");
    }

    #[test]
    fn governance_template_has_witness_quorum() {
        let ir = canonical_ir(ContractClass::Governance, "CouncilDao");
        let pol = ir.policy.as_ref().unwrap();
        assert_eq!(pol.witness_quorum, 7);
        assert!(pol.requires_council);
    }

    #[test]
    fn financial_template_has_pool_asset() {
        let ir = canonical_ir(ContractClass::Financial, "AsePool");
        assert_eq!(ir.assets[0].name, "Pool");
    }

    #[test]
    fn evidence_template_has_receipt_hash_field() {
        let ir = canonical_ir(ContractClass::Evidence, "ZangbetoProof");
        assert!(ir.assets[0].fields.iter().any(|f| f.name == "receipt_hash"));
    }
}
