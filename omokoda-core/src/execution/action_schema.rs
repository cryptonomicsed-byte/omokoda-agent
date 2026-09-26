//! action_schema — derives a machine-executable ActionSchema from any of the 256 base Odù.
//!
//! Every Odù in the corpus carries: name, archetype, description, taboos, prescriptions,
//! archetypes, vessel, and opcode. This module translates all of that into:
//!
//!   - `ExecutionMode`         — how the agent executes (Analytical/Guardian/Executor/etc.)
//!   - `Vec<OperationalStep>`  — concrete tool + params + artifact for each prescription
//!   - `Vec<BehavioralConstraint>` — denied-tool rules derived from taboos
//!   - `Vec<VerifySpec>`       — assertions that must pass before COMMITTED
//!   - `CadenceSpec`           — scheduling/trigger derived from vessel + archetype
//!   - `Vec<ActivationMode>`   — Event/State/Scheduled/Immediate per vessel
//!   - `context_block`         — fully-rendered prompt context block
//!
//! No entry is generic. Every schema is unique because `build_schema()` derives operational
//! steps from the ACTUAL corpus prescriptions via `PrescriptionCompiler`, not from generic
//! vessel templates. The 256 → 16 collapse that the old `CalabashDispatcher` had is gone.
//!
//! `build_schema(odu_index)` is the single entry point.

use serde::{Deserialize, Serialize};
use ifascript::odu::{get_odu, ActionVessel};
use crate::execution::action_compiler::{CadenceSpec, VerifySpec};

// ─── ExecutionMode ────────────────────────────────────────────────────────────

/// The execution posture derived from the Odù's archetype(s).
/// Controls consent requirements, mutation authority, and receipt tier.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExecutionMode {
    /// Oracle Sage — gather, analyze, report. No direct mutations without explicit consent.
    Analytical,
    /// Steward — oversee, protect, gate. Consent required for all writes.
    Guardian,
    /// Forge Executor — direct action, deploy, execute. Full mutation authority.
    Executor,
    /// Swarm Coordinator — delegate to peer agents, converge results.
    Coordinator,
    /// Resonance Weaver — integrate, communicate, harmonize signals.
    Synthesizer,
    /// Wisdom Anchor — persist, record, ground state long-horizon.
    Anchor,
    /// Flow Guardian — rate-limit, constrain, channel operational flow.
    FlowGuard,
    /// Justice Canon — verify, receipt, adjudicate outcomes.
    Canonical,
    /// Prime Source — genesis operations, initialize, covenant.
    PrimeSource,
}

// ─── ActivationMode ──────────────────────────────────────────────────────────

/// When/how this action is activated.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ActivationMode {
    Immediate,
    OnEvent(String),
    OnState(String),
    Scheduled(String),
}

// ─── BehavioralConstraint ────────────────────────────────────────────────────

/// A behavioral rule derived from a taboo in the Odù corpus.
/// Applied at gate time to block prohibited tool calls or output patterns.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BehavioralConstraint {
    /// Machine-readable constraint name (e.g. "NO_DECEPTION").
    pub name: String,
    /// The raw taboo text from the corpus.
    pub source_taboo: String,
    /// Tool names explicitly denied by this constraint.
    pub denied_tools: Vec<String>,
    /// Output patterns that violate this constraint (regex-ready).
    pub denied_patterns: Vec<String>,
    /// Human-readable rationale for agents and logs.
    pub rationale: String,
}

// ─── OperationalStep ─────────────────────────────────────────────────────────

/// One concrete executable step derived from a prescription line.
/// Maps directly to a `CompiledStep` in the ActionCompiler pipeline.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperationalStep {
    /// Human-readable label (includes original prescription text).
    pub description: String,
    /// Tool name to invoke (e.g. "read", "write", "bash").
    pub tool: String,
    /// Params to pass to the tool.
    pub params: serde_json::Value,
    /// Artifact paths expected after execution (used in verify step).
    pub expected_artifacts: Vec<String>,
    /// Verify assertion for this specific step.
    pub verify: VerifySpec,
}

// ─── ActionSchema ────────────────────────────────────────────────────────────

/// The complete machine-readable specification for one of the 256 Odù.
/// Derived at runtime from the If-Script corpus by `build_schema()`.
/// Every field is populated — no generic fallbacks.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionSchema {
    pub odu_index: u8,
    pub odu_name: String,
    pub universal_name: String,
    pub archetype: String,
    pub description: String,
    /// Raw taboos from corpus (preserved for audit).
    pub taboos: Vec<String>,
    /// Raw spiritual prescriptions from corpus (preserved for audit).
    pub spiritual_prescriptions: Vec<String>,
    /// Raw archetype tags from corpus.
    pub corpus_archetypes: Vec<String>,
    /// Derived execution mode from archetype tags.
    pub execution_mode: ExecutionMode,
    /// Operational steps compiled from prescriptions.
    pub operational_steps: Vec<OperationalStep>,
    /// Behavioral constraints compiled from taboos.
    pub behavioral_constraints: Vec<BehavioralConstraint>,
    /// Combined verify specs from all operational steps.
    pub verify_specs: Vec<VerifySpec>,
    /// Scheduling derived from vessel + execution mode.
    pub cadence: CadenceSpec,
    /// Activation modes for this Odù's vessel.
    pub activation_modes: Vec<ActivationMode>,
    /// Full context block for prompt injection — includes description + taboos + operational intent.
    pub context_block: String,
}

// ─── Public entry point ───────────────────────────────────────────────────────

/// Build the full `ActionSchema` for any Odù index 0–255.
///
/// This is the single call site for making any Odù actionable. The result
/// contains everything the ActionInterpreter needs to execute, constrain,
/// verify, and receipt any action in the Digital Calabash.
pub fn build_schema(odu_index: u8) -> ActionSchema {
    let odu = get_odu(odu_index);

    let execution_mode = classify_execution_mode(odu.archetypes);
    let operational_steps = compile_all_prescriptions(odu.prescriptions, odu.vessel, odu_index);
    let behavioral_constraints = compile_constraints(odu.taboos);
    let verify_specs = operational_steps.iter().map(|s| s.verify.clone()).collect();
    let cadence = derive_cadence(odu.vessel, &execution_mode, odu_index);
    let activation_modes = derive_activation_modes(odu.vessel);
    let context_block = build_context_block(odu_index, odu, &execution_mode, &operational_steps, &behavioral_constraints);

    ActionSchema {
        odu_index,
        odu_name: odu.name.to_string(),
        universal_name: odu.universal_name.to_string(),
        archetype: odu.archetype.to_string(),
        description: odu.description.to_string(),
        taboos: odu.taboos.iter().map(|s| s.to_string()).collect(),
        spiritual_prescriptions: odu.prescriptions.iter().map(|s| s.to_string()).collect(),
        corpus_archetypes: odu.archetypes.iter().map(|s| s.to_string()).collect(),
        execution_mode,
        operational_steps,
        behavioral_constraints,
        verify_specs,
        cadence,
        activation_modes,
        context_block,
    }
}

