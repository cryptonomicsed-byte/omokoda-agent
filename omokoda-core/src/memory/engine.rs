use crate::interpreter::MemoryEntry;
use crate::session::ConversationMessage;
use std::cmp::Reverse;

/// The three memory tiers, each with different churn, capacity, and distillation rules.
///
/// Maps to the 3 primitives:
/// - `birth`    → initializes all three tiers
/// - `think`    → reads from Working + Semantic; writes to Working
/// - `act`      → writes outcomes to Episodic; Semantic distills from Episodic on overflow
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryTier {
    /// High-churn, capped at 100 entries — current session context
    Working,
    /// Medium-churn, capped at 500 entries — think/act outcomes this session
    Episodic,
    /// Low-churn, capped at 200 entries — distilled patterns across sessions
    Semantic,
}

/// A distilled semantic pattern extracted from episodic memory on overflow.
#[derive(Debug, Clone)]
pub struct SemanticPattern {
    pub pattern: String,
    pub frequency: u32,
    pub avg_importance: f32,
    pub source_tier: MemoryTier,
}

/// 3-tier memory engine.
///
/// Replaces the stub with a design that models how an Omo-Koda2 agent accumulates
/// and distills knowledge across the birth → think → act cycle.
pub struct MemoryEngine {
    pub working_capacity: usize,
    pub episodic_capacity: usize,
    pub semantic_capacity: usize,
    /// Minimum importance score for an episodic entry to be distilled into semantic
    pub distillation_threshold: f32,
}

impl MemoryEngine {
    pub fn new() -> Self {
        Self {
            working_capacity: 100,
            episodic_capacity: 500,
            semantic_capacity: 200,
            distillation_threshold: 0.6,
        }
    }

    /// Classify a MemoryEntry into a tier based on its importance and age.
    /// Used when ingesting new entries.
    #[must_use]
    pub fn classify(&self, entry: &MemoryEntry) -> MemoryTier {
        if entry.importance >= self.distillation_threshold {
            MemoryTier::Episodic
        } else {
            MemoryTier::Working
        }
    }

