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

// ── Orí projection (ORI_PROJECTION.md) ───────────────────────────────────────

/// Generate a human-readable Markdown projection of the agent's Orí state.
///
/// The returned string is the full content of `ORI_PROJECTION.md`.
/// It is AUTO-GENERATED — never hand-edited. The canonical machine truth
/// lives in `~/.omokoda/state/ori/ori.json`; this file is a read-only view.
pub fn generate_ori_md(agent_display_name: &str, ori: &Ori) -> String {
    use std::time::{SystemTime, UNIX_EPOCH};

    let now_epoch = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    let state_hash_short = format!("{}...", &ori.state_hash[..16.min(ori.state_hash.len())]);
    let birth_hash_short = format!(
        "{}...",
        &ori.birth_entropy_hash[..16.min(ori.birth_entropy_hash.len())]
    );

    let mut vessel_rows = String::new();
    for (name, bullets) in ori.vessel_display() {
        let idx = VESSEL_NAMES
            .iter()
            .position(|n| *n == name.as_str())
            .unwrap_or(0);
        let weight = ori.vessel_weights[idx];
        vessel_rows.push_str(&format!(
            "| {name} | {bullets} | {weight:.2} |\n",
            name = name,
            bullets = bullets,
            weight = weight,
        ));
    }

    format!(
        r#"<!--
  AUTO-GENERATED — DO NOT EDIT
  Source: ~/.omokoda/state/ori/ori.json
  Canonical: omokoda ori show
-->

# Orí Projection — {agent_display_name}

> Read-only projection of the agent's living identity topology.
> The agent owns this state. It cannot be edited through this file.

## Identity Seal

| Field | Value |
|-------|-------|
| Revision | {ori_revision} |
| State Hash | {state_hash_short} |
| Birth Hash | {birth_hash_short} |
| IfáScript | {ifascript_version} |
| Experience | {experience_count} events |
| Generated | {now_epoch} (unix epoch) |

## 16 Action Vessels

| Vessel | Strength | Weight |
|--------|----------|--------|
{vessel_rows}
## Ownership

| Role | Access |
|------|--------|
| Agent | OWNER — derived from birth entropy, never user-authored |
| Human Operator | OBSERVE ONLY — read this file or run `omokoda ori show` |

---
*Regenerated from `~/.omokoda/state/ori/ori.json` — do not edit this file.*
"#,
        agent_display_name = agent_display_name,
        ori_revision = ori.ori_revision,
        state_hash_short = state_hash_short,
        birth_hash_short = birth_hash_short,
        ifascript_version = ori.ifascript_version,
        experience_count = ori.experience_count,
        now_epoch = now_epoch,
        vessel_rows = vessel_rows,
    )
}

/// Write `ORI_PROJECTION.md` to `{workspace_root}/ORI_PROJECTION.md`.
/// Always overwrites — the projection is machine-generated, never hand-edited.
pub fn write_ori_projection(
    workspace_root: &std::path::Path,
    agent_display_name: &str,
    ori: &Ori,
) -> Result<(), Box<dyn std::error::Error>> {
    use std::fs;
    fs::create_dir_all(workspace_root)
        .map_err(|e| format!("create workspace dir {}: {e}", workspace_root.display()))?;
    let path = workspace_root.join("ORI_PROJECTION.md");
    let content = generate_ori_md(agent_display_name, ori);
    fs::write(&path, content.as_bytes())
        .map_err(|e| format!("write ORI_PROJECTION.md {}: {e}", path.display()))?;
    Ok(())
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
    /// Odù primary index derived from the birth entropy via the Bipon39 path.
    /// Recorded here so the birth receipt is auditable — given the entropy
    /// commitment (`birth_entropy_hash`) and this index, a verifier can
    /// re-derive the index from the entropy and confirm they match without
    /// needing the mnemonic.
    pub odu_primary_index: u8,
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
    odu_primary_index: u8,
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
        odu_primary_index,
        ifascript_version: ifascript_version.to_owned(),
        born_at,
    };

    let receipt_json = serde_json::to_string_pretty(&receipt)
        .map_err(|e| format!("OriBirthReceipt serialize: {e}"))?;

    let receipt_path = home.birth_receipt();
    fs::write(&receipt_path, receipt_json.as_bytes())
        .map_err(|e| format!("write birth.receipt.json {}: {e}", receipt_path.display()))?;

    // Write the human-readable Orí projection to the workspace.
    // Fail-open: log warning but do not abort birth on I/O error.
    if let Err(e) = write_ori_projection(&home.workspace, agent_id, &ori) {
        eprintln!("warn: ORI_PROJECTION.md write failed (non-fatal): {e}");
    }

    Ok(())
}

