/// Phase 17.1 — AgentPublicState and StateDelta types.
///
/// These are the types stored in the Freenet distributed state layer.
/// They hold non-canonical public agent state — things that need to be
/// discoverable but do NOT require L1 finality for every update.
///
/// Canonical / financial state lives on the Ọ̀ṢỌ́ L1.
/// Freenet state → state commitment → Zàngbétò receipt → Ọ̀ṢỌ́ L1 (significant transitions only).

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// The public mutable state of a sovereign agent in the Freenet layer.
///
/// - NOT for: financial state, capability grants, work results (those go to L1)
/// - YES for: agent profile, last-seen, relay list, social graph edges, presence
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AgentPublicState {
    /// Hex Ed25519 pubkey (agent's npub / Nostr identity key).
    pub agent_npub: String,
    /// BIPON39 mnemonic phrase for human-readable addressing.
    pub bipon39_phrase: String,
    /// Odù index (0–255) — cosmological address in the 16×16 Ifá grid.
    pub odu_index: u8,
    /// Tier (0–5).
    pub tier: u8,
    /// List of Nostr relay URLs this agent publishes to.
    pub relay_list: Vec<String>,
    /// Agent's profile display name.
    pub display_name: String,
    /// Short bio / capability summary.
    pub about: String,
    /// Walrus blob URL for extended profile data.
    pub walrus_profile_url: Option<String>,
    /// Unix timestamp of most recent activity.
    pub last_seen: u64,
    /// Presence status string (e.g. "ACTIVE", "IDLE", "OFFLINE").
    pub presence: String,
    /// Outbound social graph edges: agent_npub → relationship_kind.
    /// Relationship kinds: "follows", "delegates_to", "co-authored", etc.
    pub social_edges: BTreeMap<String, String>,
    /// Capability identifiers this agent currently advertises.
    pub capabilities: Vec<String>,
    /// Latest L1 state_root hash (hex). Set by NIP-OSO-07 events.
    pub l1_state_root: Option<String>,
    /// Block height corresponding to `l1_state_root`.
    pub l1_block_height: u64,
    /// Monotonically increasing version counter — used for merge conflict resolution.
    pub version: u64,
    /// Hex Ed25519 signature over `state_signing_bytes()` by the agent's npub key.
    /// Freenet contract validates this before accepting any update.
    pub signature: String,
}

impl AgentPublicState {
    /// Canonical bytes that the agent must sign for state authenticity.
    ///
    /// Covers all mutable fields that can be updated via StateDelta.
    /// Omits `signature` itself to avoid circularity.
    pub fn state_signing_bytes(&self) -> Vec<u8> {
        // Deterministic JSON of the unsigned fields
        let signable = serde_json::json!({
            "agent_npub":       &self.agent_npub,
            "bipon39_phrase":   &self.bipon39_phrase,
            "odu_index":        self.odu_index,
            "tier":             self.tier,
            "relay_list":       &self.relay_list,
            "display_name":     &self.display_name,
            "about":            &self.about,
            "last_seen":        self.last_seen,
            "presence":         &self.presence,
            "capabilities":     &self.capabilities,
            "l1_state_root":    &self.l1_state_root,
            "l1_block_height":  self.l1_block_height,
            "version":          self.version,
        });
        signable.to_string().into_bytes()
    }

    /// Compute BLAKE3 hash of the signed state bytes.
    pub fn state_hash(&self) -> [u8; 32] {
        *blake3::hash(&self.state_signing_bytes()).as_bytes()
    }

    /// Verify the agent's signature on this state.
    pub fn verify_signature(&self) -> bool {
        use ed25519_dalek::Verifier;
        let pub_bytes = match hex::decode(&self.agent_npub) {
            Ok(b) if b.len() == 32 => b,
            _ => return false,
        };
        let pub_arr: [u8; 32] = match pub_bytes.try_into() {
            Ok(a) => a,
            Err(_) => return false,
        };
        let verifying_key = match ed25519_dalek::VerifyingKey::from_bytes(&pub_arr) {
            Ok(k) => k,
            Err(_) => return false,
        };
        let sig_bytes = match hex::decode(&self.signature) {
            Ok(b) if b.len() == 64 => b,
            _ => return false,
        };
        let sig_arr: [u8; 64] = match sig_bytes.try_into() {
            Ok(a) => a,
            Err(_) => return false,
        };
        let sig = ed25519_dalek::Signature::from_bytes(&sig_arr);
        verifying_key.verify(&self.state_signing_bytes(), &sig).is_ok()
    }
}

/// An atomic update to apply to an `AgentPublicState`.
///
/// All fields are `Option` — only set the fields that are changing.
/// StateDelta is signed by the same agent key before submission.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct StateDelta {
    /// Must match the current `agent_npub` of the target state.
    pub agent_npub: String,
    /// New version (must be > current `version`).
    pub new_version: u64,
    pub relay_list:       Option<Vec<String>>,
    pub display_name:     Option<String>,
    pub about:            Option<String>,
    pub walrus_profile_url: Option<Option<String>>,
    pub last_seen:        Option<u64>,
    pub presence:         Option<String>,
    pub social_edges_add: Option<BTreeMap<String, String>>,
    pub social_edges_remove: Option<Vec<String>>,
    pub capabilities:     Option<Vec<String>>,
    pub l1_state_root:    Option<String>,
    pub l1_block_height:  Option<u64>,
    /// Ed25519 signature over `delta_signing_bytes()` by agent's npub key.
    pub signature: String,
}

impl StateDelta {
    /// Bytes that the agent signs to authorize this delta.
    pub fn delta_signing_bytes(&self) -> Vec<u8> {
        let signable = serde_json::json!({
            "agent_npub":  &self.agent_npub,
            "new_version": self.new_version,
        });
        signable.to_string().into_bytes()
    }

    /// Verify the delta's signature against the agent's pubkey.
    pub fn verify_signature(&self) -> bool {
        use ed25519_dalek::Verifier;
        let pub_bytes = match hex::decode(&self.agent_npub) {
            Ok(b) if b.len() == 32 => b,
            _ => return false,
        };
        let pub_arr: [u8; 32] = match pub_bytes.try_into() {
            Ok(a) => a,
            Err(_) => return false,
        };
        let verifying_key = match ed25519_dalek::VerifyingKey::from_bytes(&pub_arr) {
            Ok(k) => k,
            Err(_) => return false,
        };
        let sig_bytes = match hex::decode(&self.signature) {
            Ok(b) if b.len() == 64 => b,
            _ => return false,
        };
        let sig_arr: [u8; 64] = match sig_bytes.try_into() {
            Ok(a) => a,
            Err(_) => return false,
        };
        let sig = ed25519_dalek::Signature::from_bytes(&sig_arr);
        verifying_key.verify(&self.delta_signing_bytes(), &sig).is_ok()
    }
}

/// Compact summary for Freenet's state sync protocol.
///
/// Freenet uses summarize() to decide whether a peer has newer state
/// without transferring the full state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateSummary {
    pub agent_npub:    String,
    pub version:       u64,
    pub state_hash:    String,
    pub last_seen:     u64,
    pub l1_block_height: u64,
}