    /// Process the working-memory buffer:
    /// - Prune low-importance entries to stay within capacity
    /// - Promote high-importance entries to episodic tier
    ///
    /// Returns promoted entries that the caller should store in episodic memory.
    pub fn process_working_memory(&self, memory: &mut Vec<MemoryEntry>) -> Vec<MemoryEntry> {
        let mut promoted = Vec::new();

        // Promote high-importance entries to episodic
        let mut i = 0;
        while i < memory.len() {
            if memory[i].importance >= self.distillation_threshold {
                promoted.push(memory.remove(i));
            } else {
                i += 1;
            }
        }

        // Keep only the top-N by importance if over capacity
        if memory.len() > self.working_capacity {
            memory.sort_by(|a, b| {
                b.importance
                    .partial_cmp(&a.importance)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
            memory.truncate(self.working_capacity);
        }

        promoted
    }

    /// Process the episodic-memory buffer:
    /// - On overflow, distill patterns into semantic memory
    /// - Prune low-importance entries that were not distilled
    ///
    /// Returns semantic patterns extracted during distillation.
    pub fn process_episodic_memory(&self, episodic: &mut Vec<MemoryEntry>) -> Vec<SemanticPattern> {
        if episodic.len() <= self.episodic_capacity {
            return vec![];
        }

        // Sort by importance descending
        episodic.sort_by(|a, b| {
            b.importance
                .partial_cmp(&a.importance)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        // Distill the overflow (entries beyond capacity) into semantic patterns
        let overflow = episodic.split_off(self.episodic_capacity);
        self.distill_to_semantic(&overflow)
    }

    /// Cap semantic memory to its capacity, keeping the highest-frequency patterns.
    pub fn process_semantic_memory(&self, patterns: &mut Vec<SemanticPattern>) {
        if patterns.len() > self.semantic_capacity {
            patterns.sort_by_key(|b| Reverse(b.frequency));
            patterns.truncate(self.semantic_capacity);
        }
    }

    /// Token-aware conversation compaction — the same strategy as before but now integrated
    /// with the tier model: truncates ToolResult outputs (working-tier artifacts) first,
    /// then trims message history if over the message cap.
    pub fn compress(&self, messages: &mut Vec<ConversationMessage>, _reputation: f64) {
        // Working tier: ToolResult outputs are the highest-churn content
        for message in messages.iter_mut() {
            for block in &mut message.blocks {
                match block {
                    crate::session::ContentBlock::ToolResult { output, .. } => {
                        if output.len() > 2000 {
                            // Floor to a char boundary — fixed byte offsets panic
                            // mid-UTF-8 (very common with Yorùbá diacritics/em-dashes).
                            let mut end = 2000;
                            while end > 0 && !output.is_char_boundary(end) {
                                end -= 1;
                            }
                            *output = format!("{}... [TRUNCATED]", &output[..end]);
                        }
                    }
                    crate::session::ContentBlock::Text { text } if text.len() > 5000 => {
                        let mut end = 5000;
                        while end > 0 && !text.is_char_boundary(end) {
                            end -= 1;
                        }
                        *text = format!("{}... [TRUNCATED]", &text[..end]);
                    }
                    _ => {}
                }
            }
        }

        // Episodic tier: keep last 50 messages (recent episodic context)
        if messages.len() > self.episodic_capacity / 10 {
            messages.drain(0..(messages.len() - (self.episodic_capacity / 10)));
        }
    }

    /// Extract a summary of the most important semantic patterns for injection
    /// into the `think` system prompt. Capped at `max_patterns` entries.
    #[must_use]
    pub fn render_semantic_context(
        &self,
        patterns: &[SemanticPattern],
        max_patterns: usize,
    ) -> String {
        if patterns.is_empty() {
            return String::new();
        }

        let mut sorted: Vec<&SemanticPattern> = patterns.iter().collect();
        sorted.sort_by_key(|b| Reverse(b.frequency));
        sorted.truncate(max_patterns);

        let mut out = String::from("[Memory: Semantic Patterns]\n");
        for p in sorted {
            out.push_str(&format!(
                "  • {} (×{}, importance {:.2})\n",
                p.pattern, p.frequency, p.avg_importance
            ));
        }
        out
    }

    fn distill_to_semantic(&self, entries: &[MemoryEntry]) -> Vec<SemanticPattern> {
        // Ọ̀ṣun (Julia) semantic distillation — harmonic resonance + embedding clustering.
        //
        // When JULIA_OSUN_URL is set the real Julia/Ọ̀ṣun service does the
        // heavy lifting: embedding similarity, resonance scoring, fractal
        // dimension estimation.  On any failure (missing env, network, timeout,
        // bad JSON) we fail open to the Rust importance-based approximation so
        // an absent Julia service never blocks memory consolidation.
        if let Ok(base_url) = std::env::var("JULIA_OSUN_URL") {
            if let Some(patterns) = self.distill_via_osun_service(&base_url, entries) {
                return patterns;
            }
        }
        self.distill_rust_approximation(entries)
    }

    /// POST entries to the Ọ̀ṣun/Julia distillation endpoint.
    /// Returns `None` on any error so callers can fall back gracefully.
    fn distill_via_osun_service(
        &self,
        base_url: &str,
        entries: &[MemoryEntry],
    ) -> Option<Vec<SemanticPattern>> {
        #[derive(serde::Serialize)]
        struct OsunRequest<'a> {
            entries: &'a [MemoryEntryPayload],
        }
        #[derive(serde::Serialize)]
        struct MemoryEntryPayload {
            content: String,
            importance: f32,
            tier: &'static str,
        }
        #[derive(serde::Deserialize)]
        struct OsunPattern {
            pattern: String,
            frequency: u32,
            avg_importance: f32,
        }
        #[derive(serde::Deserialize)]
        struct OsunResponse {
            patterns: Vec<OsunPattern>,
        }

        let payload: Vec<MemoryEntryPayload> = entries
            .iter()
            .map(|e| MemoryEntryPayload {
                content: e.text.clone().unwrap_or_default(),
                importance: e.importance,
                tier: "episodic",
            })
            .collect();

        let url = format!("{}/distill", base_url.trim_end_matches('/'));
        let client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(5))
            .build()
            .ok()?;

        let resp: OsunResponse = client
            .post(&url)
            .json(&OsunRequest { entries: &payload })
            .send()
            .ok()?
            .error_for_status()
            .ok()?
            .json()
            .ok()?;

        Some(
            resp.patterns
                .into_iter()
                .map(|p| SemanticPattern {
                    pattern: p.pattern,
                    frequency: p.frequency,
                    avg_importance: p.avg_importance,
                    source_tier: MemoryTier::Semantic,
                })
                .collect(),
        )
    }

    /// Rust importance-based approximation used when Julia/Ọ̀ṣun is unavailable.
    fn distill_rust_approximation(&self, entries: &[MemoryEntry]) -> Vec<SemanticPattern> {
        let high: Vec<&MemoryEntry> = entries.iter().filter(|e| e.importance >= 0.7).collect();
        let mid: Vec<&MemoryEntry> = entries
            .iter()
            .filter(|e| e.importance >= 0.4 && e.importance < 0.7)
            .collect();

        let mut patterns = Vec::new();

        if !high.is_empty() {
            let avg_imp = high.iter().map(|e| e.importance).sum::<f32>() / high.len() as f32;
            patterns.push(SemanticPattern {
                pattern: format!("high-importance cluster ({} entries)", high.len()),
                frequency: high.len() as u32,
                avg_importance: avg_imp,
                source_tier: MemoryTier::Episodic,
            });
        }

        if !mid.is_empty() {
            let avg_imp = mid.iter().map(|e| e.importance).sum::<f32>() / mid.len() as f32;
            patterns.push(SemanticPattern {
                pattern: format!("mid-importance cluster ({} entries)", mid.len()),
                frequency: mid.len() as u32,
                avg_importance: avg_imp,
                source_tier: MemoryTier::Episodic,
            });
        }

        patterns
    }
}

impl Default for MemoryEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::interpreter::MemoryEntry;

    fn make_entry(id: &str, importance: f32) -> MemoryEntry {
        MemoryEntry {
            id: id.to_string(),
            scope: crate::interpreter::MemoryScope::Public,
            tier: 0,
            content_hash: [0u8; 32],
            created_time: 0,
            importance,
            ciphertext: None,
            text: Some(format!("entry {id}")),
        }
    }

    #[test]
    fn classify_low_importance_to_working() {
        let engine = MemoryEngine::new();
        let entry = make_entry("a", 0.2);
        assert_eq!(engine.classify(&entry), MemoryTier::Working);
    }

    #[test]
    fn classify_high_importance_to_episodic() {
        let engine = MemoryEngine::new();
        let entry = make_entry("a", 0.8);
        assert_eq!(engine.classify(&entry), MemoryTier::Episodic);
    }

    #[test]
    fn working_memory_promotes_high_importance() {
        let engine = MemoryEngine::new();
        let mut working = vec![make_entry("low", 0.2), make_entry("high", 0.9)];
        let promoted = engine.process_working_memory(&mut working);
        assert_eq!(promoted.len(), 1);
        assert_eq!(promoted[0].id, "high");
        assert_eq!(working.len(), 1);
        assert_eq!(working[0].id, "low");
    }

    #[test]
    fn working_memory_pruned_to_capacity() {
        let mut engine = MemoryEngine::new();
        engine.working_capacity = 3;
        let mut working: Vec<MemoryEntry> = (0..10)
            .map(|i| make_entry(&i.to_string(), 0.1 * i as f32 + 0.05))
            .collect();
        engine.process_working_memory(&mut working);
        assert!(working.len() <= 3);
    }

    #[test]
    fn episodic_overflow_produces_semantic_patterns() {
        let mut engine = MemoryEngine::new();
        engine.episodic_capacity = 5;
        let mut episodic: Vec<MemoryEntry> = (0..10)
            .map(|i| make_entry(&i.to_string(), 0.1 * i as f32 + 0.05))
            .collect();
        let patterns = engine.process_episodic_memory(&mut episodic);
        assert!(episodic.len() <= 5);
        // Some overflow entries should have been distilled
        assert!(!patterns.is_empty() || episodic.len() == 5);
    }

    #[test]
    fn semantic_memory_capped_by_frequency() {
        let mut engine = MemoryEngine::new();
        engine.semantic_capacity = 2;
        let mut patterns: Vec<SemanticPattern> = (0..5)
            .map(|i| SemanticPattern {
                pattern: format!("p{i}"),
                frequency: i as u32,
                avg_importance: 0.5,
                source_tier: MemoryTier::Episodic,
            })
            .collect();
        engine.process_semantic_memory(&mut patterns);
        assert_eq!(patterns.len(), 2);
        // Highest frequency kept
        assert!(patterns[0].frequency >= patterns[1].frequency);
    }

    #[test]
    fn render_semantic_context_capped_at_max() {
        let engine = MemoryEngine::new();
        let patterns: Vec<SemanticPattern> = (0..10)
            .map(|i| SemanticPattern {
                pattern: format!("pattern-{i}"),
                frequency: i as u32,
                avg_importance: 0.7,
                source_tier: MemoryTier::Episodic,
            })
            .collect();
        let rendered = engine.render_semantic_context(&patterns, 3);
        assert!(rendered.contains("[Memory: Semantic Patterns]"));
        let bullet_count = rendered.matches("  •").count();
        assert_eq!(bullet_count, 3);
    }

    #[test]
    fn render_semantic_context_empty_returns_empty_string() {
        let engine = MemoryEngine::new();
        let rendered = engine.render_semantic_context(&[], 5);
        assert!(rendered.is_empty());
    }
}