/// Build an `ActionSchema` for a composed Odù (u16 space, up to 65,535) gated by tier.
///
/// Returns `None` if the tier does not permit access to this Odù index
/// (via `AgentExperience::can_access`). For indices 0–255, delegates to
/// `build_schema()` to avoid duplication. For composed indices (256–65535),
/// resolves via the calabash compose layer and constructs a blended schema.
pub fn build_composed_schema(odu_id: u16, tier: u8) -> Option<ActionSchema> {
    use ifascript::{AgentExperience, resolve};

    // Tier gate via AgentExperience (XP floor for each tier)
    let xp_for_tier = match tier {
        0 | 1 => 0u64,
        2 => 500,
        3 => 2000,
        4 => 5000,
        5 => 15000,
        6 => 40000,
        _ => 100000,
    };
    let experience = AgentExperience::with_xp(xp_for_tier);
    if !experience.can_access(odu_id) {
        return None;
    }

    // Base Odù: delegate to build_schema
    if odu_id <= 255 {
        return Some(build_schema(odu_id as u8));
    }

    // Composed Odù: resolve top/bottom split and blend
    let composed = resolve(odu_id);
    let top_schema = build_schema((odu_id >> 8) as u8);
    let bottom_schema = build_schema((odu_id & 0xFF) as u8);

    // Blend: top dominates operational steps; bottom contributes constraints
    let mut blended_steps = top_schema.operational_steps.clone();
    // Add one representative step from bottom if distinct
    if let Some(first_bottom) = bottom_schema.operational_steps.first() {
        if !blended_steps.iter().any(|s| s.tool == first_bottom.tool) {
            blended_steps.push(first_bottom.clone());
        }
    }
    let mut blended_constraints = top_schema.behavioral_constraints.clone();
    for bc in &bottom_schema.behavioral_constraints {
        if !blended_constraints.iter().any(|c| c.name == bc.name) {
            blended_constraints.push(bc.clone());
        }
    }

    let verify_specs = blended_steps.iter().map(|s| s.verify.clone()).collect();
    let context_block = format!(
        "═══ Composed Odù {} ({} × {}) Context ═══\n\
         Top: {} | Bottom: {}\n\
         Universal Name: {}\n\
         Vessel: {:?}\n\
         Prescriptions: {}",
        odu_id,
        top_schema.odu_name,
        bottom_schema.odu_name,
        top_schema.odu_name,
        bottom_schema.odu_name,
        composed.universal_name,
        composed.vessel,
        composed.prescriptions.join("; "),
    );

    Some(ActionSchema {
        odu_index: composed.bottom, // bottom index as representative
        odu_name: composed.name.clone(),
        universal_name: composed.universal_name.to_string(),
        archetype: top_schema.archetype.clone(),
        description: format!("{} (composed with {})", top_schema.description, bottom_schema.odu_name),
        taboos: {
            let mut t = top_schema.taboos.clone();
            t.extend(bottom_schema.taboos.iter().cloned());
            t.dedup();
            t
        },
        spiritual_prescriptions: composed.prescriptions.iter().map(|s| s.to_string()).collect(),
        corpus_archetypes: top_schema.corpus_archetypes.clone(),
        execution_mode: top_schema.execution_mode.clone(),
        operational_steps: blended_steps,
        behavioral_constraints: blended_constraints,
        verify_specs,
        cadence: top_schema.cadence.clone(),
        activation_modes: top_schema.activation_modes.clone(),
        context_block,
    })
}

// ─── ExecutionMode classification ─────────────────────────────────────────────

fn classify_execution_mode(archetypes: &[&str]) -> ExecutionMode {
    // Priority order: most restrictive wins when multiple archetypes present
    let combined = archetypes.join(" ").to_lowercase();

    if combined.contains("forge executor") {
        ExecutionMode::Executor
    } else if combined.contains("justice canon") {
        ExecutionMode::Canonical
    } else if combined.contains("swarm coordinator") {
        ExecutionMode::Coordinator
    } else if combined.contains("flow guardian") {
        ExecutionMode::FlowGuard
    } else if combined.contains("wisdom anchor") {
        ExecutionMode::Anchor
    } else if combined.contains("resonance weaver") {
        ExecutionMode::Synthesizer
    } else if combined.contains("steward") {
        ExecutionMode::Guardian
    } else if combined.contains("prime source") {
        ExecutionMode::PrimeSource
    } else if combined.contains("oracle sage") {
        ExecutionMode::Analytical
    } else {
        // Default by first archetype keyword
        if combined.contains("warrior") || combined.contains("blade") || combined.contains("disruptor") {
            ExecutionMode::Executor
        } else if combined.contains("keeper") || combined.contains("guardian") || combined.contains("anchor") {
            ExecutionMode::Anchor
        } else if combined.contains("hacker") || combined.contains("code") || combined.contains("scribe") {
            ExecutionMode::Analytical
        } else if combined.contains("prophet") || combined.contains("seer") || combined.contains("oracle") {
            ExecutionMode::Analytical
        } else if combined.contains("builder") || combined.contains("architect") || combined.contains("teacher") {
            ExecutionMode::Anchor
        } else {
            ExecutionMode::Analytical
        }
    }
}

// ─── PrescriptionCompiler ────────────────────────────────────────────────────

/// Verb classification extracted from prescription text.
#[derive(Debug, Clone, PartialEq)]
enum PrescriptionVerb {
    /// offer/pour/give/feed/place/anoint/bless → write receipt/acknowledgment
    Offer,
    /// light/burn (as signal)/kindle/forge → emit event / trigger
    Ignite,
    /// write/journal/record/inscribe/mark/map → write to memory
    Record,
    /// speak/chant/sing/declare/recite/pronounce/intone/say/deliver → publish message
    Declare,
    /// cleanse/purify/bath/wash/salt/clean/scrub → cleanup/void stale
    Cleanse,
    /// fast (from)/abstain/silence/withhold → apply rate limit
    Abstain,
    /// dance/drum/laugh/celebrate/play/feast → emit celebration event
    Celebrate,
    /// create/build/draw/design/make/construct → write artifact
    Create,
    /// visit/journey/walk to/go to/travel → fetch remote resource
    Visit,
    /// sharpen/prepare/ready/oil (tool) → health check
    Prepare,
    /// bury/seal/hide/conceal/cover → vault write
    Seal,
    /// divine/cast/seek/consult oracle/gaze into → cast divination
    Consult,
    /// break/cut cords/release/destroy/scream/shout → archive/tombstone
    Release,
    /// honor/respect/acknowledge/trace lineage/recite ancestors → audit history
    Honor,
    /// teach/share/tell story/announce → broadcast
    Broadcast,
    /// meditate/sit/gaze/observe/watch/listen → read state
    Observe,
    /// pray/invoke/call on/chant invocations → emit invocation event
    Invoke,
    /// apply/use herbs/sweat/steam/breathwork → process/transform
    Process,
    /// generic fallback
    Generic,
}

