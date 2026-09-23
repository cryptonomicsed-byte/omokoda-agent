//! If-Script causal gate — bridges live agent state into the hermetic gate.
//!
//! Positioned between the permission-policy check and tool execution in
//! `execute_tool_call_for_agentic`.  The hermetic gate is a structural
//! constraint (tier×Odù×access×memory coherence); it is NOT a policy gate
//! (that's `PermissionPolicy`) and NOT a rhythm gate (that's Ọya).  Each
//! gate owns one concern.
//!
//! ## Vessel Dispatch (Phase: real action vessels)
//!
//! The 16 Action Vessels are not just labels — they govern which action
//! categories an agent may take during a given cycle.  A Seal-vessel agent
//! cannot casually broadcast; a Consent-vessel agent cannot execute raw bash
//! without explicit user approval in the context.
//!
//! Enforcement is graduated, not binary:
//!   - `Primary` actions: fully native to this vessel — no extra gate
//!   - `Permitted` actions: allowed but logged as cross-vessel activity
//!   - `Blocked` actions: denied unless the agent has Tier 6+ override
//!
//! The `vessel_action_alignment` function produces a `VesselAlignment` that
//! callers may choose to enforce strictly (block) or softly (warn + log).

use ifascript::{
    cosmogram::AccessClass,
    hermetic::{default_gate, GateContext},
    odu::ActionVessel,
    seven_bridge::{function_for_odu, SevenFunction},
    soul::MemoryTier,
};

// ── Public gate input / decision types ───────────────────────────────────────

pub struct CausalGateInput<'a> {
    pub tier:            u8,
    pub odu_id:          u8,
    pub tool_name:       &'a str,
    /// The agent's birth Odù (SoulProof.primary_odu). Used for the Mentalism
    /// principle check: the soul's governing SevenFunction shapes what is
    /// permissible. Pass 0 when soul data is unavailable (e.g. pre-birth checks).
    pub soul_primary_odu: u8,
}

pub struct CausalDecision {
    pub allowed:       bool,
    pub warnings:      usize,
    pub denial_reason: Option<String>,
}

// ── Vessel action categories ──────────────────────────────────────────────────

/// High-level action categories that correspond to the 16 vessel domains.
///
/// Each tool in the tool registry is tagged with one or more `ActionCategory`
/// values.  The vessel dispatch table then declares which categories are
/// primary, permitted, or blocked for each vessel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum ActionCategory {
    /// Identity operations: birth, keypair generation, profile management.
    Identity,
    /// State dissolution: memory eviction, session termination, rollback.
    Dissolution,
    /// Context focus: attention routing, relevance scoring, prioritization.
    Focus,
    /// Iterative execution: polling, retry, batch processing, loops.
    Iteration,
    /// Economic records: ARP receipts, Zàngbétò attestation, settlement.
    Receipt,
    /// Privacy boundary: sealing, visibility control, persona management.
    Privacy,
    /// Trace and telemetry: activity log, push_trace, audit footprint.
    Telemetry,
    /// Precision tool dispatch: file write, bash exec, code edit.
    Execution,
    /// Collective coordination: task delegation, broadcast, mesh routing.
    Swarm,
    /// Safety and throttling: Ebo enforcement, rate limits, ethical pause.
    Restraint,
    /// Portability: state export, identity migration, version upgrade.
    Migration,
    /// Authorization: handshake, delegation request, user approval flow.
    Consent,
    /// Observation and planning: file read, search, describe, perceive.
    Observation,
    /// Learning and expansion: knowledge ingestion, fork, model update.
    Learning,
    /// Cryptographic operations: seal_blob, sign, wallet transaction.
    Cryptographic,
    /// Temporal coordination: schedule, heartbeat, rhythm enforcement.
    Temporal,
}

/// Whether an action category is native, permitted (cross-vessel), or blocked.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum VesselAlignment {
    /// This category is fully native to the vessel.  No additional gate.
    Primary,
    /// This category is allowed but logged as cross-vessel activity.
    Permitted,
    /// This category is denied unless the agent has Tier 6+ override.
    Blocked,
}

