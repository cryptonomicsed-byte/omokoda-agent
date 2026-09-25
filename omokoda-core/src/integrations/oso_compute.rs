/// OSO-Compute interface — Phase 16.3
///
/// Wraps the UCX `ComputeProvider` trait behind an OSO-level interface that
/// adds attestation, result verification, and storage anchoring.  The UCX
/// adapters (GPU.ai, Akash, Vast) plug in unchanged.
use serde::{Deserialize, Serialize};

/// An OSO-level compute job with attestation requirements.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OsoWorkload {
    pub workload_id: String,
    /// Human-readable description (used in receipts).
    pub description: String,
    /// Docker image or WASM module reference.
    pub image: String,
    /// CLI args / entrypoint override.
    pub args: Vec<String>,
    /// Environment variables.
    pub env: Vec<(String, String)>,
    /// Minimum GPU VRAM in GB (0 = CPU-only acceptable).
    pub min_vram_gb: u32,
    /// Maximum wall-clock seconds before the job is cancelled.
    pub timeout_secs: u64,
    /// Whether a cryptographic attestation is required for the result.
    pub require_attestation: bool,
}

/// An in-flight or completed OSO compute job.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OsoComputeJob {
    pub job_id: String,
    pub workload: OsoWorkload,
    pub provider_name: String,
    pub submitted_at: u64,
    pub status: OsoJobStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum OsoJobStatus {
    Pending,
    Running,
    Completed,
    Failed(String),
    Cancelled,
}

/// The output of a completed job plus optional attestation proof.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OsoComputeResult {
    pub job_id: String,
    /// Raw stdout / output bytes.
    pub output: Vec<u8>,
    /// BLAKE3 hash of the output for storage anchoring.
    pub output_hash: [u8; 32],
    pub completed_at: u64,
    /// Provider-issued attestation (TEE quote, signed manifest, …).
    pub attestation: Option<OsoAttestation>,
}

/// A provider-issued proof that the computation ran on specific hardware/code.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OsoAttestation {
    pub provider_name: String,
    pub attestation_kind: AttestationKind,
    /// Raw attestation bytes (TEE quote, signed receipt, …).
    pub proof: Vec<u8>,
    pub issued_at: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AttestationKind {
    SignedManifest,
    TeeQuote,
    ZkProof,
    None,
}

#[derive(Debug, thiserror::Error)]
pub enum ComputeError {
    #[error("no provider available for workload: {0}")]
    NoProvider(String),
    #[error("submission failed: {0}")]
    SubmitFailed(String),
    #[error("attestation required but not provided")]
    AttestationMissing,
    #[error("provider error: {0}")]
    Provider(String),
}

/// OSO compute interface — a single entry-point regardless of backend.
pub trait OsoComputeProvider: Send + Sync {
    fn name(&self) -> &str;

    /// Returns true if this provider can handle the workload.
    fn can_handle(&self, workload: &OsoWorkload) -> bool;

    /// Submit a workload for execution.
    fn submit(&self, workload: OsoWorkload) -> Result<OsoComputeJob, ComputeError>;

    /// Poll job status.
    fn status(&self, job_id: &str) -> Result<OsoJobStatus, ComputeError>;

    /// Retrieve the result of a completed job.
    fn result(&self, job_id: &str) -> Result<OsoComputeResult, ComputeError>;

    /// Cancel a running job.
    fn cancel(&self, job_id: &str) -> Result<(), ComputeError>;
}

// ── VeilSim provider (OSOVM simulation backend) ───────────────────────────────

pub struct VeilSimProvider {
    pub osovm_url: String,
}

impl VeilSimProvider {
    pub fn new(url: impl Into<String>) -> Self {
        Self { osovm_url: url.into() }
    }

    pub fn from_env() -> Self {
        Self::new(
            std::env::var("OSOVM_URL")
                .unwrap_or_else(|_| "http://127.0.0.1:7800".into()),
        )
    }
}

impl OsoComputeProvider for VeilSimProvider {
    fn name(&self) -> &str { "veilsim" }

    fn can_handle(&self, workload: &OsoWorkload) -> bool {
        workload.min_vram_gb == 0 || workload.image.starts_with("osovm:")
    }

