// H7: Ọ̀ṣọ́ language → hive directives compiler bridge
//
// Ọ̀ṣọ́ (Aether) is the 3-primitive agent language (born/feel/act).
// This module translates Ọ̀ṣọ́ intent expressions into HiveActions
// that can be dispatched through the RitualPhase cycle.
//
// Translation table:
//   BORN <agent_config>   → HiveAction::SpawnLobe
//   FEEL <urgency> <goal> → LobeGoalProposal (injected into Receive phase)
//   ACT <tool> <args>     → HiveAction::BroadcastGoal (wrapped as task directive)
//
// The compiler is deliberately minimal — it only handles the subset of
// Ọ̀ṣọ́ expressions that are meaningful at the hive level. Agent-level
// Ọ̀ṣọ́ evaluation remains in omokoda-core.

use serde::{Deserialize, Serialize};
use crate::lobe::{HiveAction, OrisaLobe};
use crate::goal_vector::LobeGoalProposal;

/// A parsed Ọ̀ṣọ́ hive directive.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OsoHiveDirective {
    /// Spawn a new lobe-agent with the given config
    SpawnLobe {
        lobe: OrisaLobe,
        config: serde_json::Value,
    },
    /// Inject a goal proposal into the Receive phase buffer
    Feel {
        lobe: OrisaLobe,
        urgency: f32,
        goal_topic: String,
        description: String,
    },
    /// Broadcast a tool-use directive to a specific lobe
    Act {
        lobe: OrisaLobe,
        tool: String,
        args: serde_json::Value,
    },
}

#[derive(Debug, thiserror::Error)]
pub enum OsoCompileError {
    #[error("unknown directive keyword: {0}")]
    UnknownKeyword(String),
    #[error("invalid urgency value: {0}")]
    InvalidUrgency(String),
    #[error("unknown lobe name: {0}")]
    UnknownLobe(String),
    #[error("malformed expression: {0}")]
    Malformed(String),
}

/// Minimal Ọ̀ṣọ́ → HiveDirective compiler.
pub struct OsoHiveCompiler;

impl OsoHiveCompiler {
    /// Compile a single Ọ̀ṣọ́ expression into a hive directive.
    ///
    /// Syntax:
    ///   BORN <lobe_name> [json_config]
    ///   FEEL <lobe_name> <urgency_0_to_1> <goal_topic> [description...]
    ///   ACT <lobe_name> <tool_name> [json_args]
    pub fn compile(expression: &str) -> Result<OsoHiveDirective, OsoCompileError> {
        let parts: Vec<&str> = expression.trim().splitn(5, ' ').collect();
        if parts.is_empty() {
            return Err(OsoCompileError::Malformed("empty expression".into()));
        }

        match parts[0].to_uppercase().as_str() {
            "BORN" => {
                let lobe_name = parts.get(1).ok_or_else(|| {
                    OsoCompileError::Malformed("BORN requires lobe name".into())
                })?;
                let lobe = Self::parse_lobe(lobe_name)?;
                let config = parts
                    .get(2)
                    .and_then(|s| serde_json::from_str(s).ok())
                    .unwrap_or(serde_json::json!({}));
                Ok(OsoHiveDirective::SpawnLobe { lobe, config })
            }
            "FEEL" => {
                let lobe_name = parts.get(1).ok_or_else(|| {
                    OsoCompileError::Malformed("FEEL requires lobe name".into())
                })?;
                let lobe = Self::parse_lobe(lobe_name)?;
                let urgency_str = parts.get(2).ok_or_else(|| {
                    OsoCompileError::Malformed("FEEL requires urgency".into())
                })?;
                let urgency = urgency_str
                    .parse::<f32>()
                    .map_err(|_| OsoCompileError::InvalidUrgency(urgency_str.to_string()))?
                    .clamp(0.0, 1.0);
                let goal_topic = parts
                    .get(3)
                    .ok_or_else(|| OsoCompileError::Malformed("FEEL requires goal topic".into()))?
                    .to_string();
                let description = parts.get(4).unwrap_or(&"").to_string();
                Ok(OsoHiveDirective::Feel { lobe, urgency, goal_topic, description })
            }
            "ACT" => {
                let lobe_name = parts.get(1).ok_or_else(|| {
                    OsoCompileError::Malformed("ACT requires lobe name".into())
                })?;
                let lobe = Self::parse_lobe(lobe_name)?;
                let tool = parts
                    .get(2)
                    .ok_or_else(|| OsoCompileError::Malformed("ACT requires tool name".into()))?
                    .to_string();
                let args = parts
                    .get(3)
                    .and_then(|s| serde_json::from_str(s).ok())
                    .unwrap_or(serde_json::json!({}));
                Ok(OsoHiveDirective::Act { lobe, tool, args })
            }
            other => Err(OsoCompileError::UnknownKeyword(other.to_string())),
        }
    }

