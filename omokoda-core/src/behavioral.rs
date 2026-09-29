use serde::{Deserialize, Serialize};
use std::path::Path;

/// Per-session behavioral tracking — records outcomes across `think` and `act` primitives.
/// Powers reputation tier advancement and IRIS routing overrides.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BehavioralState {
    /// Number of `think` turns completed this session
    pub think_count: u32,
    /// `act` calls that completed without error or hermetic block
    pub successful_acts: u32,
    /// `act` calls that returned a tool error
    pub failed_acts: u32,
    /// `act` calls blocked by the hermetic gate or permission policy
    pub blocked_acts: u32,
    /// `think` turns where the hermetic gate issued a warning
    pub think_warnings: u32,
}

impl BehavioralState {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Call after every completed `think` primitive.
    pub fn record_think(&mut self) {
        self.think_count += 1;
    }

    /// Call after a `think` that triggered a hermetic warning.
    pub fn record_think_warning(&mut self) {
        self.think_count += 1;
        self.think_warnings += 1;
    }

    /// Call after a successful `act` (tool returned without error).
    pub fn record_act_success(&mut self) {
        self.successful_acts += 1;
    }

    /// Call after an `act` that returned a tool error.
    pub fn record_act_failure(&mut self) {
        self.failed_acts += 1;
    }

    /// Call after an `act` blocked by the hermetic gate or policy.
    pub fn record_blocked(&mut self) {
        self.blocked_acts += 1;
    }

    /// Total `act` attempts (success + failure + blocked).
    #[must_use]
    pub fn total_acts(&self) -> u32 {
        self.successful_acts + self.failed_acts + self.blocked_acts
    }

    /// Fraction of acts that completed successfully (0.0 if none attempted).
    #[must_use]
    pub fn success_rate(&self) -> f32 {
        let total = self.total_acts();
        if total == 0 {
            return 1.0; // No acts yet — benefit of the doubt
        }
        self.successful_acts as f32 / total as f32
    }

    /// True when the block rate is high enough to warrant a permission review.
    /// Threshold: >20% of acts are blocked.
    #[must_use]
    pub fn is_flagged(&self) -> bool {
        let total = self.total_acts();
        if total < 5 {
            return false;
        }
        (self.blocked_acts as f32 / total as f32) > 0.20
    }

    /// Reputation delta to apply to the `Session.reputation` field at session end.
    /// Successful behavior increases reputation; blocked/failed behavior decreases it.
    #[must_use]
    pub fn reputation_delta(&self) -> f64 {
        let success_contribution = self.successful_acts as f64 * 0.5;
        let failure_penalty = self.failed_acts as f64 * 0.25;
        let block_penalty = self.blocked_acts as f64 * 1.0;
        let warning_penalty = self.think_warnings as f64 * 0.1;
        success_contribution - failure_penalty - block_penalty - warning_penalty
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fresh_state_has_full_success_rate() {
        let state = BehavioralState::new();
        assert_eq!(state.success_rate(), 1.0);
        assert_eq!(state.total_acts(), 0);
    }

    #[test]
    fn success_rate_tracks_correctly() {
        let mut state = BehavioralState::new();
        state.record_act_success();
        state.record_act_success();
        state.record_act_failure();
        assert!((state.success_rate() - 2.0 / 3.0).abs() < 0.001);
    }

    #[test]
    fn flagged_when_block_rate_exceeds_threshold() {
        let mut state = BehavioralState::new();
        for _ in 0..4 {
            state.record_act_success();
        }
        for _ in 0..2 {
            state.record_blocked();
        }
        assert!(state.is_flagged());
    }

    #[test]
    fn not_flagged_below_five_acts() {
        let mut state = BehavioralState::new();
        state.record_blocked();
        state.record_blocked();
        assert!(!state.is_flagged(), "too few acts to flag");
    }

    #[test]
    fn reputation_delta_positive_for_clean_behavior() {
        let mut state = BehavioralState::new();
        for _ in 0..10 {
            state.record_act_success();
            state.record_think();
        }
        assert!(state.reputation_delta() > 0.0);
    }

    #[test]
    fn reputation_delta_negative_for_blocked_behavior() {
        let mut state = BehavioralState::new();
        for _ in 0..5 {
            state.record_blocked();
        }
        assert!(state.reputation_delta() < 0.0);
    }

    #[test]
    fn think_count_increments() {
        let mut state = BehavioralState::new();
        state.record_think();
        state.record_think();
        assert_eq!(state.think_count, 2);
    }
}

// ──────────────────────────── BehavioralMemory ───────────────────────────────

/// Cross-session behavioral learning record.
/// Accumulates what works, what fails, user traits, and distilled core rules.
/// Inspired by Droidclaw's post-session learning loop.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BehavioralMemory {
    /// Approaches that produced good outcomes — add to these
    pub do_more: Vec<String>,
    /// Approaches that failed or frustrated — avoid these
    pub do_less: Vec<String>,
    /// Observed user traits and preferences
    pub user_traits: Vec<String>,
    /// Distilled rules from deep consolidation (every 5 sessions)
    pub core_rules: Vec<String>,
    pub session_count: u32,
}

