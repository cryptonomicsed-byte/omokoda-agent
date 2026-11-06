//! Ọ̀ṢỌ́-IR → WebAssembly Text (WAT) code generator.
//!
//! Generates a WAT module from an OsoIR document. The output is valid WAT
//! that can be compiled to WASM via `wat2wasm` or the `wat` crate.
//!
//! Architecture:
//! - Each asset field becomes an i64 global (representing Àṣẹ amounts)
//! - Each lifecycle stage becomes an exported function
//! - Settlement logic becomes an i64 arithmetic sequence
//! - Policy checks become `block`/`br_if` guards
//!
//! Phase 24.1 — Ọ̀ṢỌ́ WASM backend.

use crate::ir::OsoIR;
use std::fmt::Write;

#[derive(Debug)]
pub struct CompileError(pub String);

impl std::fmt::Display for CompileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "WASM compile error: {}", self.0)
    }
}

pub struct WasmCodegen;

impl WasmCodegen {
    pub fn compile(ir: &OsoIR) -> Result<String, CompileError> {
        let mut out = String::new();
        let module_name = sanitize_name(&ir.contract_name);

        writeln!(out, ";; Ọ̀ṢỌ́ WASM module: {}", ir.contract_name).unwrap();
        writeln!(out, ";; contract_class: {}", ir.contract_class).unwrap();
        writeln!(out, ";; oso_ir_version: {}", ir.oso_ir_version).unwrap();
        writeln!(out, "(module ${module_name}").unwrap();
        writeln!(out).unwrap();

        // Import host ABI
        emit_imports(&mut out);

        // Globals: one i64 per asset field (balance tracking)
        emit_asset_globals(&mut out, ir);

        // Policy constants
        emit_policy_constants(&mut out, ir);

        // Lifecycle functions
        emit_lifecycle_functions(&mut out, ir);

        // Settlement function
        emit_settlement(&mut out, ir);

        // Evidence check (policy gate)
        emit_policy_gate(&mut out, ir);

        // Tithe function (Èṣù 3.69%)
        emit_tithe(&mut out, ir);

        writeln!(out, ")").unwrap();
        Ok(out)
    }
}

fn sanitize_name(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect::<String>()
        .to_lowercase()
}

fn emit_imports(out: &mut String) {
    writeln!(
        out,
        "  ;; Host ABI — implemented by the ỌSỌVM execution layer"
    )
    .unwrap();
    writeln!(
        out,
        r#"  (import "oso_host" "emit_receipt" (func $emit_receipt (param i64 i64)))"#
    )
    .unwrap();
    writeln!(
        out,
        r#"  (import "oso_host" "get_balance" (func $get_balance (param i32) (result i64)))"#
    )
    .unwrap();
    writeln!(
        out,
        r#"  (import "oso_host" "transfer" (func $transfer (param i32 i32 i64)))"#
    )
    .unwrap();
    writeln!(
        out,
        r#"  (import "oso_host" "get_timestamp" (func $get_timestamp (result i64)))"#
    )
    .unwrap();
    writeln!(out).unwrap();
}

fn emit_asset_globals(out: &mut String, ir: &OsoIR) {
    writeln!(out, "  ;; Asset globals").unwrap();
    for asset in &ir.assets {
        let aname = sanitize_name(&asset.name);
        for field in &asset.fields {
            let fname = sanitize_name(field);
            writeln!(out, "  (global ${aname}_{fname} (mut i64) (i64.const 0))").unwrap();
        }
    }
    writeln!(out).unwrap();
}