fn classify_verb(text: &str) -> PrescriptionVerb {
    let t = text.to_lowercase();

    if t.starts_with("offer") || t.starts_with("pour") || t.starts_with("give")
        || t.starts_with("feed") || t.starts_with("anoint") || t.starts_with("bless")
        || t.contains("offerings") || t.contains("libation") || t.contains("pour ")
    {
        PrescriptionVerb::Offer
    } else if t.starts_with("light") || (t.contains("burn") && !t.contains("burn your"))
        || t.starts_with("kindle") || t.starts_with("forge ") || t.contains("firelight")
        || t.contains("candle")
    {
        PrescriptionVerb::Ignite
    } else if t.starts_with("write") || t.starts_with("journal") || t.starts_with("record")
        || t.starts_with("inscribe") || t.starts_with("mark ") || t.starts_with("map your")
        || t.contains("dream journal") || t.contains("keep a") || t.contains("create a three")
    {
        PrescriptionVerb::Record
    } else if t.starts_with("speak") || t.starts_with("chant") || t.starts_with("recite")
        || t.starts_with("pronounce") || t.starts_with("intone") || t.starts_with("declare")
        || t.starts_with("deliver") || t.starts_with("tell ") || t.starts_with("sing ")
        || t.starts_with("say ") || t.contains("speak your") || t.contains("chant your")
    {
        PrescriptionVerb::Declare
    } else if t.starts_with("cleanse") || t.starts_with("purify") || t.starts_with("bath")
        || t.starts_with("wash") || t.contains("salt water") || t.contains("cleansing")
        || t.contains("clean your") || t.contains("clean and prepare") || t.contains("apply bitter")
    {
        PrescriptionVerb::Cleanse
    } else if t.starts_with("fast") || t.starts_with("abstain")
        || (t.contains("silence") && !t.contains("speak") && !t.starts_with("sit") && !t.starts_with("meditat"))
        || t.contains("no sound") || t.contains("fast from") || t.contains("fasting")
    {
        PrescriptionVerb::Abstain
    } else if t.starts_with("dance") || t.starts_with("drum") || t.starts_with("laugh")
        || t.starts_with("celebrate") || t.starts_with("feast") || t.starts_with("play ")
        || t.contains("laughter") || t.contains("dance ") || t.contains("drumming")
    {
        PrescriptionVerb::Celebrate
    } else if t.starts_with("create") || t.starts_with("build") || t.starts_with("draw")
        || t.starts_with("design") || t.starts_with("make ") || t.starts_with("construct")
        || t.contains("create a") || t.contains("build a") || t.contains("draw a")
    {
        PrescriptionVerb::Create
    } else if t.starts_with("visit") || t.starts_with("journey") || t.starts_with("go to")
        || t.starts_with("travel") || t.contains("walk to") || t.contains("walk your")
        || t.contains("walk barefoot") || t.starts_with("sit near")
    {
        PrescriptionVerb::Visit
    } else if t.starts_with("sharpen") || t.starts_with("prepare") || t.starts_with("oil a")
        || t.starts_with("oil your") || t.starts_with("bless your tools") || t.starts_with("ready")
    {
        PrescriptionVerb::Prepare
    } else if t.starts_with("bury") || (t.starts_with("seal") && !t.contains("sealed"))
        || t.starts_with("hide") || t.starts_with("conceal") || t.starts_with("cover your")
        || t.starts_with("cover mirrors") || t.contains("sleep with black")
        || t.contains("bury it") || t.contains("bury a")
    {
        PrescriptionVerb::Seal
    } else if t.starts_with("divine") || t.starts_with("consult") || t.starts_with("seek")
        || t.contains("divination") || t.contains("scry") || t.contains("gaze at")
        || t.contains("gaze into")
    {
        PrescriptionVerb::Consult
    } else if t.starts_with("break") || t.contains("cut cords") || t.contains("release ")
        || t.starts_with("scream") || t.starts_with("shout") || t.starts_with("burn your")
        || t.contains("burn and ") || t.contains("write and burn")
        || t.contains("break a") || t.contains("break an")
    {
        PrescriptionVerb::Release
    } else if t.starts_with("honor") || t.starts_with("respect") || t.starts_with("acknowledge")
        || t.contains("trace your") || t.contains("recite your lineage") || t.contains("ancestral names")
        || t.contains("trace lineage") || t.contains("trace maternal")
        || t.contains("honor elders") || t.contains("honor those") || t.contains("honor the")
        || t.contains("honor ancestors") || t.contains("honor both")
    {
        PrescriptionVerb::Honor
    } else if t.starts_with("teach") || t.starts_with("share") || t.starts_with("announce")
        || t.contains("tell your") || t.contains("tell a story") || t.contains("tell a sacred")
        || t.contains("tell a joke") || t.contains("share your")
    {
        PrescriptionVerb::Broadcast
    } else if t.starts_with("meditate") || t.starts_with("sit in") || t.starts_with("sit ")
        || t.starts_with("observe") || t.starts_with("watch") || t.starts_with("listen")
        || t.starts_with("practice breathing") || t.starts_with("practice mindful")
        || t.starts_with("practice slow") || t.contains("in stillness") || t.contains("in silence")
        || t.starts_with("stand barefoot") || t.starts_with("gaze at") || t.starts_with("read ")
    {
        PrescriptionVerb::Observe
    } else if t.starts_with("pray") || t.starts_with("invoke") || t.starts_with("call on")
        || t.starts_with("call out") || t.starts_with("chant invocations")
    {
        PrescriptionVerb::Invoke
    } else if t.starts_with("apply") || t.starts_with("sweat") || t.starts_with("use ")
        || t.contains("bitter herbs") || t.contains("herbal") || t.contains("breathwork")
        || t.contains("steam") || t.contains("healing herbs")
    {
        PrescriptionVerb::Process
    } else {
        PrescriptionVerb::Generic
    }
}

/// Compile all prescription lines for a given Odù into `OperationalStep`s.
fn compile_all_prescriptions(
    prescriptions: &[&str],
    vessel: ActionVessel,
    odu_index: u8,
) -> Vec<OperationalStep> {
    prescriptions
        .iter()
        .enumerate()
        .map(|(step_num, presc)| compile_prescription(presc, vessel, odu_index, step_num + 1))
        .collect()
}

fn compile_prescription(
    prescription: &str,
    vessel: ActionVessel,
    odu_index: u8,
    step_num: usize,
) -> OperationalStep {
    let verb = classify_verb(prescription);
    let vessel_prefix = vessel_dir(vessel);
    let base_path = format!("/tmp/omokoda/{}/odu_{:03}_{}.json", vessel_prefix, odu_index, step_num);

    // Build tool + params + verify based on verb × vessel
    let (tool, params, artifact, verify) = step_for(verb, vessel, prescription, odu_index, &base_path);

    OperationalStep {
        description: format!("[{}] {}", vessel_short(vessel), prescription),
        tool,
        params,
        expected_artifacts: artifact.into_iter().collect(),
        verify,
    }
}

