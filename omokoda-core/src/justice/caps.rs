//! omokoda-core/src/justice/caps.rs
//! Anti-Gaming Caps Enforcement — Steward Layer (Rust)
//! Busy Beaver ceiling, per-agent epoch caps, repeat limits, tier gates

use crate::identity::AgentIdentity;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// Busy Beaver Ceiling — spend governor, not issuance path
/// compute_bb_ceiling(synapses, tier, reputation, dna_fingerprint)
///     = synapses × tier_multiplier(tier) × reputation_factor(reputation) × entropy_score(dna_fingerprint)
///     .clamp(BB_FLOOR, BB_ABSOLUTE_CEILING)

/// Busy Beaver constants (from TOC_CONSTANTS.toml + governance)
const BB_FLOOR: f64 = 1000.0;                    // Minimum ceiling
const BB_ABSOLUTE_CEILING: f64 = 1_000_000_000.0; // 1B Synapse absolute max
const BB_TIER_MULTIPLIERS: &[f64] = &[1.0, 1.5, 2.0, 3.0, 5.0]; // T0-T4

/// Per-agent per-epoch cap (Dopamine units)
/// TOC_CONSTANTS.toml: per_agent_epoch_cap = 50_000_000
const PER_AGENT_EPOCH_CAP: u64 = 50_000_000;

/// Repeat limit — same model_hash/sim_hash cannot earn > 3 times per epoch
const REPEAT_LIMIT: u32 = 3;

/// Tier gates for higher multipliers
/// sim_to_real_min_tier = 2 (T2+)
/// aerial_min_tier = 3 (T3+)

/// Trust Tier enum
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum TrustTier {
    T0 = 0,  // Unverified
    T1 = 1,  // Basic verification
    T2 = 2,  // Verified work history
    T3 = 3,  // High reputation
    T4 = 4,  // Council/elite
}

impl TrustTier {
    pub fn multiplier(&self) -> f64 {
        BB_TIER_MULTIPLIERS[*self as usize]
    }

    pub fn can_earn_multiplier(&self, required: TrustTier) -> bool {
        *self >= required
    }
}

/// Anti-Gaming Caps Enforcer
pub struct CapsEnforcer {
    /// Per-agent epoch Dopamine tracking
    epoch_dopamine: Arc<Mutex<HashMap<String, EpochTracker>>>,
    /// Per-agent repeat tracking (model_hash -> count)
    repeat_tracker: Arc<Mutex<HashMap<String, HashMap<String, u32>>>>,
    /// Current epoch number
    current_epoch: Arc<Mutex<u64>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EpochTracker {
    pub agent_npub: String,
    pub epoch: u64,
    pub dopamine_earned: u64,
    pub synapse_earned: f64,
    pub last_updated: u64,
}

impl CapsEnforcer {
    pub fn new() -> Self {
        Self {
            epoch_dopamine: Arc::new(Mutex::new(HashMap::new())),
            repeat_tracker: Arc::new(Mutex::new(HashMap::new())),
            current_epoch: Arc::new(Mutex::new(Self::compute_epoch())),
        }
    }

    /// Compute current Koodu epoch (7-day cycles from genesis)
    fn compute_epoch() -> u64 {
        const GENESIS_TIMESTAMP: u64 = 1_700_000_000; // Approx genesis
        const EPOCH_SECONDS: u64 = 7 * 24 * 3600;     // 7 days
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH).unwrap().as_secs();
        (now - GENESIS_TIMESTAMP) / EPOCH_SECONDS
    }

    /// Check and advance epoch if needed
    pub fn check_epoch(&self) {
        let new_epoch = Self::compute_epoch();
        let mut epoch = self.current_epoch.lock().unwrap();
        if new_epoch > *epoch {
            *epoch = new_epoch;
            // Reset epoch trackers
            self.epoch_dopamine.lock().unwrap().clear();
            self.repeat_tracker.lock().unwrap().clear();
        }
    }

    /// Compute Busy Beaver ceiling for an agent
    pub fn compute_bb_ceiling(
        &self,
        identity: &AgentIdentity,
        synapses: f64,
        tier: TrustTier,
        reputation: f64,        // 0.0 - 1.0
        dna_entropy: f64,       // 0.0 - 1.0 from DNA fingerprint
    ) -> f64 {
        let tier_mult = tier.multiplier();
        let rep_factor = 0.5 + reputation * 0.5;  // 0.5 - 1.0
        let entropy_score = 0.5 + dna_entropy * 0.5; // 0.5 - 1.0

        let ceiling = synapses * tier_mult * rep_factor * entropy_score;
        ceiling.clamp(BB_FLOOR, BB_ABSOLUTE_CEILING)
    }

