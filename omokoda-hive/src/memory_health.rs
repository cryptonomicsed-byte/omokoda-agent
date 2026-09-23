// H6: Memory Health metric + GlyphNode lifecycle integration
//
// Memory Health is a [0.0, 1.0] score computed across the GlyphIndex.
// It drives the Consolidate phase: below threshold triggers REM consolidation.
//
// Components:
//   1. Recency: fraction of beliefs witnessed in last N ticks
//   2. Diversity: Shannon entropy across belief topics
//   3. Coherence: 1.0 - (cumulative_delta / 1.0) — how aligned sim is with real
//   4. Coverage: lobes with at least one recent proposal / 11
//
// Health = 0.35 × recency + 0.25 × diversity + 0.25 × coherence + 0.15 × coverage

use std::collections::HashMap;
use serde::{Deserialize, Serialize};
use crate::epistemic::EpistemicState;
use crate::lobe::OrisaLobe;

pub const RECENCY_WEIGHT: f64 = 0.35;
pub const DIVERSITY_WEIGHT: f64 = 0.25;
pub const COHERENCE_WEIGHT: f64 = 0.25;
pub const COVERAGE_WEIGHT: f64 = 0.15;

pub const CONSOLIDATION_TRIGGER_THRESHOLD: f64 = 0.45;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryHealthReport {
    pub tick: u64,
    pub score: f64,
    pub recency: f64,
    pub diversity: f64,
    pub coherence: f64,
    pub coverage: f64,
    pub requires_consolidation: bool,
}

/// Lobe activity summary for coverage calculation.
#[derive(Debug, Default)]
pub struct LobeActivitySummary {
    pub active_lobes: std::collections::HashSet<String>,
    pub total_proposals_this_cycle: usize,
}

pub struct MemoryHealthMonitor {
    pub recency_window_ticks: u64,
}

impl MemoryHealthMonitor {
    pub fn new(recency_window_ticks: u64) -> Self {
        Self { recency_window_ticks }
    }

    pub fn compute(
        &self,
        epistemic: &EpistemicState,
        lobe_activity: &LobeActivitySummary,
    ) -> MemoryHealthReport {
        let recency = self.compute_recency(epistemic);
        let diversity = self.compute_diversity(epistemic);
        let coherence = self.compute_coherence(epistemic);
        let coverage = self.compute_coverage(lobe_activity);

        let score = RECENCY_WEIGHT * recency
            + DIVERSITY_WEIGHT * diversity
            + COHERENCE_WEIGHT * coherence
            + COVERAGE_WEIGHT * coverage;

        MemoryHealthReport {
            tick: epistemic.tick,
            score,
            recency,
            diversity,
            coherence,
            coverage,
            requires_consolidation: score < CONSOLIDATION_TRIGGER_THRESHOLD,
        }
    }

    /// Fraction of beliefs witnessed within the recency window.
    fn compute_recency(&self, epistemic: &EpistemicState) -> f64 {
        if epistemic.beliefs.is_empty() {
            return 1.0; // empty is fresh
        }
        let window_start = epistemic.tick.saturating_sub(self.recency_window_ticks);
        let recent = epistemic
            .beliefs
            .values()
            .filter(|b| b.tick_born >= window_start)
            .count();
        recent as f64 / epistemic.beliefs.len() as f64
    }

    /// Shannon entropy over belief topics (normalized to [0, 1]).
    fn compute_diversity(&self, epistemic: &EpistemicState) -> f64 {
        if epistemic.beliefs.len() <= 1 {
            return if epistemic.beliefs.is_empty() { 0.0 } else { 1.0 };
        }
        let n = epistemic.beliefs.len() as f64;
        // Confidence as proxy for probability mass
        let total_conf: f64 = epistemic.beliefs.values().map(|b| b.confidence).sum();
        if total_conf == 0.0 {
            return 0.0;
        }
        let entropy: f64 = epistemic
            .beliefs
            .values()
            .map(|b| {
                let p = b.confidence / total_conf;
                if p > 0.0 { -p * p.ln() } else { 0.0 }
            })
            .sum();
        let max_entropy = n.ln();
        if max_entropy == 0.0 { 1.0 } else { (entropy / max_entropy).min(1.0) }
    }

    /// 1.0 - normalized cumulative_delta.
    fn compute_coherence(&self, epistemic: &EpistemicState) -> f64 {
        (1.0 - epistemic.cumulative_delta).max(0.0).min(1.0)
    }

    /// Fraction of 11 Ọrìṣà lobes with recent activity.
    fn compute_coverage(&self, activity: &LobeActivitySummary) -> f64 {
        let total_lobes = OrisaLobe::all().len() as f64;
        activity.active_lobes.len() as f64 / total_lobes
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::epistemic::BeliefSource;

    fn monitor() -> MemoryHealthMonitor {
        MemoryHealthMonitor::new(20)
    }

    #[test]
    fn empty_epistemic_is_healthy() {
        let state = EpistemicState::new();
        let activity = LobeActivitySummary::default();
        let report = monitor().compute(&state, &activity);
        // coherence = 1.0, recency = 1.0, diversity = 0.0, coverage = 0.0
        assert!(report.score > 0.4); // still above consolidation trigger
    }

    #[test]
    fn high_delta_degrades_health() {
        let mut state = EpistemicState::new();
        state.cumulative_delta = 0.9;
        let activity = LobeActivitySummary::default();
        let report = monitor().compute(&state, &activity);
        assert!(report.coherence < 0.15);
        assert!(report.score < 0.7);
    }

    #[test]
    fn full_lobe_coverage_boosts_score() {
        let state = EpistemicState::new();
        let mut activity = LobeActivitySummary::default();
        for lobe in OrisaLobe::all() {
            activity.active_lobes.insert(lobe.name().to_string());
        }
        let report = monitor().compute(&state, &activity);
        assert_eq!(report.coverage, 1.0);
    }

    #[test]
    fn low_health_triggers_consolidation() {
        let mut state = EpistemicState::new();
        state.cumulative_delta = 0.99;
        let activity = LobeActivitySummary::default(); // no lobes active
        let report = monitor().compute(&state, &activity);
        assert!(report.requires_consolidation);
    }
}
