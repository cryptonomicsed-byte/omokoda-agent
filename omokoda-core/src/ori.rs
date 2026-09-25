// ori.rs — Ori state: the agent's living identity topology.
// Derived deterministically from birth entropy. Never user-edited.
// Hash-chained across revisions for tamper-evidence.
//
// ENERGY NAMES:
//   SPARK = Rust (this file)
//   MIND  = Clojure (query / reasoning layer)
//   EMOTION = Julia (simulation / training)
//   FOUNDATION = Python (tooling / gateway)
//   FIRE = Move (on-chain contracts)

use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

type HmacSha256 = Hmac<Sha256>;

// ── Vessel index ─────────────────────────────────────────────────────────────
// 16 vessels in canonical order. The index position determines which byte
// region of the HMAC output is used during genesis weight derivation.

pub const VESSEL_NAMES: [&str; 16] = [
    "Genesis",    // 0
    "Void",       // 1
    "Attention",  // 2
    "Loop",       // 3
    "Receipt",    // 4
    "Mask",       // 5
    "Residue",    // 6
    "Execution",  // 7
    "Swarm",      // 8
    "Restraint",  // 9
    "Migration",  // 10
    "Consent",    // 11
    "Vision",     // 12
    "Growth",     // 13
    "Seal",       // 14
    "Rhythm",     // 15
];

// ── Core struct ───────────────────────────────────────────────────────────────

/// Ori is the agent's living identity topology — a sealed, hash-chained
/// record of vessel weights derived from birth entropy (SPARK layer).
///
/// Rules:
/// - `birth_entropy_hash` is immutable after genesis.
/// - `vessel_weights` are derived deterministically; never user-edited.
/// - `state_hash` covers all fields *except* `state_hash` itself.
/// - `previous_ori_hash` is sixty-four `'0'` characters at revision 0.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Ori {
    /// Monotonically increasing revision counter. Starts at 0.
    pub ori_revision: u64,

    /// Normalised weights [0.0, 1.0] for each of the 16 vessels.
    /// Order matches `VESSEL_NAMES`.
    pub vessel_weights: [f32; 16],

    /// SHA-256 (hex) of the canonical JSON serialisation of this Ori,
    /// excluding the `state_hash` field itself.
    pub state_hash: String,

    /// SHA-256 (hex) of the raw birth entropy bytes. Set once at genesis;
    /// never changes across revisions.
    pub birth_entropy_hash: String,

    /// IfáScript version string at the time this Ori was created/advanced.
    pub ifascript_version: String,

    /// `state_hash` of the immediately preceding Ori revision.
    /// Equals sixty-four `'0'` characters for revision 0 (genesis).
    pub previous_ori_hash: String,

    /// Cumulative count of experiences that have advanced this Ori.
    pub experience_count: u64,
}

// ── Wire type used for hashing (excludes state_hash) ─────────────────────────

/// Intermediate structure used exclusively for canonical JSON hashing.
/// Mirrors `Ori` but omits `state_hash`.
#[derive(Serialize)]
struct OriForHash<'a> {
    ori_revision: u64,
    vessel_weights: &'a [f32; 16],
    birth_entropy_hash: &'a str,
    ifascript_version: &'a str,
    previous_ori_hash: &'a str,
    experience_count: u64,
}

// ── Implementation ────────────────────────────────────────────────────────────

impl Ori {
    /// Genesis constructor — derives the initial vessel weights from
    /// `entropy_seed` using an HMAC-SHA256 "HKDF-like" approach:
    ///
    ///   For each vessel name V:
    ///     mac = HMAC-SHA256(key=entropy_seed, data=V)
    ///     take first 4 bytes → u32 → normalise to [0.0, 1.0]
    ///
    /// Sets revision=0, previous_ori_hash to all-zeros, and computes
    /// `state_hash` and `birth_entropy_hash`.
    pub fn genesis(entropy_seed: &[u8], ifascript_version: &str) -> Self {
        let mut vessel_weights = [0.0f32; 16];
        for (i, name) in VESSEL_NAMES.iter().enumerate() {
            vessel_weights[i] = derive_vessel_weight(entropy_seed, name);
        }

        let birth_entropy_hash = sha256_hex(entropy_seed);
        let previous_ori_hash = "0".repeat(64);

        let mut ori = Ori {
            ori_revision: 0,
            vessel_weights,
            state_hash: String::new(),
            birth_entropy_hash,
            ifascript_version: ifascript_version.to_owned(),
            previous_ori_hash,
            experience_count: 0,
        };

        ori.state_hash = ori.compute_state_hash();
        ori
    }

