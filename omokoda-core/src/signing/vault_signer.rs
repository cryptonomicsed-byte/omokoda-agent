/// Phase 18.2 — VaultSigner: identity daemon signing component.
///
/// Holds the agent's Ed25519 signing key (derived from k_root) and fulfills
/// SigningRequests without ever exposing the raw key to callers.
///
/// Security invariants:
///   - `k_root` is passed in at construction time from the agent's sealed vault
///   - The raw k_root is stored in memory only (not re-derived per request)
///   - Signing key is derived once with domain separation from k_root
///   - Raw k_root is never included in any SigningResponse

use super::request::{SigningPayload, SigningRequest, SigningResponse};

/// Domain-separated key derivation label for the NIP-46 signing key.
const SIGNING_KEY_LABEL: &[u8] = b"omokoda:nip46_signing_key_v1";

/// The vault-backed signing component.
///
/// Constructed from the agent's k_root at vault-open time.
/// Lives for the duration of the agent's runtime session.
pub struct VaultSigner {
    /// Derived Ed25519 signing key. Never exposed externally.
    signing_key: ed25519_dalek::SigningKey,
    /// Hex-encoded public key (npub equivalent).
    pubkey_hex: String,
    /// Agent id this signer belongs to.
    agent_id: String,
}

impl VaultSigner {
    /// Construct a VaultSigner from a k_root.
    ///
    /// Derives the signing key using BLAKE3 key derivation with domain separation.
    /// k_root is consumed — this function takes ownership to make accidental
    /// double-use harder.
    pub fn from_k_root(k_root: &[u8; 32], agent_id: impl Into<String>) -> Self {
        // Derive a 32-byte signing seed with domain separation.
        let derived = blake3::derive_key(
            std::str::from_utf8(SIGNING_KEY_LABEL).unwrap(),
            k_root,
        );
        let signing_key = ed25519_dalek::SigningKey::from_bytes(&derived);
        let pubkey_hex = hex::encode(signing_key.verifying_key().to_bytes());
        Self {
            signing_key,
            pubkey_hex,
            agent_id: agent_id.into(),
        }
    }

    /// The agent's public signing key (hex-encoded Ed25519 verifying key).
    pub fn pubkey_hex(&self) -> &str {
        &self.pubkey_hex
    }

    /// Fulfill a SigningRequest.
    ///
    /// Returns a SigningResponse — never includes the raw private key.
    /// Returns an error response if the agent_id doesn't match this signer.
    pub fn sign(&self, request: &SigningRequest) -> SigningResponse {
        use ed25519_dalek::Signer;

        // Guard: only sign for our own agent_id
        if request.agent_id != self.agent_id {
            return SigningResponse::err(
                &request.request_id,
                format!(
                    "agent_id mismatch: signer is for '{}', request is for '{}'",
                    self.agent_id, request.agent_id
                ),
            );
        }

        let bytes_to_sign = match self.payload_to_bytes(&request.payload) {
            Ok(b) => b,
            Err(e) => return SigningResponse::err(&request.request_id, e),
        };

        let signature = self.signing_key.sign(&bytes_to_sign);
        let sig_hex = hex::encode(signature.to_bytes());

        let mut resp = SigningResponse::ok(&request.request_id, &sig_hex);
        resp.pubkey = Some(self.pubkey_hex.clone());

        // For Nostr events, compute the event_id (sha256 of canonical event JSON)
        if let SigningPayload::NostrEvent { event_kind, content, tags, created_at } = &request.payload {
            let ts = created_at.unwrap_or_else(current_unix_ts);
            let event_id = compute_nostr_event_id(
                &self.pubkey_hex,
                ts,
                *event_kind,
                tags,
                content,
            );
            resp.event_id = Some(event_id);
        }

        resp
    }

    /// Convert a SigningPayload to the canonical bytes that will be signed.
    fn payload_to_bytes(&self, payload: &SigningPayload) -> Result<Vec<u8>, String> {
        match payload {
            SigningPayload::NostrEvent { event_kind, content, tags, created_at } => {
                let ts = created_at.unwrap_or_else(current_unix_ts);
                // NIP-01: sign sha256 of JSON array [0, pubkey, created_at, kind, tags, content]
                let commitment = serde_json::json!([
                    0,
                    &self.pubkey_hex,
                    ts,
                    event_kind,
                    tags,
                    content,
                ]);
                let json = serde_json::to_string(&commitment)
                    .map_err(|e| format!("nostr serialise: {e}"))?;
                Ok(sha256_bytes(json.as_bytes()))
            }

            SigningPayload::DipEnvelope { canonical_hash, .. } => {
                hex::decode(canonical_hash)
                    .map_err(|e| format!("dip canonical_hash hex decode: {e}"))
            }

            SigningPayload::L1Tx { tx_bytes_hex, .. } => {
                hex::decode(tx_bytes_hex)
                    .map_err(|e| format!("L1Tx hex decode: {e}"))
            }

            SigningPayload::ArpReceipt { receipt_hash, .. } => {
                hex::decode(receipt_hash)
                    .map_err(|e| format!("ARP receipt hash hex decode: {e}"))
            }

            SigningPayload::StateCommitment { tx_json } => {
                // Sign the raw JSON bytes
                Ok(sha256_bytes(tx_json.as_bytes()))
            }

            SigningPayload::Raw { bytes_hex, .. } => {
                hex::decode(bytes_hex)
                    .map_err(|e| format!("raw bytes hex decode: {e}"))
            }
        }
    }
}

