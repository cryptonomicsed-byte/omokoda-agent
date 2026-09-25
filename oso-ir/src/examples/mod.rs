/// Phase 21.3 — 6 example Ọ̀ṢỌ́-IR documents, one per contract class.
///
/// These serve as reference implementations and test fixtures.
/// Each is a valid OsoIr document that passes validation.

use crate::types::*;
use std::collections::BTreeMap;

/// FinancialContract example: AsePool payment contract.
pub fn financial_ase_pool() -> OsoIr {
    OsoIr {
        ir_version:     "1.0".to_string(),
        contract_class: ContractClass::Financial,
        name:           "AsePool".to_string(),
        version:        "1.0.0".to_string(),
        assets: vec![
            AssetDef {
                name:        "AseBalance".to_string(),
                fields:      vec![
                    FieldDef { name: "holder".to_string(),  field_type: "AgentId".to_string(), required: true },
                    FieldDef { name: "amount".to_string(),  field_type: "u64".to_string(),     required: true },
                    FieldDef { name: "pool_id".to_string(), field_type: "u8".to_string(),      required: true },
                ],
                transferable: true,
                divisible:    true,
            },
        ],
        capabilities: vec![
            CapabilityRef { name: "TRANSFER_ASE".to_string(),   minimum_tier: 0, required: true  },
            CapabilityRef { name: "MINT_EMISSION".to_string(),  minimum_tier: 5, required: false },
        ],
        actions: vec![
            ActionDef {
                name:          "deposit".to_string(),
                requires:      vec![PolicyExpr::Capability { name: "TRANSFER_ASE".to_string() }],
                emits:         vec!["AseDeposited".to_string()],
                mutates_state: true,
                vessel:        Some("Transfer".to_string()),
            },
            ActionDef {
                name:          "withdraw".to_string(),
                requires:      vec![
                    PolicyExpr::Principal { role: "owner".to_string() },
                    PolicyExpr::Numeric {
                        field: "balance".to_string(),
                        op:    ">=".to_string(),
                        value: serde_json::json!(1),
                    },
                ],
                emits:         vec!["AseWithdrawn".to_string()],
                mutates_state: true,
                vessel:        Some("Transfer".to_string()),
            },
        ],
        evidence: None,
        settlement: Some(SettlementPolicy {
            currency:     "ASE".to_string(),
            fee_routing:  "8-pool".to_string(),
            treasury_pct: 3.69,
        }),
        policy: None,
        backend_targets: vec![BackendTarget::Native, BackendTarget::Move],
        metadata: {
            let mut m = BTreeMap::new();
            m.insert("description".to_string(), serde_json::json!("8-pool Àṣẹ emission and distribution contract"));
            m
        },
    }
}

/// AgentContract example: AgentRegistry.
pub fn agent_registry() -> OsoIr {
    OsoIr {
        ir_version:     "1.0".to_string(),
        contract_class: ContractClass::Agent,
        name:           "AgentRegistry".to_string(),
        version:        "1.0.0".to_string(),
        assets: vec![
            AssetDef {
                name:        "AgentRecord".to_string(),
                fields:      vec![
                    FieldDef { name: "agent_id".to_string(),   field_type: "AgentId".to_string(),   required: true  },
                    FieldDef { name: "npub".to_string(),       field_type: "Pubkey".to_string(),    required: true  },
                    FieldDef { name: "bipon39".to_string(),    field_type: "String".to_string(),    required: true  },
                    FieldDef { name: "tier".to_string(),       field_type: "u8".to_string(),        required: true  },
                    FieldDef { name: "reputation".to_string(), field_type: "u32".to_string(),       required: false },
                ],
                transferable: false,
                divisible:    false,
            },
        ],
        capabilities: vec![
            CapabilityRef { name: "REGISTER_AGENT".to_string(), minimum_tier: 0, required: true },
        ],
        actions: vec![
            ActionDef {
                name:          "register".to_string(),
                requires:      vec![PolicyExpr::Proof { proof_type: "BirthReceipt".to_string() }],
                emits:         vec!["AgentRegistered".to_string()],
                mutates_state: true,
                vessel:        Some("Create".to_string()),
            },
            ActionDef {
                name:          "update_reputation".to_string(),
                requires:      vec![
                    PolicyExpr::Principal { role: "oracle".to_string() },
                    PolicyExpr::Proof { proof_type: "ZangbetoReceipt".to_string() },
                ],
                emits:         vec!["ReputationUpdated".to_string()],
                mutates_state: true,
                vessel:        Some("Attest".to_string()),
            },
        ],
        evidence: Some(EvidencePolicy {
            required:      true,
            evidence_type: "BirthReceipt".to_string(),
            minimum_count: 1,
        }),
        settlement: None,
        policy: Some(WitnessPolicy {
            witness_quorum:    0,
            quality_threshold: 0,
            upgradeable:       true,
            requires_council:  false,
        }),
        backend_targets: vec![BackendTarget::Native],
        metadata: BTreeMap::new(),
    }
}

