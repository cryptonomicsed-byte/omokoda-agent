//! Tamper-evident agent heartbeat — each beat chains to the previous via SHA-256.
//!
//! The hash chain proves *continuity of life*: if any beat is dropped or altered,
//! the chain breaks. Zàngbétò can audit the chain as a receipts provenance trace.
//!
//! Ed25519 signatures: call `beat.sign_with_key(priv_b64url)` after construction.
//! Fail-open: if the key is absent or invalid, `signature` stays `None` and a
//! `warn!` is emitted. The chain is still valid without signatures; signatures add
//! an extra layer of tamper-evidence on top of the SHA-256 chain.

use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use ed25519_dalek::{Signer, SigningKey};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

/// Snapshot of emotional state captured at heartbeat time.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct SomaVector {
    pub energy: f32,
    pub tension: f32,
    pub focus: f32,
    pub gpu_util: f32,
}

/// One heartbeat record. Serialises to canonical JSON for hashing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentHeartbeat {
    pub agent_id:               String,
    pub boot_id:                String,
    pub sequence:               u64,
    pub timestamp:              u64,
    pub state:                  HeartbeatState,
    pub tier:                   String,
    pub active_daemons:         Vec<String>,
    pub current_work:           Option<String>,
    pub previous_heartbeat_hash: Option<String>,
    pub signature:              Option<String>,  // ed25519, future
    /// Emotion snapshot at beat time
    pub soma_vector:            Option<SomaVector>,
    /// Total messages processed since boot
    pub message_count:          u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HeartbeatState {
    Alive,
    Thinking,
    Working,
    Resting,  // Sabbath
    Offline,
}

impl AgentHeartbeat {
    /// Compute the canonical SHA-256 hash of this beat (excluding the signature field).
    pub fn hash(&self) -> String {
        let canonical = serde_json::json!({
            "agent_id":               self.agent_id,
            "boot_id":                self.boot_id,
            "sequence":               self.sequence,
            "timestamp":              self.timestamp,
            "state":                  self.state,
            "tier":                   self.tier,
            "active_daemons":         self.active_daemons,
            "current_work":           self.current_work,
            "previous_heartbeat_hash": self.previous_heartbeat_hash,
            "message_count":          self.message_count,
        });
        let bytes = canonical.to_string();
        let digest = Sha256::digest(bytes.as_bytes());
        hex::encode(digest)
    }

    /// Build the first beat in a new chain (no predecessor).
    pub fn genesis(agent_id: impl Into<String>, tier: impl Into<String>) -> Self {
        Self {
            agent_id: agent_id.into(),
            boot_id:  Uuid::new_v4().to_string(),
            sequence: 0,
            timestamp: now_secs(),
            state:    HeartbeatState::Alive,
            tier:     tier.into(),
            active_daemons: vec![],
            current_work: None,
            previous_heartbeat_hash: None,
            signature: None,
            soma_vector: None,
            message_count: 0,
        }
    }

    /// Produce the next beat, chaining from `prev`.
    pub fn next_from(
        prev: &AgentHeartbeat,
        state: HeartbeatState,
        active_daemons: Vec<String>,
        current_work: Option<String>,
    ) -> Self {
        Self {
            agent_id:  prev.agent_id.clone(),
            boot_id:   prev.boot_id.clone(),
            sequence:  prev.sequence + 1,
            timestamp: now_secs(),
            state,
            tier:      prev.tier.clone(),
            active_daemons,
            current_work,
            previous_heartbeat_hash: Some(prev.hash()),
            signature: None,
            soma_vector: None,
            message_count: prev.message_count,
        }
    }

    /// Generate a shutdown receipt compatible with ARP on SIGTERM.
    pub fn shutdown_receipt(
        &self,
        boot_timestamp: u64,
        exit_reason: impl Into<String>,
    ) -> serde_json::Value {
        let uptime = self.timestamp.saturating_sub(boot_timestamp);
        serde_json::json!({
            "kind": "shutdown_receipt",
            "agent_id": self.agent_id,
            "boot_id": self.boot_id,
            "final_sequence": self.sequence,
            "final_hash": self.hash(),
            "uptime_secs": uptime,
            "message_count": self.message_count,
            "exit_reason": exit_reason.into(),
            "timestamp": now_secs(),
        })
    }

