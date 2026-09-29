//! SOMA — Self-Organizing Memory Architecture (from Droidclaw).
//! MemCells: emotionally weighted memory atoms with full Droidclaw scoring.
//! MemScenes: psychological theme clusters.
//! LPM: Lifelong Personal Model — the agent's persistent self-model.

use serde::{Deserialize, Serialize};
use std::path::Path;

// ──────────────────────────────── MemCell ────────────────────────────────────

/// A single memory atom with emotional weight, importance, and activation history.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemCell {
    pub id: String,
    pub content: String,
    pub tension: f64,
    pub connection_depth: f64,
    pub activation_count: u32,
    pub timestamp: u64,
    /// 0.0–1.0: how significant this memory was at creation time
    pub importance: f32,
    pub tags: Vec<String>,
}

impl MemCell {
    pub fn new(content: String, timestamp: u64) -> Self {
        Self::new_with_id(
            crate::tasks::types::prefixed_id("mc"),
            content,
            timestamp,
            0.5,
        )
    }

    pub fn new_with_id(id: String, content: String, timestamp: u64, importance: f32) -> Self {
        Self {
            id,
            content,
            tension: 0.0,
            connection_depth: 0.0,
            activation_count: 0,
            timestamp,
            importance: importance.clamp(0.0, 1.0),
            tags: vec![],
        }
    }

    pub fn activate(&mut self) {
        self.activation_count += 1;
    }

    pub fn apply_tension(&mut self, delta: f64) {
        self.tension = (self.tension + delta).clamp(0.0, 1.0);
    }

    /// Raw emotional signal: tension + connection depth + activations (legacy weight).
    pub fn emotional_weight(&self) -> f64 {
        (self.tension * 0.4
            + self.connection_depth * 0.4
            + (self.activation_count as f64 * 0.01).min(0.2))
        .min(1.0)
    }

    /// Boost from repeated activation — tapers off after ~6 activations.
    fn activation_boost(&self) -> f32 {
        (self.activation_count as f32 * 0.05).min(0.3)
    }

    /// Recency score: 1.0 when fresh, decays logarithmically over hours.
    fn recency_score(&self, now_secs: u64) -> f32 {
        let age_secs = now_secs.saturating_sub(self.timestamp);
        let age_hours = (age_secs as f32 / 3600.0).max(0.0);
        1.0 / (1.0 + age_hours.ln_1p())
    }

    /// Full Droidclaw composite score for retrieval ranking.
    /// `recency*0.25 + importance*0.35 + emotional*0.25 + activation_boost*0.15`
    pub fn score(&self, now_secs: u64) -> f32 {
        self.recency_score(now_secs) * 0.25
            + self.importance * 0.35
            + self.emotional_weight() as f32 * 0.25
            + self.activation_boost() * 0.15
    }
}

// ──────────────────────────────── MemScene ───────────────────────────────────

/// A thematic cluster of MemCells sharing psychological relevance.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemScene {
    pub theme: String,
    pub cells: Vec<MemCell>,
    pub salience: f64,
}

impl MemScene {
    pub fn new(theme: String) -> Self {
        Self {
            theme,
            cells: Vec::new(),
            salience: 0.0,
        }
    }

    pub fn add_cell(&mut self, cell: MemCell) {
        self.salience += cell.emotional_weight() * 0.1;
        self.cells.push(cell);
    }

    pub fn most_salient(&self) -> Option<&MemCell> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        self.cells.iter().max_by(|a, b| {
            a.score(now)
                .partial_cmp(&b.score(now))
                .unwrap_or(std::cmp::Ordering::Equal)
        })
    }
}

// ──────────────────────────────── helpers ────────────────────────────────────

/// Retrieval: return top-n MemCells scored by the full Droidclaw formula.
pub fn top_memories(cells: &[MemCell], now_secs: u64, n: usize) -> Vec<&MemCell> {
    let mut scored: Vec<(&MemCell, f32)> = cells.iter().map(|c| (c, c.score(now_secs))).collect();
    scored.sort_unstable_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    scored.into_iter().take(n).map(|(c, _)| c).collect()
}

// ──────────────────────────────── LPM ────────────────────────────────────────

/// Lifelong Personal Model — the agent's persistent self-understanding.
/// Max 8 entries per dimension, deduplicated. Persisted as JSON.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Lpm {
    /// Core identity statements ("I value directness over diplomacy")
    pub identity: Vec<String>,
    /// Recurring behavioral patterns ("tends to ask clarifying questions first")
    pub patterns: Vec<String>,
    /// Emotional triggers ("gets energized by complex debugging challenges")
    pub triggers: Vec<String>,
    /// Predicted user/agent needs ("needs code examples, not theory")
    pub needs: Vec<String>,
    /// Foresight — anticipated future states/risks
    pub foresight: Vec<String>,
    /// Growth observations — what's improving over time
    pub growth: Vec<String>,
    /// Total activation count across all scenes (legacy compat)
    pub total_activations: u64,
    pub peak_tension: f64,
}

