/// Phase 22 — Ọ̀ṢỌ́ dApp language parser.
///
/// Compiles `.oso` source (dapp/asset/action/evidence/settlement DSL)
/// → `DappAst` → `OsoIr`.
///
/// Pipeline:
///   .oso source → DappLexer → Vec<DappToken> → DappParser → DappAst → lower → OsoIr

pub mod token;
pub mod lexer;
pub mod ast;
pub mod parser;
pub mod lower;

pub use lexer::DappLexer;
pub use parser::DappParser;
pub use ast::DappAst;

/// Parse and lower a `.oso` source string to an `OsoIr` document.
pub fn compile_dapp(source: &str) -> Result<oso_ir::OsoIr, String> {
    let tokens = DappLexer::new(source).tokenize()?;
    let ast    = DappParser::new(tokens).parse()?;
    lower::lower(ast)
}

#[cfg(test)]
mod tests {
    use super::*;
    use oso_ir::validator::validate;
    use oso_ir::types::ContractClass;

    const FINANCIAL: &str = r#"
dapp AsePool {
    class financial

    asset Pool {
        pool_id:  string
        balance:  u64
        owner:    address
    }

    capability SIGN_TX

    action deposit(amount) {
        require principal.authorized
        vessel Store
        emit PoolDeposited
    }

    action withdraw(amount) {
        require principal.owner
        vessel Transfer
        emit PoolWithdrawn
    }

    evidence {
        required:      true
        type:          ZangbetoReceipt
        minimum_count: 1
    }

    settlement {
        currency:     ASE
        fee_routing:  "6-pool"
        treasury_pct: 3.69
    }
}
"#;

    const WORK: &str = r#"
dapp GpuComputeMarketplace {
    class work

    asset ComputeJob {
        job_id:  string
        creator: address
        budget:  u64
    }

    asset ComputeResult {
        job_id:      string
        output_hash: hash
    }

    capability GPU_COMPUTE {
        tier: T2
        required: true
    }

    action submit_job(job) {
        require principal.authorized
        vessel Create
        emit JobSubmitted
    }

    action submit_result(job, result) {
        require capability(GPU_COMPUTE)
        require proof.valid
        vessel Compute
        emit ResultSubmitted
    }

    action settle(job) {
        require evidence.accepted
        pay provider from budget
        vessel Transfer
        emit JobSettled
    }

    evidence {
        required:      true
        type:          ComputeReceipt
        minimum_count: 1
    }

    settlement {
        currency:     ASE
        fee_routing:  "6-pool"
        treasury_pct: 3.69
    }

    witness {
        quorum: 2
    }
}
"#;

    const GOVERNANCE: &str = r#"
dapp CouncilDao {
    class governance

    asset Proposal {
        proposal_id: string
        proposer:    address
    }

    asset Vote {
        proposal_id: string
        voter:       address
    }

    capability PROPOSAL_CREATE {
        tier: T3
    }

    action create_proposal(proposal) {
        require capability(PROPOSAL_CREATE)
        vessel Govern
        emit ProposalCreated
    }

    action vote(proposal, in_favor) {
        require principal.authorized
        vessel Validate
        emit VoteCast
    }

    action enact(proposal) {
        require evidence.accepted
        vessel Execute
        emit ProposalEnacted
    }

    evidence {
        required:      true
        type:          WitnessBundle
        minimum_count: 7
    }

    settlement {
        currency:     ASE
        fee_routing:  "dao-pool"
        treasury_pct: 3.69
    }

    witness {
        quorum:           7
        requires_council: true
    }
}
"#;

    const DEVICE: &str = r#"
dapp DeviceRegistry {
    class device

    asset DeviceRecord {
        device_id: string
        agent_id:  string
    }

    capability DEVICE_INHABIT {
        tier: T1
    }

    action bind(device_id, agent_id) {
        require capability(DEVICE_INHABIT)
        vessel Attest
        emit DeviceBound
    }

    evidence {
        required:      true
        type:          DeviceAttestation
        minimum_count: 1
    }

    settlement {
        currency:     ASE
        fee_routing:  "direct"
        treasury_pct: 3.69
    }
}
"#;

    #[test]
    fn parse_financial_contract() {
        let ir = compile_dapp(FINANCIAL).expect("should parse");
        assert_eq!(ir.contract_class, ContractClass::Financial);
        assert_eq!(ir.name, "AsePool");
        assert_eq!(ir.assets.len(), 1);
        assert_eq!(ir.actions.len(), 2);
        let result = validate(&ir);
        assert!(result.valid, "errors: {:?}", result.errors);
    }

    #[test]
    fn parse_work_contract() {
        let ir = compile_dapp(WORK).expect("should parse");
        assert_eq!(ir.contract_class, ContractClass::Work);
        assert_eq!(ir.name, "GpuComputeMarketplace");
        assert_eq!(ir.assets.len(), 2);
        assert_eq!(ir.actions.len(), 3);
        assert!(ir.evidence.as_ref().map(|e| e.required).unwrap_or(false));
        let result = validate(&ir);
        assert!(result.valid, "errors: {:?}", result.errors);
    }

    #[test]
    fn parse_governance_contract() {
        let ir = compile_dapp(GOVERNANCE).expect("should parse");
        assert_eq!(ir.contract_class, ContractClass::Governance);
        assert_eq!(ir.name, "CouncilDao");
        assert_eq!(ir.actions.len(), 3);
        let pol = ir.policy.as_ref().expect("witness policy");
        assert_eq!(pol.witness_quorum, 7);
        assert!(pol.requires_council);
        let result = validate(&ir);
        assert!(result.valid, "errors: {:?}", result.errors);
    }

    #[test]
    fn parse_device_contract() {
        let ir = compile_dapp(DEVICE).expect("should parse");
        assert_eq!(ir.contract_class, ContractClass::Device);
        let cap = &ir.capabilities[0];
        assert_eq!(cap.name, "DEVICE_INHABIT");
        assert_eq!(cap.minimum_tier, 1);
        let result = validate(&ir);
        assert!(result.valid, "errors: {:?}", result.errors);
    }

    #[test]
    fn vessel_mapped_to_action() {
        let ir = compile_dapp(WORK).expect("should parse");
        let submit = ir.actions.iter().find(|a| a.name == "submit_job").unwrap();
        assert_eq!(submit.vessel.as_deref(), Some("Create"));
        let settle = ir.actions.iter().find(|a| a.name == "settle").unwrap();
        assert_eq!(settle.vessel.as_deref(), Some("Transfer"));
    }

    #[test]
    fn settlement_fields_parsed() {
        let ir = compile_dapp(FINANCIAL).expect("should parse");
        let s = ir.settlement.as_ref().expect("settlement");
        assert_eq!(s.currency, "ASE");
        assert_eq!(s.fee_routing, "6-pool");
        assert!((s.treasury_pct - 3.69).abs() < 0.001);
    }
}
