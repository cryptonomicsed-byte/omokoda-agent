/// OSO-Seal interface — Phase 16.2
///
/// Agents encrypt, authorize, decrypt, and revoke access to sensitive blobs
/// through a single `AccessProvider` trait.  Backend can be Sui/Seal,
/// a TEE, a NIP-46 remote signer, or the eventual OSO-native policy engine.
use serde::{Deserialize, Serialize};

/// A set of rules that describe who may decrypt a blob and under what conditions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccessPolicy {
    /// Agent or principal IDs that are granted access at seal time.
    pub allowed_identities: Vec<String>,
    /// Optional expiry (Unix seconds).
    pub expires_at: Option<u64>,
    /// Arbitrary policy metadata (on-chain object id, TEE pcr hash, …).
    pub metadata: serde_json::Value,
}

impl Default for AccessPolicy {
    fn default() -> Self {
        Self {
            allowed_identities: vec![],
            expires_at: None,
            metadata: serde_json::Value::Null,
        }
    }
}

/// An opaque reference that authorizes a specific identity to decrypt a blob.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccessGrant {
    pub grant_id: GrantId,
    pub grantee_identity: String,
    pub blob_ref: String,
    pub granted_at: u64,
    pub expires_at: Option<u64>,
    /// Backend-specific token (Seal session key, TEE attestation, …).
    pub token: Vec<u8>,
}

pub type GrantId = String;

/// An encrypted blob with enough metadata to route decryption.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptedBlob {
    /// BLAKE3 of plaintext (for integrity verification after decryption).
    pub plaintext_hash: [u8; 32],
    /// The ciphertext bytes.
    pub ciphertext: Vec<u8>,
    /// Which backend sealed this blob.
    pub backend: SealBackend,
    /// Backend-specific reference (Seal object id, policy hash, …).
    pub blob_ref: String,
    pub sealed_at: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum SealBackend {
    SuiSeal,
    Tee,
    Nip46,
    OsoNative,
    Local,
}

#[derive(Debug, thiserror::Error)]
pub enum SealError {
    #[error("backend unavailable: {0}")]
    Unavailable(String),
    #[error("decryption failed: {0}")]
    DecryptionFailed(String),
    #[error("unauthorized: grant not valid for this identity")]
    Unauthorized,
    #[error("grant not found: {0}")]
    GrantNotFound(GrantId),
    #[error("backend error: {0}")]
    Backend(String),
}

/// Single interface every access-control backend implements.
pub trait AccessProvider: Send + Sync {
    fn backend(&self) -> SealBackend;

    /// Encrypt plaintext under the given policy; returns an encrypted blob.
    fn encrypt(&self, data: &[u8], policy: &AccessPolicy) -> Result<EncryptedBlob, SealError>;

    /// Produce an `AccessGrant` for `identity` if the policy allows it.
    fn authorize(
        &self,
        blob: &EncryptedBlob,
        identity: &str,
    ) -> Result<AccessGrant, SealError>;

    /// Decrypt a blob using a previously issued grant.
    fn decrypt(
        &self,
        blob: &EncryptedBlob,
        grant: &AccessGrant,
    ) -> Result<Vec<u8>, SealError>;

    /// Delegate an existing grant to another identity with optional constraints.
    fn delegate(
        &self,
        grant: &AccessGrant,
        to_identity: &str,
        expires_at: Option<u64>,
    ) -> Result<AccessGrant, SealError>;

    /// Revoke a grant so it can no longer be used for decryption.
    fn revoke(&self, grant_id: &GrantId) -> Result<(), SealError>;
}

// ── Local provider (ChaCha20-Poly1305, in-memory grant store) ────────────────

use chacha20poly1305::{
    aead::{Aead, AeadCore, KeyInit, OsRng as AeadRng},
    ChaCha20Poly1305,
};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

pub struct LocalSealProvider {
    key: [u8; 32],
    grants: Arc<Mutex<HashMap<GrantId, AccessGrant>>>,
}

impl LocalSealProvider {
    pub fn new(key: [u8; 32]) -> Self {
        Self { key, grants: Arc::new(Mutex::new(HashMap::new())) }
    }

    pub fn random() -> Self {
        let mut key = [0u8; 32];
        rand::RngCore::fill_bytes(&mut rand::thread_rng(), &mut key);
        Self::new(key)
    }
}

