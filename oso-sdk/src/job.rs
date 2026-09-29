//! Job client — create, find, assign, wait_for_proof, settle.

use crate::error::{SdkError, SdkResult};
use serde::{Deserialize, Serialize};
use ucx_protocol::job::{ComputeConstraints, Job, JobStatus, WorkloadRequirements, WorkloadType};
use ucx_protocol::receipt::ComputeReceipt;
use uuid::Uuid;

/// In-memory job store (production: HTTP to UCX broker).
#[derive(Default)]
pub struct JobStore {
    jobs: std::collections::HashMap<Uuid, PendingJob>,
}

/// A submitted job, tracked by the SDK.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingJob {
    pub job: Job,
    pub status: JobStatus,
    pub receipt: Option<ComputeReceipt>,
}

/// High-level job client.
pub struct JobClient {
    pub agent_id: String,
    pub store: std::sync::Arc<std::sync::Mutex<JobStore>>,
}

impl JobClient {
    pub fn new(agent_id: impl Into<String>) -> Self {
        Self {
            agent_id: agent_id.into(),
            store: Default::default(),
        }
    }

    /// Start building a new job.
    pub fn create(&self) -> JobBuilder {
        JobBuilder::new(self.agent_id.clone(), self.store.clone())
    }

    /// Find a job by ID.
    pub fn find(&self, id: &Uuid) -> SdkResult<PendingJob> {
        let store = self.store.lock().unwrap();
        store
            .jobs
            .get(id)
            .cloned()
            .ok_or_else(|| SdkError::JobNotFound(id.to_string()))
    }

    /// Assign a job to a specific provider (updates status to Allocated).
    pub fn assign(&self, id: &Uuid, _provider_id: impl Into<String>) -> SdkResult<()> {
        let mut store = self.store.lock().unwrap();
        let job = store
            .jobs
            .get_mut(id)
            .ok_or_else(|| SdkError::JobNotFound(id.to_string()))?;
        if job.status != JobStatus::Pending {
            return Err(SdkError::InvalidState(format!(
                "job {} is {:?}, not Pending",
                id, job.status
            )));
        }
        job.status = JobStatus::Allocated;
        Ok(())
    }

    /// Simulate receiving a compute receipt (production: webhook from provider).
    pub fn complete(&self, id: &Uuid, receipt: ComputeReceipt) -> SdkResult<()> {
        let mut store = self.store.lock().unwrap();
        let job = store
            .jobs
            .get_mut(id)
            .ok_or_else(|| SdkError::JobNotFound(id.to_string()))?;
        job.status = JobStatus::Completed;
        job.receipt = Some(receipt);
        Ok(())
    }

    /// Wait for a proof receipt (blocking in production; polling stub here).
    /// Returns error if job is Failed or Cancelled.
    pub fn wait_for_proof(&self, id: &Uuid) -> SdkResult<ComputeReceipt> {
        let store = self.store.lock().unwrap();
        let job = store
            .jobs
            .get(id)
            .ok_or_else(|| SdkError::JobNotFound(id.to_string()))?;
        match &job.status {
            JobStatus::Completed => job.receipt.clone().ok_or(SdkError::ProofTimeout),
            JobStatus::Failed => Err(SdkError::JobFailed(id.to_string())),
            JobStatus::Cancelled => Err(SdkError::InvalidState("job was cancelled".into())),
            _ => Err(SdkError::ProofTimeout),
        }
    }

    /// Mark a completed job as settled (triggers Àṣẹ payment flow).
    /// Returns the settlement receipt hash.
    pub fn settle(&self, id: &Uuid) -> SdkResult<String> {
        let receipt = self.wait_for_proof(id)?;
        Ok(receipt.hash())
    }

    /// List all jobs by status.
    pub fn list_by_status(&self, status: JobStatus) -> Vec<PendingJob> {
        let store = self.store.lock().unwrap();
        store
            .jobs
            .values()
            .filter(|j| j.status == status)
            .cloned()
            .collect()
    }
}

/// Builder for `Job` — implements the fluent API.
pub struct JobBuilder {
    agent_id: String,
    workload: WorkloadType,
    requirements: WorkloadRequirements,
    constraints: ComputeConstraints,
    runtime_spec: serde_json::Value,
    store: std::sync::Arc<std::sync::Mutex<JobStore>>,
}

