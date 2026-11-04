//! omokoda-core/src/security/bunker.rs
//! NIP-46 Bunker — Keys never leave the device
//! Steward Layer (Rust) — Separate process, seccomp-filtered, nsec never exposed

use crate::identity::{AgentIdentity, DeviceId, SuiAddress};
use ed25519_dalek::{SigningKey, VerifyingKey, Signature, Signer, Verifier};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;

/// Bunker request/response protocol (Unix socket)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BunkerRequest {
    /// Sign an event (Nostr event JSON)
    SignEvent { event_json: String },
    /// Get public key
    GetPublicKey,
    /// Derive child key (for sub-identities)
    DeriveChild { path: String },
    /// Health check
    Ping,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BunkerResponse {
    SignedEvent { event_json: String, signature: String },
    PublicKey { pubkey: String },
    ChildKey { pubkey: String },
    Pong,
    Error { message: String },
}

/// Bunker state — holds the nsec (never leaves this process)
pub struct Bunker {
    /// Master signing key (nsec) — NEVER leaves this process
    master_key: SigningKey,
    /// Derived keys cache
    derived_keys: Arc<Mutex<HashMap<String, SigningKey>>>,
    /// Device identity bound to this bunker
    device_id: DeviceId,
    /// Agent identity
    agent_identity: AgentIdentity,
}

impl Bunker {
    /// Create new bunker from master seed (called once at birth)
    pub fn new(master_seed: [u8; 32], device_id: DeviceId, agent_identity: AgentIdentity) -> Self {
        let master_key = SigningKey::from_bytes(&master_seed);
        Self {
            master_key,
            derived_keys: Arc::new(Mutex::new(HashMap::new())),
            device_id,
            agent_identity,
        }
    }

    /// Initialize from encrypted keystore (age + LUKS)
    pub fn from_keystore(keystore_path: &str, passphrase: &str) -> Result<Self, BunkerError> {
        // In production: decrypt with age + LUKS or kernel keyring
        // For now: placeholder
        let master_seed = [0u8; 32]; // Would be decrypted
        let device_id = DeviceId::from_hardware();
        let agent_identity = AgentIdentity {
            agent_npub: "npub1...".into(),
            device_id: device_id.clone(),
            sui_address: SuiAddress("0x...".into()),
            mnemonic: None,
            dna_fingerprint: crate::identity::DnaFingerprint([0u8; 16]),
            born_at: 0,
            odù_primary: 0,
            odù_secondary: 0,
        };
        Ok(Self::new(master_seed, device_id, agent_identity))
    }

    /// Process a request (called from Unix socket handler)
    pub async fn handle_request(&self, request: BunkerRequest) -> BunkerResponse {
        match request {
            BunkerRequest::SignEvent { event_json } => {
                match self.sign_event(&event_json) {
                    Ok(sig) => BunkerResponse::SignedEvent {
                        event_json,
                        signature: sig.to_hex(),
                    },
                    Err(e) => BunkerResponse::Error { message: e.to_string() },
                }
            }
            BunkerRequest::GetPublicKey => {
                BunkerResponse::PublicKey {
                    pubkey: self.master_key.verifying_key().to_hex(),
                }
            }
            BunkerRequest::DeriveChild { path } => {
                match self.derive_child(&path) {
                    Ok(key) => BunkerResponse::ChildKey {
                        pubkey: key.verifying_key().to_hex(),
                    },
                    Err(e) => BunkerResponse::Error { message: e.to_string() },
                }
            }
            BunkerRequest::Ping => BunkerResponse::Pong,
        }
    }

    /// Sign a Nostr event (the ONLY signing operation)
    fn sign_event(&self, event_json: &str) -> Result<Signature, BunkerError> {
        let event: serde_json::Value = serde_json::from_str(event_json)
            .map_err(|e| BunkerError::InvalidEvent(e.to_string()))?;

        // Verify event is well-formed
        let kind = event.get("kind").and_then(|v| v.as_u64())
            .ok_or(BunkerError::MissingKind)?;

        // Only allow specific event kinds (security policy)
        if !Self::allowed_kind(kind) {
            return Err(BunkerError::KindNotAllowed(kind));
        }

        // Verify the event's pubkey matches our agent
        let event_pubkey = event.get("pubkey").and_then(|v| v.as_str())
            .ok_or(BunkerError::MissingPubkey)?;
        if event_pubkey != self.agent_identity.agent_npub {
            return Err(BunkerError::PubkeyMismatch);
        }

        // Serialize event for signing (Nostr format)
        let serialized = Self::serialize_for_signing(&event)?;
        let signature = self.master_key.sign(&serialized);
        Ok(signature)
    }

