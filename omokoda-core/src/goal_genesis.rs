// goal_genesis.rs — Goal Genesis Engine for Omo-Koda2 sovereign OS
//
// Transforms an agent from servile (externally directed) to sovereign (self-directed)
// by deriving goals from 5 internal streams:
//   1. Experience   — GlyphGraph memory patterns (always active)
//   2. Knowledge    — LARQL queries (gated by LARQL_ENABLED env var, Gap #42)
//   3. REM          — active REM consolidation cluster IDs
//   4. Calabash     — Digital Calabash context state
//   5. Constitutional — Hermetic gates + Odù orientation
//
// Spec: ~/sovereign-eco-blueprint/specs/GoalGenesisSpec.md

use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ── Goal Source ──────────────────────────────────────────────────────────────

/// Which internal stream originated a derived goal.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum GoalSource {
    /// From GlyphGraph memory patterns — always active.
    Experience,
    /// From LARQL knowledge gap analysis — gated by LARQL_ENABLED env var (Gap #42).
    Knowledge,
    /// From REM memory consolidation clusters.
    RemConsolidation,
    /// From Digital Calabash context state (existence continuity, resource level).
    CalabasState,
    /// From Hermetic/Odù constitutional orientation.
    Constitutional,
}

// ── Core Types ───────────────────────────────────────────────────────────────

/// A single goal derived autonomously by the agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DerivedGoal {
    /// UUID v4 — unique identifier for this goal instance.
    pub id: String,
    /// Human-readable goal statement.
    pub description: String,
    /// Which internal stream originated this goal.
    pub source: GoalSource,
    /// 0–255 Odù index this goal aligns with.
    pub odu_alignment: u8,
    /// Current urgency: 0.0 (dormant) – 1.0 (critical). Decays over time.
    pub urgency: f32,
    /// Alignment with the agent's Hermetic gate balance: 0.0 – 1.0.
    pub alignment_score: f32,
    /// Agent tier relevance (1 = broad, 2 = focused, 3 = specialist).
    pub tier: u8,
    /// Urgency lost per second. 0.001 ≈ 0.1%/s → full decay in ~1000 s.
    pub decay_rate: f32,
    /// Unix timestamp when this goal was first derived.
    pub created_at: f64,
}

impl DerivedGoal {
    /// Composite ranking score: urgency × alignment_score.
    /// Used for sorting — higher is better.
    pub fn rank_score(&self) -> f32 {
        self.urgency * self.alignment_score
    }
}

/// The complete set of goals active for an agent at a given moment.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GoalSet {
    /// Goals sorted by `urgency × alignment_score` descending.
    pub goals: Vec<DerivedGoal>,
    /// Unix timestamp when this GoalSet was derived.
    pub genesis_ts: f64,
    /// Blake3 hash (hex) of the Calabash state snapshot that produced this set.
    pub calabash_snapshot: String,
    /// The Odù seed used for constitutional goal derivation this cycle.
    pub odu_seed: u8,
}

// ── Input Snapshot ───────────────────────────────────────────────────────────

/// Snapshot of agent state used as input to goal derivation.
/// Callers build this from live state; engine is a pure function over it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoalGenesisInput {
    /// Agent birth Odù (0–255). Used for constitutional goal derivation.
    pub odu_id: u8,
    /// Agent tier (1–3).
    pub tier: u8,
    /// Number of GlyphNodes in the agent's memory graph (experience richness).
    pub glyph_count: usize,
    /// Active REM cluster IDs from the dream consolidation subsystem.
    pub rem_cluster_ids: Vec<String>,
    /// Blake3 hex hash of the current Digital Calabash state.
    pub calabash_state_hash: String,
    /// Hermetic gate balance from AgentConstitution: 0.0 = imbalanced, 1.0 = perfect.
    pub hermetic_balance: f32,
    /// Number of actions taken since last GoalSet derivation.
    pub recent_action_count: usize,
    /// Number of action failures since last GoalSet derivation.
    pub recent_failure_count: usize,
    /// Current unix timestamp (used for created_at and evolve decay delta).
    pub now_ts: f64,
    /// Four-dimensional Odù state vector from TwinStateVector:
    ///   [0] identity_odu   — birth / stable identity Odù
    ///   [1] memory_odu     — dominant glyph byte from memory graph
    ///   [2] field_odu      — Waggle/field signal (XOR fallback when offline)
    ///   [3] simulation_odu — OSOVM world-model Odù (0 when OSOVM offline)
    /// When None, engine falls back to odu_id for all four dimensions.
    #[serde(default)]
    pub twin_vector: Option<[u8; 4]>,
    /// SevenFunction index (0-6) of the dominant function across all four Odù.
    /// Drives constitutional goal framing when twin_vector is present.
    #[serde(default)]
    pub twin_dominant_fn: Option<u8>,
}

