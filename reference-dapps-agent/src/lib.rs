/// Phase 27 — Ọ̀ṢỌ́ reference dApp agent backends.
///
/// Each reference dApp has a canonical `.oso` source string and a
/// `DappBackend::compile_and_validate()` entry point that runs:
///   1. Parse      — oso-compiler::dapp::compile_dapp
///   2. Validate   — oso_ir::validator::validate
///
/// The full deployment pipeline (lint → authorize → simulate → deploy)
/// lives in `oso-deployer::dapp_pipeline` and is exercised in integration
/// tests there. This crate focuses on the contract semantics.
use serde::{Deserialize, Serialize};

pub mod contracts;

use oso_ir::types::OsoIr;
use oso_ir::validator::validate as ir_validate;

// ── Result types ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct CompileResult {
    pub dapp_name: String,
    pub valid:     bool,
    pub errors:    Vec<String>,
    pub warnings:  Vec<String>,
    pub ir:        Option<OsoIr>,
}

// ── DappBackend ───────────────────────────────────────────────────────────────

pub struct DappBackend;

impl DappBackend {
    /// Compile an Ọ̀ṢỌ́ source string and validate the resulting IR.
    pub fn compile_and_validate(source: &str) -> CompileResult {
        let ir = match oso_compiler::dapp::compile_dapp(source) {
            Ok(ir) => ir,
            Err(e) => {
                return CompileResult {
                    dapp_name: "(parse error)".to_string(),
                    valid:     false,
                    errors:    vec![e],
                    warnings:  vec![],
                    ir:        None,
                };
            }
        };

        let name     = ir.name.clone();
        let result   = ir_validate(&ir);
        let errors   = result.errors.iter().map(|e| e.to_string()).collect();
        let warnings = result.warnings.clone();

        CompileResult {
            dapp_name: name,
            valid:     result.valid,
            errors,
            warnings,
            ir: if result.valid { Some(ir) } else { None },
        }
    }

