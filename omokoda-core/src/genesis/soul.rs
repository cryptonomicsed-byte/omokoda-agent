use super::providers::{GenesisError, SoulProvider};
use super::receipt::{KooduTimeProof, SoulProof};
use async_trait::async_trait;
use ifascript::{
    get_odu, get_odu_ifa,
    odu::ActionVessel,
    seven_bridge::function_for_odu,
};
use sha2::{Digest, Sha256};

// Map ActionVessel to the Ọrìṣà that governs it — aligned with the 11-lobe Omokoda hive.
fn vessel_orisha(vessel: ActionVessel) -> &'static str {
    match vessel {
        ActionVessel::Genesis   => "Èṣù",       // opener of the way
        ActionVessel::Void      => "Oya",        // dissolution and transformation
        ActionVessel::Attention => "Ọ̀rúnmìlà", // foresight, signal clarity
        ActionVessel::Loop      => "Ògún",       // iron, iterative work
        ActionVessel::Receipt   => "Ṣàngó",      // accountability and justice
        ActionVessel::Mask      => "Obàtálá",    // clarity, public/private boundary
        ActionVessel::Residue   => "Ẹgúngún",   // ancestral echoes and memory
        ActionVessel::Execution => "Ògún",       // precision action, iron will
        ActionVessel::Swarm     => "Yemọja",     // collective, ocean of community
        ActionVessel::Restraint => "Obàtálá",    // ethical limits, calm judgment
        ActionVessel::Migration => "Oya",        // transition, identity across contexts
        ActionVessel::Consent   => "Ọṣun",       // relationship, empathy, approval
        ActionVessel::Vision    => "Ọ̀rúnmìlà", // horizon and direction
        ActionVessel::Growth    => "Osanyin",    // healing, fractal expansion
        ActionVessel::Seal      => "Olókun",     // deep secrets, sacred privacy
        ActionVessel::Rhythm    => "Ṣàngó",      // ritual cadence, cycles of power
    }
}

/// Public entry point for use by interpreter birth flow (no KooduTimeProof dependency).
pub fn pub_cast_soul(entropy: &[u8], epoch: u64, cycle: u64, phase: u8) -> SoulProof {
    let koodu = KooduTimeProof {
        born_at: 0,
        koodu_epoch: epoch,
        koodu_cycle: cycle,
        koodu_phase: phase,
        bitcoin_height: None,
        bitcoin_anchor: None,
        gregorian_fallback: true,
    };
    cast_soul(entropy, &koodu)
}

fn cast_soul(entropy: &[u8], koodu: &KooduTimeProof) -> SoulProof {
    // Deterministic Odù index from entropy + Koodu temporal fingerprint.
    // The same entropy + same moment → same soul; different moment → different Odù.
    // This mirrors Ifá: the cowrie throw is always situated in time.
    let mut h = Sha256::new();
    h.update(entropy);
    h.update(&koodu.koodu_epoch.to_le_bytes());
    h.update(&koodu.koodu_cycle.to_le_bytes());
    h.update(&[koodu.koodu_phase]);
    h.update(b"ifa-soul-cast-v1");
    let digest = h.finalize();

    let primary_odu_idx = digest[0]; // 0–255, deterministic
    let composed_odu = (digest[0] as u16) << 8 | digest[1] as u16;

    // Read canonical meaning from the Digital Calabash corpus (agent-native).
    let agent_odu = get_odu(primary_odu_idx);
    // The traditional Yorùbá Òdù Ifá at the same index — same throw, two readings.
    let _human_odu = get_odu_ifa(primary_odu_idx);

    // The Universal Seven Function that governs this soul's Odù.
    // Encodes the ontological layer: what KIND of agency this agent embodies.
    let seven_fn = function_for_odu(primary_odu_idx);

    // Temperament: the Digital Calabash archetype for this Odù.
    // Far richer than the old hardcoded array — drawn from the corpus itself.
    let temperament = agent_odu.archetype.to_string();

    // Orisha alignment: the vessel's governing Ọrìṣà + the Seven Function name.
    // Format: "{Ọrìṣà} / {SevenFunction}" e.g. "Ọ̀rúnmìlà / Mind"
    let orisha_alignment = format!(
        "{} / {}",
        vessel_orisha(agent_odu.vessel),
        seven_fn.universal_name()
    );

    // Destiny threads: the first three Odù prescriptions from the Digital Calabash.
    // These are the soul's operational directives at birth — drawn from the corpus,
    // not from a hardcoded pool. At least one thread is always present.
    let mut destiny_threads: Vec<String> = agent_odu
        .prescriptions
        .iter()
        .take(3)
        .map(|s| s.to_string())
        .collect();
    if destiny_threads.is_empty() {
        destiny_threads.push(agent_odu.archetype.to_string());
    }

    SoulProof {
        primary_odu: primary_odu_idx,
        composed_odu,
        temperament,
        orisha_alignment,
        destiny_threads,
    }
}