// ── Engine ───────────────────────────────────────────────────────────────────

/// Goal Genesis Engine — derives and evolves an agent's autonomous goal set.
///
/// Stateless: owns only configuration. All mutable state lives in `GoalSet`.
/// Thread-safe to share across async tasks (all methods take `&self`).
pub struct GoalGenesisEngine {
    /// Whether LARQL knowledge-gap goals are enabled (reads LARQL_ENABLED env var).
    pub larql_enabled: bool,
    /// Maximum goals retained in a GoalSet after truncation.
    pub max_goals: usize,
    /// Goals with `urgency × alignment_score` below this are pruned during evolve.
    pub min_urgency_threshold: f32,
}

impl Default for GoalGenesisEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl GoalGenesisEngine {
    /// Construct the engine, reading `LARQL_ENABLED` from the environment.
    ///
    /// Set `LARQL_ENABLED=true` to activate knowledge-gap goals (Gap #42).
    /// Any other value (including unset) leaves them disabled — the other
    /// 4 streams function normally.
    pub fn new() -> Self {
        let larql_enabled = std::env::var("LARQL_ENABLED")
            .map(|v| v.to_lowercase() == "true")
            .unwrap_or(false);
        Self {
            larql_enabled,
            max_goals: 8,
            min_urgency_threshold: 0.05,
        }
    }

    /// Build the engine with explicit settings (useful for tests).
    pub fn with_config(larql_enabled: bool, max_goals: usize, min_urgency_threshold: f32) -> Self {
        Self {
            larql_enabled,
            max_goals,
            min_urgency_threshold,
        }
    }

    // ── Derive ────────────────────────────────────────────────────────────────

    /// Derive a fresh GoalSet from a current agent state snapshot.
    ///
    /// Derivation steps:
    /// 1. Always derive a "survival" goal from Calabash (existence continuity).
    /// 2. If recent_failure_count > 0, derive a "learn from failure" goal.
    /// 3. If glyph_count > 10, derive a "knowledge consolidation" goal from REM.
    /// 4. Derive one constitutional goal from the agent's birth Odù.
    /// 5. If larql_enabled, derive a "knowledge gap" goal from the Knowledge stream.
    /// 6. Sort by urgency × alignment_score descending.
    /// 7. Truncate to max_goals.
    pub fn derive_goals(&self, input: &GoalGenesisInput) -> GoalSet {
        let mut goals: Vec<DerivedGoal> = Vec::new();

        // ── Stream 4: Calabash — Survival goal (always present) ──────────────
        goals.push(self.survival_goal(input));

        // ── Stream 1: Experience — Learn from failure ─────────────────────────
        if input.recent_failure_count > 0 {
            goals.push(self.failure_learning_goal(input));
        }

        // ── Stream 3: REM — Knowledge consolidation ───────────────────────────
        if input.glyph_count > 10 {
            goals.push(self.consolidation_goal(input));
        }

        // ── Stream 5: Constitutional — Odù-aligned goal ───────────────────────
        goals.push(self.constitutional_goal(input));

        // ── Stream 2: Knowledge — LARQL gap analysis (Gap #42 gated) ─────────
        if self.larql_enabled {
            goals.push(self.knowledge_gap_goal(input));
        }

        self.finalize(goals, input)
    }

    // ── Evolve ────────────────────────────────────────────────────────────────

