//! IfDecision — the output of If-Script evaluation, input to Ọ̀ṢỌ́ compiler.

use serde::{Deserialize, Serialize};

/// Gate score for one of the 7 hermetic gates (0.0 = failed, 1.0 = perfect).
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct GateScore {
    pub gate_index: u8, // 0..6
    pub score: f32,
}

/// The result of evaluating an If-Script ritual decision.
/// This is the sole input the Ọ̀ṢỌ́ compiler needs from the If-Script layer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IfDecision {
    /// Odù archetype index (0..255) selected by divination.
    pub odu_index: u8,
    /// Hermetic gate evaluation results (up to 7 scores).
    pub gate_scores: Vec<GateScore>,
    /// Name of the Ọ̀ṢỌ́ ritual to execute (key into RitualRegistry).
    pub ritual: String,
    /// Parameters to inject into the Ọ̀ṢỌ́ program as compile-time constants.
    pub params: serde_json::Value,
}

impl IfDecision {
    /// Aggregate gate alignment: mean of all gate scores.
    /// Used as the `gate_alignment` constant injected into Ọ̀ṢỌ́ programs.
    pub fn gate_alignment(&self) -> f32 {
        if self.gate_scores.is_empty() {
            return 0.0;
        }
        let sum: f32 = self.gate_scores.iter().map(|g| g.score).sum();
        sum / self.gate_scores.len() as f32
    }

    /// Compute the Àṣẹ emission multiplier (from ECONOMICS_DECISIONS.md justice rule).
    /// m = 0.8 + (hermetic_balance × 0.25) + (gate_alignment × 0.15)
    pub fn ase_multiplier(&self, hermetic_balance: f32) -> f32 {
        0.8 + (hermetic_balance * 0.25) + (self.gate_alignment() * 0.15)
    }
}
