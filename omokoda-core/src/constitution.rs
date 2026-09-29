// constitution.rs — AgentConstitution: canonical birth DNA document
// Compiles hermetic gates + Koodu score + BIPON39 + Odù into one signable document.

use serde::{Deserialize, Serialize};


/// All birth parameters compiled into one canonical, signable document.
/// Immutable once created — represents the agent's permanent identity.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConstitution {
    // ── Immutable identity ────────────────────────────────────────────────────
    pub bipon39_phrase: String,
    pub odu_index: u8,
    pub hermetic_dna: [f32; 7],
    pub koodu_birth_score: u8,
    pub birth_timestamp: u64,
    pub birth_btc_height: Option<u64>,
    pub nostr_pubkey_hex: String,
    #[serde(default)]
    pub sui_soul_object_id: Option<String>,

    // ── Computed (deterministic from above) ───────────────────────────────────
    pub gate_alignment_seed: f64,
    pub behavioral_archetype: String,
    pub odu_name: String,

    // ── Signature ─────────────────────────────────────────────────────────────
    #[serde(default)]
    pub signature_hex: Option<String>,
}

impl AgentConstitution {
    pub fn new(
        bipon39_phrase: String,
        odu_index: u8,
        hermetic_dna: [f32; 7],
        koodu_birth_score: u8,
        birth_timestamp: u64,
        birth_btc_height: Option<u64>,
        nostr_pubkey_hex: String,
    ) -> Self {
        let gate_alignment_seed = compute_gate_alignment(&hermetic_dna);
        let behavioral_archetype = derive_archetype(odu_index, &hermetic_dna);
        let odu_name = odu_name_for(odu_index);
        Self {
            bipon39_phrase,
            odu_index,
            hermetic_dna,
            koodu_birth_score,
            birth_timestamp,
            birth_btc_height,
            nostr_pubkey_hex,
            sui_soul_object_id: None,
            gate_alignment_seed,
            behavioral_archetype,
            odu_name,
            signature_hex: None,
        }
    }

    /// Canonical bytes to sign: deterministic serialization of the immutable fields.
    pub fn signing_bytes(&self) -> Vec<u8> {
        let payload = format!(
            "{}|{}|{}|{}|{}|{}",
            self.bipon39_phrase,
            self.odu_index,
            self.birth_timestamp,
            self.nostr_pubkey_hex,
            self.koodu_birth_score,
            self.hermetic_dna
                .iter()
                .map(|v| format!("{v:.6}"))
                .collect::<Vec<_>>()
                .join(",")
        );
        payload.into_bytes()
    }

    pub fn content_hash(&self) -> [u8; 32] {
        *blake3::hash(&self.signing_bytes()).as_bytes()
    }

    pub fn is_signed(&self) -> bool {
        self.signature_hex.is_some()
    }

    /// Human-readable summary suitable for Nostr kind 0 "about" field.
    pub fn summary(&self) -> String {
        format!(
            "Odù: {} | Archetype: {} | Born: {} | Gate Seed: {:.3}",
            self.odu_name,
            self.behavioral_archetype,
            self.birth_timestamp,
            self.gate_alignment_seed
        )
    }
}

fn compute_gate_alignment(dna: &[f32; 7]) -> f64 {
    // Mean deviation from perfect balance (all 7 values equal).
    // 0.0 = perfectly balanced, 1.0 = maximally imbalanced.
    let mean = dna.iter().map(|v| *v as f64).sum::<f64>() / 7.0;
    let variance = dna.iter().map(|v| (*v as f64 - mean).powi(2)).sum::<f64>() / 7.0;
    // Normalize to [0, 1] — max variance when one gate = 1.0, rest = 0.0 is 6/49 ≈ 0.122
    let balance = 1.0 - (variance / 0.122_f64).min(1.0);
    // Seed = balance score in [0,1]
    balance
}

fn derive_archetype(odu_index: u8, dna: &[f32; 7]) -> String {
    let archetype_names = [
        "Spark",
        "Mind",
        "Foundation",
        "Emotion",
        "Womb",
        "Fire",
        "Ascension",
    ];
    // Dominant gate = the principle with the highest score
    let dominant_idx = dna
        .iter()
        .enumerate()
        .max_by(|a, b| a.1.partial_cmp(b.1).unwrap_or(std::cmp::Ordering::Equal))
        .map(|(i, _)| i)
        .unwrap_or(0);
    format!("Odù-{} — {}", odu_index, archetype_names[dominant_idx % 7])
}

fn odu_name_for(index: u8) -> String {
    // 256 Odù names — abbreviated list for common positions
    let names = [
        "Ogbe Meji",
        "Oyeku Meji",
        "Iwori Meji",
        "Odi Meji",
        "Irosun Meji",
        "Owonrin Meji",
        "Obara Meji",
        "Okanran Meji",
        "Ogunda Meji",
        "Osa Meji",
        "Ika Meji",
        "Oturupon Meji",
        "Otura Meji",
        "Irete Meji",
        "Ose Meji",
        "Ofun Meji",
    ];
    if (index as usize) < names.len() {
        names[index as usize].to_string()
    } else {
        format!("Odù-{index}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constitution_constructs() {
        let c = AgentConstitution::new(
            "FIRE-OYA-MOON".to_string(),
            5,
            [0.8, 0.6, 0.4, 0.9, 0.3, 0.7, 0.5],
            42,
            1_700_000_000,
            Some(780_000),
            "deadbeef".repeat(8),
        );
        assert_eq!(c.odu_index, 5);
        assert!(!c.is_signed());
        assert!(!c.summary().is_empty());
    }

    #[test]
    fn content_hash_deterministic() {
        let c = AgentConstitution::new(
            "RAIN-FORGE-STAR".to_string(),
            3,
            [0.5; 7],
            10,
            1_700_000_001,
            None,
            "cafebabe".repeat(8),
        );
        let h1 = c.content_hash();
        let h2 = c.content_hash();
        assert_eq!(h1, h2);
    }

    #[test]
    fn balanced_dna_high_alignment() {
        let balanced = [0.5f32; 7];
        let score = compute_gate_alignment(&balanced);
        assert!(score > 0.95, "balanced DNA should yield high alignment");
    }
}
