/// OsoRouter — Phase 16.4
///
/// A single entry-point that routes storage / seal / compute calls to the
/// right backend based on a preference policy.  Agents call `OsoRouter`,
/// never the individual providers directly.
use super::oso_compute::{ComputeError, OsoComputeJob, OsoComputeProvider, OsoJobStatus, OsoWorkload};
use super::oso_seal::{AccessGrant, AccessPolicy, AccessProvider, EncryptedBlob, GrantId, SealError};
use super::oso_storage::{StorageCommitment, StorageError, StorageProvider};
use std::sync::Arc;
use std::time::Duration;

/// Policy governing which backend to prefer.
#[derive(Debug, Clone)]
pub struct RoutingPolicy {
    /// Preferred storage backend name ("walrus", "arweave", "local", "freenet").
    pub preferred_storage: &'static str,
    /// Fall back to local if preferred is unavailable.
    pub storage_fallback_local: bool,
    /// Preferred seal backend name ("sui_seal", "nip46", "local").
    pub preferred_seal: &'static str,
}

impl Default for RoutingPolicy {
    fn default() -> Self {
        Self {
            preferred_storage: "local",
            storage_fallback_local: true,
            preferred_seal: "local",
        }
    }
}

pub struct OsoRouter {
    storage_backends: Vec<(String, Arc<dyn StorageProvider>)>,
    seal_backends: Vec<(String, Arc<dyn AccessProvider>)>,
    compute_backends: Vec<Arc<dyn OsoComputeProvider>>,
    policy: RoutingPolicy,
}

impl OsoRouter {
    pub fn new(policy: RoutingPolicy) -> Self {
        Self {
            storage_backends: vec![],
            seal_backends: vec![],
            compute_backends: vec![],
            policy,
        }
    }

    pub fn with_storage(mut self, name: impl Into<String>, p: Arc<dyn StorageProvider>) -> Self {
        self.storage_backends.push((name.into(), p));
        self
    }

    pub fn with_seal(mut self, name: impl Into<String>, p: Arc<dyn AccessProvider>) -> Self {
        self.seal_backends.push((name.into(), p));
        self
    }

    pub fn with_compute(mut self, p: Arc<dyn OsoComputeProvider>) -> Self {
        self.compute_backends.push(p);
        self
    }

    // ── Storage routing ───────────────────────────────────────────────────────

    pub fn store(&self, data: &[u8]) -> Result<StorageCommitment, StorageError> {
        if let Some(backend) = self.find_storage(self.policy.preferred_storage) {
            match backend.put(data) {
                Ok(c) => return Ok(c),
                Err(StorageError::Unavailable(_)) if self.policy.storage_fallback_local => {}
                Err(e) => return Err(e),
            }
        }
        if self.policy.storage_fallback_local {
            if let Some(local) = self.find_storage("local") {
                return local.put(data);
            }
        }
        Err(StorageError::Unavailable("no storage backend available".into()))
    }

    pub fn retrieve(&self, commitment: &StorageCommitment) -> Result<Vec<u8>, StorageError> {
        // Try the hint backend first, then all others.
        let hint = format!("{:?}", commitment.provider_hint).to_lowercase();
        if let Some(backend) = self.find_storage(&hint) {
            if let Ok(data) = backend.get(commitment) {
                return Ok(data);
            }
        }
        for (_, backend) in &self.storage_backends {
            if let Ok(data) = backend.get(commitment) {
                return Ok(data);
            }
        }
        Err(StorageError::NotFound(commitment.provider_ref.clone()))
    }

    pub fn pin_storage(&self, commitment: &StorageCommitment, duration: Duration) -> Result<(), StorageError> {
        if let Some(backend) = self.find_storage(&format!("{:?}", commitment.provider_hint).to_lowercase()) {
            return backend.pin(commitment, duration);
        }
        Err(StorageError::NotFound(commitment.provider_ref.clone()))
    }

    // ── Seal routing ──────────────────────────────────────────────────────────

    pub fn seal_encrypt(&self, data: &[u8], policy: &AccessPolicy) -> Result<EncryptedBlob, SealError> {
        let backend = self.find_seal(self.policy.preferred_seal)
            .ok_or_else(|| SealError::Unavailable("no seal backend configured".into()))?;
        backend.encrypt(data, policy)
    }