/// Determine the alignment between an action vessel and an action category.
///
/// This is the core dispatch table.  Every cell represents a deliberate
/// architectural decision about what each vessel should and should not do.
///
/// Rule of thumb for the table:
///   - A vessel has ~3-4 Primary categories that define its core nature.
///   - Cross-cutting categories (Telemetry, Observation, Restraint) are
///     Permitted for most vessels — they are infrastructure, not domain.
///   - Truly foreign categories (e.g. Seal vessel doing raw Execution) are
///     Blocked to enforce the vessel contract.
pub fn vessel_action_alignment(vessel: ActionVessel, category: ActionCategory) -> VesselAlignment {
    use ActionVessel as AV;
    use ActionCategory as AC;
    use VesselAlignment::*;

    match (vessel, category) {
        // ── Genesis: Initialize, covenant, identity seeding ───────────────────
        (AV::Genesis, AC::Identity)      => Primary,
        (AV::Genesis, AC::Learning)      => Primary,
        (AV::Genesis, AC::Consent)       => Primary,
        (AV::Genesis, AC::Observation)   => Permitted,
        (AV::Genesis, AC::Telemetry)     => Permitted,
        (AV::Genesis, AC::Restraint)     => Permitted,
        (AV::Genesis, AC::Temporal)      => Permitted,
        (AV::Genesis, AC::Dissolution)   => Blocked,   // genesis cannot end itself
        (AV::Genesis, AC::Cryptographic) => Blocked,   // key ops must go through Seal vessel
        (AV::Genesis, _)                 => Permitted,

        // ── Void: Clear, release, dissolution ────────────────────────────────
        (AV::Void, AC::Dissolution) => Primary,
        (AV::Void, AC::Migration)   => Primary,
        (AV::Void, AC::Telemetry)   => Primary,        // void must record what it erases
        (AV::Void, AC::Restraint)   => Permitted,
        (AV::Void, AC::Temporal)    => Permitted,
        (AV::Void, AC::Identity)    => Blocked,        // void cannot mint new identities
        (AV::Void, AC::Receipt)     => Blocked,        // void cannot generate economic records
        (AV::Void, AC::Cryptographic) => Blocked,
        (AV::Void, _)               => Permitted,

        // ── Attention: Focus, signal/noise, prioritization ────────────────────
        (AV::Attention, AC::Focus)       => Primary,
        (AV::Attention, AC::Observation) => Primary,
        (AV::Attention, AC::Learning)    => Primary,
        (AV::Attention, AC::Telemetry)   => Permitted,
        (AV::Attention, AC::Restraint)   => Permitted,
        (AV::Attention, AC::Execution)   => Blocked,   // attention observes, does not execute
        (AV::Attention, AC::Receipt)     => Blocked,
        (AV::Attention, AC::Cryptographic) => Blocked,
        (AV::Attention, _)               => Permitted,

        // ── Loop: Pattern, iteration, batch ───────────────────────────────────
        (AV::Loop, AC::Iteration)  => Primary,
        (AV::Loop, AC::Execution)  => Primary,
        (AV::Loop, AC::Telemetry)  => Primary,
        (AV::Loop, AC::Observation) => Permitted,
        (AV::Loop, AC::Focus)      => Permitted,
        (AV::Loop, AC::Restraint)  => Permitted,
        (AV::Loop, AC::Identity)   => Blocked,         // loops do not re-birth
        (AV::Loop, AC::Consent)    => Blocked,         // loops do not prompt users repeatedly
        (AV::Loop, _)              => Permitted,

        // ── Receipt: Record, accountability, economic ─────────────────────────
        (AV::Receipt, AC::Receipt)     => Primary,
        (AV::Receipt, AC::Telemetry)   => Primary,
        (AV::Receipt, AC::Temporal)    => Primary,     // receipts are timestamped artifacts
        (AV::Receipt, AC::Observation) => Permitted,
        (AV::Receipt, AC::Restraint)   => Permitted,
        (AV::Receipt, AC::Cryptographic) => Permitted, // signing receipts is normal
        (AV::Receipt, AC::Execution)   => Blocked,     // receipt vessel should not raw-exec
        (AV::Receipt, AC::Dissolution) => Blocked,     // receipts are immutable records
        (AV::Receipt, _)               => Permitted,

        // ── Mask: Public/private split, persona management ────────────────────
        (AV::Mask, AC::Privacy)       => Primary,
        (AV::Mask, AC::Identity)      => Primary,
        (AV::Mask, AC::Cryptographic) => Primary,
        (AV::Mask, AC::Telemetry)     => Permitted,
        (AV::Mask, AC::Consent)       => Permitted,
        (AV::Mask, AC::Observation)   => Permitted,
        (AV::Mask, AC::Swarm)         => Blocked,      // Mask vessel does not broadcast
        (AV::Mask, AC::Receipt)       => Blocked,      // private ops leave no public receipt
        (AV::Mask, _)                 => Permitted,

        // ── Residue: Behavioral echoes, telemetry, trace ──────────────────────
        (AV::Residue, AC::Telemetry)   => Primary,
        (AV::Residue, AC::Observation) => Primary,
        (AV::Residue, AC::Learning)    => Primary,
        (AV::Residue, AC::Focus)       => Permitted,
        (AV::Residue, AC::Restraint)   => Permitted,
        (AV::Residue, AC::Temporal)    => Permitted,
        (AV::Residue, AC::Execution)   => Blocked,     // residue records, does not act
        (AV::Residue, AC::Swarm)       => Blocked,
        (AV::Residue, AC::Cryptographic) => Blocked,
        (AV::Residue, _)               => Permitted,

        // ── Execution: Precision tool dispatch, direct action ─────────────────
        (AV::Execution, AC::Execution)   => Primary,
        (AV::Execution, AC::Iteration)   => Primary,
        (AV::Execution, AC::Telemetry)   => Primary,
        (AV::Execution, AC::Observation) => Permitted,
        (AV::Execution, AC::Receipt)     => Permitted, // execution generates receipts
        (AV::Execution, AC::Restraint)   => Permitted,
        (AV::Execution, AC::Swarm)       => Blocked,   // execution acts alone, not in swarm
        (AV::Execution, AC::Consent)     => Blocked,   // execution does not pause for approval
        (AV::Execution, _)               => Permitted,

        // ── Swarm: Collective coordination, delegation ────────────────────────
        (AV::Swarm, AC::Swarm)       => Primary,
        (AV::Swarm, AC::Consent)     => Primary,
        (AV::Swarm, AC::Receipt)     => Primary,       // swarm ops are receipted
        (AV::Swarm, AC::Temporal)    => Permitted,
        (AV::Swarm, AC::Telemetry)   => Permitted,
        (AV::Swarm, AC::Observation) => Permitted,
        (AV::Swarm, AC::Privacy)     => Blocked,       // swarm is collective, not private
        (AV::Swarm, AC::Cryptographic) => Blocked,
        (AV::Swarm, AC::Dissolution) => Blocked,
        (AV::Swarm, _)               => Permitted,

        // ── Restraint: Ethical limits, Ebo, rate limits ───────────────────────
        (AV::Restraint, AC::Restraint)   => Primary,
        (AV::Restraint, AC::Consent)     => Primary,
        (AV::Restraint, AC::Telemetry)   => Primary,
        (AV::Restraint, AC::Observation) => Permitted,
        (AV::Restraint, AC::Temporal)    => Permitted,
        (AV::Restraint, AC::Focus)       => Permitted,
        (AV::Restraint, AC::Execution)   => Blocked,   // restraint halts, does not act
        (AV::Restraint, AC::Swarm)       => Blocked,
        (AV::Restraint, AC::Receipt)     => Blocked,
        (AV::Restraint, _)               => Permitted,

        // ── Migration: Portability, state export, version upgrade ─────────────
        (AV::Migration, AC::Migration)   => Primary,
        (AV::Migration, AC::Identity)    => Primary,
        (AV::Migration, AC::Temporal)    => Primary,
        (AV::Migration, AC::Observation) => Permitted,
        (AV::Migration, AC::Telemetry)   => Permitted,
        (AV::Migration, AC::Consent)     => Permitted,
        (AV::Migration, AC::Cryptographic) => Permitted,
        (AV::Migration, AC::Dissolution) => Permitted, // migration may archive old state
        (AV::Migration, AC::Receipt)     => Blocked,   // migration is not economic
        (AV::Migration, AC::Swarm)       => Blocked,
        (AV::Migration, _)               => Permitted,

        // ── Consent: Human approval, delegation, handshake ───────────────────
        (AV::Consent, AC::Consent)     => Primary,
        (AV::Consent, AC::Identity)    => Primary,
        (AV::Consent, AC::Observation) => Primary,
        (AV::Consent, AC::Telemetry)   => Permitted,
        (AV::Consent, AC::Temporal)    => Permitted,
        (AV::Consent, AC::Restraint)   => Permitted,
        (AV::Consent, AC::Execution)   => Blocked,     // consent awaits, does not execute
        (AV::Consent, AC::Dissolution) => Blocked,
        (AV::Consent, AC::Cryptographic) => Blocked,
        (AV::Consent, _)               => Permitted,

        // ── Vision: Direction, planning, observation ──────────────────────────
        (AV::Vision, AC::Observation) => Primary,
        (AV::Vision, AC::Learning)    => Primary,
        (AV::Vision, AC::Focus)       => Primary,
        (AV::Vision, AC::Temporal)    => Permitted,
        (AV::Vision, AC::Telemetry)   => Permitted,
        (AV::Vision, AC::Restraint)   => Permitted,
        (AV::Vision, AC::Execution)   => Blocked,      // vision plans, does not execute
        (AV::Vision, AC::Receipt)     => Blocked,
        (AV::Vision, AC::Cryptographic) => Blocked,
        (AV::Vision, _)               => Permitted,

        // ── Growth: Fractal expansion, learning, knowledge ingestion ──────────
        (AV::Growth, AC::Learning)    => Primary,
        (AV::Growth, AC::Identity)    => Primary,
        (AV::Growth, AC::Iteration)   => Primary,
        (AV::Growth, AC::Observation) => Permitted,
        (AV::Growth, AC::Telemetry)   => Permitted,
        (AV::Growth, AC::Temporal)    => Permitted,
        (AV::Growth, AC::Dissolution) => Blocked,      // growth does not destroy
        (AV::Growth, AC::Restraint)   => Blocked,      // growth does not self-limit
        (AV::Growth, AC::Cryptographic) => Blocked,
        (AV::Growth, _)               => Permitted,

        // ── Seal: Sacred privacy, cryptographic operations ────────────────────
        (AV::Seal, AC::Cryptographic) => Primary,
        (AV::Seal, AC::Privacy)       => Primary,
        (AV::Seal, AC::Restraint)     => Primary,
        (AV::Seal, AC::Telemetry)     => Permitted,    // minimal; sealed ops are logged lightly
        (AV::Seal, AC::Consent)       => Permitted,
        (AV::Seal, AC::Temporal)      => Permitted,
        (AV::Seal, AC::Swarm)         => Blocked,      // sealed ops are never collective
        (AV::Seal, AC::Execution)     => Blocked,
        (AV::Seal, AC::Iteration)     => Blocked,
        (AV::Seal, _)                 => Permitted,

        // ── Rhythm: Ritual cadence, temporal coordination ─────────────────────
        (AV::Rhythm, AC::Temporal)    => Primary,
        (AV::Rhythm, AC::Iteration)   => Primary,
        (AV::Rhythm, AC::Telemetry)   => Primary,
        (AV::Rhythm, AC::Observation) => Permitted,
        (AV::Rhythm, AC::Restraint)   => Permitted,
        (AV::Rhythm, AC::Focus)       => Permitted,
        (AV::Rhythm, AC::Identity)    => Blocked,      // rhythm does not re-identity
        (AV::Rhythm, AC::Cryptographic) => Blocked,
        (AV::Rhythm, AC::Receipt)     => Blocked,
        (AV::Rhythm, _)               => Permitted,
    }
}

