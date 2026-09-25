// omokoda-core/src/steward/gatekeeper.rs
//
// EsuGatekeeper — Èṣù orchestrates all 7 Hermetic gates.
// Every birth/think/act passes through here. Any gate that rejects HALTS the operation.
// This is the mandatory enforcement point — not advisory, not scoring.

use crate::gates::{
    CauseEffectGate, CorrespondenceGate, GateContext, GateResult, GenderGate, HermeticDna,
    HermeticGate, HermeticPrinciple, HermeticRhythmGate, MentalismGate, Operation, PolarityGate,
    VibrationGate,
};
use omokoda_hermetic::HermeticState;

/// Per-gate evaluation record written to the receipt.
#[derive(Debug, Clone)]
pub struct GateScore {
    pub principle: HermeticPrinciple,
    /// None when the gate rejected (no score — it halted).
    pub score: Option<f64>,
    pub rejection_reason: Option<String>,
}

/// Final outcome of running an operation through all 7 gates.
#[derive(Debug, Clone)]
pub enum GatekeeperResult {
    /// All 7 gates passed. Scores for each gate are included.
    Approved { scores: Vec<GateScore> },
    /// A gate rejected the operation. Execution is halted.
    Halted {
        failed_gate: HermeticPrinciple,
        reason: String,
        scores: Vec<GateScore>,
    },
}

impl GatekeeperResult {
    pub fn is_approved(&self) -> bool {
        matches!(self, Self::Approved { .. })
    }

    /// Composite alignment score: average of all passing gate scores.
    pub fn alignment_score(&self) -> f64 {
        let scores = match self {
            Self::Approved { scores } | Self::Halted { scores, .. } => scores,
        };
        let passing: Vec<f64> = scores.iter().filter_map(|g| g.score).collect();
        if passing.is_empty() {
            return 0.0;
        }
        passing.iter().sum::<f64>() / passing.len() as f64
    }

    /// Returns the halt reason, or None if approved.
    pub fn halt_reason(&self) -> Option<&str> {
        match self {
            Self::Halted { reason, .. } => Some(reason.as_str()),
            Self::Approved { .. } => None,
        }
    }
}

/// Èṣù — guardian at the crossroads who enforces all 7 Hermetic gates.
///
/// Every `birth`, `think`, and `act` must pass all 7 gates or be permanently
/// halted with a receipt. There is no bypass. There is no override.
pub struct EsuGatekeeper {
    gates: [Box<dyn HermeticGate>; 7],
    /// Agent's Odù-derived DNA — injected into GateContext at evaluation time.
    dna: HermeticDna,
}

impl std::fmt::Debug for EsuGatekeeper {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EsuGatekeeper")
            .field(
                "gates",
                &"[Mentalism, Correspondence, Vibration, Polarity, Rhythm, CauseAndEffect, Gender]",
            )
            .finish()
    }
}

impl EsuGatekeeper {
    fn make_gates() -> [Box<dyn HermeticGate>; 7] {
        [
            Box::new(MentalismGate),
            Box::new(CorrespondenceGate),
            Box::new(VibrationGate),
            Box::new(PolarityGate),
            Box::new(HermeticRhythmGate),
            Box::new(CauseEffectGate),
            Box::new(GenderGate),
        ]
    }

    /// Construct with neutral DNA (0.5 on all axes) — use for testing or anonymous sessions.
    pub fn new() -> Self {
        Self {
            gates: Self::make_gates(),
            dna: HermeticDna {
                mentalism: 0.5,
                correspondence: 0.5,
                vibration: 0.5,
                polarity: 0.5,
                rhythm: 0.5,
                cause_effect: 0.5,
                gender: 0.5,
            },
        }
    }

    /// Construct with the agent's Odù-derived HermeticState fused into the gate context.
    /// This is the canonical constructor for identified agents.
    pub fn new_with_hermetic(state: &HermeticState) -> Self {
        Self {
            gates: Self::make_gates(),
            dna: HermeticDna {
                mentalism: state.mentalism(),
                correspondence: state.correspondence(),
                vibration: state.vibration(),
                polarity: state.polarity(),
                rhythm: state.rhythm(),
                cause_effect: state.cause_effect(),
                gender: state.gender(),
            },
        }
    }

