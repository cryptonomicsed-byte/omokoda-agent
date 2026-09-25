//! OSO-Compute Interface — Phase 16.3
//! Wraps UCX adapters (GPU.ai/Akash/Local) behind a single trait.
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Workload {
    pub workload_id: String,
    pub image: String,
    pub command: Vec<String>,
    pub gpu_count: u32,
    pub memory_gb: u32,
    pub max_secs: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComputeJob {
    pub job_id: String,
    pub workload_id: String,
    pub provider: String,
    pub submitted_at: u64,
    pub status: JobStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum JobStatus {
    Queued,
    Running,
    Completed,
    Failed(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComputeResult {
    pub job_id: String,
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
    pub duration_secs: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Attestation {
    pub job_id: String,
    pub result_hash: [u8; 32],
    pub provider: String,
    pub attested_at: u64,
    pub signature: String,
}

#[async_trait]
pub trait ComputeProvider: Send + Sync {
    async fn submit(&self, workload: Workload) -> Result<ComputeJob, String>;
    async fn status(&self, job: &ComputeJob) -> Result<JobStatus, String>;
    async fn result(&self, job: &ComputeJob) -> Result<ComputeResult, String>;
    async fn attest(&self, result: &ComputeResult) -> Result<Attestation, String>;
    fn provider_name(&self) -> &str;
}

/// Stub providers — wired to real UCX adapters in Phase 16.3 follow-up.
pub struct GpuAiProvider { pub api_key: String, pub broker_url: String }
pub struct AkashProvider { pub broker_url: String }
pub struct LocalGpuProvider;

#[async_trait]
impl ComputeProvider for GpuAiProvider {
    async fn submit(&self, w: Workload) -> Result<ComputeJob, String> {
        Ok(ComputeJob {
            job_id: format!("gpuai-{}", w.workload_id),
            workload_id: w.workload_id,
            provider: "gpu_ai".into(),
            submitted_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs()).unwrap_or(0),
            status: JobStatus::Queued,
        })
    }
    async fn status(&self, job: &ComputeJob) -> Result<JobStatus, String> {
        Ok(job.status.clone())
    }
    async fn result(&self, job: &ComputeJob) -> Result<ComputeResult, String> {
        Err(format!("GpuAiProvider: job {} result not yet wired", job.job_id))
    }
    async fn attest(&self, result: &ComputeResult) -> Result<Attestation, String> {
        let hash = blake3::hash(result.stdout.as_bytes());
        Ok(Attestation {
            job_id: result.job_id.clone(),
            result_hash: *hash.as_bytes(),
            provider: "gpu_ai".into(),
            attested_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs()).unwrap_or(0),
            signature: String::new(), // Phase 16.3: sign with provider key
        })
    }
    fn provider_name(&self) -> &str { "gpu_ai" }
}

#[async_trait]
impl ComputeProvider for AkashProvider {
    async fn submit(&self, w: Workload) -> Result<ComputeJob, String> {
        Err(format!("AkashProvider: not yet wired for workload {}", w.workload_id))
    }
    async fn status(&self, job: &ComputeJob) -> Result<JobStatus, String> { Ok(job.status.clone()) }
    async fn result(&self, job: &ComputeJob) -> Result<ComputeResult, String> {
        Err(format!("AkashProvider: job {} not wired", job.job_id))
    }
    async fn attest(&self, result: &ComputeResult) -> Result<Attestation, String> {
        Ok(Attestation {
            job_id: result.job_id.clone(),
            result_hash: *blake3::hash(result.stdout.as_bytes()).as_bytes(),
            provider: "akash".into(),
            attested_at: 0,
            signature: String::new(),
        })
    }
    fn provider_name(&self) -> &str { "akash" }
}
