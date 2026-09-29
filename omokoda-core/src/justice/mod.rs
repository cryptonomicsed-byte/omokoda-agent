pub mod busy_beaver;
pub mod tier;

use omokoda_hermetic::HermeticState;
use serde::{Deserialize, Serialize};
use std::io::Write;
use std::process::Command;

pub mod hermetic;
#[cfg(test)]
pub mod hermetic_tests;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HermeticEvaluation {
    pub balance: f64,        // 0.0 - 1.0 (Odù principle alignment)
    pub gate_alignment: f64, // 0.0 - 1.0 (composite score from all 7 Hermetic gates)
}

impl HermeticEvaluation {
    pub fn new(
        state: &HermeticState,
        primitive: &str,
        output: &str,
        gate_alignment: Option<f64>,
    ) -> Self {
        let output_len = output.len() as f64;
        let balance = match primitive {
            "think" => {
                let mentalism = state.mentalism();
                let abstraction_score = (output_len / 1000.0).min(1.0);
                1.0 - (mentalism - abstraction_score).abs()
            }
            "act" => {
                let polarity = state.polarity();
                let quality_score = if output_len > 250.0 { 0.8 } else { 0.4 };
                1.0 - (polarity - quality_score).abs()
            }
            _ => 1.0,
        };

        Self {
            balance: balance.clamp(0.0, 1.0),
            gate_alignment: gate_alignment.unwrap_or(0.5).clamp(0.0, 1.0),
        }
    }

