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
pub mod lobe;
pub mod ritual;
pub mod stewardship;

pub use breath::{HiveBreath, HiveBreathConfig};
pub use epistemic::{EpistemicDelta, EpistemicState};
pub use lobe::{LobeAgent, OrisaLobe};
pub use ritual::{RitualPhase, RitualTransition};
pub use stewardship::StewardshipInvariant;
