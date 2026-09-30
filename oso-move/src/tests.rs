//! Integration tests for oso-move codegen.
//!
//! Tests load example JSON files from src/examples/ and verify
//! the generated Move source contains required structural elements.

use crate::codegen::MoveCodegen;
use crate::ir::OsoIR;

fn load_example(name: &str) -> OsoIR {
    let path = format!("{}/src/examples/{}.json", env!("CARGO_MANIFEST_DIR"), name);
    let bytes = std::fs::read(&path).expect(&format!("cannot read example: {}", path));
    serde_json::from_slice(&bytes).expect(&format!("invalid JSON in {}", path))
}

// ---------------------------------------------------------------------------
// AsePool (financial contract)
// ---------------------------------------------------------------------------

#[test]
fn ase_pool_compiles_to_move() {
    let ir = load_example("ase_pool");
    let result = MoveCodegen::compile(&ir);
    assert!(result.is_ok(), "compile failed: {:?}", result.err());
    let src = result.unwrap();
    assert!(src.contains("module"), "missing 'module' keyword");
    assert!(src.contains("public entry fun"), "missing entry functions");
    assert!(
        src.contains("AsePool") || src.contains("ase_pool"),
        "contract name missing"
    );
}

#[test]
fn ase_pool_has_struct_pool() {
    let ir = load_example("ase_pool");
    let src = MoveCodegen::compile(&ir).unwrap();
    assert!(src.contains("struct Pool"), "missing Pool struct");
}

#[test]
fn ase_pool_has_settle_fn() {
    let ir = load_example("ase_pool");
    let src = MoveCodegen::compile(&ir).unwrap();
    assert!(
        src.contains("public entry fun settle"),
        "missing settle function"
    );
}

#[test]
fn ase_pool_has_esu_tithe_comment() {
    let ir = load_example("ase_pool");
    let src = MoveCodegen::compile(&ir).unwrap();
    assert!(
        src.contains("tithe") || src.contains("0.0369"),
        "missing tithe comment"
    );
}

// ---------------------------------------------------------------------------
// JobContract (work contract)
// ---------------------------------------------------------------------------

#[test]
fn job_contract_compiles_to_move() {
    let ir = load_example("job_contract");
    let result = MoveCodegen::compile(&ir);
    assert!(result.is_ok(), "compile failed: {:?}", result.err());
    let src = result.unwrap();
    assert!(src.contains("module"), "missing 'module' keyword");
    assert!(src.contains("public entry fun"), "missing entry functions");
}

#[test]
fn job_contract_has_capability_comment() {
    let ir = load_example("job_contract");
    let src = MoveCodegen::compile(&ir).unwrap();
    assert!(
        src.contains("GPU_COMPUTE") || src.contains("capability"),
        "missing capability comment"
    );
}

#[test]
fn job_contract_has_lifecycle_transitions() {
    let ir = load_example("job_contract");
    let src = MoveCodegen::compile(&ir).unwrap();
    // Work contracts go CREATED→ASSIGNED→…→SETTLED
    assert!(
        src.contains("transition_created_to_assigned"),
        "missing first transition"
    );
    assert!(
        src.contains("transition_executed_to_verified"),
        "missing evidence transition"
    );
    assert!(
        src.contains("transition_verified_to_settled"),
        "missing settlement transition"
    );
}

// ---------------------------------------------------------------------------
// AgentRegistry (agent contract)
// ---------------------------------------------------------------------------

#[test]
fn agent_registry_compiles_to_move() {
    let ir = load_example("agent_registry");
    let result = MoveCodegen::compile(&ir);
    assert!(result.is_ok(), "compile failed: {:?}", result.err());
    let src = result.unwrap();
    assert!(src.contains("module"), "missing 'module' keyword");
    assert!(src.contains("public entry fun"), "missing entry functions");
}

#[test]
fn agent_registry_has_agent_record_struct() {
    let ir = load_example("agent_registry");
    let src = MoveCodegen::compile(&ir).unwrap();
    assert!(
        src.contains("struct AgentRecord"),
        "missing AgentRecord struct"
    );
}

#[test]
fn agent_registry_has_registered_state_constant() {
    let ir = load_example("agent_registry");
    let src = MoveCodegen::compile(&ir).unwrap();
    assert!(
        src.contains("STATE_REGISTERED"),
        "missing REGISTERED state constant"
    );
}

// ---------------------------------------------------------------------------
// Error cases
// ---------------------------------------------------------------------------

#[test]
fn compile_rejects_bad_version() {
    let mut ir = load_example("ase_pool");
    ir.oso_ir_version = "99.0".to_string();
    let result = MoveCodegen::compile(&ir);
    assert!(matches!(
        result,
        Err(crate::codegen::CompileError::UnknownVersion(_))
    ));
}

#[test]
fn compile_rejects_bad_contract_name() {
    let mut ir = load_example("ase_pool");
    ir.contract_name = "lowercase_name".to_string();
    let result = MoveCodegen::compile(&ir);
    assert!(matches!(
        result,
        Err(crate::codegen::CompileError::InvalidContractName(_))
    ));
}

#[test]
fn compile_rejects_empty_lifecycle() {
    let mut ir = load_example("ase_pool");
    ir.lifecycle = vec![];
    let result = MoveCodegen::compile(&ir);
    assert!(matches!(
        result,
        Err(crate::codegen::CompileError::EmptyLifecycle)
    ));
}
