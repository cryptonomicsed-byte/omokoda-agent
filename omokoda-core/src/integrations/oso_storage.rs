/// OSO-Storage interface — Phase 16.1
///
/// Agents store and retrieve bytes through a single `StorageProvider` trait.
/// The caller never knows whether the backend is Walrus, Arweave, local FS,
/// or Freenet.  All backends produce a `StorageCommitment` that is
/// independently verifiable.
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// An opaque, provider-agnostic proof that bytes were stored.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageCommitment {
    /// BLAKE3 content hash of the stored bytes.
    pub content_hash: [u8; 32],
    /// Which backend holds the data (informational hint — not enforced).
    pub provider_hint: StorageBackend,
    /// Size of the stored payload in bytes.
    pub size_bytes: u64,
    /// Unix timestamp when the data was stored.
    pub stored_at: u64,
    /// Provider-specific opaque reference (blob id, tx id, content-id …).
    pub provider_ref: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum StorageBackend {
    Walrus,
    Arweave,
    LocalFs,
    Freenet,
    Unknown,
}

#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    #[error("backend unavailable: {0}")]
    Unavailable(String),
    #[error("content not found: {0}")]
    NotFound(String),
    #[error("verification failed: hash mismatch")]
    VerificationFailed,
    #[error("backend error: {0}")]
    Backend(String),
}

/// Single interface every storage backend implements.
pub trait StorageProvider: Send + Sync {
    fn backend(&self) -> StorageBackend;

    /// Store bytes; returns a commitment that can be used to retrieve them.
    fn put(&self, data: &[u8]) -> Result<StorageCommitment, StorageError>;

    /// Retrieve bytes by commitment.
    fn get(&self, commitment: &StorageCommitment) -> Result<Vec<u8>, StorageError>;

    /// Extend the pin duration so data is not garbage-collected.
    fn pin(&self, commitment: &StorageCommitment, duration: Duration) -> Result<(), StorageError>;

    /// Verify the commitment is still intact (checks hash against live data).
    fn verify(&self, commitment: &StorageCommitment) -> Result<bool, StorageError>;
}

// ── Local FS backend (always available — used in tests + offline mode) ───────

pub struct LocalFsProvider {
    pub root: std::path::PathBuf,
}

impl LocalFsProvider {
    pub fn new(root: impl Into<std::path::PathBuf>) -> Self {
        Self { root: root.into() }
    }
}

impl StorageProvider for LocalFsProvider {
    fn backend(&self) -> StorageBackend { StorageBackend::LocalFs }

    fn put(&self, data: &[u8]) -> Result<StorageCommitment, StorageError> {
        let hash = *blake3::hash(data).as_bytes();
        let ref_name = hex::encode(hash);
        let path = self.root.join(&ref_name);
        std::fs::create_dir_all(&self.root)
            .and_then(|_| std::fs::write(&path, data))
            .map_err(|e| StorageError::Backend(e.to_string()))?;
        Ok(StorageCommitment {
            content_hash: hash,
            provider_hint: StorageBackend::LocalFs,
            size_bytes: data.len() as u64,
            stored_at: now_secs(),
            provider_ref: ref_name,
        })
    }

    fn get(&self, commitment: &StorageCommitment) -> Result<Vec<u8>, StorageError> {
        let path = self.root.join(&commitment.provider_ref);
        std::fs::read(&path).map_err(|_| StorageError::NotFound(commitment.provider_ref.clone()))
    }

    fn pin(&self, _commitment: &StorageCommitment, _duration: Duration) -> Result<(), StorageError> {
        Ok(()) // local FS is always pinned
    }

    fn verify(&self, commitment: &StorageCommitment) -> Result<bool, StorageError> {
        let data = self.get(commitment)?;
        let hash = *blake3::hash(&data).as_bytes();
        Ok(hash == commitment.content_hash)
    }
}

// ── Walrus stub (HTTP backend — real impl calls Walrus publisher node) ────────

pub struct WalrusProvider {
    pub endpoint: String,
}

impl WalrusProvider {
    pub fn new(endpoint: impl Into<String>) -> Self {
        Self { endpoint: endpoint.into() }
    }
}

impl StorageProvider for WalrusProvider {
    fn backend(&self) -> StorageBackend { StorageBackend::Walrus }