/// WorkContract example: GPUComputeJob.
pub fn work_gpu_compute_job() -> OsoIr {
    OsoIr {
        ir_version:     "1.0".to_string(),
        contract_class: ContractClass::Work,
        name:           "GPUComputeJob".to_string(),
        version:        "1.0.0".to_string(),
        assets: vec![
            AssetDef {
                name:        "ComputeJob".to_string(),
                fields:      vec![
                    FieldDef { name: "requester".to_string(),  field_type: "AgentId".to_string(), required: true  },
                    FieldDef { name: "provider".to_string(),   field_type: "AgentId".to_string(), required: false },
                    FieldDef { name: "gpu_memory_gb".to_string(), field_type: "u32".to_string(), required: true  },
                    FieldDef { name: "budget_ase".to_string(), field_type: "u64".to_string(),    required: true  },
                    FieldDef { name: "status".to_string(),     field_type: "JobStatus".to_string(), required: true },
                ],
                transferable: false,
                divisible:    false,
            },
        ],
        capabilities: vec![
            CapabilityRef { name: "GPU_COMPUTE".to_string(), minimum_tier: 2, required: true  },
            CapabilityRef { name: "SUBMIT_JOB".to_string(),  minimum_tier: 1, required: true  },
        ],
        actions: vec![
            ActionDef {
                name:          "submit_job".to_string(),
                requires:      vec![
                    PolicyExpr::Capability { name: "SUBMIT_JOB".to_string() },
                    PolicyExpr::Numeric { field: "budget_ase".to_string(), op: ">=".to_string(), value: serde_json::json!(100) },
                ],
                emits:         vec!["JobSubmitted".to_string()],
                mutates_state: true,
                vessel:        Some("Act".to_string()),
            },
            ActionDef {
                name:          "accept_job".to_string(),
                requires:      vec![PolicyExpr::Capability { name: "GPU_COMPUTE".to_string() }],
                emits:         vec!["JobAccepted".to_string()],
                mutates_state: true,
                vessel:        Some("Act".to_string()),
            },
            ActionDef {
                name:          "submit_result".to_string(),
                requires:      vec![
                    PolicyExpr::Principal { role: "provider".to_string() },
                    PolicyExpr::Proof { proof_type: "ComputeReceipt".to_string() },
                ],
                emits:         vec!["ResultSubmitted".to_string()],
                mutates_state: true,
                vessel:        Some("Prove".to_string()),
            },
            ActionDef {
                name:          "settle".to_string(),
                requires:      vec![
                    PolicyExpr::Proof { proof_type: "ComputeReceipt".to_string() },
                    PolicyExpr::Proof { proof_type: "ZangbetoReceipt".to_string() },
                ],
                emits:         vec!["JobSettled".to_string(), "AseTransferred".to_string()],
                mutates_state: true,
                vessel:        Some("Execute".to_string()),
            },
        ],
        evidence: Some(EvidencePolicy {
            required:      true,
            evidence_type: "ComputeReceipt".to_string(),
            minimum_count: 1,
        }),
        settlement: Some(SettlementPolicy {
            currency:     "ASE".to_string(),
            fee_routing:  "6-pool".to_string(),
            treasury_pct: 3.69,
        }),
        policy: Some(WitnessPolicy {
            witness_quorum:    2,
            quality_threshold: 80,
            upgradeable:       false,
            requires_council:  false,
        }),
        backend_targets: vec![BackendTarget::Native, BackendTarget::Wasm],
        metadata: BTreeMap::new(),
    }
}

