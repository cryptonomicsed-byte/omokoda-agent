use crate::emotion::EmotionState;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

/// IRIS routing profile — determines how the `think` primitive executes.
/// Each profile sets LLM temperature, token budget, and response style guidance.
/// Routing profiles govern how the think primitive executes; IRIS only operates during `think`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum IrisProfile {
    /// Short, fast, reflexive — for greetings, one-word answers, simple lookups
    Reflex,
    /// Between Reflex and Balanced — short action-oriented replies for mid-length non-technical prompts
    Fast,
    /// Default — balanced reasoning for most requests
    Balanced,
    /// Technical precision — code, debugging, architecture, precise analysis
    Sharp,
    /// Deep reasoning — complex problems, creative synthesis, long-form planning
    Deep,
    /// Empathetic, warm — for distress signals, emotional context, personal topics
    Gentle,
}

impl IrisProfile {
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Reflex => "reflex",
            Self::Fast => "fast",
            Self::Balanced => "balanced",
            Self::Sharp => "sharp",
            Self::Deep => "deep",
            Self::Gentle => "gentle",
        }
    }
}

/// LLM execution parameters derived from an IRIS profile.
#[derive(Debug, Clone, PartialEq)]
pub struct IrisParams {
    pub profile: IrisProfile,
    /// Sampling temperature (lower = more deterministic)
    pub temperature: f32,
    /// Maximum token budget for the LLM response
    pub max_tokens: u32,
    /// Style injection injected into the system prompt
    pub style_guidance: &'static str,
    /// Whether to add warmth-boosting language to the system prompt
    pub warmth_boost: bool,
}

impl IrisParams {
    fn for_profile(profile: IrisProfile, warmth: bool) -> Self {
        let (temp, tokens, style) = match profile {
            IrisProfile::Reflex => (0.3, 256, "Be extremely concise. One sentence max."),
            IrisProfile::Fast => (0.5, 512, "Direct, 2-3 lines max. Action-oriented."),
            IrisProfile::Balanced => (0.7, 1024, "Balance clarity and depth."),
            IrisProfile::Sharp => (
                0.2,
                2048,
                "Maximum technical precision. Show your reasoning.",
            ),
            IrisProfile::Deep => (
                0.8,
                4096,
                "Think deeply. Explore all angles before concluding.",
            ),
            IrisProfile::Gentle => (
                0.9,
                1024,
                "Lead with empathy. Be warm, patient, and supportive.",
            ),
        };
        Self {
            profile,
            temperature: temp,
            max_tokens: tokens,
            style_guidance: style,
            warmth_boost: warmth,
        }
    }
}

/// One recorded routing decision, stored for pattern analysis.
#[derive(Debug, Clone)]
pub struct IrisDecision {
    pub profile: IrisProfile,
    pub energy: f32,
    pub tension: f32,
}

/// IRIS routing engine — determines the optimal `think` execution profile
/// from prompt content and current emotional state.
///
/// Priority order:
/// 1. Fatigue override — low energy forces Balanced (never Deep when drained)
/// 2. Distress/emotional signals → Gentle
/// 3. Technical signals → Sharp
/// 4. Prompt length heuristic → Deep if > 200 chars
/// 5. Mid-length non-technical → Fast
/// 6. Short prompt → Reflex
/// 7. Default → Balanced
pub struct IrisEngine {
    /// Rolling window of last 200 routing decisions for pattern analysis
    decisions: VecDeque<IrisDecision>,
}

impl Default for IrisEngine {
    fn default() -> Self {
        Self {
            decisions: VecDeque::with_capacity(200),
        }
    }
}

impl IrisEngine {
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a routing decision for pattern learning.
    pub fn record_decision(&mut self, profile: IrisProfile, emotion: &EmotionState) {
        if self.decisions.len() >= 200 {
            self.decisions.pop_front();
        }
        self.decisions.push_back(IrisDecision {
            profile,
            energy: emotion.energy,
            tension: emotion.tension,
        });
    }

    /// Returns a profile distribution summary string for logging.
    #[must_use]
    pub fn get_stats(&self) -> String {
        let total = self.decisions.len();
        if total == 0 {
            return "no decisions recorded".to_string();
        }
        let mut counts = [0usize; 6]; // reflex, fast, balanced, sharp, deep, gentle
        for d in &self.decisions {
            let idx = match d.profile {
                IrisProfile::Reflex => 0,
                IrisProfile::Fast => 1,
                IrisProfile::Balanced => 2,
                IrisProfile::Sharp => 3,
                IrisProfile::Deep => 4,
                IrisProfile::Gentle => 5,
            };
            counts[idx] += 1;
        }
        let labels = ["reflex", "fast", "balanced", "sharp", "deep", "gentle"];
        let parts: Vec<String> = labels
            .iter()
            .zip(counts.iter())
            .filter(|(_, &c)| c > 0)
            .map(|(l, c)| format!("{}:{}", l, c))
            .collect();
        format!("iris_stats({} total) {}", total, parts.join(" "))
    }

