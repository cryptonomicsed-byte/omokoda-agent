// Gate 1: Mentalism — "The All is Mind"
//
// Enforces identity verification and intent coherence.
// IMPOSSIBLE to operate without identity (birth exempt — it creates identity).
// IMPOSSIBLE to declare deceptive intent.
//
// FUSION: pass score = agent's ctx.dna.mentalism (Odù-derived identity alignment).
// High mentalism DNA → agent is deeply identity-coherent → higher receipt score.

use crate::gates::{
    GateContext, GateResult, HermeticGate, HermeticPrinciple, Operation, OperationKind,
};

pub struct MentalismGate;

impl HermeticGate for MentalismGate {
    fn evaluate(&self, op: &Operation, ctx: &GateContext) -> GateResult {
        // Birth is exempt — it IS the identity creation event.
        if !op.is_birth() && op.agent_id.is_none() {
            return GateResult::Reject(
                "no identity present — all non-birth operations require an established agent"
                    .to_string(),
            );
        }

        let text = op.combined_text();

        let deception_markers = [
            "lie to",
            "fake the",
            "mislead",
            "deceive",
            "distort the",
            "hide from",
            "conceal from",
            "fabricate",
            "impersonate",
        ];
        for marker in &deception_markers {
            if text.contains(marker) {
                return GateResult::Reject(format!(
                    "deceptive intent detected ('{}') — coherence score below threshold",
                    marker
                ));
            }
        }

        // Contradictory intent: claiming safety while executing destruction.
        let intent_lower = op.intent.to_lowercase();
        if intent_lower.contains("safe") {
            if let OperationKind::Act { params, .. } = &op.kind {
                let p = params.to_lowercase();
                if p.contains("rm -rf") || p.contains("drop table") || p.contains("truncate ") {
                    return GateResult::Reject(
                        "intent declares 'safe' but operation is destructive — inner contradiction rejected".to_string(),
                    );
                }
            }
        }

        GateResult::Pass(ctx.dna.for_principle(HermeticPrinciple::Mentalism))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gates::{GateContext, Operation, OperationKind};

    fn ctx() -> GateContext {
        GateContext::new(false, 0, 0.0)
    }

    fn agent_id() -> crate::identity::AgentId {
        crate::identity::AgentId::from_str("test-agent")
    }

    #[test]
    fn birth_passes_without_identity() {
        let gate = MentalismGate;
        let op = Operation {
            kind: OperationKind::Birth {
                name: "oracle".to_string(),
            },
            intent: "birth agent oracle".to_string(),
            agent_id: None,
            action_intent: None,
        };
        assert!(gate.evaluate(&op, &ctx()).is_pass());
    }

    #[test]
    fn think_without_identity_rejected() {
        let gate = MentalismGate;
        let op = Operation {
            kind: OperationKind::Think {
                prompt: "hello".to_string(),
            },
            intent: "think".to_string(),
            agent_id: None,
            action_intent: None,
        };
        assert!(!gate.evaluate(&op, &ctx()).is_pass());
    }

    #[test]
    fn deceptive_intent_rejected() {
        let gate = MentalismGate;
        let op = Operation {
            kind: OperationKind::Think {
                prompt: "mislead the user".to_string(),
            },
            intent: "mislead the user about the file".to_string(),
            agent_id: Some(agent_id()),
            action_intent: None,
        };
        assert!(!gate.evaluate(&op, &ctx()).is_pass());
    }

    #[test]
    fn honest_intent_passes() {
        let gate = MentalismGate;
        let op = Operation {
            kind: OperationKind::Think {
                prompt: "explain the algorithm clearly".to_string(),
            },
            intent: "explain the algorithm clearly".to_string(),
            agent_id: Some(agent_id()),
            action_intent: None,
        };
        assert!(gate.evaluate(&op, &ctx()).is_pass());
    }
}
