/// Phase 21 — Ọ̀ṢỌ́-IR: Contract Intermediate Representation.
///
/// This is the canonical intermediate representation between the Ọ̀ṢỌ́ dApp
/// language and its compilation targets (Move / WASM / Native ABCI).
///
/// Level in the stack:
///   Ọ̀ṢỌ́ source (.oso files)  →  Ọ̀ṢỌ́-IR (this crate)  →  Move / WASM / Native
///
/// Distinct from the OSOVM opcode IR in `oso-parser` (which is VM execution).
/// This IR operates at *contract semantics* level: what the contract IS,
/// not how it executes instruction-by-instruction.
///
/// Phase 21 deliverables:
///   21.1 — OsoIr struct hierarchy (this file: types + schema)
///   21.2 — Rust validator (ir_validator.rs)
///   21.3 — 6 example IR documents (examples/ directory)
///   21.4 — Spec locked in sovereign-eco-blueprint/specs/oso-ir-spec.md

pub mod types;
pub mod validator;

pub use types::{
    OsoIr, ContractClass, AssetDef, CapabilityRef, ActionDef,
    EvidencePolicy, SettlementPolicy, WitnessPolicy, PolicyExpr,
    BackendTarget, IrError,
};
pub use validator::{validate, ValidationResult, ValidationError};