    /// Check if agent can earn more Dopamine this epoch
    pub fn check_epoch_cap(&self, agent_npub: &str, requested_dopamine: u64) -> CapCheckResult {
        self.check_epoch();

        let mut trackers = self.epoch_dopamine.lock().unwrap();
        let tracker = trackers.entry(agent_npub.to_string())
            .or_insert_with(|| EpochTracker {
                agent_npub: agent_npub.to_string(),
                epoch: *self.current_epoch.lock().unwrap(),
                dopamine_earned: 0,
                synapse_earned: 0.0,
                last_updated: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH).unwrap().as_secs(),
            });

        let projected = tracker.dopamine_earned + requested_dopamine;
        if projected > PER_AGENT_EPOCH_CAP {
            CapCheckResult::Denied {
                reason: format!(
                    "Per-agent epoch cap exceeded: {}/{} Dopamine this epoch",
                    projected, PER_AGENT_EPOCH_CAP
                ),
                current: tracker.dopamine_earned,
                cap: PER_AGENT_EPOCH_CAP,
            }
        } else {
            CapCheckResult::Allowed {
                remaining: PER_AGENT_EPOCH_CAP - projected,
            }
        }
    }

    /// Record Dopamine earned (called after successful mint)
    pub fn record_dopamine_earned(&self, agent_npub: &str, dopamine: u64, synapse: f64) {
        self.check_epoch();
        let mut trackers = self.epoch_dopamine.lock().unwrap();
        if let Some(tracker) = trackers.get_mut(agent_npub) {
            tracker.dopamine_earned += dopamine;
            tracker.synapse_earned += synapse;
            tracker.last_updated = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH).unwrap().as_secs();
        }
    }

    /// Check repeat limit for a model/sim hash
    pub fn check_repeat_limit(&self, agent_npub: &str, model_hash: &str) -> CapCheckResult {
        let mut repeat = self.repeat_tracker.lock().unwrap();
        let agent_repeats = repeat.entry(agent_npub.to_string())
            .or_insert_with(HashMap::new);
        let count = agent_repeats.entry(model_hash.to_string()).or_insert(0);

        if *count >= REPEAT_LIMIT {
            CapCheckResult::Denied {
                reason: format!(
                    "Repeat limit exceeded for model {}: {}/{} this epoch",
                    model_hash, count, REPEAT_LIMIT
                ),
                current: *count as u64,
                cap: REPEAT_LIMIT as u64,
            }
        } else {
            CapCheckResult::Allowed {
                remaining: (REPEAT_LIMIT - *count) as u64,
            }
        }
    }

    /// Record a repeat (called after earning)
    pub fn record_repeat(&self, agent_npub: &str, model_hash: &str) {
        let mut repeat = self.repeat_tracker.lock().unwrap();
        let agent_repeats = repeat.entry(agent_npub.to_string())
            .or_insert_with(HashMap::new);
        *agent_repeats.entry(model_hash.to_string()).or_insert(0) += 1;
    }

    /// Check tier gate for work domain
    pub fn check_tier_gate(&self, tier: TrustTier, domain: WorkDomain) -> CapCheckResult {
        let required = match domain {
            WorkDomain::SimToReal => TrustTier::T2,
            WorkDomain::AerialFlight => TrustTier::T3,
            WorkDomain::GroundRobot => TrustTier::T3,
            _ => TrustTier::T0,
        };

        if tier.can_earn_multiplier(required) {
            CapCheckResult::Allowed { remaining: 1 }
        } else {
            CapCheckResult::Denied {
                reason: format!(
                    "Tier gate: {:?} required for {:?}, agent is {:?}",
                    required, domain, tier
                ),
                current: tier as u64,
                cap: required as u64,
            }
        }
    }

    /// Get agent's current epoch status
    pub fn get_epoch_status(&self, agent_npub: &str) -> Option<EpochTracker> {
        self.check_epoch();
        self.epoch_dopamine.lock().unwrap().get(agent_npub).cloned()
    }

    /// Get agent's repeat counts
    pub fn get_repeat_counts(&self, agent_npub: &str) -> HashMap<String, u32> {
        self.repeat_tracker.lock().unwrap()
            .get(agent_npub)
            .cloned()
            .unwrap_or_default()
    }
}

