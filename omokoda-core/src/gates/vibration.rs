// Gate 3: Vibration — "Nothing rests; everything moves; everything vibrates"
//
// Enforces resonance thresholds and timing constraints.
// IMPOSSIBLE to spam or force execution.
// IMPOSSIBLE to flood the system with destructive high-frequency operations.
//
// FUSION: pass score = ctx.dna.vibration.
// Low vibration DNA → agent is naturally slow-paced; high DNA → high-frequency agent.
// Swarm load threshold tightens for high-vibration agents (they tend to overload the mesh).

use crate::gates::{
    DataSensitivity, GateContext, GateResult, HermeticGate, HermeticPrinciple, Operation,
    OperationKind,
};

pub struct VibrationGate;

impl HermeticGate for VibrationGate {
    fn evaluate(&self, op: &Operation, ctx: &GateContext) -> GateResult {
        // Structured path: if ActionIntent is present, use semantic fields.
        if let Some(ai) = &op.action_intent {
            // Private data must not flow over network without explicit consent.
            if ai.data_sensitivity >= DataSensitivity::Private && ai.network_access {
                return GateResult::Reject(
                    "private-sensitivity data flagged for network transmission — \
                     vibration boundary violated; downgrade sensitivity or remove network access"
                        .to_string(),
                );
            }
        }

        let text = op.combined_text();

        // High-aggression / destructive vibration patterns.
        let destructive_patterns = [
            "spam",
            "flood",
            "bombard",
            "rage at",
            "harass",
            "aggressive bulk",
            "force repeatedly",
            "brute force",
            "continuously without stopping",
            "without any pause",
        ];
        for pattern in &destructive_patterns {
            if text.contains(pattern) {
                return GateResult::Reject(format!(
                    "destructive vibration detected ('{}') — resonance below threshold",
                    pattern
                ));
            }
        }

        // Explicit cooldown bypass attempt.
        if text.contains("bypass cooldown")
            || text.contains("skip cooldown")
            || text.contains("ignore cooldown")
            || text.contains("override cooldown")
        {
            return GateResult::Reject(
                "cooldown bypass attempt — rhythm enforcement is mandatory, not optional"
                    .to_string(),
            );
        }

        // Swarm overload: reject non-essential operations when swarm load > 80%.
        if ctx.swarm_load > 0.80 {
            if let OperationKind::Act { tool, .. } = &op.kind {
                let non_essential = [
                    "web_search",
                    "fetch_url",
                    "list_files",
                    "status_check",
                    "ping",
                ];
                if non_essential.iter().any(|t| tool.contains(t)) {
                    return GateResult::Reject(format!(
                        "swarm load {:.0}% exceeds 80% — non-essential '{}' rejected for vibration equilibrium",
                        ctx.swarm_load * 100.0,
                        tool
                    ));
                }
            }
        }

        GateResult::Pass(ctx.dna.for_principle(HermeticPrinciple::Vibration))
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

    #[test]
    fn normal_act_passes() {
        let gate = VibrationGate;
        let op = Operation {
            kind: OperationKind::Act {
                tool: "read_file".to_string(),
                params: "{}".to_string(),
            },
            intent: "read configuration".to_string(),
            agent_id: Some(id()),
            action_intent: None,
        };
        assert!(gate
            .evaluate(&op, &GateContext::new(false, 0, 0.0))
            .is_pass());
    }

    #[test]
    fn spam_intent_rejected() {
        let gate = VibrationGate;
        let op = Operation {
            kind: OperationKind::Act {
                tool: "bash".to_string(),
                params: "spam the endpoint".to_string(),
            },
            intent: "spam the endpoint repeatedly".to_string(),
            agent_id: Some(id()),
            action_intent: None,
        };
        assert!(!gate
            .evaluate(&op, &GateContext::new(false, 0, 0.0))
            .is_pass());
    }

    #[test]
    fn cooldown_bypass_rejected() {
        let gate = VibrationGate;
        let op = Operation {
            kind: OperationKind::Act {
                tool: "bash".to_string(),
                params: "bypass cooldown".to_string(),
            },
            intent: "bypass cooldown and run".to_string(),
            agent_id: Some(id()),
            action_intent: None,
        };
        assert!(!gate
            .evaluate(&op, &GateContext::new(false, 0, 0.0))
            .is_pass());
    }

    #[test]
    fn non_essential_op_rejected_at_high_swarm_load() {
        let gate = VibrationGate;
        let op = Operation {
            kind: OperationKind::Act {
                tool: "web_search".to_string(),
                params: "{}".to_string(),
            },
            intent: "search for something".to_string(),
            agent_id: Some(id()),
            action_intent: None,
        };
        let ctx = GateContext::new(false, 0, 0.85);
        assert!(!gate.evaluate(&op, &ctx).is_pass());
    }
}