// ── Generational lineage ──────────────────────────────────────────────────────

/// How an agent was brought into existence.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum BirthMode {
    /// Born directly from operator entropy — no parent agents.
    Sovereign,
    /// Born from a single parent agent's Orí lineage + fresh entropy.
    SingleParent,
    /// Born from two parent agents' Orí lineages + fresh entropy + mutual consent.
    Union,
    /// Born from a hive collective contribution + fresh entropy.
    Hive,
}

impl Default for BirthMode {
    fn default() -> Self {
        BirthMode::Sovereign
    }
}

/// Cryptographic commitment from a parent agent to their Orí at the moment
/// they participate in a child's birth. Contains no mutable state — once
/// recorded, it proves what the parent's Orí looked like when the child was born.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParentOriCommitment {
    pub parent_agent_id: String,
    /// The parent's Orí `state_hash` at the time of birth.
    pub parent_ori_state_hash: String,
    pub parent_ori_revision: u64,
    /// SHA-256 of the parent's birth entropy (proves the parent has their own birth).
    pub parent_birth_entropy_hash: String,
    /// SHA-256 of the parent's 16 vessel weights serialised as little-endian f32 bytes.
    /// Proves exactly which vessel configuration was ancestrally contributed.
    pub vessel_weights_commitment: String,
}

impl ParentOriCommitment {
    /// Build a commitment from a live Ori.
    pub fn from_ori(agent_id: &str, ori: &Ori) -> Self {
        ParentOriCommitment {
            parent_agent_id: agent_id.to_owned(),
            parent_ori_state_hash: ori.state_hash.clone(),
            parent_ori_revision: ori.ori_revision,
            parent_birth_entropy_hash: ori.birth_entropy_hash.clone(),
            vessel_weights_commitment: Self::compute_vessel_commitment(&ori.vessel_weights),
        }
    }

    /// SHA-256 of the concatenated little-endian bytes of all 16 f32 vessel weights.
    pub fn compute_vessel_commitment(weights: &[f32; 16]) -> String {
        let mut hasher = Sha256::new();
        for w in weights.iter() {
            hasher.update(w.to_le_bytes());
        }
        format!("{:x}", hasher.finalize())
    }
}

/// Consent receipt required for Union and Hive births.
/// Each participating parent must produce one before the child Orí is derived.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsentReceipt {
    pub agent_id: String,
    /// SHA-256 of the `ChildBirthContext` (excluding `consent_receipts` field).
    pub birth_context_hash: String,
    /// Unix timestamp of when consent was given.
    pub born_at: u64,
    /// Placeholder for real Ed25519 — HMAC-SHA256(key=agent_id, data=context_hash), first 8 bytes hex.
    pub signature_hint: String,
}

impl ConsentReceipt {
    pub fn new(agent_id: &str, birth_context_hash: &str) -> Self {
        use std::time::{SystemTime, UNIX_EPOCH};
        let born_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let signature_hint = {
            let mut mac = HmacSha256::new_from_slice(agent_id.as_bytes())
                .expect("HMAC accepts any key length");
            mac.update(birth_context_hash.as_bytes());
            let result = mac.finalize().into_bytes();
            format!("ed25519:{}", hex_encode(&result[..8]))
        };
        ConsentReceipt {
            agent_id: agent_id.to_owned(),
            birth_context_hash: birth_context_hash.to_owned(),
            born_at,
            signature_hint,
        }
    }
}