    /// Verify the chain link: does `candidate.previous_heartbeat_hash == expected_prev.hash()`?
    pub fn verify_chain(expected_prev: &AgentHeartbeat, candidate: &AgentHeartbeat) -> bool {
        candidate.previous_heartbeat_hash.as_deref() == Some(&expected_prev.hash())
            && candidate.sequence == expected_prev.sequence + 1
    }

    /// Sign the `chain_hash` of this beat with an Ed25519 key.
    ///
    /// `priv_b64url` — base64url-no-pad encoded 32-byte Ed25519 private scalar,
    /// as stored in `NodeIdentity.private_key` and `IdentityVault.nostr_private_key_hex`
    /// (the latter is hex; callers should pass the node identity key here).
    ///
    /// Fail-open: on any error the `signature` field is left as `None` and a
    /// warning is logged. The heartbeat chain remains valid regardless.
    pub fn sign_with_key(&mut self, priv_b64url: &str) {
        match self.try_sign(priv_b64url) {
            Ok(sig_hex) => {
                self.signature = Some(sig_hex);
            }
            Err(e) => {
                // Fail-open: chain integrity is maintained by SHA-256 even without sig.
                #[cfg(feature = "tracing")]
                tracing::warn!("heartbeat Ed25519 sign failed (fail-open): {e}");
                #[cfg(not(feature = "tracing"))]
                eprintln!("WARN heartbeat Ed25519 sign failed (fail-open): {e}");
                self.signature = None;
            }
        }
    }

    fn try_sign(&self, priv_b64url: &str) -> Result<String, String> {
        let priv_bytes = URL_SAFE_NO_PAD
            .decode(priv_b64url)
            .map_err(|e| format!("base64 decode: {e}"))?;
        let arr: [u8; 32] = priv_bytes
            .try_into()
            .map_err(|_| "expected 32-byte Ed25519 key".to_string())?;
        let signing_key = SigningKey::from_bytes(&arr);
        // Sign the chain_hash (hex string bytes) — deterministic, no randomness needed.
        let chain_hash = self.hash();
        let signature = signing_key.sign(chain_hash.as_bytes());
        Ok(hex::encode(signature.to_bytes()))
    }

    /// Verify the Ed25519 signature stored on this beat against `pub_b64url`.
    /// Returns `true` if signature is present and valid, `false` otherwise (including
    /// when no signature is set — treat as unverified, not forged).
    pub fn verify_signature(&self, pub_b64url: &str) -> bool {
        use ed25519_dalek::{Verifier, VerifyingKey};
        let sig_hex = match &self.signature {
            Some(s) if !s.is_empty() => s,
            _ => return false,
        };
        let sig_bytes = match hex::decode(sig_hex) {
            Ok(b) => b,
            Err(_) => return false,
        };
        let sig_arr: [u8; 64] = match sig_bytes.try_into() {
            Ok(a) => a,
            Err(_) => return false,
        };
        let sig = match ed25519_dalek::Signature::from_bytes(&sig_arr) {
            sig => sig,
        };
        let pub_bytes = match URL_SAFE_NO_PAD.decode(pub_b64url) {
            Ok(b) => b,
            Err(_) => return false,
        };
        let pub_arr: [u8; 32] = match pub_bytes.try_into() {
            Ok(a) => a,
            Err(_) => return false,
        };
        let verifying_key = match VerifyingKey::from_bytes(&pub_arr) {
            Ok(k) => k,
            Err(_) => return false,
        };
        let chain_hash = self.hash();
        verifying_key.verify(chain_hash.as_bytes(), &sig).is_ok()
    }
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn genesis_has_no_predecessor() {
        let beat = AgentHeartbeat::genesis("agent:1", "resident");
        assert_eq!(beat.sequence, 0);
        assert!(beat.previous_heartbeat_hash.is_none());
    }

