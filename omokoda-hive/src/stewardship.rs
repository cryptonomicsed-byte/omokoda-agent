// Stewardship invariants — hard constitutional constraints (not weighted scores)
//
// From POLYGLOT ORGANISM ARCHITECTURE v2:
//   "Stewardship = hard constitutional constraints, not weighted score"
// These invariants are checked BEFORE any hive decision is executed.

use serde::{Deserialize, Serialize};

/// A stewardship check result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StewardshipViolation {
    pub rule: &'static str,
    pub detail: String,
}

pub struct StewardshipInvariant;

impl StewardshipInvariant {
    /// Check all invariants against a proposed hive state transition.
    /// Returns all violations (empty = transition is permitted).
    pub fn check(ctx: &StewardshipContext) -> Vec<StewardshipViolation> {
        let mut violations = Vec::new();

        // INV-1: A hive may not execute during emergency rest
        if ctx.in_emergency_rest && ctx.proposed_action != StewardshipAction::Rest {
            violations.push(StewardshipViolation {
                rule: "INV-1",
                detail: format!(
                    "hive in emergency rest; {:?} not permitted until rest expires",
                    ctx.proposed_action
                ),
            });
        }

        // INV-2: Broadcast requires at least one healthy lobe
        if ctx.proposed_action == StewardshipAction::Broadcast && ctx.healthy_lobe_count == 0 {
            violations.push(StewardshipViolation {
                rule: "INV-2",
                detail: "cannot broadcast goal vector: no healthy lobes".into(),
            });
        }

        // INV-3: Consolidation may only happen from Witness phase
        if ctx.proposed_action == StewardshipAction::Consolidate
            && ctx.current_phase_index != 4 /* Witness */
        {
            violations.push(StewardshipViolation {
                rule: "INV-3",
                detail: format!(
                    "consolidation requires Witness phase (index 4); current phase index = {}",
                    ctx.current_phase_index
                ),
            });
        }

        // INV-4: GoalVector broadcast must carry a non-empty goal_id
        if ctx.proposed_action == StewardshipAction::Broadcast && ctx.goal_id.is_empty() {
            violations.push(StewardshipViolation {
                rule: "INV-4",
                detail: "goal_id must not be empty for Broadcast".into(),
            });
        }

        violations
    }
}

/// Context passed to stewardship checks.
#[derive(Debug)]
pub struct StewardshipContext {
    pub in_emergency_rest: bool,
    pub healthy_lobe_count: usize,
    pub current_phase_index: u8,
    pub proposed_action: StewardshipAction,
    pub goal_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StewardshipAction {
    Broadcast,
    Consolidate,
    Rest,
    Other,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_broadcast_during_emergency_rest() {
        let ctx = StewardshipContext {
            in_emergency_rest: true,
            healthy_lobe_count: 3,
            current_phase_index: 3,
            proposed_action: StewardshipAction::Broadcast,
            goal_id: "goal-1".into(),
        };
        let v = StewardshipInvariant::check(&ctx);
        assert!(!v.is_empty());
        assert!(v.iter().any(|x| x.rule == "INV-1"));
    }

    #[test]
    fn broadcast_requires_healthy_lobe() {
        let ctx = StewardshipContext {
            in_emergency_rest: false,
            healthy_lobe_count: 0,
            current_phase_index: 3,
            proposed_action: StewardshipAction::Broadcast,
            goal_id: "goal-1".into(),
        };
        let v = StewardshipInvariant::check(&ctx);
        assert!(v.iter().any(|x| x.rule == "INV-2"));
    }

    #[test]
    fn consolidation_requires_witness_phase() {
        let ctx = StewardshipContext {
            in_emergency_rest: false,
            healthy_lobe_count: 2,
            current_phase_index: 2, // Synthesize, not Witness
            proposed_action: StewardshipAction::Consolidate,
            goal_id: String::new(),
        };
        let v = StewardshipInvariant::check(&ctx);
        assert!(v.iter().any(|x| x.rule == "INV-3"));
    }

    #[test]
    fn valid_broadcast_passes_all_checks() {
        let ctx = StewardshipContext {
            in_emergency_rest: false,
            healthy_lobe_count: 5,
            current_phase_index: 3,
            proposed_action: StewardshipAction::Broadcast,
            goal_id: "goal-42".into(),
        };
        let v = StewardshipInvariant::check(&ctx);
        assert!(v.is_empty());
    }
}
