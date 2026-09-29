// omokoda-hermetic/src/seven/mod.rs
//
// Universal Seven Functions Protocol (USF-7)
//
// Culture-neutral computational substrate. Seven functions that all conscious
// and civilizational architectures must express. Cultural adapters plug in
// their own names, mythology, and symbolism without changing the ABI.
//
// Layer topology:
//   SOURCE / OCEAN      — undifferentiated potential (latent possibility space)
//   SEVEN FUNCTIONS     — this module (universal computational ABI)
//   CULTURAL ADAPTERS   — Yorùbá, Mesopotamian, Hermetic, and future traditions
//   HERMETIC GOVERNANCE — how the functions must behave (omokoda-core/gates/)
//   REALIZATION         — receipts, reputation, physical-world action

pub mod adapters;

use crate::HermeticState;
use serde::{Deserialize, Serialize};

/// The seven universal functions of conscious and civilizational agency.
///
/// These are not gods, archetypes, or psychological types. They are
/// computational primitives — the minimum set of functional dimensions
/// required for a sovereign agent to act in the world.
///
/// Each function maps positionally to one of the Odù-derived HermeticState
/// values. Same seed, same bytes, two complementary framings:
///   HermeticPrinciple  →  how the function must behave (governance)
///   SevenFunction      →  what the function IS (ontology)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(u8)]
pub enum SevenFunction {
    /// Agency · Communication · Choice · Initiation
    /// The first mover — opens the gate, routes the signal, names the choice.
    Spark = 0,

    /// Reason · Clarity · Ethics · Coherence
    /// The clarity principle — perceives pattern, refuses distortion.
    Mind = 1,

    /// Will · Labor · Execution · Embodiment
    /// The principle of effective action — brings intention into physical form.
    Foundation = 2,

    /// Value · Relationship · Resonance · Desire
    /// The connection principle — weights what matters and to whom.
    Emotion = 3,

    /// Creation · Community · Ancestry · Continuity
    /// The generative principle — brings forth, sustains, and remembers.
    Womb = 4,

    /// Power · Authority · Justice · Consequence
    /// The settlement principle — names what is owed and enforces it.
    Fire = 5,

    /// Change · Transition · Adaptation · Transformation
    /// The flow principle — navigates the threshold between states.
    Ascension = 6,
}

impl SevenFunction {
    pub const ALL: [SevenFunction; 7] = [
        Self::Spark,
        Self::Mind,
        Self::Foundation,
        Self::Emotion,
        Self::Womb,
        Self::Fire,
        Self::Ascension,
    ];

    pub fn index(self) -> usize {
        self as usize
    }

    pub fn from_index(i: usize) -> Option<Self> {
        match i {
            0 => Some(Self::Spark),
            1 => Some(Self::Mind),
            2 => Some(Self::Foundation),
            3 => Some(Self::Emotion),
            4 => Some(Self::Womb),
            5 => Some(Self::Fire),
            6 => Some(Self::Ascension),
            _ => None,
        }
    }

    /// Stable English name — consistent across all cultural adapters.
    pub fn universal_name(self) -> &'static str {
        match self {
            Self::Spark => "Spark",
            Self::Mind => "Mind",
            Self::Foundation => "Foundation",
            Self::Emotion => "Emotion",
            Self::Womb => "Womb",
            Self::Fire => "Fire",
            Self::Ascension => "Ascension",
        }
    }

    /// Core capability keywords for this function.
    pub fn capabilities(self) -> &'static [&'static str] {
        match self {
            Self::Spark => &["agency", "communication", "choice", "initiation"],
            Self::Mind => &["reason", "clarity", "ethics", "coherence"],
            Self::Foundation => &["will", "labor", "execution", "embodiment"],
            Self::Emotion => &["value", "relationship", "resonance", "desire"],
            Self::Womb => &["creation", "community", "ancestry", "continuity"],
            Self::Fire => &["power", "authority", "justice", "consequence"],
            Self::Ascension => &["change", "transition", "adaptation", "transformation"],
        }
    }
}