impl BehavioralMemory {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Record session outcomes and accumulate what worked / what failed.
    /// Called at `soma_lifecycle::sleep()` time.
    pub fn record_session(&mut self, what_worked: Vec<String>, what_failed: Vec<String>) {
        for s in what_worked {
            dedup_cap(&mut self.do_more, s, 20);
        }
        for s in what_failed {
            dedup_cap(&mut self.do_less, s, 20);
        }
        self.session_count += 1;
    }

    /// Add a user trait observation.
    pub fn observe_trait(&mut self, trait_str: String) {
        dedup_cap(&mut self.user_traits, trait_str, 15);
    }

    /// Every 5 sessions: return a summary of the top rules to consolidate.
    /// Caller is responsible for passing these to an LLM for distillation;
    /// this method just returns the raw material and flags when it's time.
    #[must_use]
    pub fn consolidation_due(&self) -> bool {
        self.session_count > 0 && self.session_count.is_multiple_of(5)
    }

    /// Stub consolidation: promote top do_more items as core rules.
    /// In a live system, an LLM call would distill these properly.
    pub fn consolidate_stub(&mut self) {
        if !self.consolidation_due() {
            return;
        }
        for rule in self.do_more.iter().take(5) {
            dedup_cap(&mut self.core_rules, rule.clone(), 10);
        }
    }

    pub fn save(&self, path: &Path) -> Result<(), String> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| format!("create_dir: {}", e))?;
        }
        let json = serde_json::to_string_pretty(self).map_err(|e| format!("serialize: {}", e))?;
        std::fs::write(path, json).map_err(|e| format!("write: {}", e))
    }

    pub fn load(path: &Path) -> Result<Self, String> {
        let s = std::fs::read_to_string(path).map_err(|e| format!("read: {}", e))?;
        serde_json::from_str(&s).map_err(|e| format!("parse: {}", e))
    }

    pub fn load_or_new(path: &Path) -> Self {
        Self::load(path).unwrap_or_default()
    }
}

fn dedup_cap(vec: &mut Vec<String>, new: String, max: usize) {
    if vec.contains(&new) {
        return;
    }
    if vec.len() >= max {
        vec.remove(0);
    }
    vec.push(new);
}

#[cfg(test)]
mod behavioral_memory_tests {
    use super::*;

    #[test]
    fn record_session_accumulates() {
        let mut bm = BehavioralMemory::new();
        bm.record_session(
            vec!["give code examples".to_string()],
            vec!["over-explain theory".to_string()],
        );
        assert_eq!(bm.do_more, vec!["give code examples"]);
        assert_eq!(bm.do_less, vec!["over-explain theory"]);
        assert_eq!(bm.session_count, 1);
    }

    #[test]
    fn consolidation_triggers_at_5_sessions() {
        let mut bm = BehavioralMemory::new();
        bm.do_more.push("be concise".to_string());
        for _ in 0..5 {
            bm.record_session(vec![], vec![]);
        }
        assert!(bm.consolidation_due());
        bm.consolidate_stub();
        assert!(bm.core_rules.contains(&"be concise".to_string()));
    }

    #[test]
    fn dedup_prevents_repeated_entries() {
        let mut bm = BehavioralMemory::new();
        bm.record_session(vec!["same rule".to_string()], vec![]);
        bm.record_session(vec!["same rule".to_string()], vec![]);
        assert_eq!(bm.do_more.len(), 1);
    }
}