    /// Compile and validate all three reference dApp contracts.
    pub fn compile_all() -> [CompileResult; 3] {
        [
            Self::compile_and_validate(contracts::GPU_MARKETPLACE_OSO),
            Self::compile_and_validate(contracts::AGENT_EMPLOYMENT_OSO),
            Self::compile_and_validate(contracts::SIM_MARKETPLACE_OSO),
        ]
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use oso_ir::ContractClass;

    // ── dApp 1: GPU Marketplace ───────────────────────────────────────────────

    #[test]
    fn gpu_marketplace_compiles_and_validates() {
        let r = DappBackend::compile_and_validate(contracts::GPU_MARKETPLACE_OSO);
        assert!(r.valid, "errors: {:?}", r.errors);
        assert_eq!(r.dapp_name, "GpuMarket");
    }

    #[test]
    fn gpu_marketplace_is_work_class() {
        let r = DappBackend::compile_and_validate(contracts::GPU_MARKETPLACE_OSO);
        let ir = r.ir.unwrap();
        assert_eq!(ir.contract_class, ContractClass::Work);
    }

    #[test]
    fn gpu_marketplace_has_gpu_compute_capability() {
        let r = DappBackend::compile_and_validate(contracts::GPU_MARKETPLACE_OSO);
        let ir = r.ir.unwrap();
        assert!(ir.capabilities.iter().any(|c| c.name == "GPU_COMPUTE" && c.minimum_tier == 2));
    }

    #[test]
    fn gpu_marketplace_has_evidence_required() {
        let r = DappBackend::compile_and_validate(contracts::GPU_MARKETPLACE_OSO);
        let ir = r.ir.unwrap();
        let ev = ir.evidence.as_ref().expect("evidence block missing");
        assert!(ev.required);
        assert_eq!(ev.evidence_type, "ComputeReceipt");
    }

    #[test]
    fn gpu_marketplace_has_ase_settlement() {
        let r = DappBackend::compile_and_validate(contracts::GPU_MARKETPLACE_OSO);
        let ir = r.ir.unwrap();
        let s = ir.settlement.as_ref().expect("settlement block missing");
        assert_eq!(s.currency, "ASE");
        assert!((s.treasury_pct - 3.69).abs() < 0.001);
    }

    // ── dApp 2: Agent Employment ──────────────────────────────────────────────

    #[test]
    fn agent_employment_compiles_and_validates() {
        let r = DappBackend::compile_and_validate(contracts::AGENT_EMPLOYMENT_OSO);
        assert!(r.valid, "errors: {:?}", r.errors);
        assert_eq!(r.dapp_name, "AgentHiring");
    }

    #[test]
    fn agent_employment_is_agent_class() {
        let r = DappBackend::compile_and_validate(contracts::AGENT_EMPLOYMENT_OSO);
        let ir = r.ir.unwrap();
        assert_eq!(ir.contract_class, ContractClass::Agent);
    }

    #[test]
    fn agent_employment_has_delegation_capability() {
        let r = DappBackend::compile_and_validate(contracts::AGENT_EMPLOYMENT_OSO);
        let ir = r.ir.unwrap();
        assert!(ir.capabilities.iter().any(|c| c.name == "AGENT_DELEGATION" && c.minimum_tier == 1));
    }

    #[test]
    fn agent_employment_has_four_actions() {
        let r = DappBackend::compile_and_validate(contracts::AGENT_EMPLOYMENT_OSO);
        let ir = r.ir.unwrap();
        assert_eq!(ir.actions.len(), 4);
        let names: Vec<&str> = ir.actions.iter().map(|a| a.name.as_str()).collect();
        assert!(names.contains(&"hire"));
        assert!(names.contains(&"delegate"));
        assert!(names.contains(&"complete"));
        assert!(names.contains(&"terminate"));
    }

    // ── dApp 3: Simulation Marketplace ───────────────────────────────────────

    #[test]
    fn sim_marketplace_compiles_and_validates() {
        let r = DappBackend::compile_and_validate(contracts::SIM_MARKETPLACE_OSO);
        assert!(r.valid, "errors: {:?}", r.errors);
        assert_eq!(r.dapp_name, "SimMarket");
    }

    #[test]
    fn sim_marketplace_is_work_class() {
        let r = DappBackend::compile_and_validate(contracts::SIM_MARKETPLACE_OSO);
        let ir = r.ir.unwrap();
        assert_eq!(ir.contract_class, ContractClass::Work);
    }

    #[test]
    fn sim_marketplace_has_scarab_capability() {
        let r = DappBackend::compile_and_validate(contracts::SIM_MARKETPLACE_OSO);
        let ir = r.ir.unwrap();
        assert!(ir.capabilities.iter().any(|c| c.name == "SCARAB_SIM" && c.minimum_tier == 2));
    }

    #[test]
    fn sim_marketplace_evidence_is_proof_of_simulation() {
        let r = DappBackend::compile_and_validate(contracts::SIM_MARKETPLACE_OSO);
        let ir = r.ir.unwrap();
        let ev = ir.evidence.as_ref().expect("evidence block missing");
        assert!(ev.required);
        assert_eq!(ev.evidence_type, "ProofOfSimulation");
    }

    #[test]
    fn sim_marketplace_has_verify_proof_action() {
        let r = DappBackend::compile_and_validate(contracts::SIM_MARKETPLACE_OSO);
        let ir = r.ir.unwrap();
        assert!(ir.actions.iter().any(|a| a.name == "verify_proof"));
    }

    // ── compile_all ───────────────────────────────────────────────────────────

    #[test]
    fn all_three_reference_dapps_compile_clean() {
        let results = DappBackend::compile_all();
        for r in &results {
            assert!(r.valid, "dApp '{}' failed: {:?}", r.dapp_name, r.errors);
        }
        assert_eq!(results[0].dapp_name, "GpuMarket");
        assert_eq!(results[1].dapp_name, "AgentHiring");
        assert_eq!(results[2].dapp_name, "SimMarket");
    }
}
