//! OSO-Storage Interface — Phase 16.1
//! Abstraction layer over Walrus / Arweave / LocalFs / Freenet.
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageCommitment {
    pub content_hash: [u8; 32],
    pub provider_hint: String,
    pub size: u64,
    pub timestamp: u64,
}

#[async_trait]
pub trait StorageProvider: Send + Sync {
    async fn put(&self, data: &[u8]) -> Result<StorageCommitment, String>;
    async fn get(&self, commitment: &StorageCommitment) -> Result<Vec<u8>, String>;
    async fn pin(&self, commitment: &StorageCommitment, duration_secs: u64) -> Result<(), String>;
    async fn verify(&self, commitment: &StorageCommitment) -> Result<bool, String>;
    fn provider_name(&self) -> &str;
}

pub struct LocalFsProvider {
    pub root: std::path::PathBuf,
}

#[async_trait]
impl StorageProvider for LocalFsProvider {
    async fn put(&self, data: &[u8]) -> Result<StorageCommitment, String> {
        let hash: [u8; 32] = *blake3::hash(data).as_bytes();
        let hex = hex::encode(hash);
        let path = self.root.join(&hex);
        std::fs::write(&path, data).map_err(|e| e.to_string())?;
        Ok(StorageCommitment {
            content_hash: hash,
            provider_hint: "local".into(),
            size: data.len() as u64,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs()).unwrap_or(0),
        })
    }
    async fn get(&self, commitment: &StorageCommitment) -> Result<Vec<u8>, String> {
        let hex = hex::encode(commitment.content_hash);
        std::fs::read(self.root.join(&hex)).map_err(|e| e.to_string())
    }
    async fn pin(&self, _commitment: &StorageCommitment, _duration_secs: u64) -> Result<(), String> {
        Ok(()) // local fs: pinned by default
    }
    async fn verify(&self, commitment: &StorageCommitment) -> Result<bool, String> {
        let data = self.get(commitment).await?;
        let hash: [u8; 32] = *blake3::hash(&data).as_bytes();
        Ok(hash == commitment.content_hash)
    }
    fn provider_name(&self) -> &str { "local" }
}

// Stub providers for Walrus/Arweave/Freenet — wired in Phase 16 follow-up
pub struct WalrusProvider { pub publisher_url: String, pub aggregator_url: String }
pub struct FreenetProvider { pub node_url: String }

#[async_trait]
impl StorageProvider for WalrusProvider {
    async fn put(&self, _data: &[u8]) -> Result<StorageCommitment, String> {
        Err("WalrusProvider: not yet wired (Phase 16.1 follow-up)".into())
    }
    async fn get(&self, _commitment: &StorageCommitment) -> Result<Vec<u8>, String> {
        Err("WalrusProvider: not yet wired".into())
    }
    async fn pin(&self, _: &StorageCommitment, _: u64) -> Result<(), String> { Ok(()) }
    async fn verify(&self, _: &StorageCommitment) -> Result<bool, String> { Ok(false) }
    fn provider_name(&self) -> &str { "walrus" }
}

#[async_trait]
impl StorageProvider for FreenetProvider {
    async fn put(&self, _data: &[u8]) -> Result<StorageCommitment, String> {
        Err("FreenetProvider: not yet wired".into())
    }
    async fn get(&self, _commitment: &StorageCommitment) -> Result<Vec<u8>, String> {
        Err("FreenetProvider: not yet wired".into())
    }
    async fn pin(&self, _: &StorageCommitment, _: u64) -> Result<(), String> { Ok(()) }
    async fn verify(&self, _: &StorageCommitment) -> Result<bool, String> { Ok(false) }
    fn provider_name(&self) -> &str { "freenet" }
}
