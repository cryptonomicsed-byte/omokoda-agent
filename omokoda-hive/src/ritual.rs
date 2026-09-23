// H1: RitualPhase state machine
//
// The 7-state cycle that every HiveBreath tick traverses.
// Phase order: Receive → Deliberate → Synthesize → Broadcast → Witness → Consolidate → Rest
// Transitions are deterministic — no phase can be skipped.

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RitualPhase {
    /// Collecting CollectiveIntent signals from lobe-agents and outer world
    Receive,
    /// Twelve-thrones epistemology court deliberating over intents
    Deliberate,
    /// Hive coordinator synthesizing a unified GoalVector
    Synthesize,
    /// Broadcasting GoalVector to all lobe-agents as tasks
    Broadcast,
    /// Lobe-agents execute; hive witnesses receipts from Zàngbétò
    Witness,
    /// REM-style memory consolidation: GlyphNodes promoted/archived
    Consolidate,
    /// Quiescent period; decay and economics settle
    Rest,
}

#[derive(Debug, Error)]
pub enum TransitionError {
    #[error("illegal phase transition: {from:?} → {to:?}")]
    IllegalTransition { from: RitualPhase, to: RitualPhase },
}

/// Describes a completed phase transition with evidence.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RitualTransition {
    pub from: RitualPhase,
    pub to: RitualPhase,
    pub tick: u64,
    pub trigger: String,
}

impl RitualPhase {
    /// Advance to the next phase in the canonical cycle.
    pub fn advance(&self) -> Self {
        match self {
            Self::Receive => Self::Deliberate,
            Self::Deliberate => Self::Synthesize,
            Self::Synthesize => Self::Broadcast,
            Self::Broadcast => Self::Witness,
            Self::Witness => Self::Consolidate,
            Self::Consolidate => Self::Rest,
            Self::Rest => Self::Receive,
        }
    }

    /// Validate that a transition is legal (must follow advance() order).
    pub fn validate_transition(&self, to: &RitualPhase) -> Result<(), TransitionError> {
        if &self.advance() == to {
            Ok(())
        } else {
            Err(TransitionError::IllegalTransition {
                from: self.clone(),
                to: to.clone(),
            })
        }
    }

    pub fn index(&self) -> u8 {
        match self {
            Self::Receive => 0,
            Self::Deliberate => 1,
            Self::Synthesize => 2,
            Self::Broadcast => 3,
            Self::Witness => 4,
            Self::Consolidate => 5,
            Self::Rest => 6,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn phase_cycle_is_complete() {
        let mut phase = RitualPhase::Receive;
        let start = phase.index();
        for _ in 0..7 {
            phase = phase.advance();
        }
        assert_eq!(phase.index(), start, "7 advances must return to Receive");
    }

    #[test]
    fn validate_transition_accepts_legal() {
        assert!(RitualPhase::Receive.validate_transition(&RitualPhase::Deliberate).is_ok());
        assert!(RitualPhase::Rest.validate_transition(&RitualPhase::Receive).is_ok());
    }

    #[test]
    fn validate_transition_rejects_skip() {
        assert!(RitualPhase::Receive.validate_transition(&RitualPhase::Synthesize).is_err());
        assert!(RitualPhase::Witness.validate_transition(&RitualPhase::Rest).is_err());
    }
}
