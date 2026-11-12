use chrono::{Datelike, Timelike, Utc, Weekday};
use serde::{Deserialize, Serialize};

/// The 7-day resonance map derived from ritual-codex.
/// Each day carries a different Hermetic principle emphasis.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum DayResonance {
    SunResonance,     // Sunday — Ọbàtálá (Mentalism, Clarity)
    MoonResonance,    // Monday — Yemọja (Memory, Flow)
    MarsResonance,    // Tuesday — Ògún (Execution, Force)
    MercuryResonance, // Wednesday — Èṣù (Communication, Correspondence)
    JupiterResonance, // Thursday — Ṣàngó (Justice, Expansion)
    VenusResonance,   // Friday — Ọ̀ṣun (Memory, Attraction)
    SaturnResonance,  // Saturday — Ọ̀yá (Flow, Restriction)
}

impl DayResonance {
    pub fn today() -> Self {
        match Utc::now().weekday() {
            Weekday::Sun => Self::SunResonance,
            Weekday::Mon => Self::MoonResonance,
            Weekday::Tue => Self::MarsResonance,
            Weekday::Wed => Self::MercuryResonance,
            Weekday::Thu => Self::JupiterResonance,
            Weekday::Fri => Self::VenusResonance,
            Weekday::Sat => Self::SaturnResonance,
        }
    }

    /// Hermetic principle amplified today (0.0–1.0 bonus multiplier).
    pub fn principle_amplification(&self) -> &'static str {
        match self {
            Self::SunResonance => "mentalism",
            Self::MoonResonance => "vibration",
            Self::MarsResonance => "cause_effect",
            Self::MercuryResonance => "correspondence",
            Self::JupiterResonance => "polarity",
            Self::VenusResonance => "gender",
            Self::SaturnResonance => "rhythm",
        }
    }
}

/// Resonance engine — computes time-based behavioral modulation.
pub struct ResonanceEngine;

impl ResonanceEngine {
    /// Returns a resonance multiplier (0.8–1.2) based on day and hour.
    pub fn current_multiplier() -> f64 {
        let now = Utc::now();
        let hour = now.hour() as f64;
        // Peak resonance at noon (hour 12), trough at midnight
        let hour_factor = 1.0 + 0.2 * ((hour - 12.0) / 12.0 * std::f64::consts::PI).cos();
        // Just normalize: 0.8 to 1.2 across the day
        let normalized = 0.8 + 0.4 * (hour / 24.0);
        (hour_factor * normalized).clamp(0.8, 1.2)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn today_returns_a_valid_resonance() {
        let r = DayResonance::today();
        // Just verifies it computes without panic
        let _ = r.principle_amplification();
    }

    #[test]
    fn resonance_multiplier_in_range() {
        let m = ResonanceEngine::current_multiplier();
        assert!(
            (0.8..=1.2).contains(&m),
            "multiplier {m} out of range [0.8, 1.2]"
        );
    }

    #[test]
    fn all_resonances_have_principles() {
        let days = [
            DayResonance::SunResonance,
            DayResonance::MoonResonance,
            DayResonance::MarsResonance,
            DayResonance::MercuryResonance,
            DayResonance::JupiterResonance,
            DayResonance::VenusResonance,
            DayResonance::SaturnResonance,
        ];
        for day in &days {
            assert!(!day.principle_amplification().is_empty());
        }
    }
}