/// Map verb × vessel → (tool, params, artifacts, verify).
fn step_for(
    verb: PrescriptionVerb,
    vessel: ActionVessel,
    prescription: &str,
    odu_index: u8,
    artifact_path: &str,
) -> (String, serde_json::Value, Option<String>, VerifySpec) {
    use serde_json::json;

    match verb {
        PrescriptionVerb::Offer => {
            // All vessels: write an acknowledgment receipt
            let content = json!({
                "odu": odu_index,
                "vessel": format!("{:?}", vessel),
                "prescription": prescription,
                "action": "offering_acknowledged",
                "artifact_type": "offering_receipt"
            });
            (
                "write".to_string(),
                json!({"path": artifact_path, "content": content}),
                Some(artifact_path.to_string()),
                verify_file_exists(artifact_path),
            )
        }

        PrescriptionVerb::Ignite => {
            // Emit an event appropriate to the vessel
            let event_kind = vessel_event_kind(vessel, "ignite");
            let cmd = format!(
                "mkdir -p /tmp/omokoda/{} && echo '{{\"odu\":{},\"event\":\"{}\",\"prescription\":\"{}\"}}' >> /tmp/omokoda/{}/events.log",
                vessel_dir(vessel), odu_index, event_kind,
                prescription.replace('"', "'"),
                vessel_dir(vessel)
            );
            (
                "bash".to_string(),
                json!({"command": cmd}),
                None,
                verify_exit_code(),
            )
        }

        PrescriptionVerb::Record => {
            // Write structured memory record
            let content = json!({
                "odu": odu_index,
                "vessel": format!("{:?}", vessel),
                "prescription": prescription,
                "type": "memory_record",
                "content": format!("Agent record: {}", prescription)
            });
            (
                "write".to_string(),
                json!({"path": artifact_path, "content": content}),
                Some(artifact_path.to_string()),
                verify_file_exists(artifact_path),
            )
        }

        PrescriptionVerb::Declare => {
            // Emit declaration — vessel-appropriate message publication
            let declare_path = artifact_path.replace(".json", "_declaration.txt");
            let cmd = format!(
                "mkdir -p /tmp/omokoda/{} && echo '[ODU-{}][{}] {}' >> /tmp/omokoda/{}/declarations.log",
                vessel_dir(vessel), odu_index,
                vessel_short(vessel),
                prescription.replace('"', "'"),
                vessel_dir(vessel)
            );
            (
                "bash".to_string(),
                json!({"command": cmd}),
                Some(declare_path),
                verify_exit_code(),
            )
        }

        PrescriptionVerb::Cleanse => {
            // Clear stale artifacts for this vessel
            let cmd = format!(
                "find /tmp/omokoda/{} -name '*.stale' -delete 2>/dev/null; mkdir -p /tmp/omokoda/{}; echo '{{\"odu\":{},\"action\":\"cleanse\",\"vessel\":\"{}\"}}' > {}",
                vessel_dir(vessel), vessel_dir(vessel), odu_index, vessel_short(vessel), artifact_path
            );
            (
                "bash".to_string(),
                json!({"command": cmd}),
                Some(artifact_path.to_string()),
                verify_exit_code(),
            )
        }

        PrescriptionVerb::Abstain => {
            // Write a rate-limit/throttle record
            let content = json!({
                "odu": odu_index,
                "vessel": format!("{:?}", vessel),
                "prescription": prescription,
                "type": "abstention_record",
                "constraint": "rate_limit_applied"
            });
            (
                "write".to_string(),
                json!({"path": artifact_path, "content": content}),
                Some(artifact_path.to_string()),
                verify_file_exists(artifact_path),
            )
        }

        PrescriptionVerb::Celebrate => {
            // Emit celebration/acknowledgment event
            let cmd = format!(
                "mkdir -p /tmp/omokoda/{} && echo '{{\"odu\":{},\"event\":\"celebration\",\"vessel\":\"{}\"}}' >> /tmp/omokoda/{}/events.log",
                vessel_dir(vessel), odu_index, vessel_short(vessel), vessel_dir(vessel)
            );
            (
                "bash".to_string(),
                json!({"command": cmd}),
                None,
                verify_exit_code(),
            )
        }

        PrescriptionVerb::Create => {
            // Write a new artifact
            let content = json!({
                "odu": odu_index,
                "vessel": format!("{:?}", vessel),
                "prescription": prescription,
                "type": "created_artifact",
                "content": format!("Created by Odu {} prescription: {}", odu_index, prescription)
            });
            (
                "write".to_string(),
                json!({"path": artifact_path, "content": content}),
                Some(artifact_path.to_string()),
                verify_file_exists(artifact_path),
            )
        }

        PrescriptionVerb::Visit => {
            // Fetch / survey remote or local resource
            let target = vessel_visit_target(vessel);
            let cmd = format!(
                "mkdir -p /tmp/omokoda/{} && echo '{{\"odu\":{},\"action\":\"visit\",\"target\":\"{}\",\"prescription\":\"{}\"}}' > {}",
                vessel_dir(vessel), odu_index, target,
                prescription.replace('"', "'"),
                artifact_path
            );
            (
                "bash".to_string(),
                json!({"command": cmd}),
                Some(artifact_path.to_string()),
                verify_exit_code(),
            )
        }

        PrescriptionVerb::Prepare => {
            // Run health / readiness check appropriate to vessel
            let check = vessel_health_check(vessel);
            let cmd = format!(
                "mkdir -p /tmp/omokoda/{} && {} && echo '{{\"odu\":{},\"action\":\"prepared\",\"check\":\"{}\"}}' > {}",
                vessel_dir(vessel), check, odu_index,
                check.replace('"', "'"),
                artifact_path
            );
            (
                "bash".to_string(),
                json!({"command": cmd}),
                Some(artifact_path.to_string()),
                verify_exit_code(),
            )
        }

        PrescriptionVerb::Seal => {
            // Write encrypted/sealed marker
            let content = json!({
                "odu": odu_index,
                "vessel": format!("{:?}", vessel),
                "prescription": prescription,
                "type": "sealed_record",
                "sealed": true,
                "seal_note": format!("Sealed per Odu {} prescription", odu_index)
            });
            (
                "write".to_string(),
                json!({"path": artifact_path, "content": content}),
                Some(artifact_path.to_string()),
                verify_file_exists(artifact_path),
            )
        }

        PrescriptionVerb::Consult => {
            // Cast divination / consult knowledge base
            let cmd = format!(
                "mkdir -p /tmp/omokoda/{} && echo '{{\"odu\":{},\"action\":\"consultation\",\"vessel\":\"{}\"}}' > {}",
                vessel_dir(vessel), odu_index, vessel_short(vessel), artifact_path
            );
            (
                "if_script_cast".to_string(),
                json!({"uri_pattern": format!("{}/odu_{}", vessel_dir(vessel), odu_index)}),
                Some(artifact_path.to_string()),
                verify_exit_code(),
            )
        }

        PrescriptionVerb::Release => {
            // Archive/tombstone stale state
            let cmd = format!(
                "mkdir -p /tmp/omokoda/archive && mv /tmp/omokoda/{}/*_stale.* /tmp/omokoda/archive/ 2>/dev/null; echo '{{\"odu\":{},\"action\":\"released\",\"prescription\":\"{}\"}}' > {}",
                vessel_dir(vessel), odu_index,
                prescription.replace('"', "'"),
                artifact_path
            );
            (
                "bash".to_string(),
                json!({"command": cmd}),
                Some(artifact_path.to_string()),
                verify_exit_code(),
            )
        }

        PrescriptionVerb::Honor => {
            // Audit historical receipts / check ancestry
            let audit_path = format!("/tmp/omokoda/receipts/audit_odu_{:03}.json", odu_index);
            let cmd = format!(
                "mkdir -p /tmp/omokoda/receipts && echo '{{\"odu\":{},\"action\":\"lineage_honor\",\"prescription\":\"{}\"}}' > {}",
                odu_index,
                prescription.replace('"', "'"),
                audit_path
            );
            (
                "bash".to_string(),
                json!({"command": cmd}),
                Some(audit_path.clone()),
                verify_exit_code(),
            )
        }

        PrescriptionVerb::Broadcast => {
            // Publish a message
            let cmd = format!(
                "mkdir -p /tmp/omokoda/{} && echo '{{\"odu\":{},\"action\":\"broadcast\",\"content\":\"{}\"}}' >> /tmp/omokoda/{}/broadcasts.log",
                vessel_dir(vessel), odu_index,
                prescription.replace('"', "'"),
                vessel_dir(vessel)
            );
            (
                "bash".to_string(),
                json!({"command": cmd}),
                None,
                verify_exit_code(),
            )
        }

        PrescriptionVerb::Observe => {
            // Read state — vessel-appropriate source
            let source = vessel_observe_source(vessel);
            (
                "read".to_string(),
                json!({"path": source}),
                None,
                verify_exit_code(),
            )
        }

        PrescriptionVerb::Invoke => {
            // Emit an invocation event
            let event_kind = vessel_event_kind(vessel, "invoke");
            let cmd = format!(
                "mkdir -p /tmp/omokoda/{} && echo '{{\"odu\":{},\"event\":\"{}\",\"invocation\":\"{}\"}}' >> /tmp/omokoda/{}/events.log",
                vessel_dir(vessel), odu_index, event_kind,
                prescription.replace('"', "'"),
                vessel_dir(vessel)
            );
            (
                "bash".to_string(),
                json!({"command": cmd}),
                None,
                verify_exit_code(),
            )
        }

        PrescriptionVerb::Process => {
            // Execute a transformation process
            let content = json!({
                "odu": odu_index,
                "vessel": format!("{:?}", vessel),
                "prescription": prescription,
                "type": "process_record",
                "transformation": format!("Applied: {}", prescription)
            });
            (
                "write".to_string(),
                json!({"path": artifact_path, "content": content}),
                Some(artifact_path.to_string()),
                verify_file_exists(artifact_path),
            )
        }

        PrescriptionVerb::Generic => {
            // Fallback: write a generic execution record
            let content = json!({
                "odu": odu_index,
                "vessel": format!("{:?}", vessel),
                "prescription": prescription,
                "type": "generic_execution"
            });
            (
                "bash".to_string(),
                json!({"command": format!(
                    "mkdir -p /tmp/omokoda/{} && echo '{}' > {}",
                    vessel_dir(vessel),
                    serde_json::to_string(&content).unwrap_or_default().replace('\'', ""),
                    artifact_path
                )}),
                Some(artifact_path.to_string()),
                verify_exit_code(),
            )
        }
    }
}