    fn submit(&self, workload: OsoWorkload) -> Result<OsoComputeJob, ComputeError> {
        // Phase 16.3 stub: real impl POSTs to OSOVM /api/veilsim endpoint.
        Err(ComputeError::SubmitFailed(format!(
            "VeilSim submit not yet implemented (osovm_url={})", self.osovm_url
        )))
    }

    fn status(&self, _job_id: &str) -> Result<OsoJobStatus, ComputeError> {
        Err(ComputeError::Provider("VeilSim status not yet implemented".into()))
    }

    fn result(&self, _job_id: &str) -> Result<OsoComputeResult, ComputeError> {
        Err(ComputeError::Provider("VeilSim result not yet implemented".into()))
    }

    fn cancel(&self, _job_id: &str) -> Result<(), ComputeError> {
        Err(ComputeError::Provider("VeilSim cancel not yet implemented".into()))
    }
}

// ── GPU.ai / Akash / Vast — UCX adapter shim ─────────────────────────────────
// These wrap the existing UCX adapters (ucx-broker crate) behind OsoComputeProvider.
// The UCX adapters already exist; we just bridge job types here.

pub struct UcxBackedProvider {
    pub provider_name: String,
    pub ucx_endpoint: String,
}

impl UcxBackedProvider {
    pub fn gpu_ai() -> Self {
        Self {
            provider_name: "gpu.ai".into(),
            ucx_endpoint: std::env::var("UCX_BROKER_URL")
                .unwrap_or_else(|_| "http://127.0.0.1:7790".into()),
        }
    }

    pub fn akash() -> Self {
        Self {
            provider_name: "akash".into(),
            ucx_endpoint: std::env::var("UCX_BROKER_URL")
                .unwrap_or_else(|_| "http://127.0.0.1:7790".into()),
        }
    }
}

impl OsoComputeProvider for UcxBackedProvider {
    fn name(&self) -> &str { &self.provider_name }

    fn can_handle(&self, workload: &OsoWorkload) -> bool {
        workload.min_vram_gb > 0 || !workload.image.starts_with("osovm:")
    }

    fn submit(&self, workload: OsoWorkload) -> Result<OsoComputeJob, ComputeError> {
        // Phase 16.3 stub: real impl forwards to UCX broker HTTP API.
        Err(ComputeError::SubmitFailed(format!(
            "UCX/{} submit not yet implemented (endpoint={})",
            self.provider_name, self.ucx_endpoint
        )))
    }

    fn status(&self, _job_id: &str) -> Result<OsoJobStatus, ComputeError> {
        Err(ComputeError::Provider(format!("UCX/{} status not yet implemented", self.provider_name)))
    }

    fn result(&self, _job_id: &str) -> Result<OsoComputeResult, ComputeError> {
        Err(ComputeError::Provider(format!("UCX/{} result not yet implemented", self.provider_name)))
    }

    fn cancel(&self, _job_id: &str) -> Result<(), ComputeError> {
        Err(ComputeError::Provider(format!("UCX/{} cancel not yet implemented", self.provider_name)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn veilsim_can_handle_osovm_images() {
        let p = VeilSimProvider::new("http://localhost:7800");
        assert!(p.can_handle(&workload("osovm:my-module")));
        assert!(p.can_handle(&workload_cpu("docker:ubuntu")));
        assert!(!p.can_handle(&workload_gpu("docker:pytorch")));
    }

    #[test]
    fn ucx_backed_can_handle_gpu_workloads() {
        let p = UcxBackedProvider::gpu_ai();
        assert!(p.can_handle(&workload_gpu("docker:pytorch")));
    }

    #[test]
    fn veilsim_submit_returns_not_implemented() {
        let p = VeilSimProvider::new("http://localhost:7800");
        let w = workload("osovm:test");
        assert!(matches!(p.submit(w), Err(ComputeError::SubmitFailed(_))));
    }

    fn workload(image: &str) -> OsoWorkload {
        OsoWorkload {
            workload_id: "test".into(),
            description: "test".into(),
            image: image.into(),
            args: vec![],
            env: vec![],
            min_vram_gb: 0,
            timeout_secs: 60,
            require_attestation: false,
        }
    }

    fn workload_cpu(image: &str) -> OsoWorkload {
        workload(image)
    }

    fn workload_gpu(image: &str) -> OsoWorkload {
        OsoWorkload { min_vram_gb: 8, ..workload(image) }
    }
}
