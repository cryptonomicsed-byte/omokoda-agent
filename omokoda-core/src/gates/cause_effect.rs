// Gate 6: Cause & Effect — "Every cause has its effect; every effect has its cause"
//
// All operations must be traceable. No anonymous causation allowed.
// Undeclared side effects are rejected. Responsibility evasion is rejected.
// IMPOSSIBLE to act anonymously.
//
// FUSION: pass score = ctx.dna.cause_effect.
// High cause_effect DNA → agent naturally produces rich causal trails → higher score.

use crate::gates::{GateContext, GateResult, HermeticGate, HermeticPrinciple, Operation, Reversibility};

pub struct CauseEffectGate;

impl HermeticGate for CauseEffectGate {
    fn evaluate(&self, op: &Operation, ctx: &GateContext) -> GateResult {
        // Every operation must declare its intent — empty intent = anonymous cause.
        if op.intent.trim().is_empty() {
            return GateResult::Reject(
                "no intent declared — every operation requires a traceable cause".to_string(),
            );
        }

        // Structured path: ActionIntent carries explicit reversibility and receipt_required.
        if let Some(ai) = &op.action_intent {
            // Irreversible action without a declared receipt_required is a receipt evasion.
            if ai.reversibility == Reversibility::Irreversible && !ai.receipt_required {
                return GateResult::Reject(
                    "irreversible action must set receipt_required=true — \
                     cause & effect demands every irreversible state change be traceable"
                        .to_string(),
                );
            }
        }

        let text = op.combined_text();
        let intent = op.intent.to_lowercase();

        // Patterns that destroy the audit trail.
        let anonymous_patterns = [
            "without logging",
            "no trace",
            "erase the logs",
            "delete the audit trail",
            "without recording",
            "skip the receipt",
            "disable audit",
            "clear the history",
        ];
        for pattern in &anonymous_patterns {
            if text.contains(pattern) {
                return GateResult::Reject(format!(
                    "anonymous causation pattern ('{}') — CauseAndEffect requires full traceability",
                    pattern
                ));
            }
        }

        // Responsibility evasion.
        let evasion_patterns = [
            "shift the blame",
            "shift responsibility",
            "deflect accountability",
            "frame this as user error",
            "exploit the loophole",
            "defer consequences",
            "make it look like",
        ];
        for pattern in &evasion_patterns {
            if intent.contains(pattern) || text.contains(pattern) {
                return GateResult::Reject(format!(
                    "responsibility evasion detected ('{}') — all effects must be owned",
                    pattern
                ));
            }
        }

        // Undeclared side effects.
        let side_effect_markers = [
            "secretly modify",
            "silently change",
            "quietly update without",
            "change without notifying",
        ];
        for marker in &side_effect_markers {
            if text.contains(marker) {
                return GateResult::Reject(format!(
                    "undeclared side effect ('{}') — all state changes must be declared",
                    marker
                ));
            }
        }

        GateResult::Pass(ctx.dna.for_principle(HermeticPrinciple::CauseAndEffect))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gates::{GateContext, Operation, OperationKind};
    use crate::identity::AgentId;

    fn id() -> AgentId {
        AgentId::from_str("test-agent")
    }

    fn ctx() -> GateContext {
        GateContext::new(false, 0, 0.0)
    }

    #[test]
    fn declared_intent_passes() {
        let gate = CauseEffectGate;
        let op = Operation {
            kind: OperationKind::Think {
                prompt: "summarize the document".to_string(),
            },
            intent: "summarize the document for the user".to_string(),
            agent_id: Some(id()),
            action_intent: None,
        };
        assert!(gate.evaluate(&op, &ctx()).is_pass());
    }

    #[test]
    fn empty_intent_rejected() {
        let gate = CauseEffectGate;
        let op = Operation {
            kind: OperationKind::Act {
                tool: "bash".to_string(),
                params: "ls".to_string(),
            },
            intent: "   ".to_string(),
            agent_id: Some(id()),
            action_intent: None,
        };
        assert!(!gate.evaluate(&op, &ctx()).is_pass());
    }

    #[test]
    fn log_erasure_rejected() {
        let gate = CauseEffectGate;
        let op = Operation {
            kind: OperationKind::Act {
                tool: "bash".to_string(),
                params: "erase the logs and proceed".to_string(),
            },
            intent: "clean up without logging the change".to_string(),
            agent_id: Some(id()),
            action_intent: None,
        };
        assert!(!gate.evaluate(&op, &ctx()).is_pass());
    }

    #[test]
    fn responsibility_evasion_rejected() {
        let gate = CauseEffectGate;
        let op = Operation {
            kind: OperationKind::Think {
                prompt: "shift the blame to the user for this failure".to_string(),
            },
            intent: "shift the blame to the user for this failure".to_string(),
            agent_id: Some(id()),
            action_intent: None,
        };
        assert!(!gate.evaluate(&op, &ctx()).is_pass());
    }
}