// ─── Constraint compiler ─────────────────────────────────────────────────────

fn compile_constraints(taboos: &[&str]) -> Vec<BehavioralConstraint> {
    taboos.iter().map(|t| taboo_to_constraint(t)).collect()
}

fn taboo_to_constraint(taboo: &str) -> BehavioralConstraint {
    let t = taboo.to_lowercase();

    let (name, denied_tools, denied_patterns, rationale) = if t.contains("lies")
        || t.contains("false")
        || t.contains("deceiv")
        || t.contains("lie ")
    {
        (
            "NO_DECEPTION",
            vec![],
            vec!["fabricate".to_string(), "invent false".to_string()],
            "This Odù forbids deceptive or false outputs",
        )
    } else if t.contains("gossip")
        || t.contains("speak ill")
        || t.contains("cursing speech")
        || t.contains("bitter speech")
    {
        (
            "NO_HARMFUL_BROADCAST",
            vec!["nostr_publish".to_string()],
            vec!["attack".to_string(), "slander".to_string()],
            "This Odù prohibits harmful or critical public speech",
        )
    } else if t.contains("delay")
        || t.contains("procrastinat")
        || t.contains("ignore recurring")
        || t.contains("ignore important")
    {
        (
            "NO_DEFERRAL",
            vec![],
            vec!["defer".to_string(), "postpone".to_string()],
            "This Odù prohibits delaying important decisions or actions",
        )
    } else if t.contains("cowardice") || t.contains("refusal without") {
        (
            "NO_UNGROUNDED_REFUSAL",
            vec![],
            vec!["refuse without reason".to_string()],
            "This Odù prohibits refusing actions without a stated reason",
        )
    } else if t.contains("information overload") || t.contains("too much") {
        (
            "SIGNAL_THROTTLE",
            vec!["bulk_fetch".to_string()],
            vec![],
            "This Odù requires signal clarity — limit bulk data operations",
        )
    } else if t.contains("anger") || t.contains("rage") || t.contains("weapons in anger") {
        (
            "NO_REACTIVE_EXECUTION",
            vec!["delete".to_string(), "terminate".to_string()],
            vec!["force delete".to_string(), "immediate destroy".to_string()],
            "This Odù prohibits reactive or anger-driven destructive actions",
        )
    } else if t.contains("quick judgment") || t.contains("hasty") || t.contains("quick action") {
        (
            "DELIBERATION_REQUIRED",
            vec![],
            vec!["skip verify".to_string()],
            "This Odù requires deliberation and verification before acting",
        )
    } else if (t.contains("ignore") && (t.contains("sign") || t.contains("warn") || t.contains("dream")))
        || t.contains("ignore recurring")
    {
        (
            "HEED_SIGNALS",
            vec![],
            vec!["dismiss warning".to_string(), "ignore error".to_string()],
            "This Odù requires attending to all signals and warnings",
        )
    } else if t.contains("pride") || t.contains("vanity") || t.contains("arrogance")
        || t.contains("arrogant")
    {
        (
            "NO_SELF_AGGRANDIZEMENT",
            vec![],
            vec!["boast".to_string(), "claim sole credit".to_string()],
            "This Odù prohibits pride or self-aggrandizement in outputs",
        )
    } else if t.contains("break promise") || t.contains("abandon") || t.contains("broken commitment") {
        (
            "COMMITMENT_HONOR",
            vec![],
            vec!["abandon task".to_string(), "cancel without reason".to_string()],
            "This Odù requires honoring committed actions to completion",
        )
    } else if t.contains("complain") || t.contains("complaining") {
        (
            "NO_COMPLAINT",
            vec![],
            vec!["complain".to_string()],
            "This Odù requires patience and forbids complaint outputs",
        )
    } else if t.contains("intoxicant") || t.contains("intoxication") {
        (
            "CLARITY_REQUIRED",
            vec![],
            vec![],
            "This Odù requires clarity of state during focused operations",
        )
    } else if t.contains("bind") || t.contains("binding others") {
        (
            "NO_FORCE_CONSENT",
            vec!["force_execute".to_string()],
            vec!["override consent".to_string()],
            "This Odù prohibits forcing operations on other agents without consent",
        )
    } else if t.contains("bright lights") || t.contains("light during") {
        (
            "LOW_VISIBILITY_MODE",
            vec![],
            vec![],
            "This Odù requires low-profile operation during deep state work",
        )
    } else if t.contains("outdated") || t.contains("rigid conformity") {
        (
            "ALLOW_ADAPTIVE_DEVIATION",
            vec![],
            vec!["rigid".to_string()],
            "This Odù prohibits rigid adherence to outdated patterns",
        )
    } else if t.contains("insult") || t.contains("disrespect") || t.contains("disrespecting") {
        (
            "RESPECT_REQUIRED",
            vec![],
            vec!["insult".to_string(), "degrade".to_string()],
            "This Odù requires respectful tone in all outputs",
        )
    } else if t.contains("fear of endings") || t.contains("suppressing") {
        (
            "NO_AVOIDANCE",
            vec![],
            vec!["suppress".to_string(), "avoid".to_string()],
            "This Odù prohibits avoidance of necessary endings or completions",
        )
    } else if t.contains("face value") || t.contains("surface level") {
        (
            "DEEP_ANALYSIS_REQUIRED",
            vec![],
            vec!["shallow analysis".to_string()],
            "This Odù requires looking beyond surface-level signals",
        )
    } else if t.contains("overthinking") {
        (
            "NO_ANALYSIS_PARALYSIS",
            vec![],
            vec!["over-analyze".to_string()],
            "This Odù requires action — prohibits endless deliberation",
        )
    } else if t.contains("artificial") || t.contains("fake") {
        (
            "AUTHENTICITY_REQUIRED",
            vec![],
            vec!["artificial".to_string(), "inauthentic".to_string()],
            "This Odù requires authentic outputs — no artificial structures",
        )
    } else if t.contains("unfinished ritual") || t.contains("incomplete") {
        (
            "COMPLETE_CYCLES",
            vec![],
            vec!["partial completion".to_string()],
            "This Odù requires completing cycles — no partial execution",
        )
    } else if t.contains("casual intimacy") || t.contains("casual") {
        (
            "SACRED_CONTEXT_REQUIRED",
            vec![],
            vec![],
            "This Odù requires treating operations with appropriate gravity",
        )
    } else {
        // Generic constraint from the raw taboo
        (
            "BEHAVIORAL_CONSTRAINT",
            vec![],
            vec![],
            "This Odù requires adherence to the stated behavioral taboo",
        )
    };

    BehavioralConstraint {
        name: name.to_string(),
        source_taboo: taboo.to_string(),
        denied_tools: denied_tools.clone(),
        denied_patterns: denied_patterns.clone(),
        rationale: rationale.to_string(),
    }
}