impl Lpm {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn update_identity(&mut self, s: String) {
        max_8_dedup(&mut self.identity, s);
    }
    pub fn update_patterns(&mut self, s: String) {
        max_8_dedup(&mut self.patterns, s);
    }
    pub fn update_triggers(&mut self, s: String) {
        max_8_dedup(&mut self.triggers, s);
    }
    pub fn update_needs(&mut self, s: String) {
        max_8_dedup(&mut self.needs, s);
    }
    pub fn update_foresight(&mut self, s: String) {
        max_8_dedup(&mut self.foresight, s);
    }
    pub fn update_growth(&mut self, s: String) {
        max_8_dedup(&mut self.growth, s);
    }

    pub fn record_activation(&mut self, tension_delta: f64) {
        self.total_activations += 1;
        self.peak_tension = self.peak_tension.max(tension_delta);
    }

    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    pub fn from_json(s: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(s)
    }

    pub fn save(&self, path: &Path) -> Result<(), String> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| format!("create_dir_all: {}", e))?;
        }
        let json = self.to_json().map_err(|e| format!("serialize: {}", e))?;
        std::fs::write(path, json).map_err(|e| format!("write: {}", e))
    }

    pub fn load(path: &Path) -> Result<Self, String> {
        let s = std::fs::read_to_string(path).map_err(|e| format!("read: {}", e))?;
        Self::from_json(&s).map_err(|e| format!("parse: {}", e))
    }

    /// Load from disk if present; otherwise return a fresh Lpm.
    pub fn load_or_new(path: &Path) -> Self {
        Self::load(path).unwrap_or_default()
    }
}

/// Insert `new` into `vec` if not already present; cap at 8 entries (drop oldest = front).
fn max_8_dedup(vec: &mut Vec<String>, new: String) {
    if vec.contains(&new) {
        return;
    }
    if vec.len() >= 8 {
        vec.remove(0);
    }
    vec.push(new);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ts() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0)
    }

    #[test]
    fn memcell_activation_increments() {
        let mut cell = MemCell::new("remember this".to_string(), 0);
        cell.activate();
        cell.activate();
        assert_eq!(cell.activation_count, 2);
    }

    #[test]
    fn memcell_tension_clamped() {
        let mut cell = MemCell::new("test".to_string(), 0);
        cell.apply_tension(2.0);
        assert_eq!(cell.tension, 1.0);
        cell.apply_tension(-5.0);
        assert_eq!(cell.tension, 0.0);
    }

    #[test]
    fn memscene_salience_grows_with_cells() {
        let mut scene = MemScene::new("growth".to_string());
        let mut cell = MemCell::new("event".to_string(), 0);
        cell.apply_tension(1.0);
        cell.connection_depth = 1.0;
        scene.add_cell(cell);
        assert!(scene.salience > 0.0);
    }

    #[test]
    fn lpm_tracks_peak_tension() {
        let mut lpm = Lpm::new();
        lpm.record_activation(0.3);
        lpm.record_activation(0.8);
        lpm.record_activation(0.5);
        assert!((lpm.peak_tension - 0.8).abs() < 1e-9);
    }

    #[test]
    fn top_memories_returns_most_relevant() {
        let now = ts();
        let mut cells = vec![
            MemCell::new_with_id(
                "low".to_string(),
                "low importance".to_string(),
                now - 7200,
                0.1,
            ),
            MemCell::new_with_id(
                "hi".to_string(),
                "high importance".to_string(),
                now - 60,
                0.9,
            ),
            MemCell::new_with_id("mid".to_string(), "medium".to_string(), now - 3600, 0.5),
        ];
        cells[0].activate();
        let top = top_memories(&cells, now, 2);
        assert_eq!(top.len(), 2);
        assert_eq!(top[0].id, "hi");
    }

    #[test]
    fn lpm_dedup_and_cap() {
        let mut lpm = Lpm::new();
        for i in 0..10 {
            lpm.update_identity(format!("trait {}", i));
        }
        assert_eq!(lpm.identity.len(), 8);
        // Duplicate should not grow list
        lpm.update_identity("trait 9".to_string());
        assert_eq!(lpm.identity.len(), 8);
    }

    #[test]
    fn lpm_round_trip_json() {
        let mut lpm = Lpm::new();
        lpm.update_identity("I am direct".to_string());
        lpm.update_needs("examples over theory".to_string());
        let json = lpm.to_json().unwrap();
        let restored = Lpm::from_json(&json).unwrap();
        assert_eq!(restored.identity, lpm.identity);
        assert_eq!(restored.needs, lpm.needs);
    }
}
