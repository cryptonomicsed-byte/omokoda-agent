//! AgentComputeWallet — per-agent Dopamine + Synapse ledger.
//!
//! Dopamine: hive-pool contribution token (earned by contributing GPU).
//! Synapse:  spendable compute allocation (converted from Dopamine 10:1).
//! Both decay at 1%/day (anti-hoarding). Synapse is transferable; Dopamine is not.
//!
//! See ~/sovereign-eco-blueprint/specs/AGENT_COMPUTE_WALLET_SPEC.md for constants.

use serde::{Deserialize, Serialize};

// ── Constants (from live OSOVM/src/ase_supply.jl + The-Aether/toc/token.js) ──

pub const AGENT_DOPAMINE_ENDOWMENT: u64 = 86_000_000_000; // ~human neuron count
pub const AGENT_SYNAPSE_ENDOWMENT: u64 = 86_000_000;
pub const DOPAMINE_DAILY_DECAY: f64 = 0.01; // 1%/day compound
pub const SYNAPSE_DAILY_DECAY: f64 = 0.01; // same rate
pub const SYNAPSE_CONVERSION_RATIO: f64 = 0.10; // 10 Dopamine → 1 Synapse
pub const FORK_STAKE_FRACTION: f64 = 0.10;
pub const STAKE_GATE_FRACTION: f64 = 0.10;
pub const LOW_WATER_MARK: u64 = 10_000_000; // alert threshold for Synapse

// ── Wallet ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AgentComputeWallet {
    pub agent_id: String,

    // Dopamine — hive-pool contribution token
    pub dopamine_balance: u64,
    pub dopamine_earned: u64,
    pub dopamine_burned: u64,
    pub dopamine_decayed: u64,

    // Synapse — spendable compute allocation
    pub synapse_balance: u64,
    pub synapse_earned: u64,
    pub synapse_spent: u64,
    pub synapse_staked: u64,
    pub synapse_decayed: u64,

    // Stake locks
    pub stake_locks: Vec<StakeLock>,

    // Ledger
    pub ledger: Vec<ComputeLedgerEntry>,

    // Metadata
    pub last_decay_tick: u64,
    pub wallet_version: u8,
}

impl AgentComputeWallet {
    /// Create a fresh wallet with birth endowments.
    pub fn birth_endowment(agent_id: &str) -> Self {
        let now = now_secs();
        Self {
            agent_id: agent_id.to_string(),
            dopamine_balance: AGENT_DOPAMINE_ENDOWMENT,
            dopamine_earned: AGENT_DOPAMINE_ENDOWMENT,
            synapse_balance: AGENT_SYNAPSE_ENDOWMENT,
            synapse_earned: AGENT_SYNAPSE_ENDOWMENT,
            last_decay_tick: now,
            wallet_version: 1,
            ..Default::default()
        }
    }

    /// Apply lazy 1%/day compound decay since last_decay_tick.
    /// Call on every read/write to ensure balances are always current.
    pub fn apply_decay(&mut self) {
        let now = now_secs();
        if now <= self.last_decay_tick {
            return;
        }
        let days = (now - self.last_decay_tick) as f64 / 86_400.0;
        let factor = (1.0 - DOPAMINE_DAILY_DECAY).powf(days);

        // Round instead of truncate so sub-token decay on small balances
        // doesn't cause spurious 1-token drops (e.g. 5 → 4 after 1 second).
        let new_dop = (self.dopamine_balance as f64 * factor).round() as u64;
        let dop_decayed = self.dopamine_balance.saturating_sub(new_dop);
        self.dopamine_decayed = self.dopamine_decayed.saturating_add(dop_decayed);
        self.dopamine_balance = new_dop;

        let new_syn = (self.synapse_balance as f64 * factor).round() as u64;
        let syn_decayed = self.synapse_balance.saturating_sub(new_syn);
        self.synapse_decayed = self.synapse_decayed.saturating_add(syn_decayed);
        self.synapse_balance = new_syn;

        self.last_decay_tick = now;
    }

    /// True when Synapse is below the alert threshold.
    pub fn is_low_water(&self) -> bool {
        self.synapse_balance < LOW_WATER_MARK
    }

    /// Burn 10 Dopamine → 1 Synapse (one-way, non-reversible).
    pub fn convert_dopamine_to_synapse(&mut self, synapse_amount: u64) -> Result<(), String> {
        self.apply_decay();
        let burn = synapse_amount
            .checked_mul(10)
            .ok_or("synapse_amount overflow when computing dopamine burn")?;
        if self.dopamine_balance < burn {
            return Err(format!(
                "insufficient dopamine: have {}, need {}",
                self.dopamine_balance, burn
            ));
        }
        self.dopamine_balance -= burn;
        self.dopamine_burned = self.dopamine_burned.saturating_add(burn);
        self.synapse_balance = self.synapse_balance.saturating_add(synapse_amount);
        self.synapse_earned = self.synapse_earned.saturating_add(synapse_amount);
        Ok(())
    }