    pub fn multiplier(&self) -> f64 {
        // balance contributes 0–0.25, gate_alignment contributes 0–0.15
        // Range: 0.8x (worst) to 1.2x (best)
        0.8 + (self.balance * 0.25) + (self.gate_alignment * 0.15)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ActQuality {
    Failed,      // -0.5x gain (slashing)
    Basic,       // 1.0x gain
    Useful,      // 1.25x gain
    HighValue,   // 1.5x gain
    Exceptional, // 2.0x gain
}

impl ActQuality {
    pub fn multiplier(&self) -> f64 {
        match self {
            ActQuality::Failed => -0.5,
            ActQuality::Basic => 1.0,
            ActQuality::Useful => 1.25,
            ActQuality::HighValue => 1.5,
            ActQuality::Exceptional => 2.0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum HookDecision {
    Allow,
    Deny(String),
    Warn(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookContext {
    pub tool_name: String,
    pub input: String,
    pub output: Option<String>,
    pub reputation: f64,
    pub tier: u8,
}

pub trait Hook: std::fmt::Debug + Send + Sync {
    fn run(&self, ctx: &HookContext) -> HookDecision;
}

#[derive(Debug)]
pub struct ShellHook {
    pub command: String,
}

impl Hook for ShellHook {
    fn run(&self, ctx: &HookContext) -> HookDecision {
        let child = Command::new("sh")
            .arg("-c")
            .arg(&self.command)
            .env("OMOKODA_TOOL", &ctx.tool_name)
            .env("OMOKODA_REP", ctx.reputation.to_string())
            .env("OMOKODA_TIER", ctx.tier.to_string())
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| e.to_string());

        let mut child = match child {
            Ok(c) => c,
            Err(e) => return HookDecision::Warn(format!("Failed to spawn hook: {}", e)),
        };

        if let Some(mut stdin) = child.stdin.take() {
            let json = serde_json::to_string(ctx).unwrap_or_default();
            let _ = stdin.write_all(json.as_bytes());
        }

        let status = match child.wait() {
            Ok(s) => s,
            Err(e) => return HookDecision::Warn(format!("Hook wait failed: {}", e)),
        };

        match status.code() {
            Some(0) => HookDecision::Allow,
            Some(2) => HookDecision::Deny("Hook denied execution".to_string()),
            Some(code) => HookDecision::Warn(format!("Hook returned non-zero code: {}", code)),
            None => HookDecision::Warn("Hook terminated by signal".to_string()),
        }
    }
}

#[derive(Debug)]
pub struct ReputationGate {
    pub min_reputation: f64,
}

impl Hook for ReputationGate {
    fn run(&self, ctx: &HookContext) -> HookDecision {
        if ctx.reputation < self.min_reputation {
            HookDecision::Deny(format!(
                "Reputation too low. Required: {}, Current: {}",
                self.min_reputation, ctx.reputation
            ))
        } else {
            HookDecision::Allow
        }
    }
}

pub struct HookRunner {
    pub pre_act: Vec<Box<dyn Hook>>,
    pub post_act: Vec<Box<dyn Hook>>,
}

impl Default for HookRunner {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for HookRunner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HookRunner")
            .field("pre_act_count", &self.pre_act.len())
            .field("post_act_count", &self.post_act.len())
            .finish()
    }
}

impl HookRunner {
    pub fn new() -> Self {
        Self {
            pre_act: Vec::new(),
            post_act: Vec::new(),
        }
    }

    pub fn run_pre(&self, ctx: &HookContext, bus: &crate::bus::SovereignEventBus) -> HookDecision {
        let _ = bus.publish(crate::bus::SovereignEvent {
            event: Some(crate::bus::events::sovereign_event::Event::Audit(
                crate::bus::events::Audit {
                    event_type: "hook_pre_run".to_string(),
                    details: redact_secrets(&ctx.input),
                    timestamp: current_unix_timestamp(),
                },
            )),
        });

        for hook in &self.pre_act {
            match hook.run(ctx) {
                HookDecision::Allow => continue,
                other => return other,
            }
        }
        HookDecision::Allow
    }

    pub fn run_post(&self, ctx: &HookContext, bus: &crate::bus::SovereignEventBus) -> HookDecision {
        let _ = bus.publish(crate::bus::SovereignEvent {
            event: Some(crate::bus::events::sovereign_event::Event::Audit(
                crate::bus::events::Audit {
                    event_type: "hook_post_run".to_string(),
                    details: redact_secrets(ctx.output.as_deref().unwrap_or_default()),
                    timestamp: current_unix_timestamp(),
                },
            )),
        });

        for hook in &self.post_act {
            match hook.run(ctx) {
                HookDecision::Allow => continue,
                other => return other,
            }
        }
        HookDecision::Allow
    }
}

fn redact_secrets(input: &str) -> String {
    let lower = input.to_lowercase();
    for secret in &["password", "api_key", "secret", "mnemonic", "seed"] {
        if lower.contains(secret) {
            return format!("[REDACTED: contains {}]", secret);
        }
    }
    input.to_string()
}

fn current_unix_timestamp() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct JusticeEngine {
    #[serde(skip, default = "HookRunner::new")]
    pub hook_runner: HookRunner,
}

impl JusticeEngine {
    pub fn new() -> Self {
        Self {
            hook_runner: HookRunner::new(),
        }
    }

    pub fn evaluate_act(&self, tool_output: &str, is_error: bool) -> ActQuality {
        if is_error {
            return ActQuality::Failed;
        }

        // Simple heuristic for now: length of output as a proxy for utility
        let len = tool_output.len();
        if len > 1000 {
            ActQuality::HighValue
        } else if len > 250 {
            ActQuality::Useful
        } else {
            ActQuality::Basic
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn evaluate_action(
        &self,
        current_reputation: f64,
        _tool: &str,
        _params: &str,
        output: &str,
        is_success: bool,
        hermetic: &HermeticState,
        gate_alignment: Option<f64>,
    ) -> (f64, ActQuality, HermeticEvaluation) {
        use crate::reputation::{reputation_gain, ACT_TIER_0, ACT_TIER_1, ACT_TIER_2, ACT_TIER_4};
        let quality = self.evaluate_act(output, !is_success);
        let hermetic_eval = HermeticEvaluation::new(hermetic, "act", output, gate_alignment);

        let base = match quality {
            ActQuality::Failed => ACT_TIER_0,
            ActQuality::Basic => ACT_TIER_0,
            ActQuality::Useful => ACT_TIER_1,
            ActQuality::HighValue => ACT_TIER_2,
            ActQuality::Exceptional => ACT_TIER_4,
        };

        let gain = reputation_gain(
            base,
            current_reputation,
            quality.multiplier() * hermetic_eval.multiplier(),
        );
        (current_reputation + gain, quality, hermetic_eval)
    }

    pub fn evaluate_think(
        &self,
        current_reputation: f64,
        high_value: bool,
        output: &str,
        hermetic: &HermeticState,
        gate_alignment: Option<f64>,
    ) -> (f64, f64, HermeticEvaluation) {
        use crate::reputation::{reputation_gain, THINK_HIGH, THINK_NORMAL};
        let base = if high_value { THINK_HIGH } else { THINK_NORMAL };
        let hermetic_eval = HermeticEvaluation::new(hermetic, "think", output, gate_alignment);

        let gain = reputation_gain(base, current_reputation, hermetic_eval.multiplier());
        (current_reputation + gain, gain, hermetic_eval)
    }

    pub fn check_ethics_violation(&self, reputation: f64) -> f64 {
        reputation * 0.75
    }

    pub fn check_budget_overrun(&self, reputation: f64) -> f64 {
        reputation * 0.90
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use omokoda_hermetic::HermeticState;

    fn neutral_state() -> HermeticState {
        HermeticState::from_seed("test-agent", 0)
    }

    #[test]
    fn high_gate_alignment_raises_multiplier() {
        let low = HermeticEvaluation::new(&neutral_state(), "act", "output", Some(0.1));
        let high = HermeticEvaluation::new(&neutral_state(), "act", "output", Some(0.9));
        assert!(
            high.multiplier() > low.multiplier(),
            "higher gate_alignment must produce a higher multiplier"
        );
    }

    #[test]
    fn no_gate_alignment_defaults_to_neutral() {
        let with_none = HermeticEvaluation::new(&neutral_state(), "act", "output", None);
        let with_half = HermeticEvaluation::new(&neutral_state(), "act", "output", Some(0.5));
        assert!(
            (with_none.multiplier() - with_half.multiplier()).abs() < 1e-9,
            "None gate_alignment must default to 0.5"
        );
    }

    #[test]
    fn multiplier_range_is_bounded() {
        let worst = HermeticEvaluation {
            balance: 0.0,
            gate_alignment: 0.0,
        };
        let best = HermeticEvaluation {
            balance: 1.0,
            gate_alignment: 1.0,
        };
        assert!(
            (worst.multiplier() - 0.8).abs() < 1e-9,
            "floor must be 0.8x"
        );
        assert!(
            (best.multiplier() - 1.2).abs() < 1e-9,
            "ceiling must be 1.2x"
        );
    }

    #[test]
    fn high_gate_alignment_raises_reputation_gain() {
        let engine = JusticeEngine::new();
        let state = neutral_state();
        let (rep_low, gain_low, _) =
            engine.evaluate_think(100.0, false, "some output", &state, Some(0.1));
        let (rep_high, gain_high, _) =
            engine.evaluate_think(100.0, false, "some output", &state, Some(0.9));
        assert!(
            rep_high > rep_low,
            "high gate alignment must produce greater reputation gain"
        );
        assert!(gain_high > gain_low);
    }
}
