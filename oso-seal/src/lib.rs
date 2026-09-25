//! OSO-Seal Interface — Phase 16.2
//! Abstraction over Sui/Seal / TEE / NIP-46 / OSO-native access control.
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccessPolicy {
    pub policy_id: String,
    pub allowed_identities: Vec<String>,
    pub expiry: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptedBlob {
    pub ciphertext: Vec<u8>,
    pub nonce: [u8; 12],
    pub policy_id: String,
    pub provider: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccessGrant {
    pub grant_id: String,
    pub identity: String,
    pub policy_id: String,
    pub granted_at: u64,
    pub expiry: Option<u64>,
}

#[async_trait]
pub trait AccessProvider: Send + Sync {
    async fn encrypt(&self, data: &[u8], policy: &AccessPolicy) -> Result<EncryptedBlob, String>;
    async fn authorize(&self, blob: &EncryptedBlob, identity: &str) -> Result<AccessGrant, String>;
    async fn decrypt(&self, blob: &EncryptedBlob, grant: &AccessGrant) -> Result<Vec<u8>, String>;
    async fn revoke(&self, grant_id: &str) -> Result<(), String>;
    fn provider_name(&self) -> &str;
}

/// Software-only fallback (XOR with grant_id hash — NOT for production use).
/// Wired as the Phase 16.2 stub until SealProvider/TeeProvider are wired.
pub struct SoftwareProvider;

#[async_trait]
impl AccessProvider for SoftwareProvider {
    async fn encrypt(&self, data: &[u8], policy: &AccessPolicy) -> Result<EncryptedBlob, String> {
        // Stub: identity transform — replace with real encryption in Phase 16.2 follow-up
        Ok(EncryptedBlob {
            ciphertext: data.to_vec(),
            nonce: [0u8; 12],
            policy_id: policy.policy_id.clone(),
            provider: "software".into(),
        })
    }
    async fn authorize(&self, blob: &EncryptedBlob, identity: &str) -> Result<AccessGrant, String> {
        Ok(AccessGrant {
            grant_id: format!("grant:{}:{}", blob.policy_id, identity),
            identity: identity.to_string(),
            policy_id: blob.policy_id.clone(),
            granted_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs()).unwrap_or(0),
            expiry: None,
        })
    }
    async fn decrypt(&self, blob: &EncryptedBlob, _grant: &AccessGrant) -> Result<Vec<u8>, String> {
        Ok(blob.ciphertext.clone())
    }
    async fn revoke(&self, _grant_id: &str) -> Result<(), String> { Ok(()) }
    fn provider_name(&self) -> &str { "software" }
}

pub struct SealProvider { pub object_id: String }
pub struct TeeProvider { pub tee_url: String }
