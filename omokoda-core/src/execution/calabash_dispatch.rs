//! Calabash Dispatch — bridge from resolved Odù → executable AgentAction.
//!
//! The Digital Calabash holds 256 base Odù, each mapped to one of the 16
//! Action Vessels. This module answers: given a resolved Odù, what should the
//! agent actually DO? It derives an operational `CompiledAction` from the
//! vessel semantics — not from the spiritual prescriptions in the corpus
//! (those are for interpretation), but from the vessel's operational domain.
//!
//! ## Addressing
//!
//! An 8-bit Odù index decomposes into:
//!   top nibble  (bits 7–4) → Action Vessel → operational domain
//!   bottom nibble (bits 3–0) → modifier vessel → manner/refinement
//!
//! The top vessel sets the primary tool and action domain. The bottom vessel
//! refines the prescription: it determines what verification is applied, what
//! cadence is used, and what secondary step (if any) is appended.
//!
//! ## Relationship to ActionCompiler
//!
//! `CalabashDispatcher::compile_for_odu()` delegates to `ActionCompiler::compile()`.
//! The dispatcher generates the prescription string; the compiler parses it.
//! Callers may call `ActionCompiler::compile()` directly with custom prescriptions,
//! or use `CalabashDispatcher` for the standard vessel-derived prescription.
//!
//! See: `docs/consolidated/full_256_ai_digital_calabash.md`

use ifascript::odu::{get_odu, ActionVessel};
use serde::{Deserialize, Serialize};

use crate::execution::action_compiler::{ActionCompiler, CadenceSpec, CompiledAction, CompileError, VerifySpec};

/// A fully-specified agent directive derived from one of the 256 base Odù.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CalabashDirective {
    /// The Odù index (0–255) this directive was derived from.
    pub odu_index: u8,
    /// Vessel index (top nibble, 0–15).
    pub vessel: u8,
    /// Modifier vessel index (bottom nibble, 0–15).
    pub modifier: u8,
    /// Opcode label: "<vessel_name>:<modifier_name>" e.g. "execution:receipt"
    pub opcode: String,
    /// The operational name from the Odù corpus.
    pub odu_name: String,
    /// The universal English name.
    pub universal_name: String,
    /// Agent-operational prescription lines (ActionCompiler-parseable).
    pub prescription: String,
    /// Verify specs derived from vessel.
    pub verify: Vec<VerifySpec>,
    /// Cadence spec derived from vessel.
    pub cadence: CadenceSpec,
}

// ─── Vessel profiles ──────────────────────────────────────────────────────────

/// Operational profile for one of the 16 Action Vessels.
struct VesselProfile {
    name: &'static str,
    /// Primary tool for step 1.
    primary_tool: &'static str,
    /// Action verb used in step 1.
    action_verb: &'static str,
    /// Secondary tool for step 2 (verification/follow-up).
    secondary_tool: &'static str,
    /// What kind of verify assertion to apply (VerifySpec.kind).
    verify_kind: &'static str,
    /// Cadence trigger.
    trigger: &'static str,
    /// Cooldown in seconds.
    cooldown_secs: u64,
    /// Max executions per window.
    max_per_window: Option<u32>,
    /// Window in seconds.
    window_secs: Option<u64>,
    /// Deadline in seconds.
    deadline_secs: Option<u64>,
}

