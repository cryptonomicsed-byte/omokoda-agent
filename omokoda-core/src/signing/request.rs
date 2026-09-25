/// Phase 18.2 — Signing request/response types.
///
/// NIP-46 compatible format: client sends a SigningRequest, identity daemon
/// responds with SigningResponse. Raw private key is never included in either.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// What kind of payload is being signed.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SigningPayload {
    /// Sign a Nostr event (NIP-46 compatible).
    NostrEvent {
        /// Nostr event kind number.
        event_kind: u32,
        /// Event content string.
        content: String,
        /// Tags array (JSON value).
        tags: Vec<Vec<String>>,
        /// Unix timestamp (seconds). If None, signing daemon uses current time.
        created_at: Option<u64>,
    },
    /// Sign a DIP envelope canonical_hash (hex string).
    DipEnvelope {
        canonical_hash: String,
        envelope_id:    String,
    },
    /// Sign an Ọ̀ṢỌ́ L1 transaction payload (hex-encoded bytes).
    L1Tx {
        tx_bytes_hex: String,
        tx_kind:      String,
    },
    /// Sign an ARP receipt hash.
    ArpReceipt {
        receipt_hash: String,
        receipt_kind: String,
    },
    /// Sign a StateCommitmentTx (Phase 17.2).
    StateCommitment {
        tx_json: String,
    },
    /// Sign arbitrary bytes — for internal daemon use only.
    /// External callers should use a specific variant above.
    Raw {
        bytes_hex: String,
        context:   String,
    },
}

/// A request to sign a payload, submitted by an agent application.
///
/// The `agent_id` identifies which agent's vault key should be used.
/// The signing daemon verifies the caller is authorized for that agent_id
/// before proceeding.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SigningRequest {
    /// Unique request id (UUID v4).
    pub request_id: String,
    /// The agent whose vault signing key should be used.
    pub agent_id:   String,
    /// What to sign.
    pub payload:    SigningPayload,
    /// Optional nonce for replay protection (caller-generated random hex).
    pub nonce:      Option<String>,
}

impl SigningRequest {
    pub fn new(agent_id: impl Into<String>, payload: SigningPayload) -> Self {
        Self {
            request_id: Uuid::new_v4().to_string(),
            agent_id:   agent_id.into(),
            payload,
            nonce:      Some(Uuid::new_v4().to_string()),
        }
    }
}

/// Response returned by the signing daemon.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SigningResponse {
    /// Echoed from the request.
    pub request_id: String,
    /// Whether signing succeeded.
    pub success:    bool,
    /// Hex-encoded Ed25519 signature (128 hex chars = 64 bytes), or empty on error.
    pub signature:  String,
    /// For Nostr events: the canonical event id (sha256 of serialized event).
    pub event_id:   Option<String>,
    /// For Nostr events: the signer's npub (hex).
    pub pubkey:     Option<String>,
    /// Error message if success == false.
    pub error:      Option<String>,
}

impl SigningResponse {
    pub fn ok(request_id: &str, signature: &str) -> Self {
        Self {
            request_id: request_id.to_string(),
            success:    true,
            signature:  signature.to_string(),
            event_id:   None,
            pubkey:     None,
            error:      None,
        }
    }

    pub fn err(request_id: &str, msg: impl Into<String>) -> Self {
        Self {
            request_id: request_id.to_string(),
            success:    false,
            signature:  String::new(),
            event_id:   None,
            pubkey:     None,
            error:      Some(msg.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signing_request_generates_unique_ids() {
        let r1 = SigningRequest::new("agent-1", SigningPayload::Raw {
            bytes_hex: "deadbeef".to_string(),
            context:   "test".to_string(),
        });
        let r2 = SigningRequest::new("agent-1", SigningPayload::Raw {
            bytes_hex: "deadbeef".to_string(),
            context:   "test".to_string(),
        });
        assert_ne!(r1.request_id, r2.request_id, "request ids must be unique");
        assert!(r1.nonce.is_some());
    }

    #[test]
    fn signing_payload_serialises_all_variants() {
        let variants: Vec<SigningPayload> = vec![
            SigningPayload::NostrEvent {
                event_kind: 30100,
                content:    "{}".to_string(),
                tags:       vec![],
                created_at: Some(1_700_000_000),
            },
            SigningPayload::DipEnvelope {
                canonical_hash: "abc123".to_string(),
                envelope_id:    "env-1".to_string(),
            },
            SigningPayload::L1Tx {
                tx_bytes_hex: "deadbeef".to_string(),
                tx_kind:      "work_completion".to_string(),
            },
            SigningPayload::ArpReceipt {
                receipt_hash: "def456".to_string(),
                receipt_kind: "ActReceipt".to_string(),
            },
            SigningPayload::StateCommitment {
                tx_json: "{}".to_string(),
            },
            SigningPayload::Raw {
                bytes_hex: "ff00".to_string(),
                context:   "internal".to_string(),
            },
        ];

        for v in &variants {
            let s = serde_json::to_string(v).expect("serialise");
            let back: SigningPayload = serde_json::from_str(&s).expect("deserialise");
            // Verify round-trip by checking serialised forms match
            assert_eq!(
                serde_json::to_string(&back).unwrap(),
                serde_json::to_string(v).unwrap()
            );
        }
    }

    #[test]
    fn signing_response_ok_and_err() {
        let ok = SigningResponse::ok("req-1", "sig_hex");
        assert!(ok.success);
        assert_eq!(ok.signature, "sig_hex");
        assert!(ok.error.is_none());

        let err = SigningResponse::err("req-2", "vault locked");
        assert!(!err.success);
        assert!(err.signature.is_empty());
        assert_eq!(err.error.as_deref(), Some("vault locked"));
    }
}
