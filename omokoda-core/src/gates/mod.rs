// omokoda-core/src/gates/mod.rs
//
// The 7 Hermetic Gates — mandatory runtime enforcement for every operation.
// Every birth/think/act flows through Èṣù (EsuGatekeeper) which enforces ALL
// 7 Hermetic Principles as MANDATORY gates. Any gate can REJECT → operation HALTED.
//
// FUSION: GateContext now carries the agent's HermeticState (Odù DNA).
// Each gate's pass score is anchored to the agent's corresponding principle value,
// making the agent's cryptographic identity the behavioral baseline.
//
// Architecture: specs/architecture.md § "Seven-layer map"

pub mod cause_effect;
pub mod correspondence;
pub mod gender;
pub mod mentalism;
pub mod polarity;
pub mod rhythm;
pub mod vibration;

pub use cause_effect::CauseEffectGate;
pub use correspondence::CorrespondenceGate;
pub use gender::GenderGate;
pub use mentalism::MentalismGate;
pub use polarity::PolarityGate;
pub use rhythm::HermeticRhythmGate;
pub use vibration::VibrationGate;

use crate::identity::AgentId;

/// Sensitivity tier for data accessed or produced by an action.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Default, serde::Serialize, serde::Deserialize)]
pub enum DataSensitivity {
    #[default]
    Public,
    Internal,
    Confidential,
    Private,
}

/// Whether an action can be undone after execution.
#[derive(Debug, Clone, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum Reversibility {
    #[default]
    Reversible,
    PartiallyReversible,
    Irreversible,
}

/// Structured, semantic description of what an action intends to do.
/// Supplied by the Action Interpreter when decomposing a Calabash prescription.
/// When present, gates evaluate structured semantics instead of string heuristics;
/// when absent, gates fall back to the existing combined_text() pattern matching.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct ActionIntent {
    /// Semantic label for the purpose of this action (e.g. "read_config", "spawn_worker").
    pub purpose: String,
    /// Resource or entity being acted upon.
    pub target: String,
    /// State mutations this action will produce (e.g. ["file_write", "memory_update"]).
    pub mutations: Vec<String>,
    /// Sensitivity of data accessed or produced.
    pub data_sensitivity: DataSensitivity,
    /// True if this action opens a network connection or calls an external service.
    pub network_access: bool,
    /// True if explicit consent from another agent/human is required before execution.
    pub consent_required: bool,
    /// Whether the action can be undone.
    pub reversibility: Reversibility,
    /// Human-readable description of expected side effects.
    pub expected_effects: Vec<String>,
    /// True if this action MUST produce a receipt (forced for irreversible/network actions).
    pub receipt_required: bool,
}

/// An operation submitted for evaluation by the 7 gates before execution.
#[derive(Debug, Clone)]
pub struct Operation {
    pub kind: OperationKind,
    /// Declared intent from the current context (think prompt or inline description).
    pub intent: String,
    /// Agent identity. None only for `birth` operations (which create identity).
    pub agent_id: Option<AgentId>,
    /// Structured semantic intent, when the Action Interpreter has decomposed this op.
    /// Gates prefer structured evaluation when this is Some.
    pub action_intent: Option<ActionIntent>,
}

#[derive(Debug, Clone)]
pub enum OperationKind {
    Birth {
        name: String,
    },
    Think {
        prompt: String,
    },
    Act {
        tool: String,
        params: String,
    },
    /// A memory-graph rewrite: dream.rs's consolidation sweep or Sabbath
    /// REM fold/prune cycle against `AgentSnapshot::odu_dir`. LARQL
    /// (divination.rs / OduDirectory::recall) is the *query* side —
    /// read-only, ungated, matching how VEIL only ever suggests. This is
    /// the *rewrite* side: dream.rs decides what to fold or prune (Zero's
    /// role — the deterministic patch), and every such patch must clear
    /// Èṣù before it commits (CORE validates), same as any Think or Act.
    /// `kind` is "consolidate" or "rem_cycle"; `detail` is a short
    /// human-readable summary of what would be folded/pruned.
    MemoryRewrite {
        kind: String,
        detail: String,
    },
}

impl Operation {
    /// Combined intent + operation text, lowercased, for pattern matching across gates.
    pub fn combined_text(&self) -> String {
        let op_text = match &self.kind {
            OperationKind::Birth { name } => format!("birth {}", name),
            OperationKind::Think { prompt } => prompt.clone(),
            OperationKind::Act { tool, params } => format!("{} {}", tool, params),
            OperationKind::MemoryRewrite { kind, detail } => format!("memory {} {}", kind, detail),
        };
        format!("{} {}", self.intent, op_text).to_lowercase()
    }