    fn parse_lobe(name: &str) -> Result<OrisaLobe, OsoCompileError> {
        match name.to_uppercase().as_str() {
            "OBATALA" => Ok(OrisaLobe::Obatala),
            "OGUN" => Ok(OrisaLobe::Ogun),
            "SHANGO" => Ok(OrisaLobe::Shango),
            "YEMOJA" => Ok(OrisaLobe::Yemoja),
            "OSHUN" => Ok(OrisaLobe::Oshun),
            "ESHU" => Ok(OrisaLobe::Eshu),
            "ORUNMILA" => Ok(OrisaLobe::Orunmila),
            "ODUDUWA" => Ok(OrisaLobe::Oduduwa),
            "OYA" => Ok(OrisaLobe::Oya),
            "OSOOSI" => Ok(OrisaLobe::Osoosi),
            "SANGO" => Ok(OrisaLobe::Sango),
            other => Err(OsoCompileError::UnknownLobe(other.to_string())),
        }
    }

    /// Convert a compiled directive into a HiveAction (for SpawnLobe/Act)
    /// or a LobeGoalProposal (for Feel).
    pub fn to_hive_action(directive: OsoHiveDirective, tick: u64) -> HiveDirectiveOutput {
        match directive {
            OsoHiveDirective::SpawnLobe { lobe, config } => {
                HiveDirectiveOutput::Action(HiveAction::SpawnLobe { lobe, config })
            }
            OsoHiveDirective::Feel { lobe, urgency, goal_topic, description } => {
                HiveDirectiveOutput::Proposal(LobeGoalProposal {
                    lobe,
                    topic: goal_topic,
                    description,
                    urgency,
                    alignment_score: 1.0,
                    tick_proposed: tick,
                })
            }
            OsoHiveDirective::Act { lobe, tool, args } => {
                HiveDirectiveOutput::Action(HiveAction::BroadcastGoal {
                    goal_id: format!("act-{lobe:?}-{tool}"),
                    goal_vector: serde_json::json!({ "tool": tool, "args": args, "lobe": format!("{lobe:?}") }),
                })
            }
        }
    }
}

/// Output of directive-to-action conversion.
pub enum HiveDirectiveOutput {
    Action(HiveAction),
    Proposal(LobeGoalProposal),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compile_born() {
        let d = OsoHiveCompiler::compile("BORN OBATALA").unwrap();
        assert!(matches!(d, OsoHiveDirective::SpawnLobe { lobe: OrisaLobe::Obatala, .. }));
    }

    #[test]
    fn compile_feel() {
        let d = OsoHiveCompiler::compile("FEEL YEMOJA 0.8 memory_health store all glyphs").unwrap();
        if let OsoHiveDirective::Feel { lobe, urgency, goal_topic, .. } = d {
            assert_eq!(lobe, OrisaLobe::Yemoja);
            assert!((urgency - 0.8).abs() < 0.001);
            assert_eq!(goal_topic, "memory_health");
        } else {
            panic!("unexpected variant");
        }
    }

    #[test]
    fn compile_act() {
        let d = OsoHiveCompiler::compile("ACT OGUN read_file /tmp/state.json").unwrap();
        assert!(matches!(d, OsoHiveDirective::Act { lobe: OrisaLobe::Ogun, .. }));
    }

    #[test]
    fn unknown_keyword_errors() {
        assert!(OsoHiveCompiler::compile("DANCE OBATALA").is_err());
    }

    #[test]
    fn unknown_lobe_errors() {
        assert!(OsoHiveCompiler::compile("BORN UNKNOWN_ORISA").is_err());
    }

    #[test]
    fn urgency_clamped() {
        let d = OsoHiveCompiler::compile("FEEL ESHU 999.0 topic").unwrap();
        if let OsoHiveDirective::Feel { urgency, .. } = d {
            assert_eq!(urgency, 1.0);
        }
    }
}