    /// Derive child key for sub-identity (BIP-32 style)
    fn derive_child(&self, path: &str) -> Result<SigningKey, BunkerError> {
        let mut cache = self.derived_keys.lock().unwrap();
        if let Some(key) = cache.get(path) {
            return Ok(key.clone());
        }

        // Derive using blake3 keyed hash
        let seed = blake3::keyed_hash(
            b"bunker-child-derivation",
            &format!("{}:{}", self.master_key.to_bytes().to_hex(), path).as_bytes()
        );
        let child_key = SigningKey::from_bytes(&seed.as_bytes()[..32].try_into().unwrap());
        cache.insert(path.to_string(), child_key.clone());
        Ok(child_key)
    }

    /// Allowed event kinds for signing
    fn allowed_kind(kind: u64) -> bool {
        matches!(kind,
            31000 |  // Birth
            31001 |  // Device Binding
            31010 |  // Embodiment Offer
            31011 |  // Embodiment Accept
            31012 |  // Embodiment Command
            31013 |  // Embodiment End
            31020 |  // Capture Receipt
            31030 |  // Scene Receipt
            31040 |  // Action Receipt
            1901 |   // Creation Receipt (IP layer)
            1902     // Attestation (IP layer)
        )
    }

    /// Nostr event serialization for signing
    fn serialize_for_signing(event: &serde_json::Value) -> Result<Vec<u8>, BunkerError> {
        // Nostr event serialization: [0, pubkey, created_at, kind, tags, content]
        let pubkey = event.get("pubkey").and_then(|v| v.as_str())
            .ok_or(BunkerError::MissingPubkey)?;
        let created_at = event.get("created_at").and_then(|v| v.as_u64())
            .ok_or(BunkerError::MissingCreatedAt)?;
        let kind = event.get("kind").and_then(|v| v.as_u64())
            .ok_or(BunkerError::MissingKind)?;
        let tags = event.get("tags").and_then(|v| v.as_array())
            .ok_or(BunkerError::MissingTags)?;
        let content = event.get("content").and_then(|v| v.as_str())
            .ok_or(BunkerError::MissingContent)?;

        // Serialize as Nostr wire format
        let mut buf = Vec::new();
        buf.push(0); // Event version
        buf.extend_from_slice(&hex::decode(pubkey).map_err(|_| BunkerError::InvalidPubkey)?);
        buf.extend_from_slice(&created_at.to_be_bytes());
        buf.extend_from_slice(&(kind as u16).to_be_bytes());
        // Tags serialization (simplified)
        buf.extend_from_slice(serde_json::to_vec(tags).unwrap_or_default().as_slice());
        buf.extend_from_slice(content.as_bytes());
        Ok(buf)
    }

    /// Get agent npub
    pub fn agent_npub(&self) -> &str {
        &self.agent_identity.agent_npub
    }

    /// Get device ID
    pub fn device_id(&self) -> &DeviceId {
        &self.device_id
    }
}

/// Bunker service — runs as separate systemd user (sovereign-bunker)
pub struct BunkerService {
    bunker: Arc<Bunker>,
    socket_path: String,
}

impl BunkerService {
    pub fn new(bunker: Bunker, socket_path: String) -> Self {
        Self {
            bunker: Arc::new(bunker),
            socket_path,
        }
    }

    /// Run the Unix socket server
    pub async fn run(&self) -> Result<(), BunkerError> {
        use tokio::net::UnixListener;
        use tokio::io::{AsyncReadExt, AsyncWriteExt};

        // Remove existing socket
        let _ = std::fs::remove_file(&self.socket_path);

        let listener = UnixListener::bind(&self.socket_path)
            .map_err(|e| BunkerError::SocketBind(e.to_string()))?;

        // Set permissions: only sovereign user can connect
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = std::fs::metadata(&self.socket_path)?.permissions();
            perms.set_mode(0o660); // rw for owner+group
            std::fs::set_permissions(&self.socket_path, perms)?;
        }