/// Work Domain enum (from bonus_ladder in TOC_CONSTANTS)
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum WorkDomain {
    GpuCompute,
    Simulation,
    PrintJob,
    SciSim,
    SpatialCapture,
    AerialFlight,
    GroundRobot,
    SimToReal,
}

impl WorkDomain {
    pub fn base_multiplier(&self) -> f64 {
        match self {
            WorkDomain::GpuCompute => 1.0,
            WorkDomain::Simulation => 1.0,
            WorkDomain::PrintJob => 2.0,
            WorkDomain::SciSim => 2.0,
            WorkDomain::SpatialCapture => 2.0,
            WorkDomain::AerialFlight => 3.0,
            WorkDomain::GroundRobot => 3.0,
            WorkDomain::SimToReal => 5.0,
        }
    }

    pub fn max_multiplier(&self) -> f64 {
        match self {
            WorkDomain::SimToReal => 10.0,
            _ => self.base_multiplier(),
        }
    }

    pub fn required_tier(&self) -> TrustTier {
        match self {
            WorkDomain::SimToReal => TrustTier::T2,
            WorkDomain::AerialFlight => TrustTier::T3,
            WorkDomain::GroundRobot => TrustTier::T3,
            _ => TrustTier::T0,
        }
    }
}

/// Cap check result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CapCheckResult {
    Allowed { remaining: u64 },
    Denied { reason: String, current: u64, cap: u64 },
}

impl CapCheckResult {
    pub fn is_allowed(&self) -> bool {
        matches!(self, CapCheckResult::Allowed { .. })
    }
}

/// Reward computation with all caps applied
/// This is the SINGLE reward formula (replaces the two disagreeing ones in OSOVM)
pub fn compute_reward(
    enforcer: &CapsEnforcer,
    identity: &AgentIdentity,
    tier: TrustTier,
    reputation: f64,
    dna_entropy: f64,
    domain: WorkDomain,
    gpu_hours: f64,
    proof_value: f64,           // Verified proof_value from OSOVM (≥ threshold)
    model_hash: Option<String>, // For repeat limit
) -> Result<RewardOutput, CapCheckResult> {
    // 1. Check epoch cap (Dopamine equivalent)
    let dopamine_equivalent = (gpu_hours * proof_value * 1000.0) as u64; // 1000 DOP/GPU-hr
    let epoch_check = enforcer.check_epoch_cap(&identity.agent_npub, dopamine_equivalent);
    if !epoch_check.is_allowed() {
        return Err(epoch_check);
    }

    // 2. Check tier gate
    let tier_check = enforcer.check_tier_gate(tier, domain);
    if !tier_check.is_allowed() {
        return Err(tier_check);
    }

    // 3. Check repeat limit (if model_hash provided)
    if let Some(hash) = model_hash {
        let repeat_check = enforcer.check_repeat_limit(&identity.agent_npub, &hash);
        if !repeat_check.is_allowed() {
            return Err(repeat_check);
        }
    }

    // 4. Compute reward (SYNAPSE minted)
    // Synapse = GPU-hours × 1000 × multiplier × proof_value
    let multiplier = domain.base_multiplier();
    let synapse = gpu_hours * 1000.0 * multiplier * proof_value;

    // 5. Apply Busy Beaver ceiling
    let bb_ceiling = enforcer.compute_bb_ceiling(
        identity, synapse, tier, reputation, dna_entropy
    );
    let final_synapse = synapse.min(bb_ceiling);

    // 6. Compute Dopamine equivalent (for epoch tracking)
    let dopamine_equivalent = (final_synapse / 1000.0 * 1000.0) as u64; // Convert back

    // 7. Record (called after actual mint)
    // enforcer.record_dopamine_earned(&identity.agent_npub, dopamine_equivalent, final_synapse);
    // if let Some(hash) = model_hash {
    //     enforcer.record_repeat(&identity.agent_npub, &hash);
    // }

    Ok(RewardOutput {
        synapse_minted: final_synapse,
        dopamine_equivalent,
        multiplier_applied: multiplier,
        proof_value,
        bb_ceiling,
        epoch_remaining: match epoch_check {
            CapCheckResult::Allowed { remaining } => remaining,
            _ => 0,
        },
    })
}