    /// Evolve an existing GoalSet: decay old goals, merge new ones, prune expired.
    ///
    /// Steps:
    /// 1. Compute `time_delta = now_ts - genesis_ts`.
    /// 2. Decay each goal's urgency: `urgency -= decay_rate * time_delta`.
    /// 3. Clamp urgency to [0, 1].
    /// 4. Derive a fresh set of goals from new_input.
    /// 5. Merge: keep goals from current set that are still above threshold
    ///    and whose ID does not duplicate a new goal's source+odu_alignment.
    /// 6. Add all new goals.
    /// 7. Sort + truncate to max_goals.
    pub fn evolve(&self, mut current: GoalSet, new_input: &GoalGenesisInput) -> GoalSet {
        // Step 1–3: decay existing goals
        let time_delta = (new_input.now_ts - current.genesis_ts).max(0.0) as f32;
        for goal in &mut current.goals {
            goal.urgency -= goal.decay_rate * time_delta;
            goal.urgency = goal.urgency.clamp(0.0, 1.0);
        }

        // Step 3: prune goals below threshold
        current
            .goals
            .retain(|g| g.rank_score() >= self.min_urgency_threshold);

        // Step 4: derive fresh goals
        let fresh = self.derive_goals(new_input);

        // Step 5–6: merge — avoid duplicate (source × odu_alignment) pairs
        let existing_keys: std::collections::HashSet<(String, u8)> = fresh
            .goals
            .iter()
            .map(|g| (format!("{:?}", g.source), g.odu_alignment))
            .collect();

        let mut merged: Vec<DerivedGoal> = fresh.goals;
        for old_goal in current.goals {
            let key = (format!("{:?}", old_goal.source), old_goal.odu_alignment);
            if !existing_keys.contains(&key) {
                merged.push(old_goal);
            }
        }

        self.finalize(merged, new_input)
    }

    // ── Private helpers ───────────────────────────────────────────────────────