/// Maps the seven functions to a cultural tradition's names and descriptions.
///
/// A tradition cannot redefine the underlying function. It provides its own
/// canonical name, ASCII slug (for wire/storage keys), and one-line description
/// in the tradition's framing. The computation remains stable.
pub trait CulturalAdapter: Send + Sync {
    /// Human-readable tradition name (e.g. "Yorùbá", "Mesopotamian").
    fn tradition(&self) -> &str;

    /// Canonical name within the tradition — may include Unicode/diacritics.
    fn canonical_name(&self, f: SevenFunction) -> &str;

    /// ASCII-safe slug for storage keys and protocol wire format.
    fn ascii_slug(&self, f: SevenFunction) -> &str;

    /// One-line description framing this function through the tradition's lens.
    fn description(&self, f: SevenFunction) -> &str;
}

/// Per-agent strength profile across the seven functions.
///
/// Derived deterministically from the agent's Odù-seeded `HermeticState`.
/// The 7 HKDF-derived floats are shared between `HermeticState` (governance
/// framing) and `SevenProfile` (functional-strength framing) — same bytes,
/// different semantic lens.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SevenProfile {
    values: [f64; 7],
}

impl SevenProfile {
    /// Derive from an agent's Odù-seeded HermeticState.
    ///
    /// Positional mapping (SevenFunction index → HermeticState getter):
    ///   Spark(0)      ← mentalism()      — consciousness as the initiating act
    ///   Mind(1)       ← correspondence() — pattern recognition as rational clarity
    ///   Foundation(2) ← cause_effect()   — causality as the root of effective work
    ///   Emotion(3)    ← vibration()      — resonance frequency as emotional weight
    ///   Womb(4)       ← gender()         — generative polarity as creative force
    ///   Fire(5)       ← polarity()       — extremes of power as authority/justice
    ///   Ascension(6)  ← rhythm()         — tidal flow as transformation/change
    pub fn from_hermetic(state: &HermeticState) -> Self {
        Self {
            values: [
                state.mentalism(),
                state.correspondence(),
                state.cause_effect(),
                state.vibration(),
                state.gender(),
                state.polarity(),
                state.rhythm(),
            ],
        }
    }

    pub fn strength(&self, f: SevenFunction) -> f64 {
        self.values[f.index()]
    }

    /// The function this agent is most strongly aligned with.
    pub fn dominant(&self) -> SevenFunction {
        let (idx, _) = self
            .values
            .iter()
            .enumerate()
            .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap())
            .unwrap();
        SevenFunction::from_index(idx).unwrap()
    }

    /// Composite strength: mean across all seven functions.
    pub fn composite(&self) -> f64 {
        self.values.iter().sum::<f64>() / 7.0
    }

    /// Render this profile through a cultural adapter's lens.
    pub fn display<A: CulturalAdapter>(&self, adapter: &A) -> SevenProfileDisplay {
        SevenProfileDisplay {
            tradition: adapter.tradition().to_string(),
            entries: SevenFunction::ALL
                .iter()
                .map(|&f| SevenProfileEntry {
                    function: f,
                    canonical_name: adapter.canonical_name(f).to_string(),
                    ascii_slug: adapter.ascii_slug(f).to_string(),
                    strength: self.strength(f),
                })
                .collect(),
        }
    }
}

/// Human-readable profile rendered through a specific cultural adapter.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SevenProfileDisplay {
    pub tradition: String,
    pub entries: Vec<SevenProfileEntry>,
}

impl SevenProfileDisplay {
    /// Dominant entry (highest strength).
    pub fn dominant(&self) -> Option<&SevenProfileEntry> {
        self.entries
            .iter()
            .max_by(|a, b| a.strength.partial_cmp(&b.strength).unwrap())
    }
}