/// DeviceContract example: M5StackDeviceRegistry.
pub fn device_registry() -> OsoIr {
    OsoIr {
        ir_version:     "1.0".to_string(),
        contract_class: ContractClass::Device,
        name:           "M5StackDeviceRegistry".to_string(),
        version:        "1.0.0".to_string(),
        assets: vec![
            AssetDef {
                name:        "DeviceRecord".to_string(),
                fields:      vec![
                    FieldDef { name: "device_id".to_string(),   field_type: "DeviceId".to_string(),  required: true },
                    FieldDef { name: "owner_agent".to_string(), field_type: "AgentId".to_string(),   required: true },
                    FieldDef { name: "device_kind".to_string(), field_type: "String".to_string(),    required: true },
                    FieldDef { name: "pubkey".to_string(),      field_type: "Pubkey".to_string(),    required: true },
                    FieldDef { name: "firmware_hash".to_string(), field_type: "Hash32".to_string(),  required: false },
                ],
                transferable: true,
                divisible:    false,
            },
        ],
        capabilities: vec![
            CapabilityRef { name: "REGISTER_DEVICE".to_string(), minimum_tier: 1, required: true },
            CapabilityRef { name: "ATTEST_FIRMWARE".to_string(), minimum_tier: 3, required: false },
        ],
        actions: vec![
            ActionDef {
                name:          "register_device".to_string(),
                requires:      vec![
                    PolicyExpr::Principal { role: "device_owner".to_string() },
                    PolicyExpr::Proof { proof_type: "VcpReceipt".to_string() },
                ],
                emits:         vec!["DeviceRegistered".to_string()],
                mutates_state: true,
                vessel:        Some("Create".to_string()),
            },
        ],
        evidence: Some(EvidencePolicy {
            required: true,
            evidence_type: "VcpReceipt".to_string(),
            minimum_count: 1,
        }),
        settlement: None,
        policy: None,
        backend_targets: vec![BackendTarget::Native],
        metadata: BTreeMap::new(),
    }
}

/// EvidenceContract example: ZangbetoReceiptContract.
pub fn evidence_zangbeto_receipt() -> OsoIr {
    OsoIr {
        ir_version:     "1.0".to_string(),
        contract_class: ContractClass::Evidence,
        name:           "ZangbetoReceiptContract".to_string(),
        version:        "1.0.0".to_string(),
        assets: vec![
            AssetDef {
                name:        "ReceiptRecord".to_string(),
                fields:      vec![
                    FieldDef { name: "receipt_id".to_string(),   field_type: "Hash32".to_string(),  required: true },
                    FieldDef { name: "agent_id".to_string(),     field_type: "AgentId".to_string(), required: true },
                    FieldDef { name: "action_kind".to_string(),  field_type: "String".to_string(),  required: true },
                    FieldDef { name: "receipt_hash".to_string(), field_type: "Hash32".to_string(),  required: true },
                    FieldDef { name: "verified".to_string(),     field_type: "bool".to_string(),    required: true },
                ],
                transferable: false,
                divisible:    false,
            },
        ],
        capabilities: vec![
            CapabilityRef { name: "SUBMIT_RECEIPT".to_string(), minimum_tier: 0, required: true },
            CapabilityRef { name: "VERIFY_RECEIPT".to_string(), minimum_tier: 4, required: true },
        ],
        actions: vec![
            ActionDef {
                name:          "submit_receipt".to_string(),
                requires:      vec![PolicyExpr::Capability { name: "SUBMIT_RECEIPT".to_string() }],
                emits:         vec!["ReceiptSubmitted".to_string()],
                mutates_state: true,
                vessel:        Some("Attest".to_string()),
            },
            ActionDef {
                name:          "verify_receipt".to_string(),
                requires:      vec![
                    PolicyExpr::Capability { name: "VERIFY_RECEIPT".to_string() },
                    PolicyExpr::Principal { role: "zangbeto_oracle".to_string() },
                ],
                emits:         vec!["ReceiptVerified".to_string()],
                mutates_state: true,
                vessel:        Some("Validate".to_string()),
            },
        ],
        evidence: Some(EvidencePolicy {
            required:      true,
            evidence_type: "ZangbetoAttestation".to_string(),
            minimum_count: 1,
        }),
        settlement: None,
        policy: Some(WitnessPolicy {
            witness_quorum:    3,
            quality_threshold: 100,
            upgradeable:       false,
            requires_council:  false,
        }),
        backend_targets: vec![BackendTarget::Native],
        metadata: BTreeMap::new(),
    }
}

