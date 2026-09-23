// H0: HiveBreath — the top-level coordinator driving RitualPhase ticks
//
// HiveBreath orchestrates the 7-phase cycle across all 11 lobe-agents.
// Each tick: check EpistemicDelta → validate stewardship → advance phase.
//
// Phase H3–H8 will extend this struct with:
//   H3: GoalVector synthesis via GoalGenesisEngine federation
//   H4: Twelve-thrones deliberation integration
//   H5: Cross-hive federation (DIP-based)
//   H6: Memory Health metric + GlyphNode lifecycle
//   H7: Ọ̀ṣọ́ language → hive directives compiler
//   H8: Full OSOVM simulation ↔ hive belief feedback loop

use std::collections::HashMap;
use serde::{Deserialize, Serialize};
use crate::{
    epistemic::{EpistemicDelta, EpistemicState},
    lobe::{HiveAction, LobeAgent, LobeStatus, OrisaLobe, TwelfthFace},
    ritual::{RitualPhase, RitualTransition},
    stewardship::{StewardshipAction, StewardshipContext, StewardshipInvariant},
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HiveBreathConfig {
    /// Ticks between forced consolidation regardless of delta
    pub consolidation_interval: u64,
    /// Ticks in rest phase per cycle
    pub rest_duration_ticks: u64,
    /// Whether to enforce TwelfthFace invariants (always true in production)
    pub enforce_twelfth_face: bool,
}

impl Default for HiveBreathConfig {
    fn default() -> Self {
        Self {
            consolidation_interval: 50,
            rest_duration_ticks: 3,
            enforce_twelfth_face: true,
        }
    }
}

/// The macro hive-mind coordinator.
pub struct HiveBreath {
    pub config: HiveBreathConfig,
    pub tick: u64,
    pub phase: RitualPhase,
    pub lobes: HashMap<String, LobeAgent>,
    pub epistemic: EpistemicState,
    pub history: Vec<RitualTransition>,
    pub rest_ticks_remaining: u64,
    pub in_emergency_rest: bool,
}

impl HiveBreath {
    pub fn new(config: HiveBreathConfig) -> Self {
        let mut lobes = HashMap::new();
        for orisa in OrisaLobe::all() {
            let agent = LobeAgent::new(orisa.clone());
            lobes.insert(orisa.name().to_string(), agent);
        }
        Self {
            config,
            tick: 0,
            phase: RitualPhase::Receive,
            lobes,
            epistemic: EpistemicState::new(),
            history: Vec::new(),
            rest_ticks_remaining: 0,
            in_emergency_rest: false,
        }
    }

    /// Drive one tick of the HiveBreath cycle.
    /// Returns a list of actions the hive wants to execute this tick.
    pub fn tick_once(&mut self) -> Vec<HiveAction> {
        self.tick += 1;
        self.epistemic.tick = self.tick;

        let mut actions = Vec::new();

        // Handle rest countdown
        if self.rest_ticks_remaining > 0 {
            self.rest_ticks_remaining -= 1;
            if self.rest_ticks_remaining == 0 {
                self.in_emergency_rest = false;
                self.advance_phase("rest_expired");
            }
            return actions;
        }

        // Compute epistemic delta
        let delta = EpistemicDelta::compute(&self.epistemic);

        // Emergency rest overrides normal cycle
        if delta.requires_emergency_rest && !self.in_emergency_rest {
            self.in_emergency_rest = true;
            self.rest_ticks_remaining = self.config.rest_duration_ticks * 3;
            self.phase = RitualPhase::Rest;
            actions.push(HiveAction::EnterRest {
                duration_ticks: self.rest_ticks_remaining,
            });
            return actions;
        }

        // Normal phase execution
        match &self.phase {
            RitualPhase::Receive => {
                // Lobe-agents report their intents here (external in H3+)
            }
            RitualPhase::Deliberate => {
                // Twelve-thrones deliberation (external in H4+)
            }
            RitualPhase::Synthesize => {
                // Goal vector synthesis (external in H3+)
            }
            RitualPhase::Broadcast => {
                let healthy = self.healthy_lobe_count();
                let ctx = StewardshipContext {
                    in_emergency_rest: self.in_emergency_rest,
                    healthy_lobe_count: healthy,
                    current_phase_index: self.phase.index(),
                    proposed_action: StewardshipAction::Broadcast,
                    goal_id: format!("goal-tick-{}", self.tick),
                };
                if StewardshipInvariant::check(&ctx).is_empty() {
                    actions.push(HiveAction::BroadcastGoal {
                        goal_id: format!("goal-tick-{}", self.tick),
                        goal_vector: serde_json::json!({ "tick": self.tick }),
                    });
                }
            }
            RitualPhase::Witness => {
                // Receipts arrive from Zàngbétò (external in H5+)
            }
            RitualPhase::Consolidate => {
                let ctx = StewardshipContext {
                    in_emergency_rest: self.in_emergency_rest,
                    healthy_lobe_count: self.healthy_lobe_count(),
                    current_phase_index: self.phase.index(),
                    proposed_action: StewardshipAction::Consolidate,
                    goal_id: String::new(),
                };
                if StewardshipInvariant::check(&ctx).is_empty() {
                    self.epistemic.consolidate();
                    actions.push(HiveAction::TriggerConsolidation);
                }
            }
            RitualPhase::Rest => {
                self.rest_ticks_remaining = self.config.rest_duration_ticks;
                actions.push(HiveAction::EnterRest {
                    duration_ticks: self.rest_ticks_remaining,
                });
            }
        }

        // Delta-forced consolidation
        if delta.requires_consolidation && self.phase == RitualPhase::Witness {
            self.epistemic.consolidate();
        }

        // Advance phase
        let trigger = format!("tick-{}", self.tick);
        self.advance_phase(&trigger);

        // TwelfthFace veto on all proposed actions
        if self.config.enforce_twelfth_face {
            actions.retain(|a| !TwelfthFace::violates(a));
        }

        actions
    }

    fn advance_phase(&mut self, trigger: &str) {
        let next = self.phase.advance();
        let transition = RitualTransition {
            from: self.phase.clone(),
            to: next.clone(),
            tick: self.tick,
            trigger: trigger.to_string(),
        };
        self.history.push(transition);
        self.phase = next;
    }

    pub fn healthy_lobe_count(&self) -> usize {
        self.lobes
            .values()
            .filter(|l| matches!(l.status, LobeStatus::Active | LobeStatus::Executing { .. }))
            .count()
    }

    /// Activate a lobe (called when a micro-agent confirms birth).
    pub fn activate_lobe(&mut self, orisa: &OrisaLobe, agent_id: &str) {
        if let Some(lobe) = self.lobes.get_mut(orisa.name()) {
            lobe.agent_id = Some(agent_id.to_string());
            lobe.status = LobeStatus::Active;
            lobe.last_heartbeat_tick = self.tick;
        }
    }

    /// Record a heartbeat from a lobe-agent.
    pub fn lobe_heartbeat(&mut self, orisa: &OrisaLobe) {
        if let Some(lobe) = self.lobes.get_mut(orisa.name()) {
            lobe.last_heartbeat_tick = self.tick;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hive_initializes_with_eleven_lobes() {
        let hive = HiveBreath::new(HiveBreathConfig::default());
        assert_eq!(hive.lobes.len(), 11);
        assert_eq!(hive.phase, RitualPhase::Receive);
    }

    #[test]
    fn single_tick_advances_phase() {
        let mut hive = HiveBreath::new(HiveBreathConfig::default());
        hive.tick_once();
        assert_eq!(hive.phase, RitualPhase::Deliberate);
    }

    #[test]
    fn seven_ticks_completes_one_cycle() {
        let mut hive = HiveBreath::new(HiveBreathConfig {
            rest_duration_ticks: 0,
            ..Default::default()
        });
        for _ in 0..7 {
            hive.tick_once();
        }
        assert_eq!(hive.phase, RitualPhase::Receive);
    }

    #[test]
    fn emergency_rest_halts_cycle() {
        let mut hive = HiveBreath::new(HiveBreathConfig::default());
        // Force emergency rest by pushing delta over threshold
        hive.epistemic.cumulative_delta = 0.99;
        hive.tick_once();
        assert!(hive.in_emergency_rest);
        assert_eq!(hive.phase, RitualPhase::Rest);
    }

    #[test]
    fn activate_lobe_marks_healthy() {
        let mut hive = HiveBreath::new(HiveBreathConfig::default());
        hive.activate_lobe(&OrisaLobe::Obatala, "agent-001");
        assert_eq!(hive.healthy_lobe_count(), 1);
    }

    #[test]
    fn history_grows_with_ticks() {
        let mut hive = HiveBreath::new(HiveBreathConfig::default());
        hive.tick_once();
        hive.tick_once();
        assert_eq!(hive.history.len(), 2);
    }
}