    fn put(&self, _data: &[u8]) -> Result<StorageCommitment, StorageError> {
        // Phase 16.1 stub: real impl POSTs to Walrus publisher + records blob_id.
        Err(StorageError::Unavailable("Walrus PUT not yet implemented".into()))
    }

    fn get(&self, _commitment: &StorageCommitment) -> Result<Vec<u8>, StorageError> {
        Err(StorageError::Unavailable("Walrus GET not yet implemented".into()))
    }

    fn pin(&self, _commitment: &StorageCommitment, _duration: Duration) -> Result<(), StorageError> {
        Err(StorageError::Unavailable("Walrus PIN not yet implemented".into()))
    }

    fn verify(&self, _commitment: &StorageCommitment) -> Result<bool, StorageError> {
        Err(StorageError::Unavailable("Walrus VERIFY not yet implemented".into()))
    }
}

// ── Arweave stub ──────────────────────────────────────────────────────────────

pub struct ArweaveProvider {
    pub gateway: String,
}

impl StorageProvider for ArweaveProvider {
    fn backend(&self) -> StorageBackend { StorageBackend::Arweave }

    fn put(&self, _data: &[u8]) -> Result<StorageCommitment, StorageError> {
        Err(StorageError::Unavailable("Arweave PUT not yet implemented".into()))
    }

    fn get(&self, _commitment: &StorageCommitment) -> Result<Vec<u8>, StorageError> {
        Err(StorageError::Unavailable("Arweave GET not yet implemented".into()))
    }

    fn pin(&self, _commitment: &StorageCommitment, _duration: Duration) -> Result<(), StorageError> {
        Ok(()) // Arweave is permanent by design
    }

    fn verify(&self, _commitment: &StorageCommitment) -> Result<bool, StorageError> {
        Err(StorageError::Unavailable("Arweave VERIFY not yet implemented".into()))
    }
}

// ── Freenet stub ──────────────────────────────────────────────────────────────

pub struct FreenetProvider {
    pub node_addr: String,
}

impl StorageProvider for FreenetProvider {
    fn backend(&self) -> StorageBackend { StorageBackend::Freenet }

    fn put(&self, _data: &[u8]) -> Result<StorageCommitment, StorageError> {
        Err(StorageError::Unavailable("Freenet PUT not yet implemented".into()))
    }

    fn get(&self, _commitment: &StorageCommitment) -> Result<Vec<u8>, StorageError> {
        Err(StorageError::Unavailable("Freenet GET not yet implemented".into()))
    }

    fn pin(&self, _commitment: &StorageCommitment, _duration: Duration) -> Result<(), StorageError> {
        Err(StorageError::Unavailable("Freenet PIN not yet implemented".into()))
    }

    fn verify(&self, _commitment: &StorageCommitment) -> Result<bool, StorageError> {
        Err(StorageError::Unavailable("Freenet VERIFY not yet implemented".into()))
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
    fn local_fs_roundtrip() {
        let tmp = tempfile_dir();
        let provider = LocalFsProvider::new(&tmp);
        let data = b"sovereign agent memory fragment";

        let commitment = provider.put(data).expect("put must succeed");
        assert_eq!(commitment.size_bytes, data.len() as u64);
        assert_eq!(commitment.provider_hint, StorageBackend::LocalFs);

        let retrieved = provider.get(&commitment).expect("get must succeed");
        assert_eq!(retrieved, data);

        assert!(provider.verify(&commitment).expect("verify must succeed"));

        // cleanup
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn local_fs_verify_detects_tampering() {
        let tmp = tempfile_dir();
        let provider = LocalFsProvider::new(&tmp);
        let commitment = provider.put(b"original").expect("put ok");

        // Tamper with the stored file
        let path = std::path::PathBuf::from(&tmp).join(&commitment.provider_ref);
        std::fs::write(&path, b"tampered").unwrap();

        assert!(!provider.verify(&commitment).expect("verify runs"));

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn walrus_returns_unavailable() {
        let p = WalrusProvider::new("http://localhost:9999");
        assert!(matches!(p.put(b"x"), Err(StorageError::Unavailable(_))));
    }

    fn tempfile_dir() -> std::path::PathBuf {
        std::env::temp_dir().join(format!("oso-storage-test-{}", uuid::Uuid::new_v4()))
    }
}
