use super::verified_work::VerifiedGPUWork;

/// Derived quality/quantity score for a single VerifiedGPUWork event.
/// Input to OSOVM for Dopamine allocation.
#[derive(Debug, Clone)]
pub struct ComputeScore {
    pub work_id: String,
    pub raw_gpu_secs: f64,
    pub utilization: f32,
    pub witness_count: usize,
    pub score: f64, // [0.0, 1.0] normalized quality score
}

impl ComputeScore {
    /// Heuristic score: GPU-seconds × utilization × witness confidence
    pub fn compute(work: &VerifiedGPUWork) -> Self {
        let witness_factor = match work.witness_receipts.len() {
            0 => 0.0,
            1 => 0.5,
            2 => 0.75,
            _ => 1.0,
        };
        let util_factor = work.utilization.clamp(0.0, 1.0) as f64;
        let score = (work.gpu_seconds / 3600.0).min(1.0) * util_factor * witness_factor;

        Self {
            work_id: work.work_id.clone(),
            raw_gpu_secs: work.gpu_seconds,
            utilization: work.utilization,
            witness_count: work.witness_receipts.len(),
            score: score.clamp(0.0, 1.0),
        }
    }
}

/// OSOVM authorization to credit Dopamine to a contributor's balance.
/// Emitted only after full verification chain is satisfied.
#[derive(Debug, Clone)]
pub struct DopamineAllocation {
    pub alloc_id: String,
    pub work_id: String,
    pub agent_id: String,
    pub device_id: String,
    pub micro_dopamine: u64, // 1 Dopamine = 1_000_000 micro-Dopamine
    pub score: f64,
    pub osovm_epoch: u64, // OSOVM block/epoch that authorized this
    pub authorized_at: u64,
    pub zangbeto_id: String, // Zàngbétò receipt anchoring this allocation
}

impl DopamineAllocation {
    /// Dopamine per GPU-second base rate: 1 Dopamine per 10 GPU-minutes at 100% utilization.
    const MICRO_DOPAMINE_PER_GPU_SEC: f64 = 1_000_000.0 / 600.0;

    pub fn from_score(
        score: &ComputeScore,
        agent_id: impl Into<String>,
        device_id: impl Into<String>,
        osovm_epoch: u64,
        zangbeto_id: impl Into<String>,
    ) -> Self {
        let micro = (score.raw_gpu_secs * score.score * Self::MICRO_DOPAMINE_PER_GPU_SEC) as u64;
        Self {
            alloc_id: uuid_v4(),
            work_id: score.work_id.clone(),
            agent_id: agent_id.into(),
            device_id: device_id.into(),
            micro_dopamine: micro,
            score: score.score,
            osovm_epoch,
            authorized_at: now_secs(),
            zangbeto_id: zangbeto_id.into(),
        }
    }

    pub fn dopamine_units(&self) -> f64 {
        self.micro_dopamine as f64 / 1_000_000.0
    }
}

fn uuid_v4() -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut h = DefaultHasher::new();
    std::time::SystemTime::now().hash(&mut h);
    format!(
        "{:016x}{:016x}",
        h.finish(),
        h.finish().wrapping_mul(0x0bad_cafe)
    )
}

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