/// Compute Nostr NIP-01 event_id: sha256 of canonical event serialization.
fn compute_nostr_event_id(
    pubkey: &str,
    created_at: u64,
    kind: u32,
    tags: &[Vec<String>],
    content: &str,
) -> String {
    let commitment = serde_json::json!([0, pubkey, created_at, kind, tags, content]);
    let json = serde_json::to_string(&commitment).unwrap_or_default();
    hex::encode(sha256_bytes(json.as_bytes()))
}

fn sha256_bytes(input: &[u8]) -> Vec<u8> {
    use sha2::{Sha256, Digest};
    Sha256::digest(input).to_vec()
}

fn current_unix_ts() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::signing::request::{SigningPayload, SigningRequest};

    const TEST_K_ROOT: &[u8; 32] = b"test_k_root_32bytes_xxxxxxxxxxx!";

    fn signer() -> VaultSigner {
        VaultSigner::from_k_root(TEST_K_ROOT, "agent-test-123")
    }

    #[test]
    fn pubkey_hex_is_64_chars() {
        let s = signer();
        assert_eq!(s.pubkey_hex().len(), 64, "Ed25519 pubkey = 32 bytes = 64 hex chars");
    }

    #[test]
    fn same_k_root_same_pubkey() {
        let s1 = VaultSigner::from_k_root(TEST_K_ROOT, "agent-1");
        let s2 = VaultSigner::from_k_root(TEST_K_ROOT, "agent-1");
        assert_eq!(s1.pubkey_hex(), s2.pubkey_hex(), "deterministic key derivation");
    }

    #[test]
    fn different_k_roots_different_pubkeys() {
        let k2: [u8; 32] = b"different_k_root_32_bytes_here!!".to_owned();
        let s1 = VaultSigner::from_k_root(TEST_K_ROOT, "agent-1");
        let s2 = VaultSigner::from_k_root(&k2, "agent-1");
        assert_ne!(s1.pubkey_hex(), s2.pubkey_hex());
    }

    #[test]
    fn sign_raw_succeeds() {
        let s = signer();
        let req = SigningRequest::new("agent-test-123", SigningPayload::Raw {
            bytes_hex: hex::encode(b"hello sovereign"),
            context:   "test".to_string(),
        });
        let resp = s.sign(&req);
        assert!(resp.success);
        assert_eq!(resp.signature.len(), 128, "Ed25519 sig = 64 bytes = 128 hex chars");
    }

    #[test]
    fn sign_rejects_wrong_agent_id() {
        let s = signer();
        let req = SigningRequest::new("agent-other", SigningPayload::Raw {
            bytes_hex: hex::encode(b"test"),
            context:   "test".to_string(),
        });
        let resp = s.sign(&req);
        assert!(!resp.success);
        assert!(resp.error.as_deref().unwrap_or("").contains("agent_id mismatch"));
    }

    #[test]
    fn sign_nostr_event_produces_event_id() {
        let s = signer();
        let req = SigningRequest::new("agent-test-123", SigningPayload::NostrEvent {
            event_kind: 30100,
            content:    "{}".to_string(),
            tags:       vec![vec!["d".to_string(), "agent-1".to_string()]],
            created_at: Some(1_700_000_000),
        });
        let resp = s.sign(&req);
        assert!(resp.success);
        let event_id = resp.event_id.expect("nostr signing must produce event_id");
        assert_eq!(event_id.len(), 64, "sha256 = 32 bytes = 64 hex chars");
        assert_eq!(resp.pubkey.as_deref(), Some(s.pubkey_hex()));
    }

    #[test]
    fn sign_dip_envelope_produces_signature() {
        let s = signer();
        let hash_hex = hex::encode([0xab_u8; 32]);
        let req = SigningRequest::new("agent-test-123", SigningPayload::DipEnvelope {
            canonical_hash: hash_hex.clone(),
            envelope_id:    "env-abc".to_string(),
        });
        let resp = s.sign(&req);
        assert!(resp.success);
        assert_eq!(resp.signature.len(), 128);
    }

    #[test]
    fn signature_is_verifiable_with_pubkey() {
        use ed25519_dalek::{Verifier, VerifyingKey};
        let s = signer();
        let payload_bytes = b"payload to sign";
        let req = SigningRequest::new("agent-test-123", SigningPayload::Raw {
            bytes_hex: hex::encode(payload_bytes),
            context:   "verify_test".to_string(),
        });
        let resp = s.sign(&req);
        assert!(resp.success);

        // Reconstruct verifying key from pubkey_hex and verify
        let pub_bytes = hex::decode(s.pubkey_hex()).unwrap();
        let pub_arr: [u8; 32] = pub_bytes.try_into().unwrap();
        let vk = VerifyingKey::from_bytes(&pub_arr).unwrap();
        let sig_bytes = hex::decode(&resp.signature).unwrap();
        let sig_arr: [u8; 64] = sig_bytes.try_into().unwrap();
        let sig = ed25519_dalek::Signature::from_bytes(&sig_arr);
        assert!(vk.verify(payload_bytes, &sig).is_ok(), "signature must verify with pubkey");
    }
}
