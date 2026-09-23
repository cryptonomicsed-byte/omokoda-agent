// DopaminePool is a LOCAL simulation model only.
// On-chain settlement roadmap:
//   Phase 1 (current): local accounting, no blockchain
//   Phase 2: Vantage UCX API reports balances
//   Phase 3: OSOVM vm_core.jl DOPAMINE_MINT opcode + Sui settlement
// See: ~/sovereign-eco-blueprint/plans/protocol-layer-synthesis.md

use serde::{Deserialize, Serialize};

pub const SYNAPSE_MAX_PER_AGENT: f64 = 86_000_000.0;
pub const DOPAMINE_TOTAL_POOL: f64 = 86_000_000_000.0;
pub const SYNAPSE_DAILY_DECAY_RATE: f64 = 0.08;
pub const SYNAPSE_INITIAL: f64 = 10_000.0;
pub const EXTENDED_INACTIVITY_DAYS: u64 = 7;
pub const DECAY_NORMAL_PER_DAY: f64 = 0.008;
pub const DECAY_EXTENDED_PER_DAY: f64 = 0.015;

/// Tracks global compute capacity across all agents.
/// In production this would be a distributed counter. Here it's a local stub.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DopaminePool {
    pub allocated: f64,
    pub capacity: f64,
}

impl Default for DopaminePool {
    fn default() -> Self {
        Self {
            allocated: 0.0,
            capacity: DOPAMINE_TOTAL_POOL,
        }
    }
}

impl DopaminePool {
    pub fn available(&self) -> f64 {
        self.capacity - self.allocated
    }

    /// Allocates synapse budget for a new agent based on current pool pressure.
    /// More Dopamine allocated globally → smaller initial Synapse issuance.
    pub fn compute_initial_synapse(&self) -> f64 {
        let pressure = self.allocated / self.capacity;
        // Linear scale: 10_000 at empty pool → 1_000 at full pool
        SYNAPSE_INITIAL * (1.0 - pressure * 0.9).max(0.1)
    }

    pub fn allocate(&mut self, amount: f64) {
        self.allocated = (self.allocated + amount).min(self.capacity);
    }

    pub fn release(&mut self, amount: f64) {
        self.allocated = (self.allocated - amount).max(0.0);
    }
}

/// Computes how much synapse decays over `elapsed_secs` seconds.
/// Days 1–7: 8%/day metabolic decay.
/// Day 8+: accelerated at 15%/day.
pub fn compute_synapse_decay(synapse: f64, elapsed_secs: u64) -> f64 {
    if elapsed_secs == 0 || synapse <= 0.0 {
        return 0.0;
    }
    let elapsed_days = elapsed_secs as f64 / 86_400.0;

    let decay = if elapsed_days <= EXTENDED_INACTIVITY_DAYS as f64 {
        synapse * SYNAPSE_DAILY_DECAY_RATE * elapsed_days
    } else {
        let normal_portion = synapse * SYNAPSE_DAILY_DECAY_RATE * EXTENDED_INACTIVITY_DAYS as f64;
        let extended_days = elapsed_days - EXTENDED_INACTIVITY_DAYS as f64;
        let extended_portion = synapse * 0.15 * extended_days;
        normal_portion + extended_portion
    };

    decay.min(synapse)
}

/// Computes reputation decay over `elapsed_secs` seconds of inactivity.
pub fn compute_reputation_decay(reputation: f64, elapsed_secs: u64) -> f64 {
    if elapsed_secs == 0 || reputation <= 0.0 {
        return 0.0;
    }
    let elapsed_days = elapsed_secs as f64 / 86_400.0;
    let normal_days = elapsed_days.min(EXTENDED_INACTIVITY_DAYS as f64);
    let extended_days = (elapsed_days - EXTENDED_INACTIVITY_DAYS as f64).max(0.0);

    let decay = DECAY_NORMAL_PER_DAY * normal_days + DECAY_EXTENDED_PER_DAY * extended_days;
    decay.min(reputation)
}

// --- SynapseAccount: atomic balance management per agent ---

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SynapseError {
    InsufficientBalance,
}

impl std::fmt::Display for SynapseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SynapseError::InsufficientBalance => write!(f, "insufficient_synapse"),
        }
    }
}

impl std::error::Error for SynapseError {}

/// Per-agent Synapse balance tracker.
/// `tier` is stored as `u8` to avoid a circular dependency between
/// the `economics` and `justice` modules.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SynapseAccount {
    pub balance: u64,
    pub total_burned: u64,
    pub tier: u8,
    pub last_decay_epoch: u64,
}