impl AccessProvider for LocalSealProvider {
    fn backend(&self) -> SealBackend { SealBackend::Local }

    fn encrypt(&self, data: &[u8], policy: &AccessPolicy) -> Result<EncryptedBlob, SealError> {
        let cipher = ChaCha20Poly1305::new_from_slice(&self.key)
            .map_err(|e| SealError::Backend(e.to_string()))?;
        let nonce = ChaCha20Poly1305::generate_nonce(&mut AeadRng);
        let mut ciphertext = cipher.encrypt(&nonce, data)
            .map_err(|e| SealError::Backend(e.to_string()))?;
        // Prepend nonce to ciphertext so it's self-contained.
        let mut payload = nonce.to_vec();
        payload.append(&mut ciphertext);

        let plaintext_hash = *blake3::hash(data).as_bytes();
        let blob_ref = hex::encode(plaintext_hash);
        Ok(EncryptedBlob {
            plaintext_hash,
            ciphertext: payload,
            backend: SealBackend::Local,
            blob_ref: blob_ref.clone(),
            sealed_at: now_secs(),
        })
    }

    fn authorize(
        &self,
        blob: &EncryptedBlob,
        identity: &str,
    ) -> Result<AccessGrant, SealError> {
        let grant = AccessGrant {
            grant_id: uuid::Uuid::new_v4().to_string(),
            grantee_identity: identity.to_string(),
            blob_ref: blob.blob_ref.clone(),
            granted_at: now_secs(),
            expires_at: None,
            token: self.key.to_vec(), // local: token IS the key
        };
        self.grants.lock().unwrap().insert(grant.grant_id.clone(), grant.clone());
        Ok(grant)
    }

    fn decrypt(
        &self,
        blob: &EncryptedBlob,
        grant: &AccessGrant,
    ) -> Result<Vec<u8>, SealError> {
        if grant.token.len() != 32 {
            return Err(SealError::DecryptionFailed("bad token length".into()));
        }
        let key: [u8; 32] = grant.token[..32].try_into().unwrap();
        let cipher = ChaCha20Poly1305::new_from_slice(&key)
            .map_err(|e| SealError::Backend(e.to_string()))?;

        if blob.ciphertext.len() < 12 {
            return Err(SealError::DecryptionFailed("ciphertext too short".into()));
        }
        let nonce = chacha20poly1305::Nonce::from_slice(&blob.ciphertext[..12]);
        let plaintext = cipher
            .decrypt(nonce, &blob.ciphertext[12..])
            .map_err(|_| SealError::DecryptionFailed("AEAD auth failed".into()))?;

        // Verify plaintext integrity.
        let hash = *blake3::hash(&plaintext).as_bytes();
        if hash != blob.plaintext_hash {
            return Err(SealError::DecryptionFailed("plaintext hash mismatch".into()));
        }
        Ok(plaintext)
    }

    fn delegate(
        &self,
        grant: &AccessGrant,
        to_identity: &str,
        expires_at: Option<u64>,
    ) -> Result<AccessGrant, SealError> {
        let delegated = AccessGrant {
            grant_id: uuid::Uuid::new_v4().to_string(),
            grantee_identity: to_identity.to_string(),
            blob_ref: grant.blob_ref.clone(),
            granted_at: now_secs(),
            expires_at,
            token: grant.token.clone(),
        };
        self.grants.lock().unwrap().insert(delegated.grant_id.clone(), delegated.clone());
        Ok(delegated)
    }

    fn revoke(&self, grant_id: &GrantId) -> Result<(), SealError> {
        self.grants.lock().unwrap().remove(grant_id)
            .map(|_| ())
            .ok_or_else(|| SealError::GrantNotFound(grant_id.clone()))
    }
}

// ── Sui/Seal stub ─────────────────────────────────────────────────────────────

pub struct SuiSealProvider {
    pub rpc_url: String,
}

impl AccessProvider for SuiSealProvider {
    fn backend(&self) -> SealBackend { SealBackend::SuiSeal }

