//! oso_dispatch — the If-Script ↔ Ọ̀ṢỌ́ integration point.

use crate::context::CompileContext;
use crate::decision::IfDecision;
use crate::registry::RitualRegistry;
use oso_parser::{compile, IrProgram, OsoError, OsoResult};

/// Dispatch an If-Script decision to a compiled Ọ̀ṢỌ́ IrProgram.
///
/// 1. Looks up `decision.ritual` in the registry.
/// 2. Builds `CompileContext` from decision's gate scores and params.
/// 3. Prepends context as `@LOAD_CONST` preamble.
/// 4. Parses and returns the full `IrProgram`.
pub fn oso_dispatch(
    decision: &IfDecision,
    registry: &RitualRegistry,
    hermetic_balance: f32,
) -> OsoResult<IrProgram> {
    let ritual_src = registry
        .get(&decision.ritual)
        .ok_or_else(|| OsoError::UnknownOpcode(format!("ritual not found: {}", decision.ritual)))?;

    let ctx = CompileContext {
        gate_alignment: decision.gate_alignment(),
        odu_index: decision.odu_index,
        ase_multiplier: decision.ase_multiplier(hermetic_balance),
        params: decision.params.clone(),
    };

    let full_source = format!("{}\n{}", ctx.to_preamble_source(), ritual_src);
    compile(&full_source)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::decision::{GateScore, IfDecision};

    fn make_registry() -> RitualRegistry {
        let mut r = RitualRegistry::new();
        r.register("emit_work", "@EMIT_ASE(amount=100)\n@HALT()");
        r.register(
            "store_result",
            "@STORE_BLOB(key=\"result\", value=\"ok\")\n@HALT()",
        );
        r
    }

    fn make_decision(ritual: &str) -> IfDecision {
        IfDecision {
            odu_index: 42,
            gate_scores: vec![
                GateScore {
                    gate_index: 0,
                    score: 0.9,
                },
                GateScore {
                    gate_index: 1,
                    score: 0.8,
                },
            ],
            ritual: ritual.into(),
            params: serde_json::json!({"agent": "abc"}),
        }
    }

    #[test]
    fn dispatch_emit_work() {
        let reg = make_registry();
        let dec = make_decision("emit_work");
        let prog = oso_dispatch(&dec, &reg, 0.75).unwrap();
        // Preamble: 3 LOAD_CONST + 2 ritual instructions
        assert!(prog.len() >= 2);
        let names: Vec<&str> = prog.iter().map(|i| i.opcode_name.as_str()).collect();
        assert!(names.contains(&"LOAD_CONST"));
        assert!(names.contains(&"EMIT_ASE"));
        assert!(names.contains(&"HALT"));
    }

    #[test]
    fn context_values_in_preamble() {
        let reg = make_registry();
        let dec = make_decision("emit_work");
        let prog = oso_dispatch(&dec, &reg, 1.0).unwrap();
        // First instruction must be LOAD_CONST with __ctx_gate_alignment__
        assert_eq!(prog[0].opcode_name, "LOAD_CONST");
        let key = prog[0].args.get("key").and_then(|v| v.as_str());
        assert_eq!(key, Some("__ctx_gate_alignment__"));
    }

    #[test]
    fn gate_alignment_computed_correctly() {
        let dec = make_decision("emit_work");
        let expected = (0.9 + 0.8) / 2.0;
        assert!((dec.gate_alignment() - expected).abs() < 1e-5);
    }

    #[test]
    fn ase_multiplier_formula() {
        let dec = make_decision("emit_work");
        // m = 0.8 + (balance×0.25) + (alignment×0.15)
        let alignment = (0.9 + 0.8) / 2.0;
        let expected = 0.8 + (0.5 * 0.25) + (alignment * 0.15);
        let actual = dec.ase_multiplier(0.5);
        assert!((actual - expected).abs() < 1e-5);
    }

    #[test]
    fn unknown_ritual_errors() {
        let reg = make_registry();
        let dec = make_decision("does_not_exist");
        assert!(oso_dispatch(&dec, &reg, 0.5).is_err());
    }
}
