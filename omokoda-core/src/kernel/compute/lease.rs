use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LeaseState {
    Active,
    Completed,
    Cancelled,
    Expired,
}

/// A time-bounded exclusive reservation of a GPU device for a specific agent workload.
#[derive(Debug, Clone)]
pub struct GpuLease {
    pub lease_id: String,
    pub device_id: String,
    pub agent_id: String,
    pub workload_id: String,
    pub started_at: u64,
    pub expires_at: u64, // hard deadline
    pub ended_at: Option<u64>,
    pub state: LeaseState,
    /// GPU-seconds actually consumed (filled on completion)
    pub gpu_seconds: Option<f64>,
}

impl GpuLease {
    pub fn new(
        lease_id: impl Into<String>,
        device_id: impl Into<String>,
        agent_id: impl Into<String>,
        workload_id: impl Into<String>,
        duration_secs: u64,
    ) -> Self {
        let now = now_secs();
        Self {
            lease_id: lease_id.into(),
            device_id: device_id.into(),
            agent_id: agent_id.into(),
            workload_id: workload_id.into(),
            started_at: now,
            expires_at: now + duration_secs,
            ended_at: None,
            state: LeaseState::Active,
            gpu_seconds: None,
        }
    }

    pub fn is_expired(&self) -> bool {
        now_secs() > self.expires_at && self.state == LeaseState::Active
    }

    pub fn complete(&mut self, gpu_seconds: f64) {
        self.state = LeaseState::Completed;
        self.ended_at = Some(now_secs());
        self.gpu_seconds = Some(gpu_seconds);
    }

    pub fn cancel(&mut self) {
        self.state = LeaseState::Cancelled;
        self.ended_at = Some(now_secs());
    }

    /// Elapsed GPU-seconds from start (or end) of lease
    pub fn elapsed_gpu_seconds(&self) -> f64 {
        let end = self.ended_at.unwrap_or_else(now_secs);
        (end.saturating_sub(self.started_at)) as f64
    }
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