    /// Select the routing profile for this `think` invocation.
    #[must_use]
    pub fn route(prompt: &str, emotion: &EmotionState) -> IrisProfile {
        let lower = prompt.to_ascii_lowercase();

        // Energy override — a fatigued agent can't go deep
        if emotion.is_fatigued() {
            if emotion.is_tense() {
                return IrisProfile::Gentle;
            }
            return IrisProfile::Balanced;
        }

        // Distress signals → Gentle
        let distress = [
            "exhausted",
            "stressed",
            "overwhelmed",
            "sad",
            "anxious",
            "hurt",
            "scared",
            "alone",
            "desperate",
            "hopeless",
            "crying",
            "worried",
            "afraid",
            "confused about feelings",
        ];
        if distress.iter().any(|d| lower.contains(d)) {
            return IrisProfile::Gentle;
        }

        // Emotional tension override → Gentle even if prompt looks technical
        if emotion.is_tense() && emotion.connection > 0.5 {
            return IrisProfile::Gentle;
        }

        // Technical/code signals → Sharp
        let technical = [
            "error",
            "bug",
            "fix",
            "debug",
            "implement",
            "compile",
            "build",
            "test",
            "deploy",
            "function",
            "struct",
            "algorithm",
            "optimize",
            "refactor",
            "architecture",
            "api",
            "database",
            "exception",
            "panic",
            "crash",
            "stack trace",
            "lint",
        ];
        if technical.iter().any(|t| lower.contains(t)) {
            return IrisProfile::Sharp;
        }

        // Long, complex prompts → Deep
        if prompt.len() > 200 {
            return IrisProfile::Deep;
        }

        // Very short prompts → Reflex
        if prompt.len() < 20 {
            return IrisProfile::Reflex;
        }

        // Mid-length (20-60 chars), non-technical, non-emotional → Fast
        if prompt.len() <= 60 {
            return IrisProfile::Fast;
        }

        IrisProfile::Balanced
    }

    /// Get execution parameters for a routed profile, incorporating emotion state.
    #[must_use]
    pub fn params(prompt: &str, emotion: &EmotionState) -> IrisParams {
        let profile = Self::route(prompt, emotion);
        IrisParams::for_profile(profile, emotion.is_connected())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fresh() -> EmotionState {
        EmotionState::birth()
    }

    #[test]
    fn short_prompt_routes_to_reflex() {
        assert_eq!(IrisEngine::route("hi", &fresh()), IrisProfile::Reflex);
    }

    #[test]
    fn technical_prompt_routes_to_sharp() {
        assert_eq!(
            IrisEngine::route("fix this bug in the compiler", &fresh()),
            IrisProfile::Sharp
        );
    }

    #[test]
    fn distress_prompt_routes_to_gentle() {
        assert_eq!(
            IrisEngine::route("I'm exhausted and overwhelmed", &fresh()),
            IrisProfile::Gentle
        );
    }

    #[test]
    fn long_prompt_routes_to_deep() {
        let long = "a".repeat(201);
        assert_eq!(IrisEngine::route(&long, &fresh()), IrisProfile::Deep);
    }

    #[test]
    fn moderate_short_prompt_routes_to_fast() {
        assert_eq!(
            IrisEngine::route("what should I have for dinner tonight", &fresh()),
            IrisProfile::Fast
        );
    }

    #[test]
    fn moderate_long_prompt_routes_to_balanced() {
        // Over 60 chars but under 200 → Balanced
        let prompt = "what should I have for dinner tonight given that I prefer low-carb options";
        assert_eq!(IrisEngine::route(prompt, &fresh()), IrisProfile::Balanced);
    }

    #[test]
    fn fast_profile_mid_token_budget() {
        let p = IrisParams::for_profile(IrisProfile::Fast, false);
        assert!(p.max_tokens >= 256 && p.max_tokens <= 1024);
        assert!(p.temperature >= 0.4 && p.temperature <= 0.7);
    }

    #[test]
    fn record_decision_and_stats() {
        let mut engine = IrisEngine::new();
        let e = EmotionState::birth();
        engine.record_decision(IrisProfile::Sharp, &e);
        engine.record_decision(IrisProfile::Sharp, &e);
        engine.record_decision(IrisProfile::Balanced, &e);
        let stats = engine.get_stats();
        assert!(stats.contains("sharp:2"));
        assert!(stats.contains("balanced:1"));
    }

    #[test]
    fn fatigued_agent_overrides_to_balanced() {
        let tired = EmotionState {
            energy: 0.2,
            tension: 0.1,
            connection: 0.5,
            focus: 0.4,
        };
        // Even a technical prompt gets downgraded when energy is low
        let profile = IrisEngine::route("implement a B-tree in Rust", &tired);
        assert_eq!(profile, IrisProfile::Balanced);
    }

    #[test]
    fn fatigued_and_tense_routes_to_gentle() {
        let stressed = EmotionState {
            energy: 0.2,
            tension: 0.8,
            connection: 0.5,
            focus: 0.3,
        };
        assert_eq!(
            IrisEngine::route("I need help", &stressed),
            IrisProfile::Gentle
        );
    }

    #[test]
    fn params_has_warmth_when_connected() {
        let connected = EmotionState {
            energy: 0.9,
            tension: 0.1,
            connection: 0.85,
            focus: 0.7,
        };
        let params = IrisEngine::params("tell me a story", &connected);
        assert!(params.warmth_boost);
    }

    #[test]
    fn sharp_profile_has_low_temperature() {
        let p = IrisParams::for_profile(IrisProfile::Sharp, false);
        assert!(p.temperature < 0.5);
        assert!(p.max_tokens >= 2048);
    }

    #[test]
    fn gentle_profile_has_high_temperature() {
        let p = IrisParams::for_profile(IrisProfile::Gentle, false);
        assert!(p.temperature > 0.7);
    }
}