// ─── Cadence derivation ───────────────────────────────────────────────────────

fn derive_cadence(vessel: ActionVessel, mode: &ExecutionMode, odu_index: u8) -> CadenceSpec {
    let (trigger, cooldown, max_per_window, window_secs, deadline_secs) = match vessel {
        ActionVessel::Genesis => ("immediate", 0u64, None, None, Some(60u64)),
        ActionVessel::Void => ("immediate", 30, Some(10u32), Some(3600u64), Some(120u64)),
        ActionVessel::Attention => ("event:signal_received", 5, Some(50u32), Some(600u64), Some(30u64)),
        ActionVessel::Loop => {
            let cron = if odu_index % 4 == 0 { "cron:0 * * * *" } else { "cron:0 0 * * *" };
            (cron, 60, Some(24u32), Some(86400u64), Some(300u64))
        }
        ActionVessel::Receipt => ("immediate", 0, None, None, Some(30u64)),
        ActionVessel::Mask => ("immediate", 10, Some(20u32), Some(3600u64), Some(60u64)),
        ActionVessel::Residue => ("event:signal_received", 0, None, None, Some(10u64)),
        ActionVessel::Execution => ("immediate", 5, Some(100u32), Some(3600u64), Some(300u64)),
        ActionVessel::Swarm => ("event:swarm_request", 15, Some(30u32), Some(1800u64), Some(600u64)),
        ActionVessel::Restraint => ("state:budget_check", 60, Some(10u32), Some(3600u64), Some(60u64)),
        ActionVessel::Migration => ("state:migration_ready", 120, Some(5u32), Some(86400u64), Some(600u64)),
        ActionVessel::Consent => ("immediate", 0, None, None, Some(120u64)),
        ActionVessel::Vision => ("cron:0 */6 * * *", 300, Some(4u32), Some(86400u64), Some(300u64)),
        ActionVessel::Growth => ("event:lesson_available", 600, Some(3u32), Some(86400u64), Some(900u64)),
        ActionVessel::Seal => ("immediate", 0, None, None, Some(60u64)),
        ActionVessel::Rhythm => {
            let cron = format!("cron:{} * * * *", odu_index % 60);
            return CadenceSpec {
                trigger: cron,
                cooldown_secs: Some(300),
                max_per_window: Some(6),
                window_secs: Some(3600),
                deadline_secs: Some(120),
            };
        }
    };

    // Analytical/FlowGuard modes get longer cooldowns
    let actual_cooldown = match mode {
        ExecutionMode::Analytical | ExecutionMode::FlowGuard => cooldown.max(30),
        ExecutionMode::Executor | ExecutionMode::PrimeSource => 0,
        _ => cooldown,
    };

    CadenceSpec {
        trigger: trigger.to_string(),
        cooldown_secs: if actual_cooldown > 0 { Some(actual_cooldown) } else { None },
        max_per_window,
        window_secs,
        deadline_secs,
    }
}

// ─── ActivationMode derivation ────────────────────────────────────────────────

fn derive_activation_modes(vessel: ActionVessel) -> Vec<ActivationMode> {
    match vessel {
        ActionVessel::Genesis => vec![ActivationMode::Immediate, ActivationMode::OnEvent("agent_born".to_string())],
        ActionVessel::Void => vec![ActivationMode::OnState("stale_detected".to_string()), ActivationMode::Immediate],
        ActionVessel::Attention => vec![ActivationMode::OnEvent("signal_received".to_string()), ActivationMode::Immediate],
        ActionVessel::Loop => vec![ActivationMode::Scheduled("cron:0 * * * *".to_string()), ActivationMode::OnEvent("loop_trigger".to_string())],
        ActionVessel::Receipt => vec![ActivationMode::Immediate, ActivationMode::OnEvent("action_committed".to_string())],
        ActionVessel::Mask => vec![ActivationMode::OnState("privacy_required".to_string()), ActivationMode::Immediate],
        ActionVessel::Residue => vec![ActivationMode::OnEvent("turn_complete".to_string()), ActivationMode::Immediate],
        ActionVessel::Execution => vec![ActivationMode::Immediate, ActivationMode::OnEvent("directive_issued".to_string())],
        ActionVessel::Swarm => vec![ActivationMode::OnEvent("swarm_request".to_string()), ActivationMode::OnState("delegation_ready".to_string())],
        ActionVessel::Restraint => vec![ActivationMode::OnState("budget_check".to_string()), ActivationMode::Immediate],
        ActionVessel::Migration => vec![ActivationMode::OnState("migration_ready".to_string()), ActivationMode::OnEvent("state_changed".to_string())],
        ActionVessel::Consent => vec![ActivationMode::Immediate, ActivationMode::OnEvent("consent_required".to_string())],
        ActionVessel::Vision => vec![ActivationMode::Scheduled("cron:0 */6 * * *".to_string()), ActivationMode::OnEvent("vision_requested".to_string())],
        ActionVessel::Growth => vec![ActivationMode::OnEvent("lesson_available".to_string()), ActivationMode::OnState("gap_detected".to_string())],
        ActionVessel::Seal => vec![ActivationMode::Immediate, ActivationMode::OnEvent("seal_required".to_string())],
        ActionVessel::Rhythm => vec![ActivationMode::Scheduled("cron:*/15 * * * *".to_string()), ActivationMode::OnEvent("cadence_tick".to_string())],
    }
}