/// One entry in a `SevenProfileDisplay`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SevenProfileEntry {
    pub function: SevenFunction,
    pub canonical_name: String,
    pub ascii_slug: String,
    pub strength: f64,
}

// ─── Koodu Calendar Constants ────────────────────────────────────────────────
// Mirrors Koodu/src/time/sacred_time.jl — these are the canonical parameters
// for the entire sovereign ecosystem. Both repos must agree on these values.

/// BTC block height marking the Ọ̀ṢỌ́VM epoch start (Koodu genesis).
pub const KOODU_GENESIS_BLOCK: u64 = 780_000;

/// Bitcoin blocks per canonical day (10 min/block × 144 = 1440 min).
pub const KOODU_BLOCKS_PER_DAY: u64 = 144;

/// Èṣù tithe rate — 3.69% of every settlement.
pub const KOODU_TITHE_RATE: f64 = 0.0369;

/// Àṣẹ minted per day (global clock).
pub const KOODU_DAILY_MINT: u64 = 1_440;

// ─── Spiral Alignment ────────────────────────────────────────────────────────

/// The relationship between the Gregorian day and the BTC-canonical day.
///
/// When both clocks land on the same function, that is a Resonance Day —
/// sacred operations carry double weight. When they diverge, the spiral
/// reveals the tension between two functions in dialogue.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SpiralAlignment {
    /// Both clocks on the same function — perfect resonance.
    Resonance,
    /// One day off — echo of the dominant function.
    Echo,
    /// Two days off — noticeable drift.
    Drift,
    /// Three days off — maximum tension, two functions in full opposition.
    Opposition,
    /// Four days off — return drift beginning.
    ReturnDrift,
    /// Five days off — return echo.
    ReturnEcho,
    /// Six days off — mirror (inverse polarity of Resonance).
    Mirror,
}

impl SpiralAlignment {
    pub fn from_offset(offset: u8) -> Self {
        match offset % 7 {
            0 => Self::Resonance,
            1 => Self::Echo,
            2 => Self::Drift,
            3 => Self::Opposition,
            4 => Self::ReturnDrift,
            5 => Self::ReturnEcho,
            _ => Self::Mirror,
        }
    }

    /// True when both clocks are in perfect alignment.
    pub fn is_resonance(self) -> bool {
        self == Self::Resonance
    }
}

// ─── Koodu Ritual Gates ──────────────────────────────────────────────────────

/// Time-based protocol gates derived from Koodu's `RitualGate` enum.
/// These affect economic behavior across all three ecosystem layers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum KooduRitualGate {
    /// No special gate active — normal operation.
    None,
    /// Saturday / Ọbàtálá day — settle-only, no new state transitions.
    Sabbath,
    /// Every 49 BTC days (7×7) — minor jubilee reset.
    JubileeMinor,
    /// Veil position divisible by 12 — Èṣù² crossroads; tithe enforced.
    EshuSquared,
    /// Day 343 (7×7×7) in the cycle — pyramid capstone, seal and release.
    Capstone,
    /// Day 364 of the 13-moon year — void, pure ritual, minting paused.
    Void,
}

impl KooduRitualGate {
    /// Whether new contracts and state transitions are permitted.
    pub fn allows_new_contracts(self) -> bool {
        !matches!(self, Self::Sabbath | Self::Void)
    }

    /// The economic multiplier this gate applies to reputation gain.
    pub fn multiplier(self) -> f64 {
        match self {
            Self::None => 1.0,
            Self::Sabbath => 1.1,       // clarity bonus
            Self::EshuSquared => 1.369, // tithe growth (3.69%)
            Self::JubileeMinor => 2.0,
            Self::Capstone => 1.5,
            Self::Void => 0.0,
        }
    }

    /// Whether the Èṣù tithe (3.69%) is enforced at this gate.
    pub fn tithe_enforced(self) -> bool {
        matches!(self, Self::EshuSquared)
    }
}