    /// Advance — produces the next Ori revision from the current one.
    ///
    /// - Increments `ori_revision` by 1.
    /// - Adds `experience_delta` to `experience_count`.
    /// - Sets `previous_ori_hash` to current `state_hash`.
    /// - `vessel_weights` and `birth_entropy_hash` are preserved as-is.
    /// - Recomputes `state_hash` for the new revision.
    pub fn advance(&self, experience_delta: u64) -> Self {
        let mut next = Ori {
            ori_revision: self.ori_revision + 1,
            vessel_weights: self.vessel_weights,
            state_hash: String::new(),
            birth_entropy_hash: self.birth_entropy_hash.clone(),
            ifascript_version: self.ifascript_version.clone(),
            previous_ori_hash: self.state_hash.clone(),
            experience_count: self.experience_count + experience_delta,
        };
        next.state_hash = next.compute_state_hash();
        next
    }

    /// Compute (or recompute) the SHA-256 hash of the canonical JSON
    /// serialisation of this Ori, excluding the `state_hash` field.
    pub fn state_hash(&self) -> String {
        self.compute_state_hash()
    }

    /// Returns a display vector of `(vessel_name, filled_circles_string)` for
    /// all 16 vessels.  Each string shows up to 5 bullets:
    ///   `weight * 5.0` filled bullets (`●`) followed by empty bullets (`○`).
    ///
    /// Example: weight 0.6  →  "●●●○○"
    pub fn vessel_display(&self) -> Vec<(String, String)> {
        VESSEL_NAMES
            .iter()
            .enumerate()
            .map(|(i, name)| {
                let weight = self.vessel_weights[i];
                let filled = (weight * 5.0).round() as usize;
                let filled = filled.min(5);
                let empty = 5 - filled;
                let display = "●".repeat(filled) + &"○".repeat(empty);
                (name.to_string(), display)
            })
            .collect()
    }

    fn compute_state_hash(&self) -> String {
        let wire = OriForHash {
            ori_revision: self.ori_revision,
            vessel_weights: &self.vessel_weights,
            birth_entropy_hash: &self.birth_entropy_hash,
            ifascript_version: &self.ifascript_version,
            previous_ori_hash: &self.previous_ori_hash,
            experience_count: self.experience_count,
        };
        let json_bytes = serde_json::to_vec(&wire)
            .expect("OriForHash serialisation is infallible");
        sha256_hex(&json_bytes)
    }
}

// ── Free helpers ──────────────────────────────────────────────────────────────

fn derive_vessel_weight(entropy_seed: &[u8], vessel_name: &str) -> f32 {
    let mut mac = HmacSha256::new_from_slice(entropy_seed)
        .expect("HMAC-SHA256 accepts any key length");
    mac.update(vessel_name.as_bytes());
    let result = mac.finalize().into_bytes();
    let raw = u32::from_be_bytes([result[0], result[1], result[2], result[3]]);
    raw as f32 / u32::MAX as f32
}

fn sha256_hex(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    format!("{:x}", hasher.finalize())
}

// ── Birth persistence ─────────────────────────────────────────────────────────

/// Minimal sealed birth receipt written alongside the Ori at agent birth.
/// Not a full ARP receipt — it is a lightweight local proof-of-birth that
/// records which Ori revision was sealed and what entropy it was derived from,
/// without including any secret material.
#[derive(Debug, Serialize, Deserialize)]
pub struct OriBirthReceipt {
    pub agent_id: String,
    pub ori_revision: u64,
    pub ori_state_hash: String,
    pub birth_entropy_hash: String,
    pub ifascript_version: String,
    /// Unix timestamp seconds at which this receipt was written.
    pub born_at: u64,
}