impl JobBuilder {
    pub fn new(agent_id: String, store: std::sync::Arc<std::sync::Mutex<JobStore>>) -> Self {
        Self {
            agent_id,
            workload: WorkloadType::Generic,
            requirements: WorkloadRequirements {
                vram_gb: None,
                ram_gb: None,
                cpu_cores: None,
                gpu_count: None,
                fp16: false,
                bf16: false,
                cuda: false,
                min_tier: None,
            },
            constraints: ComputeConstraints {
                max_price_cents: None,
                max_queue_secs: None,
                privacy: ucx_protocol::capability::TrustLevel::Standard,
                regions: vec![],
                allow_external: true,
            },
            runtime_spec: serde_json::Value::Null,
            store,
        }
    }

    pub fn workload(mut self, w: WorkloadType) -> Self {
        self.workload = w;
        self
    }
    pub fn vram_gb(mut self, v: f64) -> Self {
        self.requirements.vram_gb = Some(v);
        self
    }
    pub fn gpu_count(mut self, n: u8) -> Self {
        self.requirements.gpu_count = Some(n);
        self
    }
    pub fn cuda(mut self) -> Self {
        self.requirements.cuda = true;
        self
    }
    pub fn runtime_spec(mut self, spec: serde_json::Value) -> Self {
        self.runtime_spec = spec;
        self
    }
    pub fn max_price_cents(mut self, c: u64) -> Self {
        self.constraints.max_price_cents = Some(c);
        self
    }
    pub fn region(mut self, r: impl Into<String>) -> Self {
        self.constraints.regions.push(r.into());
        self
    }

    /// Submit the job — adds it to the store and returns the PendingJob.
    pub fn submit(self) -> SdkResult<PendingJob> {
        let job = Job::new(
            self.agent_id,
            self.workload,
            self.requirements,
            self.constraints,
            self.runtime_spec,
        );
        let pending = PendingJob {
            job: job.clone(),
            status: JobStatus::Pending,
            receipt: None,
        };
        self.store
            .lock()
            .unwrap()
            .jobs
            .insert(job.id, pending.clone());
        Ok(pending)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ucx_protocol::receipt::ComputeReceipt;

    #[test]
    fn create_and_find_job() {
        let client = JobClient::new("did:v:agent:test");
        let pending = client
            .create()
            .workload(WorkloadType::Inference)
            .runtime_spec(serde_json::json!({"model": "llama3"}))
            .submit()
            .unwrap();
        let found = client.find(&pending.job.id).unwrap();
        assert_eq!(found.status, JobStatus::Pending);
    }

    #[test]
    fn assign_changes_status() {
        let client = JobClient::new("did:v:agent:test");
        let job = client.create().submit().unwrap();
        client.assign(&job.job.id, "provider-1").unwrap();
        let found = client.find(&job.job.id).unwrap();
        assert_eq!(found.status, JobStatus::Allocated);
    }

    fn make_receipt(job_id: Uuid, provider_id: &str) -> ComputeReceipt {
        use chrono::Utc;
        use ucx_protocol::receipt::{
            BillingCurrency, BillingRecord, ResourceUsage, VerificationProof,
        };
        ComputeReceipt {
            job_id,
            provider_id: provider_id.into(),
            completed_at: Utc::now(),
            resources: ResourceUsage {
                gpu_seconds: 10.0,
                cpu_seconds: 5.0,
                ram_gb_seconds: 2.0,
                storage_gb: 0.1,
                egress_gb: 0.0,
            },
            billing: BillingRecord {
                amount_cents: 5,
                currency: BillingCurrency::Usd,
                line_items: vec![],
            },
            verification: VerificationProof {
                artifact_hash: Some("abc".into()),
                runtime_attestation: None,
                execution_hash: None,
            },
            zangbeto_anchor: None,
        }
    }

    #[test]
    fn wait_for_proof_returns_receipt_when_complete() {
        let client = JobClient::new("did:v:agent:test");
        let job = client.create().submit().unwrap();
        let receipt = make_receipt(job.job.id, "p-1");
        client.complete(&job.job.id, receipt).unwrap();
        let r = client.wait_for_proof(&job.job.id).unwrap();
        assert_eq!(r.provider_id, "p-1");
    }

    #[test]
    fn settle_returns_receipt_hash() {
        let client = JobClient::new("did:v:agent:test");
        let job = client.create().submit().unwrap();
        let receipt = make_receipt(job.job.id, "p-2");
        client.complete(&job.job.id, receipt).unwrap();
        let hash = client.settle(&job.job.id).unwrap();
        assert!(!hash.is_empty());
    }
}