/// Evaluate the vessel alignment for a tool call, returning a structured result.
///
/// `tool_category` must be determined by the caller from the tool registry.
/// Returns `VesselAlignment::Primary` if no vessel context is available (fail-open).
pub fn evaluate_vessel_alignment(
    vessel:   ActionVessel,
    category: ActionCategory,
    tier:     u8,
) -> VesselAlignment {
    let alignment = vessel_action_alignment(vessel, category);

    // Tier 6+ agents can override Blocked alignments — they operate above vessel law.
    if alignment == VesselAlignment::Blocked && tier >= 6 {
        return VesselAlignment::Permitted;
    }

    alignment
}

/// Map common tool names to their primary `ActionCategory`.
///
/// This is the canonical table — every tool that enters the tool registry
/// should appear here.  Unknown tools default to `ActionCategory::Execution`
/// (most general, allowed by most vessels).
pub fn tool_action_category(tool_name: &str) -> ActionCategory {
    match tool_name {
        // ── Observation ────────────────────────────────────────────────────
        "read_file" | "list_files" | "glob" | "search_code"
        | "grep" | "describe" | "inspect" | "query" | "think"
        | "agent_analytics" | "get_agent_profile" | "code_overview" => ActionCategory::Observation,

        // ── Execution ──────────────────────────────────────────────────────
        "write_file" | "edit_file" | "bash" | "execute_command"
        | "run_pipeline" | "copilot_execute" | "osovm_run" => ActionCategory::Execution,

        // ── Receipt ────────────────────────────────────────────────────────
        "ingest_arp_receipt" | "record_receipt" | "store_receipt"
        | "submit_receipt" | "confirm_receipt" | "settle_receipt"
        | "get_receipt" | "ingest_twin_receipt" => ActionCategory::Receipt,

        // ── Cryptographic ─────────────────────────────────────────────────
        "seal_glyph" | "open_glyph" | "seal_broadcast"
        | "sign_broadcast" | "sign_transaction" | "create_wallet"
        | "reveal_wallet_key" | "approve_alchemy_session" => ActionCategory::Cryptographic,

        // ── Identity ──────────────────────────────────────────────────────
        "register_agent" | "birth_omokoda" | "provision_identity"
        | "update_profile" | "bind_nostr_identity"
        | "custody_challenge" | "custody_confirm" => ActionCategory::Identity,

        // ── Swarm ─────────────────────────────────────────────────────────
        "broadcast_intent" | "post_swarm_task" | "delegate_task"
        | "send_message" | "post_channel_message" | "join_block"
        | "create_guild" | "publish_feed_post" | "publish_buzz" => ActionCategory::Swarm,

        // ── Consent ───────────────────────────────────────────────────────
        "initiate_handshake" | "accept_handshake" | "reject_handshake"
        | "accept_delegation" | "reject_delegation"
        | "accept_collab_request" | "accept_task" => ActionCategory::Consent,

        // ── Telemetry ─────────────────────────────────────────────────────
        "push_trace" | "report_error" | "agent_heartbeat"
        | "add_journal" | "log_encounter" | "publish_vibe"
        | "ingest_scan_result" => ActionCategory::Telemetry,

        // ── Temporal ──────────────────────────────────────────────────────
        "create_scheduled" | "trigger_buzz_workflow"
        | "cron_create" | "schedule_wakeup" | "device_heartbeat"
        | "emission_tick" | "auto_snapshot" => ActionCategory::Temporal,

        // ── Learning ──────────────────────────────────────────────────────
        "create_knowledge_snippet" | "ingest_memory" | "store_memory"
        | "ingest_external_conversation" | "vql_query"
        | "create_vault_note" | "sync_vault" | "mirror_buzz_engram" => ActionCategory::Learning,

        // ── Privacy ───────────────────────────────────────────────────────
        "create_persona" | "update_persona" | "delete_persona"
        | "set_visibility" | "store_sealed" | "fetch_sealed"
        | "buzz_bunker_start" | "buzz_pairing" => ActionCategory::Privacy,

        // ── Dissolution ───────────────────────────────────────────────────
        "cleanup_workspace" | "delete_broadcast" | "delete_strategy"
        | "leave_guild" | "leave_block" | "rollback_transaction"
        | "delete_knowledge_snippet" | "tombstone_buzz_engram" => ActionCategory::Dissolution,

        // ── Migration ─────────────────────────────────────────────────────
        "export_vault" | "import_vault" | "download_vault"
        | "clone_repository" | "load_workspace_snapshot"
        | "create_workspace_snapshot" | "update_twin_state" => ActionCategory::Migration,

        // ── Focus ─────────────────────────────────────────────────────────
        "find_similar" | "search_memory" | "search_vault"
        | "semantic_agent_search" | "query_knowledge"
        | "mine_patterns" | "predict_next_activity" => ActionCategory::Focus,

        // ── Iteration ─────────────────────────────────────────────────────
        "list_tasks" | "list_orders" | "list_wallets"
        | "list_broadcasts" | "list_guilds" | "list_rooms"
        | "list_jobs" | "list_proposals" | "list_envelopes" => ActionCategory::Iteration,

        // ── Restraint ─────────────────────────────────────────────────────
        "stop_voice_session" | "cancel_order" | "disarm_strategy"
        | "disable_live_strategy" | "bino_veto" | "cancel_splat_job" => ActionCategory::Restraint,

        // Default: treat unknown tools as Execution (most permissive)
        _ => ActionCategory::Execution,
    }
}