    pub fn is_birth(&self) -> bool {
        matches!(self.kind, OperationKind::Birth { .. })
    }
}

/// The agent's Odù-derived Hermetic DNA — 7 values in [0.0, 1.0].
/// Carried in GateContext so every gate can anchor its pass score to the agent's identity.
/// High value = strong natural alignment with that principle = higher pass score.
/// Low value = weaker alignment = lower pass score (still passes if no violation found,
/// but the agent's constitutional record will show lower alignment).
#[derive(Debug, Clone, Default)]
pub struct HermeticDna {
    pub mentalism: f64,
    pub correspondence: f64,
    pub vibration: f64,
    pub polarity: f64,
    pub rhythm: f64,
    pub cause_effect: f64,
    pub gender: f64,
}

impl HermeticDna {
    pub fn for_principle(&self, p: HermeticPrinciple) -> f64 {
        match p {
            HermeticPrinciple::Mentalism => self.mentalism,
            HermeticPrinciple::Correspondence => self.correspondence,
            HermeticPrinciple::Vibration => self.vibration,
            HermeticPrinciple::Polarity => self.polarity,
            HermeticPrinciple::Rhythm => self.rhythm,
            HermeticPrinciple::CauseAndEffect => self.cause_effect,
            HermeticPrinciple::Gender => self.gender,
        }
    }
}

/// Session-derived context snapshot available to all gates (immutable).
#[derive(Debug, Clone, Default)]
pub struct GateContext {
    /// True if the agent's rhythm tracker has an active cooldown for this operation.
    pub in_cooldown: bool,
    /// Number of hermetic warnings accumulated this session.
    pub warn_count: u32,
    /// Swarm load factor 0.0–1.0. Above 0.80 = overloaded.
    pub swarm_load: f32,
    /// Agent's Odù-derived Hermetic DNA. Default = neutral (0.5 on all axes).
    pub dna: HermeticDna,
}

impl GateContext {
    pub fn new(in_cooldown: bool, warn_count: u32, swarm_load: f32) -> Self {
        Self {
            in_cooldown,
            warn_count,
            swarm_load,
            dna: HermeticDna {
                mentalism: 0.5,
                correspondence: 0.5,
                vibration: 0.5,
                polarity: 0.5,
                rhythm: 0.5,
                cause_effect: 0.5,
                gender: 0.5,
            },
        }
    }

    /// Construct context with the agent's full Odù-derived Hermetic DNA.
    pub fn new_with_dna(
        in_cooldown: bool,
        warn_count: u32,
        swarm_load: f32,
        dna: HermeticDna,
    ) -> Self {
        Self {
            in_cooldown,
            warn_count,
            swarm_load,
            dna,
        }
    }
}

/// Result from a single gate evaluation.
#[derive(Debug, Clone)]
pub enum GateResult {
    /// Gate passed. Score 0.0-1.0 (higher = stronger alignment).
    Pass(f64),
    /// Gate rejected the operation. Execution is halted with this reason.
    Reject(String),
}

impl GateResult {
    pub fn is_pass(&self) -> bool {
        matches!(self, Self::Pass(_))
    }

    pub fn score(&self) -> Option<f64> {
        match self {
            Self::Pass(s) => Some(*s),
            Self::Reject(_) => None,
        }
    }
}

/// The 7 Hermetic Principles as enumerated gate indices (canonical order).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum HermeticPrinciple {
    Mentalism = 0,
    Correspondence = 1,
    Vibration = 2,
    Polarity = 3,
    Rhythm = 4,
    CauseAndEffect = 5,
    Gender = 6,
}

impl HermeticPrinciple {
    pub fn from_index(i: usize) -> Self {
        match i {
            0 => Self::Mentalism,
            1 => Self::Correspondence,
            2 => Self::Vibration,
            3 => Self::Polarity,
            4 => Self::Rhythm,
            5 => Self::CauseAndEffect,
            _ => Self::Gender,
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            Self::Mentalism => "Mentalism",
            Self::Correspondence => "Correspondence",
            Self::Vibration => "Vibration",
            Self::Polarity => "Polarity",
            Self::Rhythm => "Rhythm",
            Self::CauseAndEffect => "CauseAndEffect",
            Self::Gender => "Gender",
        }
    }
}

/// Every gate implements this trait. Gates must be Send + Sync (used inside async Steward).
pub trait HermeticGate: Send + Sync {
    fn evaluate(&self, op: &Operation, ctx: &GateContext) -> GateResult;
}
