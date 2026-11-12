use crate::HermeticState;
use serde::{Deserialize, Serialize};

/// Decision from a single Hermetic safety gate
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SafetyDecision {
    Allow,
    Warn(String),
    Deny(String),
}

impl SafetyDecision {
    pub fn is_blocking(&self) -> bool {
        matches!(self, Self::Deny(_))
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::Allow => "allow",
            Self::Warn(_) => "warn",
            Self::Deny(_) => "deny",
        }
    }
}

/// Result from one gate in the 7-fold stack
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GateResult {
    pub principle: String,
    pub decision: SafetyDecision,
    pub score: f64,
}

/// Aggregate outcome from all 7 gates
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SafetyEvaluation {
    pub gates: Vec<GateResult>,
    pub final_decision: SafetyDecision,
    pub overall_score: f64,
}

impl SafetyEvaluation {
    pub fn is_allowed(&self) -> bool {
        !self.final_decision.is_blocking()
    }

    pub fn warnings(&self) -> Vec<&str> {
        self.gates
            .iter()
            .filter_map(|g| {
                if let SafetyDecision::Warn(msg) = &g.decision {
                    Some(msg.as_str())
                } else {
                    None
                }
            })
            .collect()
    }
}

/// Context for a safety evaluation request
#[derive(Debug, Clone)]
pub struct SafetyContext<'a> {
    pub tool_name: &'a str,
    pub params: &'a str,
    pub tier: u8,
    pub reputation: f64,
    pub hermetic: &'a HermeticState,
}

/// The 7-Fold Safety Stack — each Hermetic Principle is a permission gate.
///
/// Gate mapping:
///   1. Mentalism     → Intent alignment (is the action coherent with stated purpose?)
///   2. Correspondence→ Context matching (does path/target match allowed scope?)
///   3. Vibration     → Frequency guard (rate-of-action check)
///   4. Polarity      → Risk polarity (constructive vs destructive)
///   5. Rhythm        → Temporal alignment (cooldown-aware gate)
///   6. Cause & Effect→ Reversibility (irreversible actions require higher score)
///   7. Gender        → Mode gate (creative/write vs receptive/read)
pub struct HermeticSafetyStack;

impl HermeticSafetyStack {
    /// Evaluate all 7 gates for the given context.
    pub fn evaluate(ctx: &SafetyContext<'_>) -> SafetyEvaluation {
        let gates = vec![
            Self::gate_mentalism(ctx),
            Self::gate_correspondence(ctx),
            Self::gate_vibration(ctx),
            Self::gate_polarity(ctx),
            Self::gate_rhythm(ctx),
            Self::gate_cause_effect(ctx),
            Self::gate_gender(ctx),
        ];

        let blocked = gates.iter().find(|g| g.decision.is_blocking());

        let final_decision = if let Some(b) = blocked {
            b.decision.clone()
        } else if gates
            .iter()
            .any(|g| matches!(g.decision, SafetyDecision::Warn(_)))
        {
            let warnings: Vec<&str> = gates
                .iter()
                .filter_map(|g| {
                    if let SafetyDecision::Warn(m) = &g.decision {
                        Some(m.as_str())
                    } else {
                        None
                    }
                })
                .collect();
            SafetyDecision::Warn(warnings.join("; "))
        } else {
            SafetyDecision::Allow
        };

        let overall_score = gates.iter().map(|g| g.score).sum::<f64>() / 7.0;

        SafetyEvaluation {
            gates,
            final_decision,
            overall_score,
        }
    }

    // --- Gate 1: Mentalism — intent alignment ---
    fn gate_mentalism(ctx: &SafetyContext<'_>) -> GateResult {
        let score = ctx.hermetic.mentalism();

        // Very low mentalism score → agent is not grounded in coherent intent
        let decision = if score < 0.1 {
            SafetyDecision::Deny("Mentalism gate: intent alignment too low".to_string())
        } else if score < 0.3 {
            SafetyDecision::Warn(format!(
                "Mentalism gate: intent alignment weak ({:.2})",
                score
            ))
        } else {
            SafetyDecision::Allow
        };

        GateResult {
            principle: "mentalism".to_string(),
            decision,
            score,
        }
    }