    #[test]
    fn next_chains_correctly() {
        let g = AgentHeartbeat::genesis("agent:1", "resident");
        let g_hash = g.hash();
        let next = AgentHeartbeat::next_from(&g, HeartbeatState::Thinking, vec![], None);
        assert_eq!(next.sequence, 1);
        assert_eq!(next.previous_heartbeat_hash.as_deref(), Some(g_hash.as_str()));
        assert!(AgentHeartbeat::verify_chain(&g, &next));
    }

    #[test]
    fn chain_breaks_on_tamper() {
        let g = AgentHeartbeat::genesis("agent:1", "resident");
        let mut next = AgentHeartbeat::next_from(&g, HeartbeatState::Alive, vec![], None);
        next.sequence = 999; // tamper
        assert!(!AgentHeartbeat::verify_chain(&g, &next));
    }

    #[test]
    fn hash_is_deterministic() {
        let g = AgentHeartbeat::genesis("agent:X", "citizen");
        assert_eq!(g.hash(), g.hash());
    }

    #[test]
    fn different_beats_have_different_hashes() {
        let g = AgentHeartbeat::genesis("agent:1", "resident");
        let n = AgentHeartbeat::next_from(&g, HeartbeatState::Working, vec![], None);
        assert_ne!(g.hash(), n.hash());
    }

    // ── Ed25519 signing tests ───────────────────────────────────────────────

    fn test_keypair() -> (String, String) {
        use ed25519_dalek::SigningKey;
        use rand::rngs::OsRng;
        let sk = SigningKey::generate(&mut OsRng);
        let priv_b64 = URL_SAFE_NO_PAD.encode(sk.to_bytes());
        let pub_b64 = URL_SAFE_NO_PAD.encode(sk.verifying_key().to_bytes());
        (priv_b64, pub_b64)
    }

    #[test]
    fn sign_and_verify_round_trip() {
        let (priv_b64, pub_b64) = test_keypair();
        let mut beat = AgentHeartbeat::genesis("agent:sig-test", "resident");
        assert!(beat.signature.is_none());
        beat.sign_with_key(&priv_b64);
        assert!(beat.signature.is_some(), "signature should be set after signing");
        assert!(beat.verify_signature(&pub_b64), "signature should verify with matching pubkey");
    }

    #[test]
    fn signature_fails_with_wrong_key() {
        let (priv_b64, _) = test_keypair();
        let (_, wrong_pub) = test_keypair();
        let mut beat = AgentHeartbeat::genesis("agent:sig-test", "resident");
        beat.sign_with_key(&priv_b64);
        assert!(!beat.verify_signature(&wrong_pub), "wrong pubkey should not verify");
    }

    #[test]
    fn fail_open_on_bad_key() {
        let mut beat = AgentHeartbeat::genesis("agent:sig-test", "resident");
        // Pass garbage — should not panic, signature stays None.
        beat.sign_with_key("not-a-valid-key!!");
        assert!(beat.signature.is_none(), "fail-open: signature should be None on bad key");
    }

    #[test]
    fn unsigned_beat_verify_returns_false_not_panic() {
        let (_, pub_b64) = test_keypair();
        let beat = AgentHeartbeat::genesis("agent:1", "resident");
        assert!(!beat.verify_signature(&pub_b64), "unsigned beat should return false, not panic");
    }

    #[test]
    fn signature_covers_hash_tamper_detected() {
        let (priv_b64, pub_b64) = test_keypair();
        let mut beat = AgentHeartbeat::genesis("agent:tamper", "resident");
        beat.sign_with_key(&priv_b64);
        // Tamper the agent_id AFTER signing — hash will differ, sig won't match.
        beat.agent_id = "agent:evil".into();
        assert!(!beat.verify_signature(&pub_b64), "tampered beat should fail signature check");
    }
}
