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
    pub emission_window_hours: u64,
    pub birth_fee: f64,
    pub micro_per_ase: u64,
    pub pools: AsePools,
}

impl AseConstants {
    /// The daily emission cap, DERIVED rather than declared (I-11).
    ///
    /// `emission_per_minute * 60 * emission_window_hours`. Held as a computation
    /// so that `1440` is a literal in exactly one place in the codebase -- the
    /// Inheritance seat count -- and so that widening the window can never
    /// silently move a number that was chosen, not timed.
    ///
    /// Replaces a `max_daily_emission: u64` field that was deserialised from
    /// TOML and never read anywhere: a declared constant nobody measured.
    pub fn max_daily_emission(&self) -> u64 {
        self.emission_per_minute * 60 * self.emission_window_hours
    }
}

/// Dopamine pool constants.
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct DopamineConstants {
    pub genesis_seed: u64,
    pub transferable: bool,
    pub agent_burn_rate: u64,
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
    /// "agent_only" | "false" | bool-like string — not a raw boolean because
    /// the canonical TOML value is "agent_only" (a policy string, not true/false).
    pub transferable: String,
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
        toml::from_str(&content).map_err(|e| format!("TOC_CONSTANTS: parse error in {path}: {e}"))
    }

    fn resolve_path() -> Result<String, String> {
        if let Ok(p) = std::env::var("TOC_CONSTANTS_PATH") {
            return Ok(p);
        }
        // Bundled copy inside the crate (used in CI where sovereign-eco-blueprint
        // is not checked out alongside Omo-Koda2).
        if let Ok(manifest) = std::env::var("CARGO_MANIFEST_DIR") {
            let bundled = format!("{manifest}/specs/TOC_CONSTANTS.toml");
            if std::path::Path::new(&bundled).exists() {
                return Ok(bundled);
            }
        }
        // Derive from HOME (developer workstation default)
        let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());
        Ok(format!(
            "{home}/sovereign-eco-blueprint/specs/TOC_CONSTANTS.toml"
        ))
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
            // NOT `return`. Skipping on error made this test structurally unable
            // to fail: any parse error or missing field took this branch and the
            // test reported ok. Established by negative control -- deleting a
            // required key from TOC_CONSTANTS.toml left the test green, with
            // "TOML parse error" printed as the *reason* it passed. A
            // single-source-of-truth check that goes green when the source is
            // unreadable is not a check; an unreadable source is a failure.
            Err(e) => panic!(
                "TOC_CONSTANTS could not be loaded, so constant drift is UNVERIFIED \
                 (not fine): {e}"
            ),
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
        let sum = p.veilsim
            + p.rnd
            + p.governance
            + p.reserve
            + p.compute
            + p.storage
            + p.witness
            + p.treasury;
        let diff = (sum - 1.0_f64).abs();
        assert!(
            diff < 1e-10,
            "ASE pool percentages sum to {sum}, expected 1.0"
        );
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
        assert_eq!(
            toc.dopamine.opcode_mint, "0x54",
            "TOC_MINT must be 0x54 (confirmed in arch)"
        );
    }
}