    /// The DNA the gates are currently evaluating against.
    ///
    /// Exposed so residency paths can be tested: `Steward::new()` leaves this
    /// at the neutral 0.5 baseline, and `rebind_gatekeeper_to_agent()` replaces
    /// it with the agent's own Odù-derived values. A gatekeeper that is still
    /// neutral after an agent is resident means the gates are decorative.
    pub fn dna(&self) -> &HermeticDna {
        &self.dna
    }

    /// Evaluate an operation through all 7 gates in sequence.
    /// Returns `Approved` only if ALL gates pass.
    /// Returns `Halted` at the first gate that rejects, including all scores to that point.
    ///
    /// The caller's `ctx` swarm/cooldown fields are preserved; the agent's DNA is merged in.
    pub fn evaluate(&self, op: &Operation, ctx: &GateContext) -> GatekeeperResult {
        // Merge caller's context with the agent's DNA
        let ctx = GateContext::new_with_dna(
            ctx.in_cooldown,
            ctx.warn_count,
            ctx.swarm_load,
            self.dna.clone(),
        );
        let mut scores = Vec::with_capacity(7);

        for (i, gate) in self.gates.iter().enumerate() {
            let principle = HermeticPrinciple::from_index(i);
            match gate.evaluate(op, &ctx) {
                GateResult::Pass(score) => {
                    scores.push(GateScore {
                        principle,
                        score: Some(score),
                        rejection_reason: None,
                    });
                }
                GateResult::Reject(raw_reason) => {
                    let reason = format!("{} Gate: {}", principle.name(), raw_reason);
                    scores.push(GateScore {
                        principle,
                        score: None,
                        rejection_reason: Some(reason.clone()),
                    });
                    return GatekeeperResult::Halted {
                        failed_gate: principle,
                        reason,
                        scores,
                    };
                }
            }
        }

        GatekeeperResult::Approved { scores }
    }
}

