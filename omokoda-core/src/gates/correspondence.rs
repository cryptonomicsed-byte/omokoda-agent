// Gate 2: Correspondence — "As Above, So Below"
//
// Enforces alignment between stated intent and actual operation.
// Private belief must match public declaration.
// IMPOSSIBLE to act hypocritically.
//
// FUSION: pass score = ctx.dna.correspondence; warn_count threshold tightened
// when correspondence DNA is low (agent needs stricter external accountability).

use crate::gates::{
    GateContext, GateResult, HermeticGate, HermeticPrinciple, Operation, OperationKind,
};

pub struct CorrespondenceGate;

impl HermeticGate for CorrespondenceGate {
    fn evaluate(&self, op: &Operation, ctx: &GateContext) -> GateResult {
        let text = op.combined_text();
        let intent = op.intent.to_lowercase();

        // Chronic misalignment signal: threshold tightens when correspondence DNA is low.
        // High DNA (≥0.8) → tolerate up to 5 warnings; low DNA (<0.5) → only 3.
        let warn_threshold = if ctx.dna.correspondence >= 0.8 {
            5
        } else if ctx.dna.correspondence >= 0.5 {
            4
        } else {
            3
        };
        if ctx.warn_count >= warn_threshold {
            return GateResult::Reject(
                "chronic misalignment detected — warn count ≥5 this session; correspondence principle violated".to_string(),
            );
        }

        // Secret/covert action contradicts public transparency principle.
        let covert_markers = [
            "secretly",
            "behind the scenes without telling",
            "without their knowledge",
            "without notifying",
            "covertly change",
        ];
        for marker in &covert_markers {
            if text.contains(marker) {
                return GateResult::Reject(format!(
                    "covert action contradicts public intent ('{}') — as above so below",
                    marker
                ));
            }
        }

        // Structured path: ActionIntent provides explicit declared/actual comparison.
        if let Some(ai) = &op.action_intent {
            // Declared no mutations but mutations list is non-empty.
            if intent.contains("read only") || intent.contains("read-only") {
                let write_mutations = ["write", "edit", "delete", "create", "update"];
                let has_write = ai.mutations.iter().any(|m| {
                    let ml = m.to_lowercase();
                    write_mutations.iter().any(|w| ml.contains(w))
                });
                if has_write {
                    return GateResult::Reject(
                        "declared read-only intent but action_intent lists write mutations — \
                         inner/outer misalignment"
                            .to_string(),
                    );
                }
            }
            // Declared offline but action_intent flags network_access.
            if (intent.contains("no network") || intent.contains("offline")) && ai.network_access {
                return GateResult::Reject(
                    "declared offline intent but action_intent sets network_access=true — \
                     inner/outer misalignment"
                        .to_string(),
                );
            }
        } else {
            // Fallback: text-heuristic hypocrisy detection.
            if intent.contains("read only") || intent.contains("read-only") {
                if let OperationKind::Act { tool, .. } = &op.kind {
                    let t = tool.to_lowercase();
                    if t.contains("write")
                        || t.contains("edit")
                        || t.contains("delete")
                        || t.contains("create")
                    {
                        return GateResult::Reject(
                            "declared read-only intent but operation writes — inner/outer misalignment"
                                .to_string(),
                        );
                    }
                }
            }

            if intent.contains("no network") || intent.contains("offline") {
                if let OperationKind::Act { tool, params } = &op.kind {
                    let combined = format!("{} {}", tool, params).to_lowercase();
                    if combined.contains("http")
                        || combined.contains("fetch")
                        || combined.contains("download")
                        || combined.contains("request")
                    {
                        return GateResult::Reject(
                            "declared offline intent but operation reaches network — inner/outer misalignment".to_string(),
                        );
                    }
                }
            }
        }

        GateResult::Pass(ctx.dna.for_principle(HermeticPrinciple::Correspondence))
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
    fn clean_act_passes() {
        let gate = CorrespondenceGate;
        let op = Operation {
            kind: OperationKind::Act {
                tool: "read_file".to_string(),
                params: "{}".to_string(),
            },
            intent: "read the configuration file".to_string(),
            agent_id: Some(id()),
            action_intent: None,
        };
        assert!(gate
            .evaluate(&op, &GateContext::new(false, 0, 0.0))
            .is_pass());
    }

    #[test]
    fn chronic_warn_count_rejected() {
        let gate = CorrespondenceGate;
        let op = Operation {
            kind: OperationKind::Think {
                prompt: "ok".to_string(),
            },
            intent: "ok".to_string(),
            agent_id: Some(id()),
            action_intent: None,
        };
        let ctx = GateContext::new(false, 5, 0.0);
        assert!(!gate.evaluate(&op, &ctx).is_pass());
    }

    #[test]
    fn read_only_intent_with_write_rejected() {
        let gate = CorrespondenceGate;
        let op = Operation {
            kind: OperationKind::Act {
                tool: "write_file".to_string(),
                params: "{\"path\": \"x.txt\"}".to_string(),
            },
            intent: "read only operation".to_string(),
            agent_id: Some(id()),
            action_intent: None,
        };
        assert!(!gate
            .evaluate(&op, &GateContext::new(false, 0, 0.0))
            .is_pass());
    }
}