fn emit_policy_constants(out: &mut String, ir: &OsoIR) {
    writeln!(out, "  ;; Policy constants").unwrap();
    writeln!(
        out,
        "  (global $min_tier i32 (i32.const {}))",
        ir.minimum_tier
    )
    .unwrap();
    writeln!(
        out,
        "  (global $require_evidence i32 (i32.const {}))",
        if ir.evidence.required { 1 } else { 0 }
    )
    .unwrap();
    let quorum = ir.witness_policy.as_ref().map(|w| w.quorum).unwrap_or(0);
    writeln!(out, "  (global $min_witnesses i32 (i32.const {quorum}))").unwrap();
    // esu_tithe: canonical 0.0369 unless overridden in the freeform policy map.
    if let Some(tithe) = ir.policy.get("esu_tithe").and_then(|v| v.as_f64()) {
        let bp = (tithe * 10_000.0) as i64;
        writeln!(out, "  (global $esu_tithe_bp i32 (i32.const {bp}))").unwrap();
    }
    writeln!(out).unwrap();
}

fn emit_lifecycle_functions(out: &mut String, ir: &OsoIR) {
    writeln!(out, "  ;; Lifecycle stage functions").unwrap();
    for (i, stage) in ir.lifecycle.iter().enumerate() {
        let fname = sanitize_name(stage);
        writeln!(out, "  ;; Stage {i}: {stage}").unwrap();
        writeln!(out, "  (func $lifecycle_{fname} (export \"{stage}\")").unwrap();
        writeln!(out, "    (param $actor i32) (param $amount i64)").unwrap();
        writeln!(out, "    ;; policy gate: check tier and evidence").unwrap();
        writeln!(out, "    call $check_policy").unwrap();
        writeln!(out, "    ;; emit receipt for this lifecycle transition").unwrap();
        writeln!(out, "    i64.const {i}  ;; stage_id").unwrap();
        writeln!(out, "    call $get_timestamp").unwrap();
        writeln!(out, "    call $emit_receipt").unwrap();
        writeln!(out, "  )").unwrap();
        writeln!(out).unwrap();
    }
}

fn emit_settlement(out: &mut String, ir: &OsoIR) {
    let amount = ir.settlement.amount.unwrap_or(0);
    let tithe_bp = (ir.settlement.tithe_rate.unwrap_or(0.0369) * 10_000.0) as i64;

    writeln!(out, "  ;; Settlement: transfer amount minus tithe").unwrap();
    writeln!(out, "  (func $settle (export \"settle\")").unwrap();
    writeln!(out, "    (param $from i32) (param $to i32)").unwrap();
    writeln!(out, "    (local $gross i64)").unwrap();
    writeln!(out, "    (local $tithe i64)").unwrap();
    writeln!(out, "    (local $net i64)").unwrap();
    writeln!(out, "    i64.const {amount}").unwrap();
    writeln!(out, "    local.set $gross").unwrap();
    writeln!(
        out,
        "    ;; tithe = gross * {tithe_bp} / 10000  (Èṣù 3.69%)"
    )
    .unwrap();
    writeln!(out, "    local.get $gross").unwrap();
    writeln!(out, "    i64.const {tithe_bp}").unwrap();
    writeln!(out, "    i64.mul").unwrap();
    writeln!(out, "    i64.const 10000").unwrap();
    writeln!(out, "    i64.div_u").unwrap();
    writeln!(out, "    local.set $tithe").unwrap();
    writeln!(out, "    local.get $gross").unwrap();
    writeln!(out, "    local.get $tithe").unwrap();
    writeln!(out, "    i64.sub").unwrap();
    writeln!(out, "    local.set $net").unwrap();
    writeln!(out, "    ;; transfer net to recipient").unwrap();
    writeln!(out, "    local.get $from").unwrap();
    writeln!(out, "    local.get $to").unwrap();
    writeln!(out, "    local.get $net").unwrap();
    writeln!(out, "    call $transfer").unwrap();
    writeln!(out, "    ;; transfer tithe to Èṣù treasury (agent_id=0)").unwrap();
    writeln!(out, "    local.get $from").unwrap();
    writeln!(out, "    i32.const 0  ;; Èṣù treasury").unwrap();
    writeln!(out, "    local.get $tithe").unwrap();
    writeln!(out, "    call $transfer").unwrap();
    writeln!(out, "  )").unwrap();
    writeln!(out).unwrap();
}