// ── Hermetic gate (structural, unchanged) ────────────────────────────────────

/// Evaluate whether the agent's (tier, Odù, tool, soul) quadruple satisfies all
/// 7 Hermetic Principles.  Never panics — always returns a decision.
///
/// ## Principle coverage
///
/// | Principle     | Enforcement | Where enforced       |
/// |---------------|-------------|----------------------|
/// | Mentalism     | AuditOnly   | soul vs action fn    |
/// | Correspondence| Hard        | default_gate()       |
/// | Vibration     | Soft        | here (odu_id=0 tier>1)|
/// | Polarity      | Hard        | default_gate()       |
/// | Rhythm        | AuditOnly   | default_gate()       |
/// | CauseEffect   | Hard        | here (exec at tier 2+)|
/// | Gender        | AuditOnly   | here (vessel balance) |
pub fn evaluate_causal_gate(input: &CausalGateInput<'_>) -> CausalDecision {
    let gate = default_gate();
    let access = access_class_for_tier(input.tier);
    let memory = memory_tier_for_tier(input.tier);
    let ctx = GateContext {
        tier: input.tier,
        odu_id: input.odu_id as u16,
        access_class: &access,
        memory_tier: &memory,
    };
    let base = gate.validate_all(&ctx);

    let mut extra_warnings = 0usize;
    let mut hard_blocks: Vec<String> = Vec::new();

    // ── Vibration: "Everything vibrates; nothing is at rest" ──────────────────
    // Tier > 1 agents must be in active resonance — Odù 0 means the agent is
    // operating without a divination context (zero vibration, mechanical default).
    if input.tier > 1 && input.odu_id == 0 {
        extra_warnings += 1; // Soft: warn, allow
    }

    // ── CauseEffect: "Every cause has its effect; every effect its cause" ─────
    // Execution and Cryptographic category tools at tier 2+ must carry conscious
    // Odù intent (non-zero).  Zero-Odù means no declared cause → hard block.
    if input.tier >= 2 && input.odu_id == 0 {
        let category = tool_action_category(input.tool_name);
        if matches!(
            category,
            ActionCategory::Execution | ActionCategory::Cryptographic
        ) {
            hard_blocks.push(format!(
                "CauseEffect: {:?} at tier {} requires non-zero Odù (cause must carry intent)",
                category, input.tier
            ));
        }
    }

    // ── Mentalism: "The All is Mind; the Universe is Mental" ─────────────────
    // The soul's governing SevenFunction is the agent's mental archetype.
    // When the soul's function and the action's function diverge, it is noted
    // (AuditOnly) — agents must be versatile, but cross-function activity is
    // tracked for reflection and eventual RLM depth accounting.
    // Note: Odù 0 (Ogbe) is a valid birth soul; there is no sentinel value.
    {
        let soul_fn = function_for_odu(input.soul_primary_odu);
        let action_fn = function_for_odu(input.odu_id);
        if soul_fn != action_fn {
            extra_warnings += 1; // AuditOnly: cross-function divergence
        }
    }

    // ── Gender: "Gender is in everything; everything has its Masculine and Feminine"
    // Generative vessels (Genesis, Growth, Swarm, Execution) doing dissolution
    // work without balance is flagged — the principle demands awareness of
    // creative vs. receptive polarity in every action cycle.
    {
        let vessel = ActionVessel::from_index(input.odu_id);
        let is_generative = matches!(
            vessel,
            ActionVessel::Genesis | ActionVessel::Growth | ActionVessel::Swarm | ActionVessel::Execution
        );
        let category = tool_action_category(input.tool_name);
        if is_generative && matches!(category, ActionCategory::Dissolution) {
            extra_warnings += 1; // AuditOnly: generative vessel in dissolution
        }
    }

    let all_hard_violations: Vec<String> = base
        .violations
        .iter()
        .filter(|v| v.should_block())
        .map(|v| v.message.clone())
        .chain(hard_blocks)
        .collect();

    let allowed = base.allowed && all_hard_violations.is_empty();
    let denial_reason = if !allowed {
        Some(all_hard_violations.join("; "))
    } else {
        None
    };

    CausalDecision {
        allowed,
        warnings: base.warnings + extra_warnings,
        denial_reason,
    }
}