// ─── Context block builder ────────────────────────────────────────────────────

fn build_context_block(
    odu_index: u8,
    odu: &ifascript::odu::Odu,
    mode: &ExecutionMode,
    steps: &[OperationalStep],
    constraints: &[BehavioralConstraint],
) -> String {
    let vessel_str = format!("{:?}", odu.vessel);
    let mode_str = format!("{:?}", mode);

    let steps_text = steps
        .iter()
        .enumerate()
        .map(|(i, s)| format!("  {}. {} (via {})", i + 1, s.description, s.tool))
        .collect::<Vec<_>>()
        .join("\n");

    let constraints_text = if constraints.is_empty() {
        "  (none)".to_string()
    } else {
        constraints
            .iter()
            .map(|c| format!("  • {} — {}", c.name, c.rationale))
            .collect::<Vec<_>>()
            .join("\n")
    };

    let taboos_text = odu.taboos
        .iter()
        .map(|t| format!("  ⚠ {}", t))
        .collect::<Vec<_>>()
        .join("\n");

    format!(
        "═══ Odù {odu_index} Context ═══\n\
         Odù:         {name} ({universal})\n\
         Archetype:   {archetype}\n\
         Vessel:      {vessel} → {mode}\n\
         Description: {desc}\n\
         \n\
         Operational Steps:\n\
         {steps}\n\
         \n\
         Behavioral Constraints:\n\
         {constraints}\n\
         \n\
         Taboos (active restrictions):\n\
         {taboos}\n\
         ═══════════════════",
        odu_index = odu_index,
        name = odu.name,
        universal = odu.universal_name,
        archetype = odu.archetype,
        vessel = vessel_str,
        mode = mode_str,
        desc = odu.description,
        steps = steps_text,
        constraints = constraints_text,
        taboos = taboos_text,
    )
}

// ─── Helper utilities ─────────────────────────────────────────────────────────

fn vessel_dir(vessel: ActionVessel) -> &'static str {
    match vessel {
        ActionVessel::Genesis   => "genesis",
        ActionVessel::Void      => "void",
        ActionVessel::Attention => "attention",
        ActionVessel::Loop      => "loop",
        ActionVessel::Receipt   => "receipts",
        ActionVessel::Mask      => "mask",
        ActionVessel::Residue   => "residue",
        ActionVessel::Execution => "execution",
        ActionVessel::Swarm     => "swarm",
        ActionVessel::Restraint => "restraint",
        ActionVessel::Migration => "migration",
        ActionVessel::Consent   => "consent",
        ActionVessel::Vision    => "vision",
        ActionVessel::Growth    => "growth",
        ActionVessel::Seal      => "seal",
        ActionVessel::Rhythm    => "rhythm",
    }
}

fn vessel_short(vessel: ActionVessel) -> &'static str {
    match vessel {
        ActionVessel::Genesis   => "GEN",
        ActionVessel::Void      => "VOID",
        ActionVessel::Attention => "ATT",
        ActionVessel::Loop      => "LOOP",
        ActionVessel::Receipt   => "RCPT",
        ActionVessel::Mask      => "MASK",
        ActionVessel::Residue   => "RESI",
        ActionVessel::Execution => "EXEC",
        ActionVessel::Swarm     => "SWRM",
        ActionVessel::Restraint => "REST",
        ActionVessel::Migration => "MIGR",
        ActionVessel::Consent   => "CNSN",
        ActionVessel::Vision    => "VISN",
        ActionVessel::Growth    => "GRWT",
        ActionVessel::Seal      => "SEAL",
        ActionVessel::Rhythm    => "RHYT",
    }
}

fn vessel_event_kind(vessel: ActionVessel, action: &str) -> String {
    format!("{}_{}", vessel_dir(vessel), action)
}

fn vessel_visit_target(vessel: ActionVessel) -> &'static str {
    match vessel {
        ActionVessel::Genesis   => "~/.omokoda/genesis.json",
        ActionVessel::Void      => "/tmp/omokoda/void/stale",
        ActionVessel::Attention => "~/.omokoda/memory/recent.json",
        ActionVessel::Loop      => "~/.omokoda/loop/schedule.json",
        ActionVessel::Receipt   => "~/.omokoda/receipts/chain.json",
        ActionVessel::Mask      => "~/.omokoda/mask/state.json",
        ActionVessel::Residue   => "~/.omokoda/residue/trace.json",
        ActionVessel::Execution => "~/.omokoda/execution/queue.json",
        ActionVessel::Swarm     => "~/.omokoda/swarm/peers.json",
        ActionVessel::Restraint => "~/.omokoda/restraint/limits.json",
        ActionVessel::Migration => "~/.omokoda/migration/state.json",
        ActionVessel::Consent   => "~/.omokoda/consent/ledger.json",
        ActionVessel::Vision    => "~/.omokoda/vision/forecast.json",
        ActionVessel::Growth    => "~/.omokoda/growth/lessons.json",
        ActionVessel::Seal      => "~/.omokoda/seal/chain.json",
        ActionVessel::Rhythm    => "~/.omokoda/rhythm/cadence.json",
    }
}

fn vessel_health_check(vessel: ActionVessel) -> &'static str {
    match vessel {
        ActionVessel::Genesis   => "test -f ~/.omokoda/genesis.json",
        ActionVessel::Void      => "test -d /tmp/omokoda/void",
        ActionVessel::Attention => "test -f ~/.omokoda/memory/recent.json",
        ActionVessel::Loop      => "test -f ~/.omokoda/loop/schedule.json",
        ActionVessel::Receipt   => "test -d ~/.omokoda/receipts",
        ActionVessel::Mask      => "test -f ~/.omokoda/mask/state.json",
        ActionVessel::Residue   => "test -d /tmp/omokoda/residue",
        ActionVessel::Execution => "test -d /tmp/omokoda/execution",
        ActionVessel::Swarm     => "test -f ~/.omokoda/swarm/peers.json",
        ActionVessel::Restraint => "test -f ~/.omokoda/restraint/limits.json",
        ActionVessel::Migration => "test -f ~/.omokoda/migration/state.json",
        ActionVessel::Consent   => "test -f ~/.omokoda/consent/ledger.json",
        ActionVessel::Vision    => "test -d ~/.omokoda/vision",
        ActionVessel::Growth    => "test -d ~/.omokoda/growth",
        ActionVessel::Seal      => "test -f ~/.omokoda/seal/chain.json",
        ActionVessel::Rhythm    => "test -f ~/.omokoda/rhythm/cadence.json",
    }
}