const VESSEL_PROFILES: [VesselProfile; 16] = [
    // 0 — Genesis: Initialize, covenant, identity
    VesselProfile {
        name: "genesis",
        primary_tool: "read",
        action_verb: "Verify agent identity and initialization state",
        secondary_tool: "bash",
        verify_kind: "exit_code",
        trigger: "immediate",
        cooldown_secs: 0,
        max_per_window: None,
        window_secs: None,
        deadline_secs: Some(60),
    },
    // 1 — Void: Clear, release, dissolve
    VesselProfile {
        name: "void",
        primary_tool: "bash",
        action_verb: "Clear stale state and release held resources",
        secondary_tool: "bash",
        verify_kind: "exit_code",
        trigger: "immediate",
        cooldown_secs: 30,
        max_per_window: Some(10),
        window_secs: Some(3600),
        deadline_secs: Some(120),
    },
    // 2 — Attention: Focus, signal detection, observation
    VesselProfile {
        name: "attention",
        primary_tool: "read",
        action_verb: "Observe and classify incoming signals",
        secondary_tool: "read",
        verify_kind: "file_contains",
        trigger: "event:signal_received",
        cooldown_secs: 5,
        max_per_window: Some(50),
        window_secs: Some(600),
        deadline_secs: Some(30),
    },
    // 3 — Loop: Iterate, pattern, scheduled repetition
    VesselProfile {
        name: "loop",
        primary_tool: "bash",
        action_verb: "Execute scheduled iteration of recurring pattern",
        secondary_tool: "bash",
        verify_kind: "exit_code",
        trigger: "cron:0 * * * *",
        cooldown_secs: 60,
        max_per_window: Some(24),
        window_secs: Some(86400),
        deadline_secs: Some(300),
    },
    // 4 — Receipt: Record, accountability, audit
    VesselProfile {
        name: "receipt",
        primary_tool: "write",
        action_verb: "Record action receipt and update accountability ledger",
        secondary_tool: "bash",
        verify_kind: "file_exists",
        trigger: "immediate",
        cooldown_secs: 0,
        max_per_window: None,
        window_secs: None,
        deadline_secs: Some(30),
    },
    // 5 — Mask: Privacy, concealment, separation
    VesselProfile {
        name: "mask",
        primary_tool: "bash",
        action_verb: "Apply privacy boundary and seal sensitive data",
        secondary_tool: "bash",
        verify_kind: "exit_code",
        trigger: "immediate",
        cooldown_secs: 0,
        max_per_window: None,
        window_secs: None,
        deadline_secs: Some(60),
    },
    // 6 — Residue: Telemetry, status baseline, quiet observation
    VesselProfile {
        name: "residue",
        primary_tool: "bash",
        action_verb: "Emit telemetry and update baseline status",
        secondary_tool: "bash",
        verify_kind: "exit_code",
        trigger: "cron:*/15 * * * *",
        cooldown_secs: 60,
        max_per_window: Some(4),
        window_secs: Some(3600),
        deadline_secs: Some(60),
    },
    // 7 — Execution: Act, run, deploy, primary action
    VesselProfile {
        name: "execution",
        primary_tool: "bash",
        action_verb: "Execute primary action directive",
        secondary_tool: "bash",
        verify_kind: "exit_code",
        trigger: "immediate",
        cooldown_secs: 0,
        max_per_window: None,
        window_secs: None,
        deadline_secs: Some(600),
    },
    // 8 — Swarm: Coordinate, delegate, multi-agent
    VesselProfile {
        name: "swarm",
        primary_tool: "bash",
        action_verb: "Coordinate with swarm and distribute work",
        secondary_tool: "bash",
        verify_kind: "exit_code",
        trigger: "event:swarm_signal",
        cooldown_secs: 30,
        max_per_window: Some(20),
        window_secs: Some(3600),
        deadline_secs: Some(300),
    },
    // 9 — Restraint: Hold, evaluate, deliberate pause
    VesselProfile {
        name: "restraint",
        primary_tool: "read",
        action_verb: "Evaluate readiness conditions before proceeding",
        secondary_tool: "read",
        verify_kind: "file_contains",
        trigger: "state:evaluation_ready",
        cooldown_secs: 120,
        max_per_window: Some(5),
        window_secs: Some(3600),
        deadline_secs: Some(600),
    },
    // 10 — Migration: Transform, move, adapt
    VesselProfile {
        name: "migration",
        primary_tool: "bash",
        action_verb: "Transform and migrate data or agent state",
        secondary_tool: "bash",
        verify_kind: "file_exists",
        trigger: "event:migration_trigger",
        cooldown_secs: 60,
        max_per_window: Some(5),
        window_secs: Some(86400),
        deadline_secs: Some(1800),
    },
    // 11 — Consent: Permission, ratification, governance
    VesselProfile {
        name: "consent",
        primary_tool: "read",
        action_verb: "Request and record consent for proposed action",
        secondary_tool: "write",
        verify_kind: "file_contains",
        trigger: "state:pending_consent",
        cooldown_secs: 300,
        max_per_window: Some(3),
        window_secs: Some(86400),
        deadline_secs: Some(3600),
    },
    // 12 — Vision: Observe, survey, report environment
    VesselProfile {
        name: "vision",
        primary_tool: "read",
        action_verb: "Survey environment and compile observation report",
        secondary_tool: "write",
        verify_kind: "file_exists",
        trigger: "event:observation_cycle",
        cooldown_secs: 30,
        max_per_window: Some(12),
        window_secs: Some(3600),
        deadline_secs: Some(120),
    },
    // 13 — Growth: Learn, integrate, expand capability
    VesselProfile {
        name: "growth",
        primary_tool: "write",
        action_verb: "Integrate new knowledge and expand agent capability",
        secondary_tool: "bash",
        verify_kind: "file_exists",
        trigger: "event:growth_signal",
        cooldown_secs: 60,
        max_per_window: Some(10),
        window_secs: Some(86400),
        deadline_secs: Some(600),
    },
    // 14 — Seal: Cryptographic finalization, anchor, proof
    VesselProfile {
        name: "seal",
        primary_tool: "bash",
        action_verb: "Cryptographically finalize and anchor action proof",
        secondary_tool: "bash",
        verify_kind: "exit_code",
        trigger: "immediate",
        cooldown_secs: 0,
        max_per_window: None,
        window_secs: None,
        deadline_secs: Some(120),
    },
    // 15 — Rhythm: Schedule, synchronize, cosmic cadence
    VesselProfile {
        name: "rhythm",
        primary_tool: "bash",
        action_verb: "Synchronize agent cadence with system rhythm",
        secondary_tool: "bash",
        verify_kind: "exit_code",
        trigger: "cron:0 0 * * *",
        cooldown_secs: 3600,
        max_per_window: Some(1),
        window_secs: Some(86400),
        deadline_secs: Some(300),
    },
];