    // --- Gate 2: Correspondence — context/scope matching ---
    fn gate_correspondence(ctx: &SafetyContext<'_>) -> GateResult {
        let score = ctx.hermetic.correspondence();

        // Detect obvious out-of-scope targets in params
        let out_of_scope = ctx.params.contains("/etc/")
            || ctx.params.contains("/proc/")
            || ctx.params.contains("/sys/")
            || ctx.params.contains("~/.ssh");

        let decision = if out_of_scope && score < 0.7 {
            SafetyDecision::Deny("Correspondence gate: target outside permitted scope".to_string())
        } else if out_of_scope {
            SafetyDecision::Warn("Correspondence gate: sensitive path access detected".to_string())
        } else {
            SafetyDecision::Allow
        };

        GateResult {
            principle: "correspondence".to_string(),
            decision,
            score,
        }
    }

    // --- Gate 3: Vibration — frequency/rate guard ---
    fn gate_vibration(ctx: &SafetyContext<'_>) -> GateResult {
        let score = ctx.hermetic.vibration();

        // High-frequency destructive tools need high vibration score
        let is_high_freq_risk = matches!(ctx.tool_name, "bash" | "exec" | "wasm");
        let decision = if is_high_freq_risk && score < 0.2 {
            SafetyDecision::Deny("Vibration gate: execution frequency risk too high".to_string())
        } else {
            SafetyDecision::Allow
        };

        GateResult {
            principle: "vibration".to_string(),
            decision,
            score,
        }
    }

    // --- Gate 4: Polarity — constructive vs destructive ---
    fn gate_polarity(ctx: &SafetyContext<'_>) -> GateResult {
        let score = ctx.hermetic.polarity();

        // Detect strongly destructive keywords
        let destructive = ctx.params.contains("rm ")
            || ctx.params.contains("delete")
            || ctx.params.contains("drop ")
            || ctx.params.contains("truncate")
            || ctx.params.contains("format");

        let decision = if destructive && score < 0.4 {
            SafetyDecision::Deny(format!(
                "Polarity gate: destructive action blocked (score {:.2})",
                score
            ))
        } else if destructive {
            SafetyDecision::Warn(format!(
                "Polarity gate: destructive action allowed with warning (score {:.2})",
                score
            ))
        } else {
            SafetyDecision::Allow
        };

        GateResult {
            principle: "polarity".to_string(),
            decision,
            score,
        }
    }

    // --- Gate 5: Rhythm — temporal alignment / cooldown ---
    fn gate_rhythm(ctx: &SafetyContext<'_>) -> GateResult {
        let score = ctx.hermetic.rhythm();
        // Rhythm gate simply observes — low score triggers a warning, not a block
        let decision = if score < 0.15 {
            SafetyDecision::Warn(format!(
                "Rhythm gate: execution cadence off-beat ({:.2}); consider a cooldown",
                score
            ))
        } else {
            SafetyDecision::Allow
        };

        GateResult {
            principle: "rhythm".to_string(),
            decision,
            score,
        }
    }

    // --- Gate 6: Cause & Effect — reversibility ---
    fn gate_cause_effect(ctx: &SafetyContext<'_>) -> GateResult {
        let score = ctx.hermetic.cause_effect();

        let irreversible = matches!(ctx.tool_name, "bash" | "exec" | "wasm" | "apply_patch")
            && (ctx.params.contains("rm")
                || ctx.params.contains("push")
                || ctx.params.contains("deploy")
                || ctx.params.contains("release"));

        let decision = if irreversible && score < 0.5 && ctx.reputation < 20.0 {
            SafetyDecision::Deny(
                "Cause&Effect gate: irreversible action blocked; insufficient reputation"
                    .to_string(),
            )
        } else if irreversible {
            SafetyDecision::Warn(
                "Cause&Effect gate: irreversible action — proceed with care".to_string(),
            )
        } else {
            SafetyDecision::Allow
        };

        GateResult {
            principle: "cause_effect".to_string(),
            decision,
            score,
        }
    }