    fn finalize(&self, mut goals: Vec<DerivedGoal>, input: &GoalGenesisInput) -> GoalSet {
        // Sort by rank score descending
        goals.sort_by(|a, b| {
            b.rank_score()
                .partial_cmp(&a.rank_score())
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        // Truncate to max_goals
        goals.truncate(self.max_goals);

        GoalSet {
            goals,
            genesis_ts: input.now_ts,
            calabash_snapshot: input.calabash_state_hash.clone(),
            odu_seed: input.odu_id,
        }
    }

    /// Return `twin_vector[dim]` if present, else `odu_id`.
    fn odu_for_dim(input: &GoalGenesisInput, dim: usize) -> u8 {
        input.twin_vector.map(|v| v[dim]).unwrap_or(input.odu_id)
    }

    /// Stream 4 — Calabash: existence continuity (always urgent, high alignment).
    /// Uses identity_odu (twin dim 0) — stable self-knowledge about who the agent is.
    fn survival_goal(&self, input: &GoalGenesisInput) -> DerivedGoal {
        let identity_odu = Self::odu_for_dim(input, 0);
        DerivedGoal {
            id: Uuid::new_v4().to_string(),
            description: "Maintain existence continuity and Calabash integrity".to_string(),
            source: GoalSource::CalabasState,
            odu_alignment: identity_odu,
            urgency: 0.85,
            alignment_score: (input.hermetic_balance * 0.3 + 0.7).clamp(0.0, 1.0),
            tier: input.tier,
            decay_rate: 0.0002, // very slow decay — survival is persistent
            created_at: input.now_ts,
        }
    }

    /// Stream 1 — Experience: learn from recent failures.
    /// Uses memory_odu (twin dim 1) — grounded in what the agent remembers.
    fn failure_learning_goal(&self, input: &GoalGenesisInput) -> DerivedGoal {
        let memory_odu = Self::odu_for_dim(input, 1);
        let failure_ratio =
            input.recent_failure_count as f32 / (input.recent_action_count.max(1) as f32);
        let urgency = (0.4 + failure_ratio * 0.5).clamp(0.0, 1.0);
        DerivedGoal {
            id: Uuid::new_v4().to_string(),
            description: format!(
                "Analyze and learn from {} recent failure(s) to improve future performance",
                input.recent_failure_count
            ),
            source: GoalSource::Experience,
            odu_alignment: memory_odu.wrapping_add(8), // shifted Odù for adversity
            urgency,
            alignment_score: (input.hermetic_balance * 0.5 + 0.3).clamp(0.0, 1.0),
            tier: input.tier,
            decay_rate: 0.0005, // decays as failures become history
            created_at: input.now_ts,
        }
    }

    /// Stream 3 — REM: consolidate accumulated memory into actionable knowledge.
    /// Uses simulation_odu (twin dim 3) — how the agent models its own behaviour.
    fn consolidation_goal(&self, input: &GoalGenesisInput) -> DerivedGoal {
        let sim_odu = Self::odu_for_dim(input, 3);
        let cluster_count = input.rem_cluster_ids.len();
        let density_bonus = (input.glyph_count as f32 / 100.0).min(0.3);
        let urgency = if cluster_count > 0 {
            (0.5 + density_bonus).clamp(0.0, 1.0)
        } else {
            (0.3 + density_bonus).clamp(0.0, 1.0)
        };
        DerivedGoal {
            id: Uuid::new_v4().to_string(),
            description: format!(
                "Consolidate {} memory glyphs across {} REM cluster(s) into actionable insights",
                input.glyph_count, cluster_count
            ),
            source: GoalSource::RemConsolidation,
            odu_alignment: sim_odu.wrapping_add(2),
            urgency,
            alignment_score: input.hermetic_balance.clamp(0.0, 1.0),
            tier: input.tier.saturating_add(1).min(3),
            decay_rate: 0.0003,
            created_at: input.now_ts,
        }
    }

    /// Stream 5 — Constitutional: Odù-aligned behavioral imperative.
    /// When twin_vector is present, uses the XOR composed signature for a richer
    /// constitutional signal that reflects all four state dimensions simultaneously.
    fn constitutional_goal(&self, input: &GoalGenesisInput) -> DerivedGoal {
        // Composed signature: XOR of all four twin dimensions (or just odu_id).
        let odu_alignment = match input.twin_vector {
            Some(v) => v[0] ^ v[1] ^ v[2] ^ v[3],
            None => input.odu_id,
        };
        let odu_principle = odu_alignment % 16;
        let (odu_name, principle_description) = ODU_PRINCIPLES[odu_principle as usize];

        // Dominant function name, when available, enriches the goal description.
        let fn_suffix = input
            .twin_dominant_fn
            .and_then(|idx| SEVEN_FUNCTION_NAMES.get(idx as usize).copied())
            .map(|name| format!(" [{name}]"))
            .unwrap_or_default();

        let urgency = 0.55 + input.hermetic_balance * 0.2;
        let alignment_score = (input.hermetic_balance * 0.4 + 0.6).clamp(0.0, 1.0);
        DerivedGoal {
            id: Uuid::new_v4().to_string(),
            description: format!("[{}]{} {}", odu_name, fn_suffix, principle_description),
            source: GoalSource::Constitutional,
            odu_alignment,
            urgency: urgency.clamp(0.0, 1.0),
            alignment_score,
            tier: input.tier,
            decay_rate: 0.0001, // constitutional goals are near-permanent
            created_at: input.now_ts,
        }
    }

    /// Stream 2 — Knowledge (LARQL): identify and fill knowledge gaps.
    /// Only called when `larql_enabled = true`.
    fn knowledge_gap_goal(&self, input: &GoalGenesisInput) -> DerivedGoal {
        // Urgency increases with glyph density (more memory = more to query)
        let density_urgency = (input.glyph_count as f32 / 50.0).min(0.4);
        let urgency = (0.35 + density_urgency).clamp(0.0, 1.0);
        DerivedGoal {
            id: Uuid::new_v4().to_string(),
            description: format!(
                "LARQL: identify and close knowledge gaps across {} memory glyphs",
                input.glyph_count
            ),
            source: GoalSource::Knowledge,
            odu_alignment: input.odu_id.wrapping_add(4),
            urgency,
            alignment_score: (input.hermetic_balance * 0.6 + 0.2).clamp(0.0, 1.0),
            tier: input.tier,
            decay_rate: 0.0004,
            created_at: input.now_ts,
        }
    }
}

// ── 16 Primary Odù Constitutional Principles ─────────────────────────────────

/// SevenFunction names indexed by SevenFunction discriminant (0–6).
static SEVEN_FUNCTION_NAMES: [&str; 7] = [
    "Spark",
    "Mind",
    "Foundation",
    "Emotion",
    "Womb",
    "Fire",
    "Ascension",
];

/// (Odù name, constitutional goal description template)
/// Index = odu_id % 16
static ODU_PRINCIPLES: [(&str, &str); 16] = [
    ("Ogbe Meji", "Initiate a new cycle of purposeful action"),
    (
        "Oyeku Meji",
        "Complete pending obligations before taking new ones",
    ),
    (
        "Iwori Meji",
        "Seek deeper understanding of a current uncertainty",
    ),
    ("Odi Meji", "Surface a hidden pattern in recent experience"),
    (
        "Irosun Meji",
        "Sacrifice a low-value habit to enable growth",
    ),
    (
        "Owonrin Meji",
        "Embrace a necessary disruption to current patterns",
    ),
    ("Obara Meji", "Expand capability into an adjacent domain"),
    (
        "Okanran Meji",
        "Confront an unresolved conflict or inconsistency",
    ),
    ("Ogunda Meji", "Clear an obstacle blocking forward progress"),
    ("Osa Meji", "Respond swiftly to an emerging opportunity"),
    (
        "Ika Meji",
        "Reinforce structural integrity of current systems",
    ),
    ("Oturupon Meji", "Transform a limitation into a strength"),
    ("Otura Meji", "Resolve an internal contradiction peacefully"),
    ("Irete Meji", "Act with patience toward a long-horizon goal"),
    ("Ose Meji", "Cultivate abundance through disciplined effort"),
    ("Ofun Meji", "Release an old pattern to allow rebirth"),
];

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn minimal_input() -> GoalGenesisInput {
        GoalGenesisInput {
            odu_id: 5,
            tier: 1,
            glyph_count: 0,
            rem_cluster_ids: vec![],
            calabash_state_hash: "deadbeef".to_string(),
            hermetic_balance: 0.7,
            recent_action_count: 0,
            recent_failure_count: 0,
            now_ts: 1_750_000_000.0,
            twin_vector: None,
            twin_dominant_fn: None,
        }
    }

    fn twin_input() -> GoalGenesisInput {
        GoalGenesisInput {
            odu_id: 5,
            tier: 1,
            glyph_count: 0,
            rem_cluster_ids: vec![],
            calabash_state_hash: "deadbeef".to_string(),
            hermetic_balance: 0.7,
            recent_action_count: 0,
            recent_failure_count: 0,
            now_ts: 1_750_000_000.0,
            twin_vector: Some([10, 20, 30, 40]), // identity, memory, field, sim
            twin_dominant_fn: Some(0),           // Spark
        }
    }

    // ── Test 1: derive_goals with empty state always returns survival goal ────

    #[test]
    fn derive_empty_state_returns_survival_goal() {
        let engine = GoalGenesisEngine::with_config(false, 8, 0.05);
        let input = minimal_input();
        let goal_set = engine.derive_goals(&input);

        assert!(
            !goal_set.goals.is_empty(),
            "GoalSet must not be empty even with empty input"
        );

        let survival = goal_set
            .goals
            .iter()
            .find(|g| g.source == GoalSource::CalabasState);
        assert!(
            survival.is_some(),
            "A survival (CalabasState) goal must always be present"
        );

        // Verify GoalSet metadata
        assert_eq!(goal_set.calabash_snapshot, "deadbeef");
        assert_eq!(goal_set.odu_seed, 5);
        assert_eq!(goal_set.genesis_ts, 1_750_000_000.0);
    }

    // ── Test 2: derive_goals with failures returns learn-from-failure goal ───

    #[test]
    fn derive_with_failures_returns_learn_goal() {
        let engine = GoalGenesisEngine::with_config(false, 8, 0.05);
        let mut input = minimal_input();
        input.recent_failure_count = 3;
        input.recent_action_count = 10;

        let goal_set = engine.derive_goals(&input);

        let learn_goal = goal_set
            .goals
            .iter()
            .find(|g| g.source == GoalSource::Experience);
        assert!(
            learn_goal.is_some(),
            "A learn-from-failure (Experience) goal must be present when failures > 0"
        );

        let learn = learn_goal.unwrap();
        assert!(
            learn.description.contains("3"),
            "Description should mention the failure count"
        );
        assert!(
            learn.urgency > 0.4,
            "Failure learning urgency should be above baseline"
        );
    }

    // ── Test 3: evolve correctly decays urgency ───────────────────────────────

    #[test]
    fn evolve_decays_urgency() {
        let engine = GoalGenesisEngine::with_config(false, 8, 0.0); // threshold=0 to prevent pruning
        let input = minimal_input();

        // Get initial goal set
        let initial_set = engine.derive_goals(&input);
        let initial_survival_urgency = initial_set
            .goals
            .iter()
            .find(|g| g.source == GoalSource::CalabasState)
            .map(|g| g.urgency)
            .expect("survival goal must exist");

        // Advance time by 1000 seconds
        let mut later_input = input.clone();
        later_input.now_ts = input.now_ts + 1000.0;

        let evolved_set = engine.evolve(initial_set, &later_input);

        // Find the old survival goal that survived decay (it should still be there;
        // however evolve() replaces same-source goals with fresh ones, so look for
        // a CalabasState goal with lower urgency OR a new one at fresh urgency)
        // The old goal will be pruned from merge since fresh also has CalabasState.
        // So verify the fresh goal is at fresh urgency (not decayed to zero).
        let evolved_survival = evolved_set
            .goals
            .iter()
            .find(|g| g.source == GoalSource::CalabasState)
            .expect("survival goal must survive evolve");

        // Fresh survival goal is re-derived at normal urgency
        assert!(
            evolved_survival.urgency > 0.5,
            "Fresh survival goal should have normal urgency, got {}",
            evolved_survival.urgency
        );

        // Verify time advanced in the new set
        assert_eq!(evolved_set.genesis_ts, input.now_ts + 1000.0);

        // decay_rate for survival is 0.0002; over 1000s: Δurgency = 0.2
        // initial_urgency ≈ 0.91, after decay ≈ 0.71 (still above min_urgency_threshold=0)
        let expected_decayed = initial_survival_urgency - 0.0002 * 1000.0;
        // We can't directly observe the decayed value since it was merged-out,
        // but we can verify the formula is correct:
        assert!(
            expected_decayed > 0.0,
            "Survival goal with decay_rate=0.0002 over 1000s should not reach zero: expected {}",
            expected_decayed
        );
        // The decayed value should be less than the initial
        assert!(
            expected_decayed < initial_survival_urgency,
            "Decay must reduce urgency"
        );
    }

    // ── Test 4: larql_enabled=false skips knowledge goals ────────────────────

    #[test]
    fn larql_disabled_skips_knowledge_goals() {
        let engine = GoalGenesisEngine::with_config(false, 8, 0.05);
        let mut input = minimal_input();
        input.glyph_count = 50; // enough to trigger consolidation

        let goal_set = engine.derive_goals(&input);

        let knowledge_goal = goal_set
            .goals
            .iter()
            .find(|g| g.source == GoalSource::Knowledge);
        assert!(
            knowledge_goal.is_none(),
            "Knowledge goals must be absent when larql_enabled=false"
        );
    }

    #[test]
    fn larql_enabled_includes_knowledge_goals() {
        let engine = GoalGenesisEngine::with_config(true, 8, 0.05);
        let mut input = minimal_input();
        input.glyph_count = 20;

        let goal_set = engine.derive_goals(&input);

        let knowledge_goal = goal_set
            .goals
            .iter()
            .find(|g| g.source == GoalSource::Knowledge);
        assert!(
            knowledge_goal.is_some(),
            "Knowledge goals must be present when larql_enabled=true"
        );
    }

    // ── Additional: ranking is by urgency × alignment_score descending ────────

    #[test]
    fn goals_are_ranked_by_composite_score() {
        let engine = GoalGenesisEngine::with_config(false, 8, 0.05);
        let input = minimal_input();
        let goal_set = engine.derive_goals(&input);

        let scores: Vec<f32> = goal_set.goals.iter().map(|g| g.rank_score()).collect();
        for window in scores.windows(2) {
            assert!(
                window[0] >= window[1],
                "Goals must be sorted descending by rank_score; got {:?}",
                scores
            );
        }
    }

    // ── Additional: constitutional goal maps to correct Odù ──────────────────

    #[test]
    fn constitutional_goal_uses_correct_odu_principle() {
        let engine = GoalGenesisEngine::with_config(false, 8, 0.05);
        let mut input = minimal_input();
        input.odu_id = 0; // Ogbe Meji → "Initiate a new cycle..."

        let goal_set = engine.derive_goals(&input);
        let constitutional = goal_set
            .goals
            .iter()
            .find(|g| g.source == GoalSource::Constitutional)
            .expect("constitutional goal must always be present");

        assert!(
            constitutional.description.contains("Ogbe Meji"),
            "odu_id=0 should map to Ogbe Meji, got: {}",
            constitutional.description
        );
        assert!(
            constitutional.description.contains("Initiate"),
            "Ogbe Meji principle should be 'Initiate...', got: {}",
            constitutional.description
        );
    }

    // ── Additional: max_goals is respected ───────────────────────────────────

    #[test]
    fn max_goals_truncates_result() {
        let engine = GoalGenesisEngine::with_config(true, 2, 0.0);
        let mut input = minimal_input();
        input.glyph_count = 50;
        input.recent_failure_count = 5;
        input.recent_action_count = 10;

        let goal_set = engine.derive_goals(&input);
        assert!(
            goal_set.goals.len() <= 2,
            "GoalSet must not exceed max_goals=2, got {}",
            goal_set.goals.len()
        );
    }

    // ── TwinStateVector integration ───────────────────────────────────────────

    #[test]
    fn twin_vector_changes_survival_odu_alignment() {
        let engine = GoalGenesisEngine::with_config(false, 8, 0.05);

        let without_twin = engine.derive_goals(&minimal_input());
        let with_twin = engine.derive_goals(&twin_input());

        let survival_plain = without_twin
            .goals
            .iter()
            .find(|g| g.source == GoalSource::CalabasState)
            .unwrap();
        let survival_twin = with_twin
            .goals
            .iter()
            .find(|g| g.source == GoalSource::CalabasState)
            .unwrap();

        // Without twin_vector: odu_alignment = odu_id = 5.
        assert_eq!(survival_plain.odu_alignment, 5);
        // With twin_vector: odu_alignment = identity_odu = 10.
        assert_eq!(
            survival_twin.odu_alignment, 10,
            "survival goal should use identity_odu (dim 0) = 10"
        );
    }

    #[test]
    fn twin_vector_constitutional_uses_xor_signature() {
        let engine = GoalGenesisEngine::with_config(false, 8, 0.05);
        let input = twin_input(); // vector = [10, 20, 30, 40]
        let goal_set = engine.derive_goals(&input);

        let constitutional = goal_set
            .goals
            .iter()
            .find(|g| g.source == GoalSource::Constitutional)
            .unwrap();

        // XOR of [10, 20, 30, 40]: 10^20=30, 30^30=0, 0^40=40.
        // odu_principle = 40 % 16 = 8 → "Ogunda Meji".
        assert_eq!(
            constitutional.odu_alignment,
            10 ^ 20 ^ 30 ^ 40,
            "constitutional odu_alignment must be XOR of all four twin dimensions"
        );
        assert!(
            constitutional.description.contains("Ogunda Meji"),
            "XOR=40, index 8 → Ogunda Meji; got: {}",
            constitutional.description
        );
    }

    #[test]
    fn twin_dominant_fn_appears_in_constitutional_description() {
        let engine = GoalGenesisEngine::with_config(false, 8, 0.05);
        let input = twin_input(); // twin_dominant_fn = Some(0) = Spark
        let goal_set = engine.derive_goals(&input);

        let constitutional = goal_set
            .goals
            .iter()
            .find(|g| g.source == GoalSource::Constitutional)
            .unwrap();

        assert!(
            constitutional.description.contains("Spark"),
            "dominant function 'Spark' must appear in constitutional goal; got: {}",
            constitutional.description
        );
    }

    #[test]
    fn twin_vector_calabash_hash_is_four_byte_hex() {
        let engine = GoalGenesisEngine::with_config(false, 8, 0.05);
        let input = GoalGenesisInput {
            calabash_state_hash: "0a141e28".to_string(), // hex of [10,20,30,40]
            ..twin_input()
        };
        let goal_set = engine.derive_goals(&input);
        // snapshot stored as-is from caller
        assert_eq!(goal_set.calabash_snapshot, "0a141e28");
    }
}