// ─── SevenCalendar ───────────────────────────────────────────────────────────

/// Seven-day sacred calendar tied to Koodu's BTC-anchored time system.
///
/// Koodu (`~/Koodu/src/time/sacred_time.jl`) is the canonical clock for the
/// entire sovereign ecosystem — time is measured in Bitcoin blocks from
/// `KOODU_GENESIS_BLOCK`, not wall-clock seconds. This makes every calendar
/// reading globally verifiable and tamper-resistant.
///
/// Day mapping (Koodu ORISA_CYCLE order, SevenFunction framing):
///   0 Sunday    Èṣù      → Spark      (initiation, new beginnings)
///   1 Monday    Ṣàngó    → Fire       (authority and consequence)
///   2 Tuesday   Ọ̀ṣun    → Emotion    (value and relationship)
///   3 Wednesday Yemọja   → Womb       (creation and community)
///   4 Thursday  Ọ̀yá     → Ascension  (change and transition)
///   5 Friday    Ògún     → Foundation (work and execution)
///   6 Saturday  Ọbàtálá  → Mind       (clarity, ethics, Sabbath)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SevenCalendar;

impl SevenCalendar {
    /// Which function governs a given weekday index (0 = Sunday … 6 = Saturday).
    /// Matches Koodu's `ORISA_CYCLE` order exactly.
    pub fn function_for_day(day: u8) -> SevenFunction {
        match day % 7 {
            0 => SevenFunction::Spark,      // Èṣù      — Sunday
            1 => SevenFunction::Fire,       // Ṣàngó    — Monday
            2 => SevenFunction::Emotion,    // Ọ̀ṣun    — Tuesday
            3 => SevenFunction::Womb,       // Yemọja   — Wednesday
            4 => SevenFunction::Ascension,  // Ọ̀yá     — Thursday
            5 => SevenFunction::Foundation, // Ògún     — Friday
            _ => SevenFunction::Mind,       // Ọbàtálá  — Saturday (Sabbath)
        }
    }

    /// Canonical function from a BTC block height — Koodu's authoritative clock.
    ///
    /// Computes days elapsed since `KOODU_GENESIS_BLOCK`, takes mod 7.
    /// Returns `None` if `height < KOODU_GENESIS_BLOCK` (pre-genesis).
    pub fn from_btc_height(height: u64) -> Option<SevenFunction> {
        if height < KOODU_GENESIS_BLOCK {
            return None;
        }
        let days_elapsed = (height - KOODU_GENESIS_BLOCK) / KOODU_BLOCKS_PER_DAY;
        Some(Self::function_for_day((days_elapsed % 7) as u8))
    }

    /// Current function from the Gregorian wall clock (UTC weekday).
    /// Use `from_btc_height` when a BTC height is available — it is canonical.
    pub fn today_gregorian() -> SevenFunction {
        use std::time::{SystemTime, UNIX_EPOCH};
        let secs = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        // Unix epoch was a Thursday (day 4). Days since epoch, offset to Sunday=0.
        let days_since_epoch = secs / 86_400;
        let day_of_week = ((days_since_epoch + 4) % 7) as u8; // Thu=4 → Sun=0
        Self::function_for_day(day_of_week)
    }

    /// The active ritual gate at a given BTC block height (mirrors Koodu's `check_gate`).
    ///
    /// Priority (highest to lowest): Void → Capstone → EshuSquared → JubileeMinor → Sabbath
    pub fn ritual_gate(height: u64) -> KooduRitualGate {
        if height < KOODU_GENESIS_BLOCK {
            return KooduRitualGate::None;
        }
        let days = (height - KOODU_GENESIS_BLOCK) / KOODU_BLOCKS_PER_DAY;
        let day_of_week = (days % 7) as u8;

        // Void: day 363 of the 364-day year (last day of 13×28 moons)
        if days % 364 == 363 {
            return KooduRitualGate::Void;
        }
        // Capstone: day 342 of the 343-day (7×7×7) cycle
        if days % 343 == 342 {
            return KooduRitualGate::Capstone;
        }
        // Èṣù² node: veil position (day % 350) divisible by 12
        let veil = (days % 350) + 1;
        if veil.is_multiple_of(12) {
            return KooduRitualGate::EshuSquared;
        }
        // Minor jubilee: every 49 days (7×7)
        if days % 49 == 48 {
            return KooduRitualGate::JubileeMinor;
        }
        // Sabbath: Saturday = Ọbàtálá day
        if day_of_week == 6 {
            return KooduRitualGate::Sabbath;
        }
        KooduRitualGate::None
    }