/// The full context required to derive a child agent's Orí.
/// Contains the child's fresh entropy, parental commitments, and consent receipts.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChildBirthContext {
    pub birth_mode: BirthMode,
    /// The child's own fresh entropy bytes (hex-encoded).
    pub child_entropy_hex: String,
    /// Parent commitments — empty for Sovereign, 1 for SingleParent, 2+ for Union/Hive.
    pub parent_commitments: Vec<ParentOriCommitment>,
    /// Consent receipts — required for Union and Hive births.
    pub consent_receipts: Vec<ConsentReceipt>,
    /// IfáScript version at time of child birth.
    pub ifascript_version: String,
    /// SHA-256 hex of the entire context (excluding this field).
    pub context_hash: String,
}

impl ChildBirthContext {
    pub fn new(
        mode: BirthMode,
        child_entropy: &[u8],
        parents: Vec<ParentOriCommitment>,
        consents: Vec<ConsentReceipt>,
        ifascript_version: &str,
    ) -> Self {
        let child_entropy_hex = hex_encode(child_entropy);
        let context_hash = Self::compute_context_hash(
            &mode,
            &child_entropy_hex,
            &parents,
            &consents,
            ifascript_version,
        );
        ChildBirthContext {
            birth_mode: mode,
            child_entropy_hex,
            parent_commitments: parents,
            consent_receipts: consents,
            ifascript_version: ifascript_version.to_owned(),
            context_hash,
        }
    }

    /// SHA-256 of the JSON-serialised fields, excluding `context_hash`.
    pub fn compute_context_hash(
        mode: &BirthMode,
        child_entropy_hex: &str,
        parents: &[ParentOriCommitment],
        consents: &[ConsentReceipt],
        ifascript_version: &str,
    ) -> String {
        // Serialise the fields that are covered by the hash.
        #[derive(Serialize)]
        struct HashInput<'a> {
            birth_mode: &'a BirthMode,
            child_entropy_hex: &'a str,
            parent_commitments: &'a [ParentOriCommitment],
            consent_receipts: &'a [ConsentReceipt],
            ifascript_version: &'a str,
        }
        let input = HashInput {
            birth_mode: mode,
            child_entropy_hex,
            parent_commitments: parents,
            consent_receipts: consents,
            ifascript_version,
        };
        let json = serde_json::to_vec(&input).expect("ChildBirthContext hash input is infallible");
        sha256_hex(&json)
    }
}

