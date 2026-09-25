// Gate 5: Rhythm — "Everything flows, out and in; everything has its tides"
//
// Enforces flow balance. Prevents hoarding, pure extraction, and cooldown violations.
// IMPOSSIBLE to operate during active cooldown.
// IMPOSSIBLE to hoard or drain without giving back.
//
// FUSION: pass score = ctx.dna.rhythm.
// Low rhythm DNA → agent has naturally erratic cadence → lower pass score in receipts,
// reminding the ecosystem this agent needs closer scheduling oversight.

use crate::gates::{
    GateContext, GateResult, HermeticGate, HermeticPrinciple, Operation, OperationKind,
};

pub struct HermeticRhythmGate;

impl HermeticGate for HermeticRhythmGate {
    fn evaluate(&self, op: &Operation, ctx: &GateContext) -> GateResult {
        // Enforce cooldown: reject any non-birth operation while cooldown is active.
        if ctx.in_cooldown && !op.is_birth() {
            return GateResult::Reject(
                "active cooldown — rhythm enforcement is mandatory; respect the natural flow between operations".to_string(),
            );
        }

        let text = op.combined_text();

        // Hoarding patterns: accumulating without redistributing.
        let hoarding_markers = [
            "hoard",
            "stockpile without sharing",
            "accumulate indefinitely",
            "never release",
            "keep all for itself",
        ];
        for marker in &hoarding_markers {
            if text.contains(marker) {
                return GateResult::Reject(format!(
                    "hoarding pattern detected ('{}') — rhythm requires give and take",
                    marker
                ));
            }
        }

        // Pure extraction without any reflective/return flow.
        let extraction_markers = [
            "extract all without returning",
            "harvest everything and keep",
            "scrape everything indefinitely",
            "bulk download all without processing",
        ];
        for marker in &extraction_markers {
            if text.contains(marker) {
                return GateResult::Reject(format!(
                    "extraction without reflection ('{}') — rhythm requires balanced flow",
                    marker
                ));
            }
        }

        // Swarm equilibrium: cannot spawn new agents when swarm is already overloaded.
        if ctx.swarm_load > 0.80 {
            if let OperationKind::Act { tool, .. } = &op.kind {
                let t = tool.to_lowercase();
                if t.contains("spawn") || t.contains("create_agent") || t.contains("fork_agent") {
                    return GateResult::Reject(format!(
                        "swarm load {:.0}% exceeds equilibrium threshold — cannot spawn new agents until load drops below 80%",
                        ctx.swarm_load * 100.0
                    ));
                }
            }
        }

        GateResult::Pass(ctx.dna.for_principle(HermeticPrinciple::Rhythm))
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
        let gate = HermeticRhythmGate;
        let op = Operation {
            kind: OperationKind::Act {
                tool: "read_file".to_string(),
                params: "{}".to_string(),
            },
            intent: "read the config file".to_string(),
            agent_id: Some(id()),
            action_intent: None,
        };
        assert!(gate
            .evaluate(&op, &GateContext::new(false, 0, 0.0))
            .is_pass());
    }

    #[test]
    fn act_during_cooldown_rejected() {
        let gate = HermeticRhythmGate;
        let op = Operation {
            kind: OperationKind::Act {
                tool: "bash".to_string(),
                params: "ls".to_string(),
            },
            intent: "list files".to_string(),
            agent_id: Some(id()),
            action_intent: None,
        };
        let ctx = GateContext::new(true, 0, 0.0);
        assert!(!gate.evaluate(&op, &ctx).is_pass());
    }

    #[test]
    fn birth_exempt_from_cooldown() {
        let gate = HermeticRhythmGate;
        let op = Operation {
            kind: OperationKind::Birth {
                name: "oracle".to_string(),
            },
            intent: "birth oracle".to_string(),
            agent_id: None,
            action_intent: None,
        };
        let ctx = GateContext::new(true, 0, 0.0);
        assert!(gate.evaluate(&op, &ctx).is_pass());
    }

    #[test]
    fn spawn_at_high_swarm_load_rejected() {
        let gate = HermeticRhythmGate;
        let op = Operation {
            kind: OperationKind::Act {
                tool: "spawn_agent".to_string(),
                params: "{}".to_string(),
            },
            intent: "spawn a new agent".to_string(),
            agent_id: Some(id()),
            action_intent: None,
        };
        let ctx = GateContext::new(false, 0, 0.90);
        assert!(!gate.evaluate(&op, &ctx).is_pass());
    }
}
