pub mod entropy;
pub mod flow;
pub mod fractal;
pub mod justice;
pub mod network_guard;
pub mod persona;
pub mod plane;
pub mod rule_engine;
pub mod safety;
pub mod seven;
pub mod spiral;
pub mod polyglot;
pub mod wisdom;

use hkdf::Hkdf;
use serde::{Deserialize, Serialize};
use sha2::Sha256;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HermeticState {
    mentalism: f64,
    correspondence: f64,
    vibration: f64,
    polarity: f64,
    rhythm: f64,
    cause_effect: f64,
    gender: f64,
}

impl HermeticState {
    pub fn from_odu_seed(seed: &[u8; 32]) -> Self {
        let hk = Hkdf::<Sha256>::new(None, seed);
        let mut okm = [0u8; 28]; // 7 principles * 4 bytes each
        hk.expand(b"omokoda-hermetic-v1", &mut okm)
            .expect("HKDF expansion failed");

        let p = |start: usize| {
            let val = u32::from_le_bytes(okm[start..start + 4].try_into().unwrap());
            val as f64 / u32::MAX as f64
        };

        Self {
            mentalism: p(0),
            correspondence: p(4),
            vibration: p(8),
            polarity: p(12),
            rhythm: p(16),
            cause_effect: p(20),
            gender: p(24),
        }
    }

    // Legacy method for backward compatibility if needed, but updated to use the new logic
    pub fn from_seed(name: &str, timestamp: u64) -> Self {
        let mut hasher = blake3::Hasher::new();
        hasher.update(name.as_bytes());
        hasher.update(&timestamp.to_le_bytes());
        let digest = hasher.finalize();
        let mut seed = [0u8; 32];
        seed.copy_from_slice(digest.as_bytes());
        Self::from_odu_seed(&seed)
    }

    pub fn mentalism(&self) -> f64 {
        self.mentalism
    }
    pub fn correspondence(&self) -> f64 {
        self.correspondence
    }
    pub fn vibration(&self) -> f64 {
        self.vibration
    }
    pub fn polarity(&self) -> f64 {
        self.polarity
    }
    pub fn rhythm(&self) -> f64 {
        self.rhythm
    }
    pub fn cause_effect(&self) -> f64 {
        self.cause_effect
    }
    pub fn gender(&self) -> f64 {
        self.gender
    }

    #[cfg(test)]
    pub fn from_raw(
        mentalism: f64,
        correspondence: f64,
        vibration: f64,
        polarity: f64,
        rhythm: f64,
        cause_effect: f64,
        gender: f64,
    ) -> Self {
        Self {
            mentalism,
            correspondence,
            vibration,
            polarity,
            rhythm,
            cause_effect,
            gender,
        }
    }

    // Logic aliases
    pub fn think_abstraction_depth(&self) -> f64 {
        self.mentalism
    }

    pub fn act_cooldown_ms(&self) -> u64 {
        // Use Rhythm to determine base cooldown
        (self.rhythm * 1000.0) as u64
    }

    pub fn fingerprint(&self) -> String {
        let mut hasher = blake3::Hasher::new();
        hasher.update(&self.mentalism.to_le_bytes());
        hasher.update(&self.correspondence.to_le_bytes());
        hasher.update(&self.vibration.to_le_bytes());
        hasher.update(&self.polarity.to_le_bytes());
        hasher.update(&self.rhythm.to_le_bytes());
        hasher.update(&self.cause_effect.to_le_bytes());
        hasher.update(&self.gender.to_le_bytes());
        hasher.finalize().to_hex().to_string()
    }
}