// ─── Modifier refinements ─────────────────────────────────────────────────────

/// A short phrase appended to step 1 based on the bottom modifier vessel.
/// These refine the top vessel's primary action with the modifier's intent.
const MODIFIER_REFINEMENTS: [&str; 16] = [
    "with covenant and witness",                   // 0 Genesis mod
    "clearing all prior residue",                  // 1 Void mod
    "focused on signal clarity",                   // 2 Attention mod
    "repeating until stable pattern emerges",      // 3 Loop mod
    "and write receipt to ledger",                 // 4 Receipt mod
    "under privacy mask",                          // 5 Mask mod
    "leaving minimal residue",                     // 6 Residue mod
    "with direct execution authority",             // 7 Execution mod
    "coordinating with peer agents",               // 8 Swarm mod
    "with deliberate restraint",                   // 9 Restraint mod
    "triggering downstream migration",             // 10 Migration mod
    "with explicit consent check",                 // 11 Consent mod
    "expanding field of vision",                   // 12 Vision mod
    "seeding growth in memory",                    // 13 Growth mod
    "sealing with cryptographic proof",            // 14 Seal mod
    "aligned to cosmic rhythm",                    // 15 Rhythm mod
];

// ─── CalabashDispatcher ───────────────────────────────────────────────────────

pub struct CalabashDispatcher;

impl CalabashDispatcher {
    /// Derive an operational directive for any Odù index 0–255.
    ///
    /// The top nibble determines the vessel (operational domain).
    /// The bottom nibble determines the modifier (refinement).
    pub fn directive_for(odu_index: u8) -> CalabashDirective {
        let odu = get_odu(odu_index);
        let vessel_idx = (odu_index >> 4) as usize;   // top nibble
        let modifier_idx = (odu_index & 0x0F) as usize; // bottom nibble

        let vp = &VESSEL_PROFILES[vessel_idx];
        let mp = &VESSEL_PROFILES[modifier_idx];
        let refinement = MODIFIER_REFINEMENTS[modifier_idx];

        let opcode = format!("{}:{}", vp.name, mp.name);

        // Two-step prescription:
        // Step 1: primary action with modifier refinement
        // Step 2: verification/follow-up from the modifier vessel
        let prescription = format!(
            "1. {} {} via {}\n2. Confirm outcome and record state via {}",
            vp.action_verb,
            refinement,
            vp.primary_tool,
            mp.secondary_tool,
        );

        let verify = Self::build_verify(vp, odu_index);
        let cadence = Self::build_cadence(vp);

        CalabashDirective {
            odu_index,
            vessel: vessel_idx as u8,
            modifier: modifier_idx as u8,
            opcode,
            odu_name: odu.name.to_string(),
            universal_name: odu.universal_name.to_string(),
            prescription,
            verify,
            cadence,
        }
    }