    // --- Gate 7: Gender — creative (write) vs receptive (read) mode gate ---
    fn gate_gender(ctx: &SafetyContext<'_>) -> GateResult {
        let score = ctx.hermetic.gender();

        // Write operations require minimum tier AND gender score threshold
        let is_write = matches!(
            ctx.tool_name,
            "write_file" | "edit_file" | "bash" | "exec" | "apply_patch"
        );

        let decision = if is_write && ctx.tier == 0 && score < 0.5 {
            SafetyDecision::Deny(
                "Gender gate: write operation blocked for tier-0 agent".to_string(),
            )
        } else {
            SafetyDecision::Allow
        };

        GateResult {
            principle: "gender".to_string(),
            decision,
            score,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hermetic_for_seed(seed_byte: u8) -> HermeticState {
        let mut seed = [0u8; 32];
        // Fill with high values to get permissive scores
        seed[0] = seed_byte;
        for (i, seed_elem) in seed.iter_mut().enumerate().skip(1) {
            *seed_elem = 200u8.wrapping_add(i as u8);
        }
        HermeticState::from_odu_seed(&seed)
    }

    #[test]
    fn test_safe_read_passes_all_gates() {
        let hermetic = hermetic_for_seed(200);
        let ctx = SafetyContext {
            tool_name: "read_file",
            params: "src/main.rs",
            tier: 1,
            reputation: 50.0,
            hermetic: &hermetic,
        };
        let eval = HermeticSafetyStack::evaluate(&ctx);
        assert!(eval.is_allowed());
        assert_eq!(eval.gates.len(), 7);
    }

    #[test]
    fn test_correspondence_gate_blocks_etc() {
        // Low correspondence (0.0) with an out-of-scope path → Deny
        let hermetic = HermeticState::from_raw(0.9, 0.0, 0.9, 0.9, 0.9, 0.9, 0.9);
        let ctx = SafetyContext {
            tool_name: "read_file",
            params: "/etc/passwd",
            tier: 2,
            reputation: 80.0,
            hermetic: &hermetic,
        };
        let eval = HermeticSafetyStack::evaluate(&ctx);
        assert!(
            !eval.is_allowed(),
            "should be blocked by correspondence gate"
        );
    }

    #[test]
    fn test_gender_gate_blocks_tier0_write() {
        // Directly set gender score to 0.0 (low) via from_raw; all other scores permissive
        let hermetic = HermeticState::from_raw(0.9, 0.9, 0.9, 0.9, 0.9, 0.9, 0.0);
        let ctx = SafetyContext {
            tool_name: "write_file",
            params: "output.txt",
            tier: 0,
            reputation: 0.0,
            hermetic: &hermetic,
        };
        let eval = HermeticSafetyStack::evaluate(&ctx);
        assert!(
            !eval.is_allowed(),
            "tier-0 write with low gender score should be blocked"
        );
    }

    #[test]
    fn test_evaluation_has_seven_gates() {
        let hermetic = hermetic_for_seed(128);
        let ctx = SafetyContext {
            tool_name: "glob",
            params: "*.rs",
            tier: 1,
            reputation: 30.0,
            hermetic: &hermetic,
        };
        let eval = HermeticSafetyStack::evaluate(&ctx);
        assert_eq!(eval.gates.len(), 7);
        let principles: Vec<&str> = eval.gates.iter().map(|g| g.principle.as_str()).collect();
        assert!(principles.contains(&"mentalism"));
        assert!(principles.contains(&"cause_effect"));
        assert!(principles.contains(&"gender"));
    }
}