    pub fn seal_authorize(&self, blob: &EncryptedBlob, identity: &str) -> Result<AccessGrant, SealError> {
        let hint = format!("{:?}", blob.backend).to_lowercase();
        let backend = self.find_seal(&hint)
            .or_else(|| self.find_seal(self.policy.preferred_seal))
            .ok_or_else(|| SealError::Unavailable("no seal backend for blob".into()))?;
        backend.authorize(blob, identity)
    }

    pub fn seal_decrypt(&self, blob: &EncryptedBlob, grant: &AccessGrant) -> Result<Vec<u8>, SealError> {
        let hint = format!("{:?}", blob.backend).to_lowercase();
        let backend = self.find_seal(&hint)
            .or_else(|| self.find_seal(self.policy.preferred_seal))
            .ok_or_else(|| SealError::Unavailable("no seal backend for blob".into()))?;
        backend.decrypt(blob, grant)
    }

    pub fn seal_revoke(&self, grant_id: &GrantId) -> Result<(), SealError> {
        for (_, backend) in &self.seal_backends {
            if backend.revoke(grant_id).is_ok() {
                return Ok(());
            }
        }
        Err(SealError::GrantNotFound(grant_id.clone()))
    }

    // ── Compute routing ───────────────────────────────────────────────────────

    pub fn compute_submit(&self, workload: OsoWorkload) -> Result<OsoComputeJob, ComputeError> {
        for backend in &self.compute_backends {
            if backend.can_handle(&workload) {
                return backend.submit(workload);
            }
        }
        Err(ComputeError::NoProvider(format!("image={}", workload.image)))
    }

    pub fn compute_status(&self, provider: &str, job_id: &str) -> Result<OsoJobStatus, ComputeError> {
        for backend in &self.compute_backends {
            if backend.name() == provider {
                return backend.status(job_id);
            }
        }
        Err(ComputeError::NoProvider(provider.into()))
    }

    // ── Helpers ───────────────────────────────────────────────────────────────

    fn find_storage(&self, name: &str) -> Option<&Arc<dyn StorageProvider>> {
        self.storage_backends.iter().find(|(n, _)| n == name).map(|(_, p)| p)
    }

    fn find_seal(&self, name: &str) -> Option<&Arc<dyn AccessProvider>> {
        self.seal_backends.iter().find(|(n, _)| n == name).map(|(_, p)| p)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::integrations::oso_seal::LocalSealProvider;
    use crate::integrations::oso_storage::LocalFsProvider;

    fn test_router() -> (OsoRouter, std::path::PathBuf) {
        let tmp = std::env::temp_dir().join(format!("oso-router-test-{}", uuid::Uuid::new_v4()));
        let router = OsoRouter::new(RoutingPolicy::default())
            .with_storage("local", Arc::new(LocalFsProvider::new(&tmp)))
            .with_seal("local", Arc::new(LocalSealProvider::random()));
        (router, tmp)
    }

    #[test]
    fn router_storage_roundtrip() {
        let (router, tmp) = test_router();
        let data = b"routing test payload";

        let commitment = router.store(data).expect("store ok");
        let retrieved = router.retrieve(&commitment).expect("retrieve ok");
        assert_eq!(retrieved, data);

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn router_seal_roundtrip() {
        let (router, tmp) = test_router();
        let plaintext = b"sealed agent secret";
        let policy = AccessPolicy::default();

        let blob = router.seal_encrypt(plaintext, &policy).expect("encrypt ok");
        let grant = router.seal_authorize(&blob, "agent-abc").expect("authorize ok");
        let decrypted = router.seal_decrypt(&blob, &grant).expect("decrypt ok");
        assert_eq!(decrypted, plaintext);

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn router_no_compute_backend_returns_error() {
        let router = OsoRouter::new(RoutingPolicy::default());
        use crate::integrations::oso_compute::OsoWorkload;
        let workload = OsoWorkload {
            workload_id: "test".into(),
            description: "test".into(),
            image: "docker:ubuntu".into(),
            args: vec![],
            env: vec![],
            min_vram_gb: 0,
            timeout_secs: 60,
            require_attestation: false,
        };
        assert!(matches!(router.compute_submit(workload), Err(ComputeError::NoProvider(_))));
    }
}