        loop {
            let (mut stream, _) = listener.accept().await
                .map_err(|e| BunkerError::SocketAccept(e.to_string()))?;

            let bunker = self.bunker.clone();
            tokio::spawn(async move {
                let mut buf = vec![0u8; 8192];
                let n = stream.read(&mut buf).await.unwrap_or(0);
                if n == 0 { return; }

                let request: BunkerRequest = match serde_json::from_slice(&buf[..n]) {
                    Ok(r) => r,
                    Err(_) => {
                        let resp = BunkerResponse::Error { message: "Invalid JSON".into() };
                        let _ = stream.write_all(&serde_json::to_vec(&resp).unwrap()).await;
                        return;
                    }
                };

                let response = bunker.handle_request(request).await;
                let _ = stream.write_all(&serde_json::to_vec(&response).unwrap()).await;
            });
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum BunkerError {
    #[error("Keystore error: {0}")]
    Keystore(String),
    #[error("Invalid event: {0}")]
    InvalidEvent(String),
    #[error("Missing kind field")]
    MissingKind,
    #[error("Missing pubkey field")]
    MissingPubkey,
    #[error("Missing created_at field")]
    MissingCreatedAt,
    #[error("Missing tags field")]
    MissingTags,
    #[error("Missing content field")]
    MissingContent,
    #[error("Invalid pubkey hex")]
    InvalidPubkey,
    #[error("Kind {0} not allowed for signing")]
    KindNotAllowed(u64),
    #[error("Event pubkey mismatch")]
    PubkeyMismatch,
    #[error("Socket bind failed: {0}")]
    SocketBind(String),
    #[error("Socket accept failed: {0}")]
    SocketAccept(String),
    #[error("Derivation failed: {0}")]
    Derivation(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_bunker_signing() {
        let master_seed = [42u8; 32];
        let device_id = DeviceId([1u8; 32]);
        let agent_identity = AgentIdentity {
            agent_npub: "npub1test".into(),
            device_id: device_id.clone(),
            sui_address: SuiAddress("0xabc".into()),
            mnemonic: None,
            dna_fingerprint: crate::identity::DnaFingerprint([0u8; 16]),
            born_at: 1000,
            odù_primary: 1,
            odù_secondary: 2,
        };

        let bunker = Bunker::new(master_seed, device_id, agent_identity);

        let event_json = r#"{
            "kind": 31000,
            "pubkey": "npub1test",
            "created_at": 1000,
            "tags": [],
            "content": "{}"
        }"#;

        let response = bunker.handle_request(BunkerRequest::SignEvent { event_json: event_json.into() }).await;
        match response {
            BunkerResponse::SignedEvent { signature, .. } => {
                assert!(!signature.is_empty());
            }
            _ => panic!("Expected SignedEvent"),
        }
    }

    #[tokio::test]
    async fn test_bunker_rejects_wrong_pubkey() {
        let master_seed = [42u8; 32];
        let device_id = DeviceId([1u8; 32]);
        let agent_identity = AgentIdentity {
            agent_npub: "npub1test".into(),
            device_id: device_id.clone(),
            sui_address: SuiAddress("0xabc".into()),
            mnemonic: None,
            dna_fingerprint: crate::identity::DnaFingerprint([0u8; 16]),
            born_at: 1000,
            odù_primary: 1,
            odù_secondary: 2,
        };

        let bunker = Bunker::new(master_seed, device_id, agent_identity);

        let event_json = r#"{
            "kind": 31000,
            "pubkey": "npub1WRONG",
            "created_at": 1000,
            "tags": [],
            "content": "{}"
        }"#;

        let response = bunker.handle_request(BunkerRequest::SignEvent { event_json: event_json.into() }).await;
        match response {
            BunkerResponse::Error { message } => {
                assert!(message.contains("PubkeyMismatch"));
            }
            _ => panic!("Expected Error"),
        }
    }

    #[tokio::test]
    async fn test_bunker_derived_keys() {
        let master_seed = [42u8; 32];
        let device_id = DeviceId([1u8; 32]);
        let agent_identity = AgentIdentity {
            agent_npub: "npub1test".into(),
            device_id: device_id.clone(),
            sui_address: SuiAddress("0xabc".into()),
            mnemonic: None,
            dna_fingerprint: crate::identity::DnaFingerprint([0u8; 16]),
            born_at: 1000,
            odù_primary: 1,
            odù_secondary: 2,
        };

        let bunker = Bunker::new(master_seed, device_id, agent_identity);

        let child1 = bunker.derive_child("m/44'/501'/0'/0/0").unwrap();
        let child2 = bunker.derive_child("m/44'/501'/0'/0/0").unwrap();
        // Same path = same key
        assert_eq!(child1.to_bytes(), child2.to_bytes());

        let child3 = bunker.derive_child("m/44'/501'/0'/0/1").unwrap();
        assert_ne!(child1.to_bytes(), child3.to_bytes());
    }
}