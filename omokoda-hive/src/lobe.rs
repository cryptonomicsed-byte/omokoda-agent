// H0: 11 Ọrìṣà lobe-agents + Twelfth Face invariant
//
// Each lobe corresponds to one of the 11 Ọrìṣà seats in the hive coordinator.
// The Twelfth Face is NOT an agent — it is a constitutional invariant that
// enforces the sovereignty boundary. No lobe can override it.

use serde::{Deserialize, Serialize};

/// The 11 Ọrìṣà lobe identities (Twelfth Face is an invariant, not a lobe).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum OrisaLobe {
    Obatala,   // Purity, wisdom, long-range planning
    Ogun,      // Execution, force, tool use
    Shango,    // Justice, enforcement, receipts
    Yemoja,    // Memory, continuity, GlyphIndex
    Oshun,     // Creativity, content, narrative
    Eshu,      // Communication, routing, DIP
    Orunmila,  // Prophecy, goal genesis, simulation
    Oduduwa,   // Governance, council, proposals
    Oya,       // Transformation, mutation, adaptation
    Osoosi,    // Exploration, search, pattern mining
    Sango,     // [Alias seat for dynamic allocation]
}

impl OrisaLobe {
    pub fn all() -> &'static [OrisaLobe] {
        use OrisaLobe::*;
        &[Obatala, Ogun, Shango, Yemoja, Oshun, Eshu, Orunmila, Oduduwa, Oya, Osoosi, Sango]
    }

    pub fn name(&self) -> &'static str {
        match self {
            Self::Obatala => "Obàtálá",
            Self::Ogun => "Ògún",
            Self::Shango => "Ṣàngó",
            Self::Yemoja => "Yemọja",
            Self::Oshun => "Ọṣun",
            Self::Eshu => "Èṣù",
            Self::Orunmila => "Ọrunmìlà",
            Self::Oduduwa => "Odùduwà",
            Self::Oya => "Ọya",
            Self::Osoosi => "Ọṣọọ̀ṣì",
            Self::Sango => "Àṣà", // dynamic seat
        }
    }

    /// Primary domain of this lobe in the collective.
    pub fn domain(&self) -> &'static str {
        match self {
            Self::Obatala => "planning",
            Self::Ogun => "execution",
            Self::Shango => "enforcement",
            Self::Yemoja => "memory",
            Self::Oshun => "creativity",
            Self::Eshu => "communication",
            Self::Orunmila => "prophecy",
            Self::Oduduwa => "governance",
            Self::Oya => "transformation",
            Self::Osoosi => "exploration",
            Self::Sango => "dynamic",
        }
    }
}

/// A lobe-agent in the hive — wraps a micro omokoda-agent with its lobe role.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LobeAgent {
    pub lobe: OrisaLobe,
    /// The agent ID of the underlying micro-agent (may be None until spawned)
    pub agent_id: Option<String>,
    pub status: LobeStatus,
    pub last_heartbeat_tick: u64,
    /// Contribution weight to collective GoalVector (0.0–1.0)
    pub weight: f64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum LobeStatus {
    Dormant,
    Active,
    Executing { task_id: String },
    Failed { reason: String },
}

impl LobeAgent {
    pub fn new(lobe: OrisaLobe) -> Self {
        Self {
            agent_id: None,
            status: LobeStatus::Dormant,
            last_heartbeat_tick: 0,
            weight: 1.0 / OrisaLobe::all().len() as f64,
            lobe,
        }
    }

    pub fn is_healthy(&self, current_tick: u64) -> bool {
        matches!(self.status, LobeStatus::Active | LobeStatus::Executing { .. })
            && current_tick.saturating_sub(self.last_heartbeat_tick) < 10
    }
}

/// Twelfth Face — constitutional invariant, not a lobe-agent.
/// Enforces the sovereignty boundary: no hive decision may violate these constraints.
pub struct TwelfthFace;

impl TwelfthFace {
    /// Returns true if a proposed hive action violates constitutional invariants.
    pub fn violates(action: &HiveAction) -> bool {
        match action {
            // No agent may be archived without its consent receipt
            HiveAction::ArchiveAgent { consent_receipt, .. } => consent_receipt.is_none(),
            // No lobe may be granted write access to another lobe's memory
            HiveAction::GrantCrossLobeWrite { .. } => true,
            // Resource requisition must not exceed tier cap
            HiveAction::RequisitionResource { amount, tier_cap, .. } => amount > tier_cap,
            _ => false,
        }
    }
}

/// Actions the hive coordinator may propose to lobe-agents.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum HiveAction {
    SpawnLobe { lobe: OrisaLobe, config: serde_json::Value },
    ArchiveAgent { agent_id: String, consent_receipt: Option<String> },
    BroadcastGoal { goal_id: String, goal_vector: serde_json::Value },
    GrantCrossLobeWrite { from: OrisaLobe, to: OrisaLobe },
    RequisitionResource { resource: String, amount: f64, tier_cap: f64 },
    TriggerConsolidation,
    EnterRest { duration_ticks: u64 },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eleven_lobes_defined() {
        assert_eq!(OrisaLobe::all().len(), 11);
    }

    #[test]
    fn twelfth_face_blocks_archival_without_consent() {
        let action = HiveAction::ArchiveAgent {
            agent_id: "agent-1".into(),
            consent_receipt: None,
        };
        assert!(TwelfthFace::violates(&action));
    }

    #[test]
    fn twelfth_face_allows_archival_with_consent() {
        let action = HiveAction::ArchiveAgent {
            agent_id: "agent-1".into(),
            consent_receipt: Some("receipt-xyz".into()),
        };
        assert!(!TwelfthFace::violates(&action));
    }

    #[test]
    fn twelfth_face_blocks_cross_lobe_write() {
        let action = HiveAction::GrantCrossLobeWrite {
            from: OrisaLobe::Obatala,
            to: OrisaLobe::Yemoja,
        };
        assert!(TwelfthFace::violates(&action));
    }

    #[test]
    fn twelfth_face_blocks_over_budget() {
        let action = HiveAction::RequisitionResource {
            resource: "gpu".into(),
            amount: 100.0,
            tier_cap: 50.0,
        };
        assert!(TwelfthFace::violates(&action));
    }
}