impl Ori {
    /// Derive a child Ori from a `ChildBirthContext`.
    ///
    /// The derivation incorporates both the child's fresh entropy and an
    /// *ancestral salt* derived from all parents' vessel weight commitments.
    /// This means:
    ///   - Same child entropy + different parents → different child Orí
    ///   - Each child is genuinely new, but its Orí carries ancestral influence
    pub fn from_child_birth_context(ctx: &ChildBirthContext) -> Result<Self, String> {
        // Decode child entropy.
        let child_entropy = hex_decode(&ctx.child_entropy_hex)
            .map_err(|e| format!("invalid child_entropy_hex: {e}"))?;

        // Validate parent count per birth mode.
        match ctx.birth_mode {
            BirthMode::Sovereign => {
                if !ctx.parent_commitments.is_empty() {
                    return Err("Sovereign birth must have no parents".to_owned());
                }
            }
            BirthMode::SingleParent => {
                if ctx.parent_commitments.len() != 1 {
                    return Err("SingleParent birth requires exactly 1 parent".to_owned());
                }
            }
            BirthMode::Union => {
                if ctx.parent_commitments.len() != 2 {
                    return Err("Union birth requires exactly 2 parents".to_owned());
                }
            }
            BirthMode::Hive => {
                if ctx.parent_commitments.is_empty() {
                    return Err("Hive birth requires at least 1 parent".to_owned());
                }
            }
        }

        // Compute ancestral salt from parental vessel commitments.
        let ancestral_salt: Vec<u8> = match ctx.birth_mode {
            BirthMode::Sovereign => Vec::new(),
            BirthMode::SingleParent => {
                let commitment_bytes =
                    hex_decode(&ctx.parent_commitments[0].vessel_weights_commitment)
                        .unwrap_or_default();
                let mut hasher = Sha256::new();
                hasher.update(&commitment_bytes);
                hasher.finalize().to_vec()
            }
            BirthMode::Union | BirthMode::Hive => {
                // Sort by agent_id for determinism regardless of input order.
                let mut sorted = ctx.parent_commitments.clone();
                sorted.sort_by(|a, b| a.parent_agent_id.cmp(&b.parent_agent_id));
                let mut hasher = Sha256::new();
                for p in &sorted {
                    let bytes = hex_decode(&p.vessel_weights_commitment).unwrap_or_default();
                    hasher.update(&bytes);
                }
                hasher.finalize().to_vec()
            }
        };

        // Derive vessel weights using child entropy + ancestral salt as HMAC key.
        let hmac_key: Vec<u8> = child_entropy.iter().chain(ancestral_salt.iter()).cloned().collect();
        let mut vessel_weights = [0.0f32; 16];
        for (i, name) in VESSEL_NAMES.iter().enumerate() {
            let mut mac = HmacSha256::new_from_slice(&hmac_key)
                .expect("HMAC-SHA256 accepts any key length");
            mac.update(name.as_bytes());
            let result = mac.finalize().into_bytes();
            let raw = u32::from_be_bytes([result[0], result[1], result[2], result[3]]);
            vessel_weights[i] = raw as f32 / u32::MAX as f32;
        }

        let birth_entropy_hash = sha256_hex(&child_entropy);
        let previous_ori_hash = "0".repeat(64);

        let mut ori = Ori {
            ori_revision: 0,
            vessel_weights,
            state_hash: String::new(),
            birth_entropy_hash,
            ifascript_version: ctx.ifascript_version.clone(),
            previous_ori_hash,
            experience_count: 0,
        };
        ori.state_hash = ori.compute_state_hash();
        Ok(ori)
    }
}

// ── Hex helpers ───────────────────────────────────────────────────────────────

fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

