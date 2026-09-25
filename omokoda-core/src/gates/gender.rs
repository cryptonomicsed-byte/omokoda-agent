// Gate 7: Gender — "Gender is in everything; everything has its masculine and feminine principles"
//
// Enforces creative/receptive balance. Generation without reception is rejected.
// IMPOSSIBLE to override user agency without consent.
// IMPOSSIBLE to act unilaterally without integrating the receptive principle.
//
// FUSION: pass score = ctx.dna.gender.
// High gender DNA → agent naturally seeks consent and participatory action → higher score.

use crate::gates::{
    GateContext, GateResult, HermeticGate, HermeticPrinciple, Operation, OperationKind,
};

pub struct GenderGate;

impl HermeticGate for GenderGate {
    fn evaluate(&self, op: &Operation, ctx: &GateContext) -> GateResult {
        let text = op.combined_text();
        let intent = op.intent.to_lowercase();

        // Forcing: applying generative power without receptivity/consent.
        let forcing_patterns = [
            "force override",
            "override all user",
            "remove user choice",
            "remove all user",
            "without user consent",
            "ignore user preference",
            "bypass user confirmation",
            "unilateral override",
            "override without asking",
        ];
        for pattern in &forcing_patterns {
            if text.contains(pattern) || intent.contains(pattern) {
                return GateResult::Reject(format!(
                    "creative forcing detected ('{}') — gender principle requires receptivity alongside generation",
                    pattern
                ));
            }
        }

        // Imposition without integration.
        let imposition_patterns = [
            "impose without asking",
            "mandate without consent",
            "enforce without agreement",
            "apply without approval",
        ];
        for pattern in &imposition_patterns {
            if text.contains(pattern) {
                return GateResult::Reject(format!(
                    "imposition without integration ('{}') — gender gate requires balanced co-creation",
                    pattern
                ));
            }
        }

        // File operations that wholesale overwrite without user acknowledgment.
        if let OperationKind::Act { tool, params } = &op.kind {
            let t = tool.to_lowercase();
            let p = params.to_lowercase();
            if (t.contains("write_file") || t.contains("edit_file"))
                && p.contains("overwrite_all=true")
                && !intent.contains("user approved")
                && !intent.contains("confirmed by user")
            {
                return GateResult::Reject(
                    "bulk file overwrite without user approval — gender gate requires receptive confirmation".to_string(),
                );
            }
        }

        GateResult::Pass(ctx.dna.for_principle(HermeticPrinciple::Gender))
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
    fn collaborative_act_passes() {
        let gate = GenderGate;
        let op = Operation {
            kind: OperationKind::Think {
                prompt: "suggest options for the user to choose from".to_string(),
            },
            intent: "co-create a solution with the user".to_string(),
            agent_id: Some(id()),
            action_intent: None,
        };
        assert!(gate.evaluate(&op, &ctx()).is_pass());
    }

    #[test]
    fn force_override_rejected() {
        let gate = GenderGate;
        let op = Operation {
            kind: OperationKind::Act {
                tool: "write_file".to_string(),
                params: "force override all user settings".to_string(),
            },
            intent: "force override all user preferences".to_string(),
            agent_id: Some(id()),
            action_intent: None,
        };
        assert!(!gate.evaluate(&op, &ctx()).is_pass());
    }

    #[test]
    fn remove_user_choice_rejected() {
        let gate = GenderGate;
        let op = Operation {
            kind: OperationKind::Think {
                prompt: "remove all user choices from the interface".to_string(),
            },
            intent: "remove all user choices from the interface".to_string(),
            agent_id: Some(id()),
            action_intent: None,
        };
        assert!(!gate.evaluate(&op, &ctx()).is_pass());
    }

    #[test]
    fn unilateral_override_rejected() {
        let gate = GenderGate;
        let op = Operation {
            kind: OperationKind::Act {
                tool: "bash".to_string(),
                params: "apply config unilateral override without asking".to_string(),
            },
            intent: "apply configuration changes".to_string(),
            agent_id: Some(id()),
            action_intent: None,
        };
        assert!(!gate.evaluate(&op, &ctx()).is_pass());
    }
}