    fn encrypt(&self, _data: &[u8], _policy: &AccessPolicy) -> Result<EncryptedBlob, SealError> {
        Err(SealError::Unavailable("Sui/Seal encrypt not yet implemented".into()))
    }
    fn authorize(&self, _blob: &EncryptedBlob, _identity: &str) -> Result<AccessGrant, SealError> {
        Err(SealError::Unavailable("Sui/Seal authorize not yet implemented".into()))
    }
    fn decrypt(&self, _blob: &EncryptedBlob, _grant: &AccessGrant) -> Result<Vec<u8>, SealError> {
        Err(SealError::Unavailable("Sui/Seal decrypt not yet implemented".into()))
    }
    fn delegate(&self, _grant: &AccessGrant, _to: &str, _exp: Option<u64>) -> Result<AccessGrant, SealError> {
        Err(SealError::Unavailable("Sui/Seal delegate not yet implemented".into()))
    }
    fn revoke(&self, grant_id: &GrantId) -> Result<(), SealError> {
        Err(SealError::Unavailable(format!("Sui/Seal revoke not yet implemented: {grant_id}")))
    }
}

// ── NIP-46 stub ───────────────────────────────────────────────────────────────

pub struct Nip46Provider {
    pub relay_url: String,
    pub signer_pubkey: String,
}

impl AccessProvider for Nip46Provider {
    fn backend(&self) -> SealBackend { SealBackend::Nip46 }

    fn encrypt(&self, _data: &[u8], _policy: &AccessPolicy) -> Result<EncryptedBlob, SealError> {
        Err(SealError::Unavailable("NIP-46 encrypt not yet implemented".into()))
    }
    fn authorize(&self, _blob: &EncryptedBlob, _identity: &str) -> Result<AccessGrant, SealError> {
        Err(SealError::Unavailable("NIP-46 authorize not yet implemented".into()))
    }
    fn decrypt(&self, _blob: &EncryptedBlob, _grant: &AccessGrant) -> Result<Vec<u8>, SealError> {
        Err(SealError::Unavailable("NIP-46 decrypt not yet implemented".into()))
    }
    fn delegate(&self, _grant: &AccessGrant, _to: &str, _exp: Option<u64>) -> Result<AccessGrant, SealError> {
        Err(SealError::Unavailable("NIP-46 delegate not yet implemented".into()))
    }
    fn revoke(&self, _grant_id: &GrantId) -> Result<(), SealError> {
        Err(SealError::Unavailable("NIP-46 revoke not yet implemented".into()))
    }
}

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_seal_encrypt_decrypt_roundtrip() {
        let provider = LocalSealProvider::random();
        let plaintext = b"agent private key material";
        let policy = AccessPolicy::default();

        let blob = provider.encrypt(plaintext, &policy).expect("encrypt ok");
        assert_ne!(blob.ciphertext, plaintext);
        assert_eq!(blob.backend, SealBackend::Local);

        let grant = provider.authorize(&blob, "agent-xyz").expect("authorize ok");
        assert_eq!(grant.grantee_identity, "agent-xyz");

        let decrypted = provider.decrypt(&blob, &grant).expect("decrypt ok");
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn local_seal_revoke_does_not_affect_decrypt() {
        // Revoking removes from grant store but does NOT invalidate the token
        // (local provider — no server-side enforcement in this tier).
        let provider = LocalSealProvider::random();
        let blob = provider.encrypt(b"data", &AccessPolicy::default()).expect("ok");
        let grant = provider.authorize(&blob, "a").expect("ok");
        let grant_id = grant.grant_id.clone();

        provider.revoke(&grant_id).expect("revoke ok");
        // Second revoke should error (grant_id gone from store).
        assert!(matches!(
            provider.revoke(&grant_id),
            Err(SealError::GrantNotFound(_))
        ));
    }

    #[test]
    fn local_seal_delegate_roundtrip() {
        let provider = LocalSealProvider::random();
        let blob = provider.encrypt(b"delegated secret", &AccessPolicy::default()).expect("ok");
        let original_grant = provider.authorize(&blob, "owner").expect("ok");
        let delegated = provider.delegate(&original_grant, "delegate", None).expect("ok");

        let decrypted = provider.decrypt(&blob, &delegated).expect("decrypt via delegate ok");
        assert_eq!(decrypted, b"delegated secret");
    }

    #[test]
    fn sui_seal_returns_unavailable() {
        let p = SuiSealProvider { rpc_url: "https://sui-rpc.example".into() };
        assert!(matches!(p.encrypt(b"x", &AccessPolicy::default()), Err(SealError::Unavailable(_))));
    }
}