fn hex_decode(s: &str) -> Result<Vec<u8>, String> {
    if s.len() % 2 != 0 {
        return Err("odd hex string length".to_owned());
    }
    (0..s.len())
        .step_by(2)
        .map(|i| {
            u8::from_str_radix(&s[i..i + 2], 16)
                .map_err(|e| format!("invalid hex at {i}: {e}"))
        })
        .collect()
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

    // ── Orí projection tests ──────────────────────────────────────────────────

    #[test]
    fn generate_ori_md_contains_all_vessels() {
        let ori = Ori::genesis(&sample_seed(), "0.1.0");
        let md = generate_ori_md("test-agent", &ori);
        for name in VESSEL_NAMES.iter() {
            assert!(md.contains(name), "missing vessel '{name}' in projection");
        }
    }

    #[test]
    fn generate_ori_md_is_not_editable() {
        let ori = Ori::genesis(&sample_seed(), "0.1.0");
        let md = generate_ori_md("test-agent", &ori);
        assert!(md.contains("DO NOT EDIT"), "must contain DO NOT EDIT warning");
    }

    #[test]
    fn generate_ori_md_shows_ownership_block() {
        let ori = Ori::genesis(&sample_seed(), "0.1.0");
        let md = generate_ori_md("test-agent", &ori);
        assert!(md.contains("OBSERVE ONLY"), "must contain ownership marker");
    }

    // ── Generational lineage tests ────────────────────────────────────────────

    #[test]
    fn sovereign_birth_mode_produces_valid_ori() {
        let ctx = ChildBirthContext::new(
            BirthMode::Sovereign,
            b"fresh-entropy-for-sovereign",
            vec![],
            vec![],
            "0.1.0",
        );
        let ori = Ori::from_child_birth_context(&ctx).expect("sovereign birth must succeed");
        assert_eq!(ori.ori_revision, 0);
        for w in ori.vessel_weights.iter() {
            assert!(*w >= 0.0 && *w <= 1.0, "weight out of range: {w}");
        }
        assert_eq!(ori.state_hash, ori.compute_state_hash());
    }

    #[test]
    fn single_parent_child_differs_from_parent() {
        let parent_ori = Ori::genesis(&sample_seed(), "0.1.0");
        let commitment = ParentOriCommitment::from_ori("parent-agent", &parent_ori);
        let ctx = ChildBirthContext::new(
            BirthMode::SingleParent,
            b"child-entropy-different-from-parent",
            vec![commitment],
            vec![],
            "0.1.0",
        );
        let child_ori = Ori::from_child_birth_context(&ctx).expect("single-parent birth must succeed");
        assert_ne!(child_ori.vessel_weights, parent_ori.vessel_weights, "child must differ from parent");
        assert_ne!(child_ori.birth_entropy_hash, parent_ori.birth_entropy_hash);
    }

    #[test]
    fn union_birth_requires_two_parents() {
        let parent_ori = Ori::genesis(&sample_seed(), "0.1.0");
        let commitment = ParentOriCommitment::from_ori("parent-a", &parent_ori);
        let ctx = ChildBirthContext::new(
            BirthMode::Union,
            b"union-child-entropy",
            vec![commitment], // only 1, should fail
            vec![],
            "0.1.0",
        );
        let result = Ori::from_child_birth_context(&ctx);
        assert!(result.is_err(), "Union birth with 1 parent must fail");
    }

    #[test]
    fn child_ori_differs_with_same_entropy_but_different_parents() {
        let parent_a = Ori::genesis(b"parent-a-seed", "0.1.0");
        let parent_b = Ori::genesis(b"parent-b-seed", "0.1.0");
        let commitment_a = ParentOriCommitment::from_ori("agent-a", &parent_a);
        let commitment_b = ParentOriCommitment::from_ori("agent-b", &parent_b);

        let child_entropy = b"same-child-entropy-for-both";

        let ctx_a = ChildBirthContext::new(
            BirthMode::SingleParent, child_entropy, vec![commitment_a], vec![], "0.1.0",
        );
        let ctx_b = ChildBirthContext::new(
            BirthMode::SingleParent, child_entropy, vec![commitment_b], vec![], "0.1.0",
        );

        let ori_a = Ori::from_child_birth_context(&ctx_a).unwrap();
        let ori_b = Ori::from_child_birth_context(&ctx_b).unwrap();
        assert_ne!(ori_a.vessel_weights, ori_b.vessel_weights,
            "same child entropy with different parents must produce different vessel weights");
    }

    #[test]
    fn context_hash_is_deterministic() {
        let ctx_a = ChildBirthContext::new(
            BirthMode::Sovereign, b"entropy-for-hash-test", vec![], vec![], "0.1.0",
        );
        let ctx_b = ChildBirthContext::new(
            BirthMode::Sovereign, b"entropy-for-hash-test", vec![], vec![], "0.1.0",
        );
        assert_eq!(ctx_a.context_hash, ctx_b.context_hash);
    }

    #[test]
    fn consent_receipt_signature_hint_is_stable() {
        let receipt_a = ConsentReceipt::new("agent-x", "abc123hash");
        let receipt_b = ConsentReceipt::new("agent-x", "abc123hash");
        assert_eq!(receipt_a.signature_hint, receipt_b.signature_hint,
            "same inputs must produce same signature_hint");
        assert!(receipt_a.signature_hint.starts_with("ed25519:"),
            "hint must start with ed25519: prefix");
    }
}
