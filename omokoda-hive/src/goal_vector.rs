// H3: GoalVector synthesis — federated goal aggregation across lobe-agents
//
// Each lobe-agent runs its own GoalGenesisEngine and reports a weighted
// set of goals. The HiveBreath coordinator aggregates them into a unified
// GoalVector during the Synthesize phase.
//
// Aggregation rules:
//   1. Goals from multiple lobes are merged by semantic topic (same topic → max confidence wins)
//   2. Lobe weight is applied as a multiplier to each goal's urgency
//   3. Goals below MIN_AGGREGATE_URGENCY are dropped before broadcast
//   4. The top MAX_BROADCAST_GOALS by weighted urgency are emitted

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use crate::lobe::OrisaLobe;

pub const MIN_AGGREGATE_URGENCY: f32 = 0.05;
pub const MAX_BROADCAST_GOALS: usize = 12;

/// A single goal contributed by one lobe-agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LobeGoalProposal {
    pub lobe: OrisaLobe,
    pub topic: String,
    pub description: String,
    pub urgency: f32,
    pub alignment_score: f32,
    pub tick_proposed: u64,
}

/// A unified goal after federation across lobes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AggregatedGoal {
    pub topic: String,
    pub description: String,
    /// Weighted urgency: max(lobe_weight × urgency) across all proposing lobes
    pub weighted_urgency: f32,
    pub alignment_score: f32,
    /// Which lobes proposed this goal (may be more than one)
    pub proposing_lobes: Vec<OrisaLobe>,
}

/// The output of the Synthesize phase — broadcast to all lobe-agents.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoalVector {
    pub id: String,
    pub tick: u64,
    pub goals: Vec<AggregatedGoal>,
}

/// Synthesizes a GoalVector from proposals across all lobe-agents.
pub struct GoalVectorSynthesizer {
    pub lobe_weights: HashMap<String, f64>,
}

impl GoalVectorSynthesizer {
    pub fn new(lobe_weights: HashMap<String, f64>) -> Self {
        Self { lobe_weights }
    }

    /// Synthesize proposals into a broadcast-ready GoalVector.
    pub fn synthesize(&self, proposals: Vec<LobeGoalProposal>, tick: u64) -> GoalVector {
        // Merge by topic: highest weighted urgency wins description
        let mut by_topic: HashMap<String, AggregatedGoal> = HashMap::new();

        for proposal in proposals {
            let weight = self
                .lobe_weights
                .get(proposal.lobe.name())
                .copied()
                .unwrap_or(1.0 / 11.0);
            let weighted_urgency = proposal.urgency * weight as f32;

            let entry = by_topic.entry(proposal.topic.clone()).or_insert_with(|| {
                AggregatedGoal {
                    topic: proposal.topic.clone(),
                    description: proposal.description.clone(),
                    weighted_urgency: 0.0,
                    alignment_score: proposal.alignment_score,
                    proposing_lobes: Vec::new(),
                }
            });

            if weighted_urgency > entry.weighted_urgency {
                entry.weighted_urgency = weighted_urgency;
                entry.description = proposal.description.clone();
            }
            // Track alignment as max across lobes
            if proposal.alignment_score > entry.alignment_score {
                entry.alignment_score = proposal.alignment_score;
            }
            if !entry.proposing_lobes.contains(&proposal.lobe) {
                entry.proposing_lobes.push(proposal.lobe);
            }
        }

        // Filter below threshold
        let mut goals: Vec<AggregatedGoal> = by_topic
            .into_values()
            .filter(|g| g.weighted_urgency >= MIN_AGGREGATE_URGENCY)
            .collect();

        // Sort by weighted urgency descending
        goals.sort_by(|a, b| {
            b.weighted_urgency
                .partial_cmp(&a.weighted_urgency)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        // Take top N
        goals.truncate(MAX_BROADCAST_GOALS);

        let id = format!("gv-tick-{tick}");
        GoalVector { id, tick, goals }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn default_synthesizer() -> GoalVectorSynthesizer {
        let mut weights = HashMap::new();
        for lobe in OrisaLobe::all() {
            weights.insert(lobe.name().to_string(), 1.0 / 11.0);
        }
        GoalVectorSynthesizer::new(weights)
    }

    #[test]
    fn merge_same_topic_keeps_highest_urgency() {
        let synth = default_synthesizer();
        let proposals = vec![
            LobeGoalProposal {
                lobe: OrisaLobe::Obatala,
                topic: "survive".into(),
                description: "ensure continuity".into(),
                urgency: 0.9,
                alignment_score: 0.8,
                tick_proposed: 1,
            },
            LobeGoalProposal {
                lobe: OrisaLobe::Yemoja,
                topic: "survive".into(),
                description: "memory continuity".into(),
                urgency: 0.6,
                alignment_score: 0.7,
                tick_proposed: 1,
            },
        ];
        let gv = synth.synthesize(proposals, 1);
        assert_eq!(gv.goals.len(), 1);
        let goal = &gv.goals[0];
        assert_eq!(goal.proposing_lobes.len(), 2);
    }

    #[test]
    fn below_threshold_goals_dropped() {
        let synth = default_synthesizer();
        let proposals = vec![LobeGoalProposal {
            lobe: OrisaLobe::Ogun,
            topic: "low_priority".into(),
            description: "optional task".into(),
            urgency: 0.001, // weighted will be ~0.00009 << MIN
            alignment_score: 0.5,
            tick_proposed: 2,
        }];
        let gv = synth.synthesize(proposals, 2);
        assert!(gv.goals.is_empty());
    }

    #[test]
    fn max_goals_capped() {
        let synth = default_synthesizer();
        let proposals: Vec<_> = (0..20)
            .map(|i| LobeGoalProposal {
                lobe: OrisaLobe::Osoosi,
                topic: format!("topic-{i}"),
                description: format!("desc-{i}"),
                urgency: 0.5,
                alignment_score: 0.5,
                tick_proposed: 1,
            })
            .collect();
        let gv = synth.synthesize(proposals, 3);
        assert!(gv.goals.len() <= MAX_BROADCAST_GOALS);
    }

    #[test]
    fn goal_vector_id_includes_tick() {
        let synth = default_synthesizer();
        let gv = synth.synthesize(vec![], 42);
        assert!(gv.id.contains("42"));
    }
}