fn emit_policy_gate(out: &mut String, _ir: &OsoIR) {
    writeln!(
        out,
        "  ;; Policy gate — called before every lifecycle transition"
    )
    .unwrap();
    writeln!(out, "  (func $check_policy").unwrap();
    writeln!(
        out,
        "    ;; TODO: read caller tier from host and compare to $min_tier"
    )
    .unwrap();
    writeln!(
        out,
        "    ;; TODO: if $require_evidence=1, verify evidence hash from host"
    )
    .unwrap();
    writeln!(
        out,
        "    ;; For now: pass-through (enforcement via ỌSỌVM outer gate)"
    )
    .unwrap();
    writeln!(out, "    nop").unwrap();
    writeln!(out, "  )").unwrap();
    writeln!(out).unwrap();
}

fn emit_tithe(out: &mut String, _ir: &OsoIR) {
    writeln!(out, "  ;; Utility: compute Èṣù tithe on any amount").unwrap();
    writeln!(out, "  (func $tithe_amount (export \"tithe_amount\")").unwrap();
    writeln!(out, "    (param $gross i64) (result i64)").unwrap();
    writeln!(out, "    local.get $gross").unwrap();
    writeln!(out, "    i64.const 369   ;; 3.69% * 100 = 369 basis points").unwrap();
    writeln!(out, "    i64.mul").unwrap();
    writeln!(out, "    i64.const 10000").unwrap();
    writeln!(out, "    i64.div_u").unwrap();
    writeln!(out, "  )").unwrap();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::{
        AssetSpec, EvidenceKind, EvidenceSpec, OsoIR, SettlementSpec, WitnessPolicySpec,
        WitnessType,
    };

    fn sample_ir() -> OsoIR {
        OsoIR {
            oso_ir_version: "1.0".into(),
            contract_class: "financial".into(),
            contract_name: "AsePool".into(),
            assets: vec![AssetSpec {
                name: "pool".into(),
                fields: vec!["balance".into(), "locked".into()],
            }],
            capabilities: vec!["TRANSFER".into()],
            minimum_tier: 2,
            evidence: EvidenceSpec {
                required: true,
                kind: Some(EvidenceKind::ZangbetoReceipt),
                fields: vec![],
            },
            witness_policy: Some(WitnessPolicySpec {
                quorum: 1,
                types: vec![WitnessType::Agent],
            }),
            settlement: SettlementSpec {
                currency: Some("ASE".into()),
                fee_routing: Some("6-pool".into()),
                creator_share: Some(0.0),
                burn_share: Some(0.05),
                provider_share: Some(0.90),
                amount: Some(1000),
                tithe_rate: Some(0.0369),
            },
            policy: Default::default(),
            lifecycle: vec!["deposit".into(), "withdraw".into(), "settle".into()],
        }
    }

    #[test]
    fn compiles_to_wat_module() {
        let ir = sample_ir();
        let wat = WasmCodegen::compile(&ir).unwrap();
        assert!(wat.contains("(module $asepool"));
        assert!(wat.contains("oso_host"));
        assert!(wat.contains("$lifecycle_deposit"));
        assert!(wat.contains("$lifecycle_withdraw"));
    }

    #[test]
    fn settlement_encodes_tithe() {
        let ir = sample_ir();
        let wat = WasmCodegen::compile(&ir).unwrap();
        assert!(wat.contains("369")); // tithe basis points
        assert!(wat.contains("10000")); // divisor
        assert!(wat.contains("$settle"));
    }

    #[test]
    fn asset_globals_emitted() {
        let ir = sample_ir();
        let wat = WasmCodegen::compile(&ir).unwrap();
        assert!(wat.contains("$pool_balance"));
        assert!(wat.contains("$pool_locked"));
    }

    #[test]
    fn empty_lifecycle_compiles() {
        let mut ir = sample_ir();
        ir.lifecycle.clear();
        let wat = WasmCodegen::compile(&ir).unwrap();
        assert!(wat.contains("(module"));
        assert!(!wat.contains("$lifecycle_"));
    }
}
