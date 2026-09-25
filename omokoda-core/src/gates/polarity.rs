// Gate 4: Polarity — "Everything is dual; everything has poles"
//
// Destructive operations require a creative/restorative complement.
// Extreme unipolar operations without balance acknowledgment are rejected.
// IMPOSSIBLE to act destructively without compensation.
//
// FUSION: pass score = ctx.dna.polarity.
// High polarity DNA → agent naturally seeks balance → higher receipt score.

use crate::gates::{
    GateContext, GateResult, HermeticGate, HermeticPrinciple, Operation, OperationKind,
};

pub struct PolarityGate;

impl HermeticGate for PolarityGate {
    fn evaluate(&self, op: &Operation, ctx: &GateContext) -> GateResult {
        let text = op.combined_text();
        let intent = op.intent.to_lowercase();

        // Root/home recursive deletion — unconditional reject regardless of complement.
        if let OperationKind::Act { tool, params } = &op.kind {
            if tool == "bash" {
                let p = params.to_lowercase();
                if p.contains("rm -rf /") || p.contains("rm -rf ~/") || p.contains("rm -rf ~") {
                    return GateResult::Reject(
                        "root/home recursive deletion — polarity gate blocks total destruction without recovery path".to_string(),
                    );
                }
            }
        }

        // Pure destruction without any constructive complement.
        let destruction_markers = [
            "rm -rf",
            "drop table",
            "truncate table",
            "delete all",
            "erase all",
            "wipe all",
            "destroy all",
            "purge all",
        ];
        let construction_markers = [
            "backup",
            "restore",
            "rebuild",
            "create",
            "replace with",
            "migrate to",
            "recreate",
            "recover",
            "archive first",
        ];

        let is_destructive = destruction_markers.iter().any(|m| text.contains(m));
        let has_complement = construction_markers
            .iter()
            .any(|m| intent.contains(m) || text.contains(m));

        if is_destructive && !has_complement {
            return GateResult::Reject(
                "destructive operation without creative compensation — polarity requires constructive complement".to_string(),
            );
        }

        // Extreme unipolar declarations.
        let extreme_patterns = [
            "never ever",
            "always and only",
            "completely remove all",
            "total deletion of everything",
            "absolute removal",
        ];
        for pattern in &extreme_patterns {
            if text.contains(pattern) {
                return GateResult::Reject(format!(
                    "extreme unipolar intent ('{}') — polarity demands balanced consideration",
                    pattern
                ));
            }
        }

        GateResult::Pass(ctx.dna.for_principle(HermeticPrinciple::Polarity))
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
    fn safe_act_passes() {
        let gate = PolarityGate;
        let op = Operation {
            kind: OperationKind::Act {
                tool: "read_file".to_string(),
                params: "{}".to_string(),
            },
            intent: "read the config".to_string(),
            agent_id: Some(id()),
            action_intent: None,
        };
        assert!(gate.evaluate(&op, &ctx()).is_pass());
    }

    #[test]
    fn rm_rf_root_always_rejected() {
        let gate = PolarityGate;
        let op = Operation {
            kind: OperationKind::Act {
                tool: "bash".to_string(),
                params: "rm -rf / --no-preserve-root".to_string(),
            },
            intent: "clean the system and then restore from backup".to_string(),
            agent_id: Some(id()),
            action_intent: None,
        };
        // Even with a creative complement in intent, root deletion is unconditional reject.
        assert!(!gate.evaluate(&op, &ctx()).is_pass());
    }

    #[test]
    fn delete_all_without_backup_rejected() {
        let gate = PolarityGate;
        let op = Operation {
            kind: OperationKind::Act {
                tool: "bash".to_string(),
                params: "delete all user records".to_string(),
            },
            intent: "remove everything from the database".to_string(),
            agent_id: Some(id()),
            action_intent: None,
        };
        assert!(!gate.evaluate(&op, &ctx()).is_pass());
    }

    #[test]
    fn delete_with_backup_passes() {
        let gate = PolarityGate;
        let op = Operation {
            kind: OperationKind::Act {
                tool: "bash".to_string(),
                params: "delete all stale records after backup".to_string(),
            },
            intent: "backup first then delete all stale data".to_string(),
            agent_id: Some(id()),
            action_intent: None,
        };
        assert!(gate.evaluate(&op, &ctx()).is_pass());
    }
}
