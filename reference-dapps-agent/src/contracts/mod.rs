/// Phase 27 — Canonical Ọ̀ṢỌ́ contract sources for the three reference dApps.
///
/// These are the authoritative `.oso` source strings. They serve as:
///   1. Inputs to `DappBackend::compile_and_validate()`
///   2. The spec that frontend UIs are built against
///   3. Integration test fixtures for the full pipeline

/// dApp 1 — GPU Compute Marketplace (Phase 27.1)
///
/// Developer → GPU dApp → ComputeJob → GPU Agent → Compute → ComputeReceipt → ASE settlement
pub const GPU_MARKETPLACE_OSO: &str = r#"
dapp GpuMarket {
    class work
    asset ComputeJob {
        job_id: string
        requester: string
        gpu_type: string
        vram_gb: u64
        budget: u64
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
    action accept_job(job) {
        require capability.GPU_COMPUTE
        vessel Act
        emit JobAccepted
    }
    action submit_result(job) {
        require capability.GPU_COMPUTE
        vessel Compute
        emit ResultSubmitted
    }
    action settle(job) {
        require evidence.accepted
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
}
"#;

/// dApp 2 — Agent Employment (Phase 27.2)
///
/// Principal → AgentHiring Contract → delegation → work → Zàngbétò receipt → ASE payment
pub const AGENT_EMPLOYMENT_OSO: &str = r#"
dapp AgentHiring {
    class agent
    asset AgentContract {
        contract_id: string
        employer_id: string
        agent_id:    string
        task_spec:   string
        budget:      u64
    }
    capability AGENT_DELEGATION {
        tier: T1
        required: true
    }
    action hire(contract) {
        require principal.authorized
        vessel Create
        emit AgentHired
    }
    action delegate(contract) {
        require capability.AGENT_DELEGATION
        vessel Act
        emit WorkDelegated
    }
    action complete(contract) {
        require evidence.accepted
        vessel Transfer
        emit ContractCompleted
    }
    action terminate(contract) {
        require principal.owner
        vessel Destroy
        emit ContractTerminated
    }
    evidence {
        required:      true
        type:          WorkReceipt
        minimum_count: 1
    }
    settlement {
        currency:     ASE
        fee_routing:  "6-pool"
        treasury_pct: 3.69
    }
}
"#;

/// dApp 3 — Simulation Marketplace (Phase 27.3)
///
/// Researcher → SimJob → ScarabSwarm capability → OSOVM execution →
/// ProofOfSimulation → Àṣẹ reward
pub const SIM_MARKETPLACE_OSO: &str = r#"
dapp SimMarket {
    class work
    asset SimJob {
        job_id:           string
        researcher_id:    string
        veil:             string
        trajectory_count: u64
        budget:           u64
    }
    capability SCARAB_SIM {
        tier: T2
        required: true
    }
    action submit_sim(job) {
        require principal.authorized
        vessel Create
        emit SimSubmitted
    }
    action run_sim(job) {
        require capability.SCARAB_SIM
        vessel Compute
        emit SimRunning
    }
    action verify_proof(job) {
        require evidence.accepted
        vessel Prove
        emit ProofVerified
    }
    action settle(job) {
        require evidence.accepted
        vessel Transfer
        emit SimSettled
    }
    evidence {
        required:      true
        type:          ProofOfSimulation
        minimum_count: 1
    }
    settlement {
        currency:     ASE
        fee_routing:  "6-pool"
        treasury_pct: 3.69
    }
}
"#;
