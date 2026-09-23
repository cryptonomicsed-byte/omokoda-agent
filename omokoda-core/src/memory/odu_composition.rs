//! Odù Composition — the mechanism by which an agent's `composed_odu` evolves
//! through accumulated experience.
//!
//! At birth, `composed_odu = (primary_odu << 8) | primary_odu` — a mirror of
//! the birth Odù, with no secondary layer yet. After each act, the agent's
//! receipt history is scanned and a secondary Odù index is derived from the
//! pattern of gate alignments across those receipts.
//!
//! Design invariants:
//! - Deterministic: same receipts → same composed_odu.
//! - Experience-grounded: only real, sealed ActReceipts count.
//! - Non-regressive: lower-quality acts reduce alignment weight but cannot
//!   reset the primary Odù — only deepen or re-orient the secondary layer.
//! - Lightweight: runs after every tool call using in-memory receipt data,
//!   never blocks execution.

use crate::receipt::act_receipt::ActReceipt;
use sha2::{Digest, Sha256};

/// Number of recent receipts considered for composition. Beyond this window
/// the oldest receipts fade out — the Orí responds to recent experience, not
/// the total sum of all history.
const COMPOSITION_WINDOW: usize = 16;

/// Result of an Odù composition pass.
#[derive(Debug, Clone, PartialEq)]
pub struct OduCompositionResult {
    /// The new composed Odù index (u16: high byte = primary, low byte = secondary).
    pub composed_odu: u16,
    /// The secondary Odù derived from experience (0-255).
    pub secondary_odu: u8,
    /// Mean gate alignment of the receipts in the window (0.5–1.0).
    pub experience_weight: f64,
    /// Number of receipts that contributed.
    pub receipt_count: usize,
}

/// Derive a new `composed_odu` from the agent's birth Odù and its recent
/// receipt history.
///
/// The secondary Odù is computed by hashing the birth Odù, the receipt ids
/// (ordered oldest-first within the window), and the mean gate alignment
/// quantised to 256 steps. This ensures:
///   - Two agents with the same birth Odù diverge in `composed_odu` as soon
///     as their action histories diverge.
///   - The same agent with the same history always produces the same result
///     (deterministic replay).
pub fn compose_odu(primary_odu: u8, recent_receipts: &[&ActReceipt]) -> OduCompositionResult {
    let window: Vec<&&ActReceipt> = recent_receipts
        .iter()
        .rev()
        .take(COMPOSITION_WINDOW)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();

    let receipt_count = window.len();

    if receipt_count == 0 {
        // No experience yet — composed_odu mirrors the birth primary on both bytes.
        return OduCompositionResult {
            composed_odu: (primary_odu as u16) << 8 | primary_odu as u16,
            secondary_odu: primary_odu,
            experience_weight: 0.0,
            receipt_count: 0,
        };
    }

    // Mean gate alignment over the window.
    let experience_weight: f64 = window
        .iter()
        .map(|r| r.gate_alignment.unwrap_or(1.0))
        .sum::<f64>()
        / receipt_count as f64;

    // Derive secondary Odù: SHA-256(primary || receipt_ids || alignment_byte).
    let alignment_byte = (experience_weight * 255.0).round() as u8;
    let mut hasher = Sha256::new();
    hasher.update([primary_odu]);
    for r in &window {
        hasher.update(r.receipt_id.as_bytes());
    }
    hasher.update([alignment_byte]);
    hasher.update(b"omokoda-odu-composition-v1");
    let digest = hasher.finalize();
    let secondary_odu = digest[0];

    let composed_odu = (primary_odu as u16) << 8 | secondary_odu as u16;

    OduCompositionResult {
        composed_odu,
        secondary_odu,
        experience_weight,
        receipt_count,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::identity::AgentId;
    use crate::receipt::act_receipt::ActReceipt;

    fn make_receipt(id_suffix: &str, alignment: f64) -> ActReceipt {
        ActReceipt::new(
            AgentId::from_str(&format!("agent-{id_suffix}")),
            "test_tool".to_string(),
            "output".to_string(),
            1_700_000_000 + id_suffix.len() as u64,
        )
        .with_gate_alignment(alignment)
    }

    #[test]
    fn no_receipts_mirrors_primary() {
        let result = compose_odu(42, &[]);
        assert_eq!(result.secondary_odu, 42);
        assert_eq!(result.composed_odu, (42u16 << 8) | 42);
        assert_eq!(result.receipt_count, 0);
    }

    #[test]
    fn same_receipts_deterministic() {
        let r1 = make_receipt("a", 1.0);
        let r2 = make_receipt("b", 0.9);
        let refs: Vec<&ActReceipt> = vec![&r1, &r2];
        let a = compose_odu(7, &refs);
        let b = compose_odu(7, &refs);
        assert_eq!(a, b);
    }

    #[test]
    fn different_histories_diverge() {
        let r1 = make_receipt("x", 1.0);
        let r2 = make_receipt("y", 1.0);
        let refs_a: Vec<&ActReceipt> = vec![&r1];
        let refs_b: Vec<&ActReceipt> = vec![&r2];
        let a = compose_odu(10, &refs_a);
        let b = compose_odu(10, &refs_b);
        // Different receipt ids → different secondary_odu (overwhelmingly likely)
        assert_ne!(a.composed_odu, b.composed_odu);
    }

    #[test]
    fn same_birth_different_alignment_diverges() {
        let r_high = make_receipt("h", 1.0);
        let r_low  = make_receipt("h", 0.5); // same id suffix, different alignment
        // Force distinct receipt_ids by using different suffixes
        let r_low2 = make_receipt("l", 0.5);
        let refs_high: Vec<&ActReceipt> = vec![&r_high];
        let refs_low:  Vec<&ActReceipt> = vec![&r_low2];
        let a = compose_odu(5, &refs_high);
        let b = compose_odu(5, &refs_low);
        assert_ne!(a.experience_weight, b.experience_weight);
    }

    #[test]
    fn window_caps_at_16() {
        let receipts: Vec<ActReceipt> = (0..20)
            .map(|i| make_receipt(&i.to_string(), 1.0))
            .collect();
        let refs: Vec<&ActReceipt> = receipts.iter().collect();
        let result = compose_odu(3, &refs);
        assert_eq!(result.receipt_count, 16);
    }

    #[test]
    fn composed_odu_high_byte_is_primary() {
        let r = make_receipt("p", 1.0);
        let refs: Vec<&ActReceipt> = vec![&r];
        let result = compose_odu(200, &refs);
        assert_eq!((result.composed_odu >> 8) as u8, 200);
    }
}