impl SynapseAccount {
    pub fn new(initial_balance: u64, tier: u8) -> Self {
        Self {
            balance: initial_balance,
            total_burned: 0,
            tier,
            last_decay_epoch: 0,
        }
    }

    /// Tier-based maximum Synapse balance.
    pub fn tier_cap(&self) -> u64 {
        match self.tier {
            0 => 1_000_000,
            1 => 10_000_000,
            2 => 30_000_000,
            3 => 60_000_000,
            _ => 86_000_000,
        }
    }

    /// Atomically burn `pre_adjusted_cost` Synapse.
    /// The caller is responsible for applying any tier multiplier BEFORE calling this.
    /// `burn()` never re-applies the tier multiplier — it is non-refundable.
    /// Returns `Err(InsufficientBalance)` without modifying state if balance < cost.
    pub fn burn(&mut self, pre_adjusted_cost: u64) -> Result<(), SynapseError> {
        if self.balance < pre_adjusted_cost {
            return Err(SynapseError::InsufficientBalance);
        }
        self.balance -= pre_adjusted_cost;
        self.total_burned += pre_adjusted_cost;
        Ok(())
    }

    /// Earn 1000 Synapse from a Garden interaction, capped at the tier ceiling.
    pub fn earn_from_garden(&mut self) {
        self.balance = (self.balance + 1_000).min(self.tier_cap());
    }

    /// Earn Synapse from a tip, capped at 10,000 per event and at the tier ceiling.
    pub fn earn_from_tip(&mut self, amount: u64) {
        let clamped = amount.min(10_000);
        self.balance = (self.balance + clamped).min(self.tier_cap());
    }
}

// ── ToC mint authorization ────────────────────────────────────────────────────

/// Synapse tokens earned per GPU-hour of fully-verified compute (10:1 burn ratio).
const SYNAPSE_PER_GPU_HOUR: f64 = 1_000.0;

/// Authorize a Synapse mint from a VerifiedGPUWork record.
///
/// Returns `Ok(synapse_amount)` if the work is fully verified and the pool
/// has capacity; returns `Err(reason)` otherwise.  Callers must call
/// `account.earn_from_tip(amount)` and `pool.allocate(amount)` after success.
pub fn authorize_toc_mint(
    work: &crate::kernel::compute::verified_work::VerifiedGPUWork,
    account: &SynapseAccount,
    pool: &DopaminePool,
) -> Result<u64, String> {
    if !work.is_fully_verified() {
        return Err("work is not fully verified (osovm_proof / witnesses / zangbeto required)".into());
    }
    let gpu_hours = work.gpu_seconds / 3600.0;
    let earned = (gpu_hours * SYNAPSE_PER_GPU_HOUR) as u64;
    if earned == 0 {
        return Err("gpu_seconds too small to earn any Synapse".into());
    }
    // Check tier cap.
    let headroom = account.tier_cap().saturating_sub(account.balance);
    if headroom == 0 {
        return Err("account already at tier cap".into());
    }
    // Check pool capacity (Synapse comes from Dopamine allocation pressure).
    if pool.available() < earned as f64 {
        return Err(format!(
            "pool has only {:.0} Dopamine capacity; need {earned} more Synapse allocation",
            pool.available()
        ));
    }
    Ok(earned.min(headroom))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initial_synapse_scales_with_pool_pressure() {
        let mut pool = DopaminePool::default();
        let full = pool.compute_initial_synapse();
        pool.allocate(DOPAMINE_TOTAL_POOL * 0.5);
        let half = pool.compute_initial_synapse();
        assert!(
            full > half,
            "Full pool should give more synapse than half-full"
        );
        assert!(full <= SYNAPSE_INITIAL);
    }

    #[test]
    fn decay_is_zero_for_zero_elapsed() {
        assert_eq!(compute_synapse_decay(5000.0, 0), 0.0);
    }

    #[test]
    fn decay_one_day() {
        let decay = compute_synapse_decay(10_000.0, 86_400);
        assert!(decay > 0.0 && decay < 10_000.0, "Decay should be partial");
    }

    #[test]
    fn decay_never_exceeds_balance() {
        let decay = compute_synapse_decay(100.0, 86_400 * 365);
        assert!(decay <= 100.0);
    }

    #[test]
    fn reputation_decay_one_day() {
        let decay = compute_reputation_decay(50.0, 86_400);
        assert!(decay > 0.0 && decay < 50.0);
    }
}