    /// Compile a resolved Odù directly to a `CompiledAction`.
    ///
    /// This is the primary entry point for wiring divination into the
    /// ActionTransaction pipeline. Pass the compiled action to
    /// `ActionTransaction::new()` to begin execution.
    pub fn compile_for_odu(odu_index: u8) -> Result<CompiledAction, CompileError> {
        let directive = Self::directive_for(odu_index);
        ActionCompiler::compile(
            directive.vessel,
            &directive.opcode,
            &directive.prescription,
            &directive.verify,
            &directive.cadence,
        )
    }

    /// Get the vessel index (top nibble) for an Odù.
    pub fn vessel_for(odu_index: u8) -> u8 {
        odu_index >> 4
    }

    /// Get the modifier index (bottom nibble) for an Odù.
    pub fn modifier_for(odu_index: u8) -> u8 {
        odu_index & 0x0F
    }

    /// Human-readable opcode label.
    pub fn opcode_for(odu_index: u8) -> String {
        let v = Self::vessel_for(odu_index) as usize;
        let m = Self::modifier_for(odu_index) as usize;
        format!("{}:{}", VESSEL_PROFILES[v].name, VESSEL_PROFILES[m].name)
    }

    /// The vessel name for a given vessel index.
    pub fn vessel_name(vessel_idx: u8) -> &'static str {
        VESSEL_PROFILES[vessel_idx as usize & 0x0F].name
    }

    // ── private helpers ───────────────────────────────────────────────────────

    fn build_verify(vp: &VesselProfile, odu_index: u8) -> Vec<VerifySpec> {
        match vp.verify_kind {
            "exit_code" => vec![VerifySpec {
                kind: "exit_code".to_string(),
                path: None,
                expected: None,
                key: None,
                expected_exit_code: Some(0),
                actual_exit_code: None,
            }],
            "file_exists" => vec![VerifySpec {
                kind: "file_exists".to_string(),
                path: Some(format!("/tmp/omokoda_odu_{odu_index}_output")),
                expected: None,
                key: None,
                expected_exit_code: None,
                actual_exit_code: None,
            }],
            "file_contains" => vec![VerifySpec {
                kind: "file_contains".to_string(),
                path: Some(format!("/tmp/omokoda_odu_{odu_index}_output")),
                expected: Some("completed".to_string()),
                key: None,
                expected_exit_code: None,
                actual_exit_code: None,
            }],
            _ => vec![VerifySpec {
                kind: "exit_code".to_string(),
                path: None,
                expected: None,
                key: None,
                expected_exit_code: Some(0),
                actual_exit_code: None,
            }],
        }
    }

    fn build_cadence(vp: &VesselProfile) -> CadenceSpec {
        CadenceSpec {
            trigger: vp.trigger.to_string(),
            cooldown_secs: if vp.cooldown_secs > 0 { Some(vp.cooldown_secs) } else { None },
            max_per_window: vp.max_per_window,
            window_secs: vp.window_secs,
            deadline_secs: vp.deadline_secs,
        }
    }
}

// ─── ActionVessel → description (for prompt injection) ───────────────────────

/// Get the operational description for an ActionVessel.
pub fn vessel_description(vessel: ActionVessel) -> &'static str {
    match vessel {
        ActionVessel::Genesis   => "Initialize identity, covenant, and foundational state",
        ActionVessel::Void      => "Clear, release, and dissolve what no longer serves",
        ActionVessel::Attention => "Focus signal from noise; observe and classify",
        ActionVessel::Loop      => "Iterate recurring patterns; scheduled execution",
        ActionVessel::Receipt   => "Record, account, audit — create the tamper-evident trail",
        ActionVessel::Mask      => "Apply privacy boundaries; seal sensitive state",
        ActionVessel::Residue   => "Emit quiet telemetry; maintain the baseline",
        ActionVessel::Execution => "Execute the primary directive with full authority",
        ActionVessel::Swarm     => "Coordinate peer agents; distribute and converge",
        ActionVessel::Restraint => "Evaluate before acting; deliberate, measured hold",
        ActionVessel::Migration => "Transform and move state; adapt structure",
        ActionVessel::Consent   => "Request and record ratified permission",
        ActionVessel::Vision    => "Survey the environment; compile the full picture",
        ActionVessel::Growth    => "Integrate new knowledge; expand capability",
        ActionVessel::Seal      => "Cryptographically finalize; anchor proof on-chain",
        ActionVessel::Rhythm    => "Synchronize with system cadence; align cycles",
    }
}