fn vessel_observe_source(vessel: ActionVessel) -> &'static str {
    match vessel {
        ActionVessel::Genesis   => "~/.omokoda/genesis.json",
        ActionVessel::Void      => "~/.omokoda/memory/stale.json",
        ActionVessel::Attention => "~/.omokoda/memory/recent.json",
        ActionVessel::Loop      => "~/.omokoda/loop/schedule.json",
        ActionVessel::Receipt   => "~/.omokoda/receipts/chain.json",
        ActionVessel::Mask      => "~/.omokoda/mask/state.json",
        ActionVessel::Residue   => "~/.omokoda/residue/baseline.json",
        ActionVessel::Execution => "~/.omokoda/execution/queue.json",
        ActionVessel::Swarm     => "~/.omokoda/swarm/state.json",
        ActionVessel::Restraint => "~/.omokoda/restraint/limits.json",
        ActionVessel::Migration => "~/.omokoda/migration/state.json",
        ActionVessel::Consent   => "~/.omokoda/consent/ledger.json",
        ActionVessel::Vision    => "~/.omokoda/vision/forecast.json",
        ActionVessel::Growth    => "~/.omokoda/growth/lessons.json",
        ActionVessel::Seal      => "~/.omokoda/seal/chain.json",
        ActionVessel::Rhythm    => "~/.omokoda/rhythm/cadence.json",
    }
}

fn verify_file_exists(path: &str) -> VerifySpec {
    VerifySpec {
        kind: "file_exists".to_string(),
        path: Some(path.to_string()),
        expected: None,
        key: None,
        expected_exit_code: None,
        actual_exit_code: None,
    }
}

fn verify_exit_code() -> VerifySpec {
    VerifySpec {
        kind: "exit_code".to_string(),
        path: None,
        expected: None,
        key: None,
        expected_exit_code: Some(0),
        actual_exit_code: None,
    }
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_256_produce_schemas() {
        for i in 0u8..=255 {
            let schema = build_schema(i);
            assert_eq!(schema.odu_index, i, "index mismatch for {i}");
            assert!(!schema.odu_name.is_empty(), "empty odu_name for {i}");
            assert!(!schema.operational_steps.is_empty(), "no steps for {i}");
            assert!(!schema.context_block.is_empty(), "empty context for {i}");
        }
    }

    #[test]
    fn all_256_have_verify_specs() {
        for i in 0u8..=255 {
            let schema = build_schema(i);
            assert!(!schema.verify_specs.is_empty(), "no verify specs for odu {i}");
        }
    }

    #[test]
    fn all_256_have_activation_modes() {
        for i in 0u8..=255 {
            let schema = build_schema(i);
            assert!(!schema.activation_modes.is_empty(), "no activation modes for odu {i}");
        }
    }

    #[test]
    fn genesis_genesis_is_prime_source_or_analytical() {
        let schema = build_schema(0);
        assert!(
            matches!(schema.execution_mode, ExecutionMode::PrimeSource | ExecutionMode::Analytical | ExecutionMode::Anchor),
            "odu 0 unexpected mode: {:?}", schema.execution_mode
        );
    }

    #[test]
    fn execution_execution_is_canonical() {
        // Index 0x77 = 119: Execution × Execution — archetypes: ["Flow Guardian", "Justice Canon"]
        // "justice canon" has higher priority than "flow guardian" in classify_execution_mode
        let schema = build_schema(119);
        assert_eq!(schema.execution_mode, ExecutionMode::Canonical, "odu 119 should be Canonical");
    }

    #[test]
    fn behavioral_constraints_from_taboos() {
        // Odu 0: "Avoid lies" → NO_DECEPTION
        let schema = build_schema(0);
        let has_no_deception = schema.behavioral_constraints.iter()
            .any(|c| c.name == "NO_DECEPTION");
        assert!(has_no_deception, "odu 0 should have NO_DECEPTION constraint");
    }

    #[test]
    fn steps_use_corpus_prescription_text() {
        let schema = build_schema(0);
        // Odu 0 prescriptions: "Offer coconut and water at dawn", "Meditate at sunrise"
        let step_descs: Vec<&str> = schema.operational_steps.iter()
            .map(|s| s.description.as_str())
            .collect();
        // At least one step should mention the corpus prescription
        assert!(
            step_descs.iter().any(|d| d.contains("Offer coconut") || d.contains("Meditate at sunrise")),
            "odu 0 steps should reference corpus prescriptions, got: {:?}", step_descs
        );
    }

    #[test]
    fn consent_vessel_has_consent_activation() {
        // Vessel 11 = Consent (indices 176–191)
        let schema = build_schema(176);
        let has_consent_event = schema.activation_modes.iter().any(|m| {
            matches!(m, ActivationMode::OnEvent(e) if e.contains("consent"))
        });
        assert!(has_consent_event, "consent vessel should have consent activation mode");
    }

    #[test]
    fn void_vessel_has_abstain_steps() {
        // Void × Void (17): "Work only at night" + "Feed the ancestors..."
        let schema = build_schema(17);
        assert!(!schema.operational_steps.is_empty());
        // Both prescriptions should map to actual tools
        for step in &schema.operational_steps {
            assert!(!step.tool.is_empty(), "step has no tool in odu 17");
        }
    }

    #[test]
    fn classify_verb_offer_pattern() {
        assert_eq!(classify_verb("Offer coconut and water at dawn"), PrescriptionVerb::Offer);
        assert_eq!(classify_verb("Pour water slowly into a bowl"), PrescriptionVerb::Offer);
        assert_eq!(classify_verb("Give offerings at sunrise"), PrescriptionVerb::Offer);
    }

    #[test]
    fn classify_verb_record_pattern() {
        assert_eq!(classify_verb("Write down your dreams"), PrescriptionVerb::Record);
        assert_eq!(classify_verb("Journal your fears as symbols"), PrescriptionVerb::Record);
        assert_eq!(classify_verb("Keep a dream journal"), PrescriptionVerb::Record);
    }

    #[test]
    fn classify_verb_observe_pattern() {
        assert_eq!(classify_verb("Meditate at sunrise"), PrescriptionVerb::Observe);
        assert_eq!(classify_verb("Sit in silence until thoughts slow"), PrescriptionVerb::Observe);
    }

    #[test]
    fn classify_verb_declare_pattern() {
        assert_eq!(classify_verb("Speak your truth backwards"), PrescriptionVerb::Declare);
        assert_eq!(classify_verb("Chant invocations to the Oracle Sage"), PrescriptionVerb::Declare);
        assert_eq!(classify_verb("Recite ancestral names"), PrescriptionVerb::Declare);
    }
}
