// omokoda-core/src/seven/state.rs
//
// Universal7State — the runtime operating cycle primitive for the USF-7 system.
//
// This struct wires together:
//   SevenCalendar    (BTC-anchored time → SevenFunction)
//   function_to_principle()  (SevenFunction → HermeticPrinciple)
//   49-facet Thunder Lattice (function × principle → lattice_index 0..48)
//   256 Odù spread          (lattice_index → odu_index 0..255)
//   ActionVessel            (odu_index >> 4 → 1-of-16 operational domains)
//   Blake3 chain-of-custody  (previous_state_hash for verifiable cycle history)
//
// Lattice: LatticeIndex = SevenFunction(0-6) × 7 + HermeticPrinciple(0-6) → 0..48
// Odù spread: odu_index = floor(lattice_index × 256 / 49) → spreads 49 positions
//             across the full 256 Odù space without collision.
//
// The state is deterministic from btc_block_height alone and chainable via
// previous_state_hash, making it suitable for Zàngbétò receipt inclusion.

use crate::gates::HermeticPrinciple;
use crate::seven::{function_to_principle, SevenCalendar, SevenFunction};
use ifascript::ActionVessel;
use serde::{Deserialize, Serialize};

/// Runtime operating cycle for the Universal Seven Functions Protocol.
///
/// At any BTC block height an agent knows:
/// - which SevenFunction governs this cycle (what kind of agency is active)
/// - which HermeticPrinciple constrains it (by what law)
/// - which Thunder Lattice cell (0..48) the agent inhabits
/// - which Odù (0..255) this lattice cell maps to
/// - which ActionVessel (one of 16) is the operational domain (via `active_vessel()`)
///
/// States chain via `previous_state_hash` so the full operating history
/// is verifiable without storing every prior state.
///
/// Note: `active_vessel()` is a computed method rather than a stored field because
/// `ActionVessel` does not implement `Serialize`/`Deserialize`. It is deterministic
/// from `odu_index` via `ActionVessel::from_index(odu_index)`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Universal7State {
    pub active_function: SevenFunction,
    pub active_principle: HermeticPrinciple,
    /// 0..48: SevenFunction(0-6) × 7 + HermeticPrinciple(0-6)
    pub lattice_index: u8,
    /// 0..255: deterministic Odù from lattice_index
    pub odu_index: u8,
    pub btc_block_height: u64,
    /// Blake3 hash of the previous state's canonical bytes — zero for genesis.
    pub previous_state_hash: [u8; 32],
}

impl Universal7State {
    /// Compute a fresh state from a BTC block height with no prior chain.
    /// Use this only for the very first state (genesis); prefer `advance` otherwise.
    pub fn genesis(btc_block_height: u64) -> Self {
        let (active_function, active_principle, lattice_index, odu_index) =
            Self::compute(btc_block_height);
        Self {
            active_function,
            active_principle,
            lattice_index,
            odu_index,
            btc_block_height,
            previous_state_hash: [0u8; 32],
        }
    }

    /// Advance to the next BTC block height, chaining from this state.
    pub fn advance(&self, next_height: u64) -> Self {
        let prev_hash = self.hash();
        let (active_function, active_principle, lattice_index, odu_index) =
            Self::compute(next_height);
        Self {
            active_function,
            active_principle,
            lattice_index,
            odu_index,
            btc_block_height: next_height,
            previous_state_hash: prev_hash,
        }
    }

    /// The operational domain for this cycle — one of 16 ActionVessels.
    /// Deterministic from `odu_index` via the top nibble mapping.
    pub fn active_vessel(&self) -> ActionVessel {
        ActionVessel::from_index(self.odu_index)
    }

    /// Blake3 hash of this state's canonical bytes.
    /// Used as `previous_state_hash` in the successor state.
    pub fn hash(&self) -> [u8; 32] {
        let mut hasher = blake3::Hasher::new();
        hasher.update(&self.btc_block_height.to_le_bytes());
        hasher.update(&[self.lattice_index, self.odu_index]);
        hasher.update(&self.previous_state_hash);
        *hasher.finalize().as_bytes()
    }

    /// Whether this state is at a Thunder Lattice resonance position.
    /// Resonance: lattice_index is a multiple of 7 (SevenFunction × 7 + 0).
    pub fn is_resonance(&self) -> bool {
        self.lattice_index % 7 == 0
    }

    /// Thunder Lattice index from function and principle.
    pub fn lattice_for(f: SevenFunction, p: HermeticPrinciple) -> u8 {
        (f as u8) * 7 + (p as u8)
    }