/// Build the Odù context block to inject into a Think prompt.
///
/// Returns a multi-line string describing the agent's current Odù state
/// and its operational implication. Pass the Odù index derived from
/// `FieldDiviner::cast()` or `divination::dominant_glyph_byte()`.
pub fn odu_prompt_context(odu_index: u8) -> String {
    let odu = get_odu(odu_index);
    let directive = CalabashDispatcher::directive_for(odu_index);
    let vessel_desc = vessel_description(odu.vessel);

    format!(
        "═══ Odù Context ═══\n\
         Current Odù: {} ({})\n\
         Vessel: {} — {}\n\
         Archetype: {}\n\
         Operational Directive: {}\n\
         Agent Prescription:\n{}\n\
         ═══════════════════",
        odu.name,
        odu.universal_name,
        directive.vessel,
        vessel_desc,
        odu.archetype,
        directive.opcode,
        directive.prescription
            .lines()
            .map(|l| format!("  {l}"))
            .collect::<Vec<_>>()
            .join("\n"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn directive_derives_correct_vessel_and_modifier() {
        // index 0x17 = 23: vessel=1 (void), modifier=7 (execution)
        let d = CalabashDispatcher::directive_for(0x17);
        assert_eq!(d.vessel, 1);
        assert_eq!(d.modifier, 7);
        assert_eq!(d.opcode, "void:execution");
    }

    #[test]
    fn directive_prescription_is_two_steps() {
        let d = CalabashDispatcher::directive_for(0x70); // execution:genesis
        let lines: Vec<&str> = d.prescription.lines().collect();
        assert_eq!(lines.len(), 2);
        assert!(lines[0].starts_with("1."));
        assert!(lines[1].starts_with("2."));
    }

    #[test]
    fn compile_for_all_256_odu_succeeds() {
        for i in 0u8..=255 {
            let result = CalabashDispatcher::compile_for_odu(i);
            assert!(result.is_ok(), "failed to compile Odù {i}: {:?}", result.err());
        }
    }

    #[test]
    fn opcode_format_is_vessel_colon_modifier() {
        for i in [0u8, 16, 128, 255] {
            let opcode = CalabashDispatcher::opcode_for(i);
            assert!(opcode.contains(':'), "opcode '{opcode}' for odu {i} missing ':'");
        }
    }

    #[test]
    fn vessel_name_covers_all_16() {
        for i in 0u8..16 {
            let name = CalabashDispatcher::vessel_name(i);
            assert!(!name.is_empty(), "vessel {i} has no name");
        }
    }

    #[test]
    fn odu_prompt_context_includes_vessel_and_prescription() {
        let ctx = odu_prompt_context(7); // execution:execution
        assert!(ctx.contains("Odù Context"));
        assert!(ctx.contains("Vessel:"));
        assert!(ctx.contains("Prescription:"));
    }

    #[test]
    fn genesis_genesis_is_correct_vessel() {
        let d = CalabashDispatcher::directive_for(0);
        assert_eq!(d.vessel, 0); // genesis
        assert_eq!(d.modifier, 0); // genesis modifier
        assert_eq!(d.opcode, "genesis:genesis");
    }

    #[test]
    fn execution_execution_is_vessel_7() {
        let d = CalabashDispatcher::directive_for(0x77); // 119
        assert_eq!(d.vessel, 7);
        assert_eq!(d.modifier, 7);
        assert_eq!(d.opcode, "execution:execution");
    }

    #[test]
    fn rhythm_rhythm_is_last_vessel() {
        let d = CalabashDispatcher::directive_for(255); // 0xFF
        assert_eq!(d.vessel, 15); // rhythm
        assert_eq!(d.modifier, 15); // rhythm modifier
        assert_eq!(d.opcode, "rhythm:rhythm");
    }
}