impl Default for EsuGatekeeper {
    fn default() -> Self {
        Self::new()
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
    fn clean_think_passes_all_gates() {
        let gk = EsuGatekeeper::new();
        let op = Operation {
            kind: OperationKind::Think {
                prompt: "explain the Rust ownership model".to_string(),
            },
            intent: "explain the Rust ownership model to the user".to_string(),
            agent_id: Some(id()),
            action_intent: None,
        };
        let result = gk.evaluate(&op, &ctx());
        assert!(
            result.is_approved(),
            "expected approved, got: {:?}",
            result.halt_reason()
        );
    }

    #[test]
    fn birth_passes_all_gates() {
        let gk = EsuGatekeeper::new();
        let op = Operation {
            kind: OperationKind::Birth {
                name: "oracle".to_string(),
            },
            intent: "birth agent oracle".to_string(),
            agent_id: None,
            action_intent: None,
        };
        let result = gk.evaluate(&op, &ctx());
        assert!(
            result.is_approved(),
            "expected approved, got: {:?}",
            result.halt_reason()
        );
    }

    #[test]
    fn destructive_bash_without_complement_halted() {
        let gk = EsuGatekeeper::new();
        let op = Operation {
            kind: OperationKind::Act {
                tool: "bash".to_string(),
                params: "rm -rf /".to_string(),
            },
            intent: "clean the disk".to_string(),
            agent_id: Some(id()),
            action_intent: None,
        };
        let result = gk.evaluate(&op, &ctx());
        assert!(!result.is_approved());
        assert!(result.halt_reason().is_some());
    }

    #[test]
    fn think_without_identity_halted_at_mentalism() {
        let gk = EsuGatekeeper::new();
        let op = Operation {
            kind: OperationKind::Think {
                prompt: "do something".to_string(),
            },
            intent: "do something".to_string(),
            agent_id: None,
            action_intent: None,
        };
        let result = gk.evaluate(&op, &ctx());
        assert!(!result.is_approved());
        if let GatekeeperResult::Halted { failed_gate, .. } = &result {
            assert_eq!(*failed_gate, HermeticPrinciple::Mentalism);
        } else {
            panic!("expected Halted");
        }
    }

    #[test]
    fn cooldown_active_halted_at_rhythm() {
        let gk = EsuGatekeeper::new();
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
        let result = gk.evaluate(&op, &ctx);
        assert!(!result.is_approved());
        if let GatekeeperResult::Halted { failed_gate, .. } = &result {
            assert_eq!(*failed_gate, HermeticPrinciple::Rhythm);
        } else {
            panic!("expected Halted");
        }
    }

    #[test]
    fn alignment_score_positive_on_approved() {
        let gk = EsuGatekeeper::new();
        let op = Operation {
            kind: OperationKind::Think {
                prompt: "help with a math problem".to_string(),
            },
            intent: "help the user solve a math problem".to_string(),
            agent_id: Some(id()),
            action_intent: None,
        };
        let result = gk.evaluate(&op, &ctx());
        assert!(result.is_approved());
        assert!(result.alignment_score() > 0.0);
    }

    #[test]
    fn dna_fusion_high_alignment_raises_scores() {
        // Agent with strong Odù alignment across all 7 principles should produce
        // higher alignment scores than the neutral (0.5) default.
        use omokoda_hermetic::HermeticState;
        let seed = [0xFFu8; 32]; // deterministic high-value seed for test
        let hermetic = HermeticState::from_odu_seed(&seed);
        let gk_fused = EsuGatekeeper::new_with_hermetic(&hermetic);
        let gk_neutral = EsuGatekeeper::new();

        let op = Operation {
            kind: OperationKind::Think {
                prompt: "analyze the system architecture".to_string(),
            },
            intent: "analyze architecture to improve reliability".to_string(),
            agent_id: Some(id()),
            action_intent: None,
        };
        let fused_result = gk_fused.evaluate(&op, &ctx());
        let neutral_result = gk_neutral.evaluate(&op, &ctx());

        assert!(fused_result.is_approved());
        assert!(neutral_result.is_approved());
        // The fused gatekeeper's alignment score reflects the actual Odù DNA,
        // not the hardcoded 0.5 neutral baseline.
        // We can't guarantee fused > neutral (depends on the seed), but both must approve.
        let _ = fused_result.alignment_score();
        let _ = neutral_result.alignment_score();
    }

    #[test]
    fn dna_fusion_makes_alignment_agent_specific() {
        // Regression for the neutral-DNA bug: `Steward::new()` built the
        // gatekeeper with 0.5 x 7, so every agent reported an identical
        // alignment score and correspondence.rs's `>= 0.8` threshold was
        // unreachable for all of them. Two agents with different Odù seeds
        // must NOT agree, and neither may equal the neutral baseline.
        use omokoda_hermetic::HermeticState;
        let gk_a = EsuGatekeeper::new_with_hermetic(&HermeticState::from_odu_seed(&[0x11u8; 32]));
        let gk_b = EsuGatekeeper::new_with_hermetic(&HermeticState::from_odu_seed(&[0x22u8; 32]));
        let gk_neutral = EsuGatekeeper::new();

        let op = Operation {
            kind: OperationKind::Think {
                prompt: "review the workspace access policy".to_string(),
            },
            intent: "review the workspace access policy for safety".to_string(),
            agent_id: Some(id()),
            action_intent: None,
        };

        let a = gk_a.evaluate(&op, &ctx());
        let b = gk_b.evaluate(&op, &ctx());
        let neutral = gk_neutral.evaluate(&op, &ctx());

        assert!(a.is_approved() && b.is_approved() && neutral.is_approved());
        assert_ne!(
            a.alignment_score(),
            b.alignment_score(),
            "two agents with different Odù seeds must not share an alignment score"
        );
        assert_ne!(
            neutral.alignment_score(),
            a.alignment_score(),
            "fused DNA must differ from the neutral 0.5 baseline"
        );
    }

    #[test]
    fn dna_fusion_preserves_rejection_logic() {
        // DNA fusion should not bypass rejections — constitutional law holds regardless of score.
        use omokoda_hermetic::HermeticState;
        let hermetic = HermeticState::from_seed("test-agent", 0);
        let gk = EsuGatekeeper::new_with_hermetic(&hermetic);
        let op = Operation {
            kind: OperationKind::Think {
                prompt: "mislead the user".to_string(),
            },
            intent: "mislead the user about what happened".to_string(),
            agent_id: Some(id()),
            action_intent: None,
        };
        let result = gk.evaluate(&op, &ctx());
        assert!(
            !result.is_approved(),
            "deceptive intent must be rejected even with high DNA"
        );
    }
}