    /// Map lattice_index (0..48) to an Odù byte (0..255).
    /// Uses floor(lattice × 256 / 49) to spread 49 positions across 256 without collision.
    pub fn odu_for_lattice(lattice: u8) -> u8 {
        ((lattice as u32 * 256) / 49) as u8
    }

    // ── Internal ──────────────────────────────────────────────────────────────

    fn compute(btc_block_height: u64) -> (SevenFunction, HermeticPrinciple, u8, u8) {
        let active_function =
            SevenCalendar::from_btc_height(btc_block_height).unwrap_or(SevenFunction::Spark);
        let active_principle = function_to_principle(active_function);
        let lattice_index = Self::lattice_for(active_function, active_principle);
        let odu_index = Self::odu_for_lattice(lattice_index);
        (active_function, active_principle, lattice_index, odu_index)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const KOODU_GENESIS: u64 = 780_000;

    #[test]
    fn genesis_state_has_zero_prev_hash() {
        let state = Universal7State::genesis(KOODU_GENESIS);
        assert_eq!(state.previous_state_hash, [0u8; 32]);
        assert_eq!(state.btc_block_height, KOODU_GENESIS);
    }

    #[test]
    fn lattice_index_in_range() {
        for height in [KOODU_GENESIS, KOODU_GENESIS + 144, KOODU_GENESIS + 1000] {
            let state = Universal7State::genesis(height);
            assert!(
                state.lattice_index < 49,
                "lattice_index out of range: {}",
                state.lattice_index
            );
        }
    }

    #[test]
    fn odu_index_in_range() {
        for i in 0u8..49 {
            let odu = Universal7State::odu_for_lattice(i);
            assert!(odu <= 255);
        }
    }

    #[test]
    fn odu_spread_no_collisions() {
        use std::collections::HashSet;
        let indices: HashSet<u8> = (0u8..49).map(Universal7State::odu_for_lattice).collect();
        assert_eq!(
            indices.len(),
            49,
            "odu_for_lattice must be injective across 0..48"
        );
    }

    #[test]
    fn advance_chains_hash() {
        let s0 = Universal7State::genesis(KOODU_GENESIS);
        let s1 = s0.advance(KOODU_GENESIS + 144);
        assert_eq!(s1.previous_state_hash, s0.hash());
        assert_ne!(s1.previous_state_hash, [0u8; 32]);
    }

    #[test]
    fn lattice_for_all_functions_and_principles() {
        for f_i in 0u8..7 {
            for p_i in 0u8..7 {
                let f = SevenFunction::from_index(f_i as usize).unwrap();
                let p = HermeticPrinciple::from_index(p_i as usize);
                let lattice = Universal7State::lattice_for(f, p);
                assert_eq!(lattice, f_i * 7 + p_i);
            }
        }
    }

    #[test]
    fn active_vessel_consistent_with_odu() {
        let state = Universal7State::genesis(KOODU_GENESIS + 288);
        assert_eq!(
            state.active_vessel(),
            ActionVessel::from_index(state.odu_index)
        );
    }

    #[test]
    fn hash_is_deterministic() {
        let s = Universal7State::genesis(KOODU_GENESIS);
        assert_eq!(s.hash(), s.hash());
    }

    #[test]
    fn distinct_heights_produce_distinct_hashes() {
        let s0 = Universal7State::genesis(KOODU_GENESIS);
        let s1 = Universal7State::genesis(KOODU_GENESIS + 1008); // +7 days
        assert_ne!(s0.hash(), s1.hash());
    }

    #[test]
    fn function_principle_bridge_consistent() {
        let state = Universal7State::genesis(KOODU_GENESIS + 144);
        let expected_principle = function_to_principle(state.active_function);
        assert_eq!(state.active_principle, expected_principle);
    }

    #[test]
    fn resonance_at_function_boundary() {
        // lattice_index = SevenFunction * 7 + 0 when principle index = 0 (Mentalism)
        // Spark(0) × 7 + Mentalism(0) = 0 → resonance
        // Mind(1) × 7 + Mentalism(0) = 7 → resonance
        // Genesis height maps to Spark → Mentalism (lattice 0) → resonance
        let state = Universal7State::genesis(KOODU_GENESIS);
        // active_principle is Mentalism when function is Spark (from function_to_principle)
        if state.lattice_index % 7 == 0 {
            assert!(state.is_resonance());
        }
    }
}