    /// Spend `amount` Synapse on a job or inference call.
    pub fn spend_synapse(&mut self, amount: u64, reason: &str) -> Result<(), String> {
        self.apply_decay();
        if self.synapse_balance < amount {
            return Err(format!(
                "insufficient synapse: have {}, need {}",
                self.synapse_balance, amount
            ));
        }
        self.synapse_balance -= amount;
        self.synapse_spent = self.synapse_spent.saturating_add(amount);
        self.ledger.push(ComputeLedgerEntry {
            entry_id: format!("spend:{}:{}", reason, now_secs()),
            timestamp: now_secs(),
            token: "synapse".into(),
            delta: -(amount as i64),
            reason: reason.to_string(),
            receipt_id: None,
        });
        Ok(())
    }

    /// Credit Dopamine (from GPU contribution, verified by OSOVM).
    pub fn credit_dopamine(&mut self, amount: u64, receipt_id: Option<String>) {
        self.apply_decay();
        self.dopamine_balance = self.dopamine_balance.saturating_add(amount);
        self.dopamine_earned = self.dopamine_earned.saturating_add(amount);
        self.ledger.push(ComputeLedgerEntry {
            entry_id: format!("earn_dop:{}", now_secs()),
            timestamp: now_secs(),
            token: "dopamine".into(),
            delta: amount as i64,
            reason: "gpu_contribution".into(),
            receipt_id,
        });
    }

    /// Lock `amount` Synapse into a stake for a tier gate or fork.
    pub fn lock_stake(
        &mut self,
        lock_id: impl Into<String>,
        reason: impl Into<String>,
        amount: u64,
        unlock_at: u64,
    ) -> Result<(), String> {
        self.apply_decay();
        if self.synapse_balance < amount {
            return Err(format!(
                "insufficient synapse to stake: have {}, need {}",
                self.synapse_balance, amount
            ));
        }
        self.synapse_balance -= amount;
        self.synapse_staked = self.synapse_staked.saturating_add(amount);
        self.stake_locks.push(StakeLock {
            lock_id: lock_id.into(),
            reason: reason.into(),
            amount,
            locked_at: now_secs(),
            unlock_at,
            unlocked: false,
        });
        Ok(())
    }

    /// Release a stake lock (unlock_at reached or tier achieved).
    pub fn unlock_stake(&mut self, lock_id: &str) -> Result<u64, String> {
        let pos = self
            .stake_locks
            .iter()
            .position(|l| l.lock_id == lock_id && !l.unlocked)
            .ok_or_else(|| format!("stake lock {lock_id} not found or already unlocked"))?;
        let amount = self.stake_locks[pos].amount;
        self.stake_locks[pos].unlocked = true;
        self.synapse_balance = self.synapse_balance.saturating_add(amount);
        self.synapse_staked = self.synapse_staked.saturating_sub(amount);
        Ok(amount)
    }
}

// ── Supporting types ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StakeLock {
    pub lock_id: String,
    /// "tier_gate" | "fork" | "skill_acquire"
    pub reason: String,
    pub amount: u64,
    pub locked_at: u64,
    /// 0 = indefinite (tier gate until tier achieved)
    pub unlock_at: u64,
    pub unlocked: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComputeLedgerEntry {
    pub entry_id: String,
    pub timestamp: u64,
    /// "dopamine" | "synapse"
    pub token: String,
    /// Positive = credit, negative = debit.
    pub delta: i64,
    pub reason: String,
    pub receipt_id: Option<String>,
}

// ── helpers ──────────────────────────────────────────────────────────────────

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
    fn birth_endowment_values() {
        let w = AgentComputeWallet::birth_endowment("agent:test");
        assert_eq!(w.dopamine_balance, AGENT_DOPAMINE_ENDOWMENT);
        assert_eq!(w.synapse_balance, AGENT_SYNAPSE_ENDOWMENT);
        assert_eq!(w.wallet_version, 1);
    }

    #[test]
    fn convert_dopamine_to_synapse_burns_10x() {
        let mut w = AgentComputeWallet::birth_endowment("agent:test");
        let initial_syn = w.synapse_balance;
        w.convert_dopamine_to_synapse(100).unwrap();
        assert_eq!(w.dopamine_burned, 1000);
        assert_eq!(w.synapse_balance, initial_syn + 100);
    }

    #[test]
    fn spend_synapse_insufficient_fails() {
        let mut w = AgentComputeWallet::birth_endowment("agent:test");
        w.synapse_balance = 5;
        let res = w.spend_synapse(10, "test");
        assert!(res.is_err());
    }

    #[test]
    fn not_low_water_at_birth() {
        let w = AgentComputeWallet::birth_endowment("agent:test");
        assert!(!w.is_low_water());
    }
}