/// GovernanceContract example: CouncilDAO.
pub fn governance_council_dao() -> OsoIr {
    OsoIr {
        ir_version:     "1.0".to_string(),
        contract_class: ContractClass::Governance,
        name:           "CouncilDAO".to_string(),
        version:        "1.0.0".to_string(),
        assets: vec![
            AssetDef {
                name:        "Proposal".to_string(),
                fields:      vec![
                    FieldDef { name: "proposal_id".to_string(), field_type: "Hash32".to_string(),  required: true },
                    FieldDef { name: "author".to_string(),      field_type: "AgentId".to_string(), required: true },
                    FieldDef { name: "title".to_string(),       field_type: "String".to_string(),  required: true },
                    FieldDef { name: "status".to_string(),      field_type: "ProposalStatus".to_string(), required: true },
                    FieldDef { name: "votes_for".to_string(),   field_type: "u32".to_string(),     required: true },
                    FieldDef { name: "votes_against".to_string(), field_type: "u32".to_string(),   required: true },
                ],
                transferable: false,
                divisible:    false,
            },
        ],
        capabilities: vec![
            CapabilityRef { name: "COUNCIL_VOTE".to_string(),   minimum_tier: 4, required: true  },
            CapabilityRef { name: "SUBMIT_PROPOSAL".to_string(), minimum_tier: 2, required: true },
            CapabilityRef { name: "BINO_VETO".to_string(),      minimum_tier: 5, required: false },
        ],
        actions: vec![
            ActionDef {
                name:          "submit_proposal".to_string(),
                requires:      vec![PolicyExpr::Capability { name: "SUBMIT_PROPOSAL".to_string() }],
                emits:         vec!["ProposalSubmitted".to_string()],
                mutates_state: true,
                vessel:        Some("Govern".to_string()),
            },
            ActionDef {
                name:          "vote".to_string(),
                requires:      vec![PolicyExpr::Capability { name: "COUNCIL_VOTE".to_string() }],
                emits:         vec!["VoteCast".to_string()],
                mutates_state: true,
                vessel:        Some("Govern".to_string()),
            },
            ActionDef {
                name:          "execute_proposal".to_string(),
                requires:      vec![
                    PolicyExpr::Numeric { field: "votes_for".to_string(), op: ">".to_string(), value: serde_json::json!(6) },
                    PolicyExpr::Proof { proof_type: "QuorumReceipt".to_string() },
                ],
                emits:         vec!["ProposalExecuted".to_string()],
                mutates_state: true,
                vessel:        Some("Govern".to_string()),
            },
        ],
        evidence: Some(EvidencePolicy {
            required:      true,
            evidence_type: "QuorumReceipt".to_string(),
            minimum_count: 1,
        }),
        settlement: None,
        policy: Some(WitnessPolicy {
            witness_quorum:    7,
            quality_threshold: 0,
            upgradeable:       false,
            requires_council:  true,
        }),
        backend_targets: vec![BackendTarget::Native],
        metadata: {
            let mut m = BTreeMap::new();
            m.insert("description".to_string(), serde_json::json!("Council of 12 — staggered quarterly elections, 24-sector governance"));
            m
        },
    }
}
