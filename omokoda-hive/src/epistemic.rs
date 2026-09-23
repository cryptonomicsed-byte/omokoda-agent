// H2: EpistemicDelta — divergence tracking between sim-belief and real-evidence
//
// EpistemicDelta is the core metric of the DNA double-helix architecture:
//   Axis A (OSOVM sim strand) produces simulated belief
//   Axis B (Omo-Koda real strand) produces witnessed evidence
//   EpistemicDelta = ||belief - evidence|| across the 65,536 divergence space
//
// A healthy hive keeps EpistemicDelta < 0.3. Above 0.7 triggers consolidation.
// Above 0.95 triggers an emergency Rest phase to prevent belief drift.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A single belief claim with confidence and source.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Belief {
    pub topic: String,
    pub confidence: f64,
    pub source: BeliefSource,
    pub tick_born: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BeliefSource {
    /// Produced by OSOVM simulation
    Simulation { run_id: String },
    /// Attested by Zàngbétò receipt
    Witnessed { receipt_id: String },
    /// Synthesized by Twelve-thrones deliberation
    Deliberation { throne_ids: Vec<String> },
    /// Inherited from prior hive cycle
    Consolidated { consolidation_tick: u64 },
}

/// The live epistemological state of the hive.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct EpistemicState {
    /// Beliefs keyed by topic hash
    pub beliefs: HashMap<String, Belief>,
    /// Current tick
    pub tick: u64,
    /// Cumulative delta since last consolidation
    pub cumulative_delta: f64,
}

impl EpistemicState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Upsert a belief, returning the delta introduced by this update.
    pub fn update_belief(&mut self, topic: &str, confidence: f64, source: BeliefSource) -> f64 {
        let prior_confidence = self.beliefs.get(topic).map(|b| b.confidence).unwrap_or(0.0);
        let delta = (confidence - prior_confidence).abs();
        self.beliefs.insert(
            topic.to_string(),
            Belief {
                topic: topic.to_string(),
                confidence: confidence.clamp(0.0, 1.0),
                source,
                tick_born: self.tick,
            },
        );
        self.cumulative_delta += delta;
        delta
    }

    /// Mark a belief as witnessed (real evidence), potentially reducing delta.
    pub fn witness_belief(&mut self, topic: &str, receipt_id: &str) {
        if let Some(belief) = self.beliefs.get_mut(topic) {
            let old_conf = belief.confidence;
            // Witnessed beliefs gain confidence toward 1.0 (evidence confirms sim)
            belief.confidence = (belief.confidence + 0.1).min(1.0);
            belief.source = BeliefSource::Witnessed {
                receipt_id: receipt_id.to_string(),
            };
            let gained = belief.confidence - old_conf;
            // Delta decreases when simulation and reality converge
            self.cumulative_delta = (self.cumulative_delta - gained).max(0.0);
        }
    }

    /// Decay old unwitnessed beliefs (called during Consolidate phase).
    pub fn consolidate(&mut self) {
        let current_tick = self.tick;
        self.beliefs.retain(|_, belief| {
            let age = current_tick.saturating_sub(belief.tick_born);
            // Archive unwitnessed beliefs older than 100 ticks
            if age > 100 {
                if matches!(belief.source, BeliefSource::Simulation { .. }) {
                    return false;
                }
            }
            true
        });
        self.cumulative_delta *= 0.9; // 10% decay per consolidation
    }
}

/// The delta between simulation belief and real evidence, per topic.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EpistemicDelta {
    pub tick: u64,
    pub total: f64,
    pub by_topic: HashMap<String, f64>,
    pub requires_consolidation: bool,
    pub requires_emergency_rest: bool,
}

impl EpistemicDelta {
    pub const CONSOLIDATION_THRESHOLD: f64 = 0.7;
    pub const EMERGENCY_REST_THRESHOLD: f64 = 0.95;

    pub fn compute(state: &EpistemicState) -> Self {
        let total = state.cumulative_delta;
        let mut by_topic = HashMap::new();

        for (topic, belief) in &state.beliefs {
            let sim_confidence = if matches!(belief.source, BeliefSource::Simulation { .. }) {
                belief.confidence
            } else {
                0.0
            };
            let witnessed_confidence = if matches!(belief.source, BeliefSource::Witnessed { .. }) {
                belief.confidence
            } else {
                0.0
            };
            let topic_delta = (sim_confidence - witnessed_confidence).abs();
            if topic_delta > 0.01 {
                by_topic.insert(topic.clone(), topic_delta);
            }
        }

        Self {
            tick: state.tick,
            total,
            by_topic,
            requires_consolidation: total >= Self::CONSOLIDATION_THRESHOLD,
            requires_emergency_rest: total >= Self::EMERGENCY_REST_THRESHOLD,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn belief_update_accumulates_delta() {
        let mut state = EpistemicState::new();
        let delta = state.update_belief("agent_count", 0.8, BeliefSource::Simulation {
            run_id: "run-1".into(),
        });
        assert!(delta > 0.0);
        assert!(state.cumulative_delta > 0.0);
    }

    #[test]
    fn witnessing_reduces_delta() {
        let mut state = EpistemicState::new();
        state.update_belief("agent_count", 0.5, BeliefSource::Simulation { run_id: "r".into() });
        let before = state.cumulative_delta;
        state.witness_belief("agent_count", "receipt-001");
        assert!(state.cumulative_delta <= before);
    }

    #[test]
    fn consolidation_decays_delta() {
        let mut state = EpistemicState::new();
        state.cumulative_delta = 0.8;
        state.consolidate();
        assert!(state.cumulative_delta < 0.8);
    }

    #[test]
    fn delta_thresholds_trigger_correctly() {
        let mut state = EpistemicState::new();
        state.cumulative_delta = 0.75;
        let delta = EpistemicDelta::compute(&state);
        assert!(delta.requires_consolidation);
        assert!(!delta.requires_emergency_rest);

        state.cumulative_delta = 0.96;
        let delta2 = EpistemicDelta::compute(&state);
        assert!(delta2.requires_emergency_rest);
    }
}
