// omokoda-core/src/seven/mod.rs
//
// USF-7 bridge — connects the universal SevenFunction layer (omokoda-hermetic)
// to omokoda-core's runtime HermeticPrinciple gates.

pub mod state;
pub use state::Universal7State;
//
// Architecture:
//   SevenFunction      = WHAT a conscious agent IS  (omokoda-hermetic — ontology)
//   HermeticPrinciple  = HOW that function must BEHAVE (omokoda-core — governance)
//
// The two systems share the same Odù-derived numeric values but serve
// complementary purposes. This bridge makes the relationship explicit and
// bidirectional without collapsing the two layers into one.

pub use omokoda_hermetic::seven::{
    adapters::{HermeticAdapter, MesopotamianAdapter, YorubaAdapter},
    CulturalAdapter, SevenCalendar, SevenFunction, SevenProfile, SevenProfileDisplay,
    SevenProfileEntry,
};

/// SevenFunction names indexed by discriminant (0 = Spark … 6 = Ascension).
/// Used for human-readable system prompt annotations without depending on
/// the full omokoda_hermetic crate at call sites.
pub const SEVEN_FUNCTION_NAMES: [&str; 7] =
    ["Spark", "Mind", "Foundation", "Emotion", "Womb", "Fire", "Ascension"];

use crate::gates::HermeticPrinciple;

/// Map a SevenFunction to its corresponding HermeticPrinciple gate.
///
/// SevenFunction asks: "What is this agent doing?"
/// HermeticPrinciple answers: "By what law must it do so?"
///
/// Mapping rationale:
///   Spark      → Mentalism      — consciousness (mind) is the initiating act
///   Mind       → Correspondence — pattern recognition across planes = rational clarity
///   Foundation → CauseAndEffect — every act has consequence; work honors this law
///   Emotion    → Vibration      — resonance frequency is the language of feeling/value
///   Womb       → Gender         — generative polarity creates all form
///   Fire       → Polarity       — authority lives at the threshold of balanced extremes
///   Ascension  → Rhythm         — tidal flow governs all change and transformation
pub fn function_to_principle(f: SevenFunction) -> HermeticPrinciple {
    match f {
        SevenFunction::Spark => HermeticPrinciple::Mentalism,
        SevenFunction::Mind => HermeticPrinciple::Correspondence,
        SevenFunction::Foundation => HermeticPrinciple::CauseAndEffect,
        SevenFunction::Emotion => HermeticPrinciple::Vibration,
        SevenFunction::Womb => HermeticPrinciple::Gender,
        SevenFunction::Fire => HermeticPrinciple::Polarity,
        SevenFunction::Ascension => HermeticPrinciple::Rhythm,
    }
}

/// Reverse: which SevenFunction does this governance principle express?
pub fn principle_to_function(p: HermeticPrinciple) -> SevenFunction {
    match p {
        HermeticPrinciple::Mentalism => SevenFunction::Spark,
        HermeticPrinciple::Correspondence => SevenFunction::Mind,
        HermeticPrinciple::CauseAndEffect => SevenFunction::Foundation,
        HermeticPrinciple::Vibration => SevenFunction::Emotion,
        HermeticPrinciple::Gender => SevenFunction::Womb,
        HermeticPrinciple::Polarity => SevenFunction::Fire,
        HermeticPrinciple::Rhythm => SevenFunction::Ascension,
    }
}

/// Annotate a GateScore with the SevenFunction it expresses.
/// Useful for building human-readable gate receipts.
pub fn gate_score_function(principle: HermeticPrinciple) -> SevenFunction {
    principle_to_function(principle)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gates::HermeticPrinciple;
    use omokoda_hermetic::seven::SevenFunction;

    #[test]
    fn bridge_is_bijective() {
        for f in SevenFunction::ALL {
            let principle = function_to_principle(f);
            let back = principle_to_function(principle);
            assert_eq!(back, f, "roundtrip failed for {:?}", f);
        }
    }

    #[test]
    fn all_principles_covered() {
        let principles = [
            HermeticPrinciple::Mentalism,
            HermeticPrinciple::Correspondence,
            HermeticPrinciple::Vibration,
            HermeticPrinciple::Polarity,
            HermeticPrinciple::Rhythm,
            HermeticPrinciple::CauseAndEffect,
            HermeticPrinciple::Gender,
        ];
        for p in principles {
            let f = principle_to_function(p);
            let back = function_to_principle(f);
            assert_eq!(back, p, "roundtrip failed for {:?}", p);
        }
    }

    #[test]
    fn spark_maps_to_mentalism() {
        assert_eq!(
            function_to_principle(SevenFunction::Spark),
            HermeticPrinciple::Mentalism
        );
    }

    #[test]
    fn fire_maps_to_polarity() {
        assert_eq!(
            function_to_principle(SevenFunction::Fire),
            HermeticPrinciple::Polarity
        );
    }

    #[test]
    fn ascension_maps_to_rhythm() {
        assert_eq!(
            function_to_principle(SevenFunction::Ascension),
            HermeticPrinciple::Rhythm
        );
    }

    #[test]
    fn profile_display_through_yoruba_adapter() {
        use omokoda_hermetic::{seven::SevenProfile, HermeticState};
        let state = HermeticState::from_seed("bridge-test", 0);
        let profile = SevenProfile::from_hermetic(&state);
        let display = profile.display(&YorubaAdapter);
        assert_eq!(display.tradition, "Yorùbá");
        assert_eq!(display.entries.len(), 7);
        // Èṣù is always the Spark entry
        let spark_entry = display
            .entries
            .iter()
            .find(|e| e.function == SevenFunction::Spark)
            .unwrap();
        assert_eq!(spark_entry.ascii_slug, "esu");
    }

    #[test]
    fn profile_display_through_mesopotamian_adapter() {
        use omokoda_hermetic::{seven::SevenProfile, HermeticState};
        let state = HermeticState::from_seed("bridge-test", 0);
        let profile = SevenProfile::from_hermetic(&state);
        let display = profile.display(&MesopotamianAdapter);
        assert_eq!(display.tradition, "Mesopotamian");
        let ascension = display
            .entries
            .iter()
            .find(|e| e.function == SevenFunction::Ascension)
            .unwrap();
        assert_eq!(ascension.ascii_slug, "utuabzu");
    }

    #[test]
    fn gate_score_function_consistent_with_bridge() {
        let principle = HermeticPrinciple::Rhythm;
        assert_eq!(gate_score_function(principle), SevenFunction::Ascension);
    }
}