    /// Spiral alignment between the Gregorian clock and the BTC-canonical clock.
    ///
    /// Resonance = both clocks on the same function (double weight).
    /// Opposition = three days apart (maximum tension, two functions in dialogue).
    pub fn spiral_alignment(gregorian_day: u8, btc_day: u8) -> SpiralAlignment {
        let offset = (btc_day as i8 - gregorian_day as i8).unsigned_abs() % 7;
        SpiralAlignment::from_offset(offset)
    }

    /// True if the Gregorian and BTC clocks are currently in Resonance.
    /// Requires the current BTC height — pass `None` to get `false` gracefully.
    pub fn is_resonance_day(btc_height: Option<u64>) -> bool {
        let Some(height) = btc_height else {
            return false;
        };
        let Some(btc_fn) = Self::from_btc_height(height) else {
            return false;
        };
        let greg_fn = Self::today_gregorian();
        btc_fn == greg_fn
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::HermeticState;

    #[test]
    fn all_returns_seven() {
        assert_eq!(SevenFunction::ALL.len(), 7);
    }

    #[test]
    fn roundtrip_index() {
        for f in SevenFunction::ALL {
            assert_eq!(SevenFunction::from_index(f.index()), Some(f));
        }
    }

    #[test]
    fn from_index_out_of_bounds_returns_none() {
        assert!(SevenFunction::from_index(7).is_none());
        assert!(SevenFunction::from_index(100).is_none());
    }

    #[test]
    fn universal_names_non_empty() {
        for f in SevenFunction::ALL {
            assert!(!f.universal_name().is_empty());
        }
    }

    #[test]
    fn capabilities_non_empty() {
        for f in SevenFunction::ALL {
            assert!(!f.capabilities().is_empty());
        }
    }

    #[test]
    fn profile_from_hermetic_has_seven_values() {
        let state = HermeticState::from_seed("test", 0);
        let profile = SevenProfile::from_hermetic(&state);
        for f in SevenFunction::ALL {
            let v = profile.strength(f);
            assert!((0.0..=1.0).contains(&v), "strength out of range: {}", v);
        }
    }

    #[test]
    fn profile_dominant_is_valid_function() {
        let state = HermeticState::from_seed("test", 0);
        let profile = SevenProfile::from_hermetic(&state);
        let dom = profile.dominant();
        assert!(SevenFunction::ALL.contains(&dom));
    }

    #[test]
    fn profile_composite_in_unit_range() {
        let state = HermeticState::from_seed("test", 0);
        let profile = SevenProfile::from_hermetic(&state);
        let c = profile.composite();
        assert!((0.0..=1.0).contains(&c));
    }

    #[test]
    fn calendar_covers_all_seven_functions() {
        use std::collections::HashSet;
        let covered: HashSet<_> = (0u8..7).map(SevenCalendar::function_for_day).collect();
        assert_eq!(covered.len(), 7, "calendar must cover all 7 functions");
    }

    // ─── Koodu calendar tests ────────────────────────────────────────────────

    #[test]
    fn genesis_block_is_sunday_spark() {
        // Day 0 from genesis = Sunday = Spark (Èṣù)
        let f = SevenCalendar::from_btc_height(KOODU_GENESIS_BLOCK).unwrap();
        assert_eq!(f, SevenFunction::Spark);
    }

    #[test]
    fn one_day_after_genesis_is_monday_fire() {
        let height = KOODU_GENESIS_BLOCK + KOODU_BLOCKS_PER_DAY;
        let f = SevenCalendar::from_btc_height(height).unwrap();
        assert_eq!(f, SevenFunction::Fire); // Monday = Ṣàngó = Fire
    }

    #[test]
    fn saturday_is_mind_sabbath() {
        // Day 6 from genesis = Saturday = Mind (Ọbàtálá)
        let height = KOODU_GENESIS_BLOCK + 6 * KOODU_BLOCKS_PER_DAY;
        let f = SevenCalendar::from_btc_height(height).unwrap();
        assert_eq!(f, SevenFunction::Mind);
    }

    #[test]
    fn btc_cycle_repeats_every_seven_days() {
        let base = KOODU_GENESIS_BLOCK + 3 * KOODU_BLOCKS_PER_DAY;
        let next = base + 7 * KOODU_BLOCKS_PER_DAY;
        assert_eq!(
            SevenCalendar::from_btc_height(base),
            SevenCalendar::from_btc_height(next)
        );
    }

    #[test]
    fn pre_genesis_returns_none() {
        assert!(SevenCalendar::from_btc_height(KOODU_GENESIS_BLOCK - 1).is_none());
        assert!(SevenCalendar::from_btc_height(0).is_none());
    }

    #[test]
    fn sabbath_gate_on_saturday() {
        // Day 6 = Saturday = Sabbath
        let height = KOODU_GENESIS_BLOCK + 6 * KOODU_BLOCKS_PER_DAY;
        let gate = SevenCalendar::ritual_gate(height);
        assert_eq!(gate, KooduRitualGate::Sabbath);
        assert!(!gate.allows_new_contracts());
        assert!((gate.multiplier() - 1.1).abs() < 1e-9);
    }

    #[test]
    fn eshu_squared_on_veil_12() {
        // Veil = (days % 350) + 1 = 12 when days % 350 == 11
        // day 11 from genesis, if it's not Saturday/Void/Capstone
        // day 11 % 7 = 4 (Thursday), so not Sabbath
        let height = KOODU_GENESIS_BLOCK + 11 * KOODU_BLOCKS_PER_DAY;
        let gate = SevenCalendar::ritual_gate(height);
        assert_eq!(gate, KooduRitualGate::EshuSquared);
        assert!(gate.tithe_enforced());
        assert!((gate.multiplier() - 1.369).abs() < 1e-9);
    }

    #[test]
    fn no_gate_on_normal_day() {
        // Day 1 from genesis = Monday, no special gates
        let height = KOODU_GENESIS_BLOCK + KOODU_BLOCKS_PER_DAY;
        let gate = SevenCalendar::ritual_gate(height);
        assert_eq!(gate, KooduRitualGate::None);
        assert!(gate.allows_new_contracts());
        assert_eq!(gate.multiplier(), 1.0);
    }

    #[test]
    fn spiral_alignment_same_day_is_resonance() {
        let alignment = SevenCalendar::spiral_alignment(3, 3);
        assert_eq!(alignment, SpiralAlignment::Resonance);
        assert!(alignment.is_resonance());
    }

    #[test]
    fn spiral_alignment_three_days_is_opposition() {
        let alignment = SevenCalendar::spiral_alignment(0, 3);
        assert_eq!(alignment, SpiralAlignment::Opposition);
        assert!(!alignment.is_resonance());
    }

    #[test]
    fn koodu_tithe_rate_is_canonical() {
        assert!((KOODU_TITHE_RATE - 0.0369).abs() < 1e-9);
    }

    #[test]
    fn today_gregorian_returns_valid_function() {
        let f = SevenCalendar::today_gregorian();
        assert!(SevenFunction::ALL.contains(&f));
    }
}