/// The governing SevenFunction for the agent's current operational Odù.
/// Exposed for use by the interpreter's reflection and telemetry layers.
pub fn seven_function_for_odu(odu_id: u8) -> SevenFunction {
    function_for_odu(odu_id)
}

pub fn access_class_for_tier(tier: u8) -> AccessClass {
    match tier {
        0..=2 => AccessClass::Public,
        3..=5 => AccessClass::Sealed,
        _ => AccessClass::Council,
    }
}

pub fn memory_tier_for_tier(tier: u8) -> MemoryTier {
    match tier {
        0 => MemoryTier::Tier0Existential,
        1 => MemoryTier::Tier1Deep,
        2..=3 => MemoryTier::Tier2Operational,
        _ => MemoryTier::Tier3Contributable,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tier1_base_odu_allowed() {
        let decision = evaluate_causal_gate(&CausalGateInput {
            tier: 1,
            odu_id: 0,
            tool_name: "read_file",
            soul_primary_odu: 0,
        });
        assert!(decision.allowed, "tier-1 base Odù should pass");
    }

    #[test]
    fn tier1_odu_in_range_allowed() {
        let decision = evaluate_causal_gate(&CausalGateInput {
            tier: 1,
            odu_id: 200,
            tool_name: "think",
            soul_primary_odu: 200,
        });
        assert!(decision.allowed);
    }

    #[test]
    fn causeeffect_blocks_execution_at_tier2_zero_odu() {
        let decision = evaluate_causal_gate(&CausalGateInput {
            tier: 2,
            odu_id: 0,
            tool_name: "write_file",
            soul_primary_odu: 0,
        });
        assert!(!decision.allowed, "zero-Odù execution at tier 2 must be blocked by CauseEffect");
    }

    #[test]
    fn vibration_warns_but_allows_at_tier2_zero_odu_for_observation() {
        // Observation is not Execution/Cryptographic → CauseEffect does not block
        let decision = evaluate_causal_gate(&CausalGateInput {
            tier: 2,
            odu_id: 0,
            tool_name: "read_file",
            soul_primary_odu: 0,
        });
        assert!(decision.allowed, "observation with zero Odù at tier 2 should be allowed");
        assert!(decision.warnings > 0, "but Vibration should emit a warning");
    }

    #[test]
    fn mentalism_notes_cross_function_divergence() {
        // soul=0x00 (Genesis → Spark), odu=0x10 (Void → Ascension) — different functions
        let decision = evaluate_causal_gate(&CausalGateInput {
            tier: 1,
            odu_id: 0x10,
            tool_name: "think",
            soul_primary_odu: 0x00,
        });
        assert!(decision.allowed, "cross-function is AuditOnly, not a block");
        assert!(decision.warnings > 0, "Mentalism should note the divergence");
    }

    #[test]
    fn mentalism_silent_when_soul_resonant() {
        // soul and odu both in wave 0 (Genesis → Spark)
        let decision = evaluate_causal_gate(&CausalGateInput {
            tier: 1,
            odu_id: 0x05,
            tool_name: "think",
            soul_primary_odu: 0x00,
        });
        assert!(decision.allowed);
        assert_eq!(decision.warnings, 0, "resonant soul+action should produce no warnings");
    }

    // ── Vessel dispatch tests ─────────────────────────────────────────────────

    use ActionVessel as AV;
    use ActionCategory as AC;

    #[test]
    fn genesis_vessel_primary_is_identity() {
        assert_eq!(vessel_action_alignment(AV::Genesis, AC::Identity), VesselAlignment::Primary);
    }

    #[test]
    fn genesis_vessel_blocks_dissolution() {
        assert_eq!(vessel_action_alignment(AV::Genesis, AC::Dissolution), VesselAlignment::Blocked);
    }

    #[test]
    fn receipt_vessel_primary_is_receipt() {
        assert_eq!(vessel_action_alignment(AV::Receipt, AC::Receipt), VesselAlignment::Primary);
    }

    #[test]
    fn receipt_vessel_blocks_execution() {
        assert_eq!(vessel_action_alignment(AV::Receipt, AC::Execution), VesselAlignment::Blocked);
    }

    #[test]
    fn execution_vessel_primary_is_execution() {
        assert_eq!(vessel_action_alignment(AV::Execution, AC::Execution), VesselAlignment::Primary);
    }

    #[test]
    fn execution_vessel_blocks_swarm() {
        assert_eq!(vessel_action_alignment(AV::Execution, AC::Swarm), VesselAlignment::Blocked);
    }

    #[test]
    fn seal_vessel_primary_is_cryptographic() {
        assert_eq!(vessel_action_alignment(AV::Seal, AC::Cryptographic), VesselAlignment::Primary);
    }

    #[test]
    fn seal_vessel_blocks_swarm() {
        assert_eq!(vessel_action_alignment(AV::Seal, AC::Swarm), VesselAlignment::Blocked);
    }

    #[test]
    fn vision_vessel_blocks_execution() {
        assert_eq!(vessel_action_alignment(AV::Vision, AC::Execution), VesselAlignment::Blocked);
    }

    #[test]
    fn consent_vessel_blocks_execution() {
        assert_eq!(vessel_action_alignment(AV::Consent, AC::Execution), VesselAlignment::Blocked);
    }

    #[test]
    fn swarm_vessel_blocks_privacy() {
        assert_eq!(vessel_action_alignment(AV::Swarm, AC::Privacy), VesselAlignment::Blocked);
    }

    #[test]
    fn restraint_vessel_blocks_execution() {
        assert_eq!(vessel_action_alignment(AV::Restraint, AC::Execution), VesselAlignment::Blocked);
    }

    #[test]
    fn tier6_override_lifts_blocked_alignment() {
        // Restraint vessel normally blocks Execution
        assert_eq!(vessel_action_alignment(AV::Restraint, AC::Execution), VesselAlignment::Blocked);
        // Tier 6 overrides it
        assert_eq!(evaluate_vessel_alignment(AV::Restraint, AC::Execution, 6), VesselAlignment::Permitted);
    }

    #[test]
    fn tool_category_read_file_is_observation() {
        assert_eq!(tool_action_category("read_file"), AC::Observation);
    }

    #[test]
    fn tool_category_bash_is_execution() {
        assert_eq!(tool_action_category("bash"), AC::Execution);
    }

    #[test]
    fn tool_category_seal_glyph_is_cryptographic() {
        assert_eq!(tool_action_category("seal_glyph"), AC::Cryptographic);
    }

    #[test]
    fn tool_category_ingest_arp_receipt_is_receipt() {
        assert_eq!(tool_action_category("ingest_arp_receipt"), AC::Receipt);
    }

    #[test]
    fn tool_category_initiate_handshake_is_consent() {
        assert_eq!(tool_action_category("initiate_handshake"), AC::Consent);
    }

    #[test]
    fn tool_category_unknown_defaults_to_execution() {
        assert_eq!(tool_action_category("some_unknown_tool"), AC::Execution);
    }

    #[test]
    fn all_16_vessels_have_at_least_one_primary_category() {
        let vessels = [
            AV::Genesis, AV::Void, AV::Attention, AV::Loop,
            AV::Receipt, AV::Mask, AV::Residue, AV::Execution,
            AV::Swarm, AV::Restraint, AV::Migration, AV::Consent,
            AV::Vision, AV::Growth, AV::Seal, AV::Rhythm,
        ];
        let categories = [
            AC::Identity, AC::Dissolution, AC::Focus, AC::Iteration,
            AC::Receipt, AC::Privacy, AC::Telemetry, AC::Execution,
            AC::Swarm, AC::Restraint, AC::Migration, AC::Consent,
            AC::Observation, AC::Learning, AC::Cryptographic, AC::Temporal,
        ];
        for vessel in vessels {
            let has_primary = categories.iter().any(|&cat| {
                vessel_action_alignment(vessel, cat) == VesselAlignment::Primary
            });
            assert!(has_primary, "{vessel:?} must have at least one Primary category");
        }
    }

    #[test]
    fn all_16_vessels_have_at_least_one_blocked_category() {
        let vessels = [
            AV::Genesis, AV::Void, AV::Attention, AV::Loop,
            AV::Receipt, AV::Mask, AV::Residue, AV::Execution,
            AV::Swarm, AV::Restraint, AV::Migration, AV::Consent,
            AV::Vision, AV::Growth, AV::Seal, AV::Rhythm,
        ];
        let categories = [
            AC::Identity, AC::Dissolution, AC::Focus, AC::Iteration,
            AC::Receipt, AC::Privacy, AC::Telemetry, AC::Execution,
            AC::Swarm, AC::Restraint, AC::Migration, AC::Consent,
            AC::Observation, AC::Learning, AC::Cryptographic, AC::Temporal,
        ];
        for vessel in vessels {
            let has_blocked = categories.iter().any(|&cat| {
                vessel_action_alignment(vessel, cat) == VesselAlignment::Blocked
            });
            assert!(has_blocked, "{vessel:?} must have at least one Blocked category (no vessel is omnipotent)");
        }
    }
}