/// Generate the genesis Ori from `entropy_seed`, persist it to
/// `~/.omokoda/state/ori/ori.json`, and write a sealed birth receipt to
/// `~/.omokoda/receipts/births/birth.receipt.json`.
///
/// # Fail-open contract
/// This function logs warnings on I/O errors but never returns a hard
/// failure — a disk write error must not abort a successful birth.  The
/// caller (birth flow) treats the `Err` variant as a warning, not a
/// fatal condition.
pub fn persist_ori_at_birth(
    agent_id: &str,
    entropy_seed: &[u8],
    ifascript_version: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    use crate::home::HomeDir;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    let ori = Ori::genesis(entropy_seed, ifascript_version);

    let ori_json =
        serde_json::to_string_pretty(&ori).map_err(|e| format!("Ori serialize: {e}"))?;

    let home = HomeDir::new();

    // Ensure both target directories exist (idempotent).
    let ori_dir = home.state.join("ori");
    fs::create_dir_all(&ori_dir)
        .map_err(|e| format!("create ori dir {}: {e}", ori_dir.display()))?;

    let receipts_births_dir = home.receipts.join("births");
    fs::create_dir_all(&receipts_births_dir).map_err(|e| {
        format!(
            "create receipts/births dir {}: {e}",
            receipts_births_dir.display()
        )
    })?;

    // Write Ori state.
    let ori_path = home.ori_json();
    fs::write(&ori_path, ori_json.as_bytes())
        .map_err(|e| format!("write ori.json {}: {e}", ori_path.display()))?;

    // Build and write birth receipt.
    let born_at = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    let receipt = OriBirthReceipt {
        agent_id: agent_id.to_owned(),
        ori_revision: ori.ori_revision,
        ori_state_hash: ori.state_hash.clone(),
        birth_entropy_hash: ori.birth_entropy_hash.clone(),
        ifascript_version: ifascript_version.to_owned(),
        born_at,
    };

    let receipt_json = serde_json::to_string_pretty(&receipt)
        .map_err(|e| format!("OriBirthReceipt serialize: {e}"))?;

    let receipt_path = home.birth_receipt();
    fs::write(&receipt_path, receipt_json.as_bytes())
        .map_err(|e| format!("write birth.receipt.json {}: {e}", receipt_path.display()))?;

    Ok(())
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_seed() -> Vec<u8> {
        b"test-entropy-seed-for-ori-genesis".to_vec()
    }

    #[test]
    fn genesis_revision_zero() {
        let ori = Ori::genesis(&sample_seed(), "0.1.0");
        assert_eq!(ori.ori_revision, 0);
        assert_eq!(ori.experience_count, 0);
        assert_eq!(ori.previous_ori_hash, "0".repeat(64));
    }

    #[test]
    fn genesis_all_weights_in_range() {
        let ori = Ori::genesis(&sample_seed(), "0.1.0");
        for w in ori.vessel_weights.iter() {
            assert!(*w >= 0.0 && *w <= 1.0, "weight out of range: {w}");
        }
    }

    #[test]
    fn genesis_state_hash_matches() {
        let ori = Ori::genesis(&sample_seed(), "0.1.0");
        assert_eq!(ori.state_hash, ori.compute_state_hash());
    }

    #[test]
    fn birth_entropy_hash_is_immutable_across_revisions() {
        let ori0 = Ori::genesis(&sample_seed(), "0.1.0");
        let ori1 = ori0.advance(10);
        let ori2 = ori1.advance(5);
        assert_eq!(ori0.birth_entropy_hash, ori1.birth_entropy_hash);
        assert_eq!(ori1.birth_entropy_hash, ori2.birth_entropy_hash);
    }

    #[test]
    fn advance_chains_hashes() {
        let ori0 = Ori::genesis(&sample_seed(), "0.1.0");
        let ori1 = ori0.advance(1);
        assert_eq!(ori1.previous_ori_hash, ori0.state_hash);
        assert_eq!(ori1.ori_revision, 1);
        assert_eq!(ori1.experience_count, 1);
    }

    #[test]
    fn vessel_display_length() {
        let ori = Ori::genesis(&sample_seed(), "0.1.0");
        let display = ori.vessel_display();
        assert_eq!(display.len(), 16);
        for (name, bullets) in &display {
            let count = bullets.chars().count();
            assert_eq!(count, 5, "vessel '{name}' display has wrong length: '{bullets}'");
        }
    }

    #[test]
    fn vessel_display_names_match_constants() {
        let ori = Ori::genesis(&sample_seed(), "0.1.0");
        let display = ori.vessel_display();
        for (i, (name, _)) in display.iter().enumerate() {
            assert_eq!(name.as_str(), VESSEL_NAMES[i]);
        }
    }

    #[test]
    fn state_hash_public_method_matches_field() {
        let ori = Ori::genesis(&sample_seed(), "0.1.0");
        assert_eq!(ori.state_hash(), ori.state_hash);
    }

    #[test]
    fn deterministic_genesis_same_seed() {
        let ori_a = Ori::genesis(&sample_seed(), "0.1.0");
        let ori_b = Ori::genesis(&sample_seed(), "0.1.0");
        assert_eq!(ori_a.state_hash, ori_b.state_hash);
        assert_eq!(ori_a.vessel_weights, ori_b.vessel_weights);
    }

    #[test]
    fn different_seeds_produce_different_weights() {
        let ori_a = Ori::genesis(b"seed-alpha", "0.1.0");
        let ori_b = Ori::genesis(b"seed-beta", "0.1.0");
        assert_ne!(ori_a.vessel_weights, ori_b.vessel_weights);
    }
}
