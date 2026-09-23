// HiveBreath Protocol — Omokoda macro hive-mind (H0–H8)
//
// Three-layer model (from POLYGLOT ORGANISM ARCHITECTURE):
//   Layer 1: Twelve-thrones — 12-model epistemology court (AI reasoning)
//   Layer 2: Omokoda — macro hive coordinator (this crate)
//   Layer 3: omokoda-agent — micro individual agents
//
// The HiveBreath cycle drives all three layers through RitualPhase state
// transitions and accumulates EpistemicDelta — the divergence between what
// the hive believes and what the simulation has proven.

pub mod breath;
pub mod epistemic;
pub mod federation;
pub mod goal_vector;
pub mod lobe;
pub mod memory_health;
pub mod oso_bridge;
pub mod osovm_bridge;
pub mod ritual;
pub mod stewardship;
pub mod thrones;

pub use breath::{HiveBreath, HiveBreathConfig};
pub use epistemic::{EpistemicDelta, EpistemicState};
pub use goal_vector::{AggregatedGoal, GoalVector, GoalVectorSynthesizer, LobeGoalProposal};
pub use lobe::{LobeAgent, OrisaLobe};
pub use ritual::{RitualPhase, RitualTransition};
pub use stewardship::StewardshipInvariant;
pub use thrones::{DeliberationRequest, DeliberationResult, ThronesClient, ThroneVerdict};
pub use federation::{HiveFederationClient, HiveFederationMessage, merge_peer_goal_vector};
pub use memory_health::{MemoryHealthMonitor, MemoryHealthReport};
pub use oso_bridge::{OsoHiveCompiler, OsoHiveDirective};
pub use osovm_bridge::{OsovmHiveBridge, SimulationRequest, SimulationResult, ZangbetoReceipt};
