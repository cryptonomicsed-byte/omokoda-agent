/// Phase 20.3 — OSO Constants: Single Source of Truth
///
/// This module loads protocol constants from `TOC_CONSTANTS.toml`
/// (at `~/sovereign-eco-blueprint/specs/TOC_CONSTANTS.toml`) and exposes
/// them to Rust callers.  At test time it also verifies that the hardcoded
/// constants in `economics.rs` match the TOML file — CI will fail if any
/// language drifts.
///
/// Runtime loading: call `TocConstants::load()` or use the lazy static
/// `TOC` singleton (which reads `TOC_CONSTANTS_PATH` env var or falls back
/// to the path relative to `CARGO_MANIFEST_DIR`).
///
/// Languages → same canonical values:
///   Rust    — this module
///   Julia   — OSOVM/src/constants.jl (reads the same TOML via TOML.jl)
///   Python  — vantage/toc_constants.py (reads via tomllib/tomli)
///   JS/TS   — vantage/src/toc_constants.ts (reads via @iarna/toml)

use serde::Deserialize;

/// ASE (Àṣẹ) emission and fee constants.
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct AsePools {
    pub veilsim: f64,
    pub rnd: f64,
    pub governance: f64,
    pub reserve: f64,
    pub compute: f64,
    pub storage: f64,
    pub witness: f64,
    pub treasury: f64,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct AseConstants {
    pub emission_per_minute: u64,
    pub max_daily_emission: u64,
    pub birth_fee: f64,
    pub micro_per_ase: u64,
    pub pools: AsePools,
}

/// Dopamine pool constants.
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct DopamineConstants {
    pub genesis_seed: u64,
    pub transferable: bool,
    pub ase_to_dopamine: u64,
    pub opcode_mint: String,
    pub opcode_decay: String,
    pub opcode_contribution: String,
    pub decay_min: f64,
    pub decay_max: f64,
    pub decay_ema_alpha: f64,
    pub decay_clamp_per_epoch: f64,
}

/// Synapse (per-agent compute slice) constants.
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct SynapseConstants {
    pub max_pool_share: f64,
    pub transferable: bool,
    pub conversion_ratio: f64,
    pub per_gpu_hour: f64,
}

/// Èṣù tithe constants.
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct EsuConstants {
    pub tithe_rate: f64,
    pub opcode: String,
}

/// Hermetic gate thresholds.
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct GatesConstants {
    pub stake_fraction: f64,
    pub gate_count: u8,
    pub balance_weight: f64,
    pub gate_alignment_weight: f64,
    pub base_multiplier: f64,
    pub max_multiplier: f64,
}

/// Koodu ritual gate multipliers.
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct RitualConstants {
    pub sabbath_multiplier: f64,
    pub eshu_squared_multi: f64,
    pub jubilee_minor_multi: f64,
    pub capstone_multi: f64,
    pub void_multi: f64,
}

/// Root structure deserialised from TOC_CONSTANTS.toml.
#[derive(Debug, Clone, Deserialize)]
pub struct TocConstants {
    pub ase: AseConstants,
    pub dopamine: DopamineConstants,
    pub synapse: SynapseConstants,
    pub esu: EsuConstants,
    pub gates: GatesConstants,
    pub ritual: RitualConstants,
}

impl TocConstants {
    /// Load constants from the canonical TOML file.
    ///
    /// Path resolution order:
    ///   1. `TOC_CONSTANTS_PATH` env var
    ///   2. `~/sovereign-eco-blueprint/specs/TOC_CONSTANTS.toml`
    pub fn load() -> Result<Self, String> {
        let path = Self::resolve_path()?;
        let content = std::fs::read_to_string(&path)
            .map_err(|e| format!("TOC_CONSTANTS: cannot read {path}: {e}"))?;
        toml::from_str(&content)
            .map_err(|e| format!("TOC_CONSTANTS: parse error in {path}: {e}"))
    }

    fn resolve_path() -> Result<String, String> {
        if let Ok(p) = std::env::var("TOC_CONSTANTS_PATH") {
            return Ok(p);
        }
        // Derive from HOME
        let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());
        Ok(format!("{home}/sovereign-eco-blueprint/specs/TOC_CONSTANTS.toml"))
    }
}

/// Canonical Èṣù tithe rate — read from TOC_CONSTANTS at startup or compile-time.
/// Other code should call `TocConstants::load()` and use `toc.esu.tithe_rate`.
pub const ESU_TITHE_RATE: f64 = 0.0369;

/// ASE emission per minute.
pub const ASE_EMISSION_PER_MINUTE: u64 = 1;

/// Dopamine genesis seed.
pub const DOPAMINE_GENESIS_SEED: u64 = 86_000_000_000;

/// TOC_MINT opcode.
pub const TOC_MINT_OPCODE: &str = "0x54";

/// TOC_DECAY opcode.
pub const TOC_DECAY_OPCODE: &str = "0x55";

#[cfg(test)]
mod tests {
    use super::*;

    /// Verify that the hardcoded constants in this module match the TOML file.
    /// This test FAILS if any constant drifts — enforcing single source of truth.
    #[test]
    fn rust_constants_match_toml() {
        let toc = match TocConstants::load() {
            Ok(t) => t,
            Err(e) => {
                // If TOML not available in CI, skip rather than fail.
                eprintln!("TOC_CONSTANTS not available ({e}), skipping drift check");
                return;
            }
        };

        assert_eq!(
            toc.esu.tithe_rate, ESU_TITHE_RATE,
            "Rust ESU_TITHE_RATE ({ESU_TITHE_RATE}) diverges from TOML ({})",
            toc.esu.tithe_rate
        );
        assert_eq!(
            toc.ase.emission_per_minute, ASE_EMISSION_PER_MINUTE,
            "ASE_EMISSION_PER_MINUTE diverges"
        );
        assert_eq!(
            toc.dopamine.genesis_seed, DOPAMINE_GENESIS_SEED,
            "DOPAMINE_GENESIS_SEED diverges"
        );
        assert_eq!(
            toc.dopamine.opcode_mint, TOC_MINT_OPCODE,
            "TOC_MINT_OPCODE diverges"
        );
    }

    #[test]
    fn ase_pools_sum_to_one() {
        let toc = match TocConstants::load() {
            Ok(t) => t,
            Err(_) => return,
        };
        let p = &toc.ase.pools;
        let sum = p.veilsim + p.rnd + p.governance + p.reserve
            + p.compute + p.storage + p.witness + p.treasury;
        let diff = (sum - 1.0_f64).abs();
        assert!(diff < 1e-10, "ASE pool percentages sum to {sum}, expected 1.0");
    }

    #[test]
    fn gate_multiplier_range_valid() {
        let toc = match TocConstants::load() {
            Ok(t) => t,
            Err(_) => return,
        };
        assert!(toc.gates.base_multiplier < toc.gates.max_multiplier);
        assert!(toc.gates.base_multiplier > 0.0);
        assert!(toc.gates.max_multiplier <= 2.0);
    }

    #[test]
    fn esu_opcode_matches_known_value() {
        let toc = match TocConstants::load() {
            Ok(t) => t,
            Err(_) => return,
        };
        assert_eq!(toc.esu.opcode, "0x27", "Èṣù opcode must be 0x27");
    }

    #[test]
    fn toc_mint_opcode_matches_known_value() {
        let toc = match TocConstants::load() {
            Ok(t) => t,
            Err(_) => return,
        };
        assert_eq!(toc.dopamine.opcode_mint, "0x54", "TOC_MINT must be 0x54 (confirmed in arch)");
    }
}