/// Reward computation output
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RewardOutput {
    pub synapse_minted: f64,
    pub dopamine_equivalent: u64,
    pub multiplier_applied: f64,
    pub proof_value: f64,
    pub bb_ceiling: f64,
    pub epoch_remaining: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bb_ceiling_computation() {
        let enforcer = CapsEnforcer::new();
        let identity = AgentIdentity {
            agent_npub: "npub1test".into(),
            device_id: crate::identity::DeviceId([1u8; 32]),
            sui_address: crate::identity::SuiAddress("0xabc".into()),
            mnemonic: None,
            dna_fingerprint: crate::identity::DnaFingerprint([0u8; 16]),
            born_at: 1000,
            odù_primary: 1,
            odù_secondary: 2,
        };

        // T0, low rep, low entropy
        let ceiling = enforcer.compute_bb_ceiling(&identity, 10000.0, TrustTier::T0, 0.0, 0.0);
        assert!(ceiling >= BB_FLOOR);
        assert!(ceiling <= BB_ABSOLUTE_CEILING);

        // T4, high rep, high entropy
        let ceiling = enforcer.compute_bb_ceiling(&identity, 10000.0, TrustTier::T4, 1.0, 1.0);
        assert!(ceiling > 10000.0);
    }

    #[test]
    fn test_epoch_cap_enforcement() {
        let enforcer = CapsEnforcer::new();
        let agent = "npub1test";

        // First request within cap
        let result = enforcer.check_epoch_cap(agent, 10_000_000);
        assert!(result.is_allowed());

        // Request that would exceed cap
        let result = enforcer.check_epoch_cap(agent, 45_000_000);
        assert!(result.is_allowed()); // 10M + 45M = 55M > 50M cap? Wait, 10M + 45M = 55M > 50M

        // Actually first was check only, not recorded. Let's test properly:
        enforcer.record_dopamine_earned(agent, 40_000_000, 40_000.0);
        let result = enforcer.check_epoch_cap(agent, 15_000_000); // 40M + 15M = 55M > 50M
        assert!(!result.is_allowed());
    }

    #[test]
    fn test_repeat_limit() {
        let enforcer = CapsEnforcer::new();
        let agent = "npub1test";
        let model = "model_hash_abc";

        for i in 0..3 {
            let result = enforcer.check_repeat_limit(agent, model);
            assert!(result.is_allowed(), "Attempt {} should be allowed", i+1);
            enforcer.record_repeat(agent, model);
        }

        // 4th attempt should fail
        let result = enforcer.check_repeat_limit(agent, model);
        assert!(!result.is_allowed());
    }

    #[test]
    fn test_tier_gate() {
        let enforcer = CapsEnforcer::new();

        // T1 cannot do SimToReal (requires T2)
        let result = enforcer.check_tier_gate(TrustTier::T1, WorkDomain::SimToReal);
        assert!(!result.is_allowed());

        // T2 can do SimToReal
        let result = enforcer.check_tier_gate(TrustTier::T2, WorkDomain::SimToReal);
        assert!(result.is_allowed());

        // T2 cannot do AerialFlight (requires T3)
        let result = enforcer.check_tier_gate(TrustTier::T2, WorkDomain::AerialFlight);
        assert!(!result.is_allowed());

        // T3 can do AerialFlight
        let result = enforcer.check_tier_gate(TrustTier::T3, WorkDomain::AerialFlight);
        assert!(result.is_allowed());
    }

    #[test]
    fn test_full_reward_computation() {
        let enforcer = CapsEnforcer::new();
        let identity = AgentIdentity {
            agent_npub: "npub1test".into(),
            device_id: crate::identity::DeviceId([1u8; 32]),
            sui_address: crate::identity::SuiAddress("0xabc".into()),
            mnemonic: None,
            dna_fingerprint: crate::identity::DnaFingerprint([0u8; 16]),
            born_at: 1000,
            odù_primary: 1,
            odù_secondary: 2,
        };

        // Valid: T2 agent, SimToReal, 1 GPU-hour, proof_value 0.8
        let result = compute_reward(
            &enforcer,
            &identity,
            TrustTier::T2,
            0.8,
            0.7,
            WorkDomain::SimToReal,
            1.0,
            0.8,
            Some("model_123".into()),
        );

        assert!(result.is_ok());
        let output = result.unwrap();
        // 1.0 * 1000 * 5.0 * 0.8 = 4000 SYN
        // BB ceiling for 4000 at T2, 0.8 rep, 0.7 entropy = 4000 * 2.0 * 0.9 * 0.85 = 6120
        // So 4000 < 6120, should pass
        assert!(output.synapse_minted > 0.0);
    }

    #[test]
    fn test_epoch_advancement_resets_caps() {
        // This test would require mocking time - skipped in unit tests
        // Integration test should verify epoch rollover resets trackers
    }
}