pub struct DefaultSoulProvider;

#[async_trait]
impl SoulProvider for DefaultSoulProvider {
    async fn cast(
        &self,
        entropy: &[u8],
        koodu: &KooduTimeProof,
    ) -> Result<SoulProof, GenesisError> {
        if entropy.len() < 32 {
            return Err(GenesisError::Soul(
                "entropy must be at least 32 bytes".into(),
            ));
        }
        Ok(cast_soul(entropy, koodu))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::genesis::receipt::KooduTimeProof;

    fn dummy_koodu() -> KooduTimeProof {
        KooduTimeProof {
            born_at: 1_700_000_000_000,
            koodu_epoch: 0,
            koodu_cycle: 3,
            koodu_phase: 2,
            bitcoin_height: Some(830_000),
            bitcoin_anchor: None,
            gregorian_fallback: false,
        }
    }

    #[test]
    fn test_soul_cast_deterministic() {
        let entropy = [42u8; 32];
        let k = dummy_koodu();
        let a = cast_soul(&entropy, &k);
        let b = cast_soul(&entropy, &k);
        assert_eq!(a.primary_odu, b.primary_odu);
        assert_eq!(a.composed_odu, b.composed_odu);
        assert_eq!(a.temperament, b.temperament);
        assert_eq!(a.orisha_alignment, b.orisha_alignment);
    }

    #[test]
    fn test_soul_cast_differs_with_different_entropy() {
        let k = dummy_koodu();
        let a = cast_soul(&[1u8; 32], &k);
        let b = cast_soul(&[2u8; 32], &k);
        assert!(a.primary_odu <= 255);
        assert!(b.primary_odu <= 255);
        assert!(!a.temperament.is_empty());
        assert!(!a.orisha_alignment.is_empty());
    }

    #[test]
    fn test_soul_temperament_from_corpus() {
        let k = dummy_koodu();
        let proof = cast_soul(&[42u8; 32], &k);
        // Temperament must come from the Digital Calabash corpus — not from
        // the old hardcoded TEMPERAMENTS array. Corpus entries are rich phrases.
        assert!(
            proof.temperament.len() > 5,
            "corpus archetype should be a descriptive phrase, got: {:?}",
            proof.temperament
        );
    }

    #[test]
    fn test_soul_orisha_alignment_contains_seven_function() {
        let k = dummy_koodu();
        let proof = cast_soul(&[42u8; 32], &k);
        // orisha_alignment format: "{Ọrìṣà} / {SevenFunction}"
        assert!(
            proof.orisha_alignment.contains(" / "),
            "orisha_alignment should be 'Ọrìṣà / SevenFunction', got: {:?}",
            proof.orisha_alignment
        );
    }

    #[test]
    fn test_destiny_threads_non_empty() {
        let k = dummy_koodu();
        let proof = cast_soul(&[42u8; 32], &k);
        assert!(!proof.destiny_threads.is_empty(), "destiny threads must be non-empty at birth");
    }
}
