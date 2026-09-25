// If-Script → Action Compiler
//
// Prescription → Task Graph compilation.
// Takes a Calabash directive's Prescription string and produces an ordered
// list of ActionSteps + Verify assertions + a Cadence descriptor.
//
// The LLM PROPOSES the action; this compiler DECOMPOSES it into the typed
// structure the ActionTransaction state machine can execute and verify.
//
// Architecture:
//   If-Script (WHAT to do) → ActionCompiler (HOW to do it)
//   → ActionTransaction (LET IT HAPPEN, track it, verify it, receipt it)

use serde::{Deserialize, Serialize};
use crate::gates::ActionIntent;
use crate::execution::verify::Assertion;
use crate::rhythm::ActionCadence;

/// A single compiled task step — tool call + expected evidence.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompiledStep {
    pub description: String,
    pub tool: String,
    pub params: serde_json::Value,
    pub expected_artifacts: Vec<String>,
}

/// The compiled output of a Calabash prescription.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompiledAction {
    /// Unique opcode label derived from the vessel + directive index.
    pub opcode: String,
    /// Which Calabash vessel (0–15) this action belongs to.
    pub vessel: u8,
    /// Structured semantic intent derived from the prescription.
    pub action_intent: ActionIntent,
    /// Ordered task steps to execute.
    pub steps: Vec<CompiledStep>,
    /// Post-execution assertions from the Verify: block.
    pub assertions: Vec<Assertion>,
    /// Cadence / scheduling metadata from the Cadence: block.
    pub cadence: ActionCadence,
    /// Optional prerequisite opcode that must succeed before this one runs.
    pub depends_on: Option<String>,
}

/// Error returned when a prescription cannot be compiled.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompileError {
    pub opcode: String,
    pub reason: String,
}

impl std::fmt::Display for CompileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ActionCompiler error for '{}': {}", self.opcode, self.reason)
    }
}

/// Compiles a Calabash directive into an executable CompiledAction.
///
/// Callers supply the raw directive fields extracted from the 256-entry corpus.
/// The compiler validates the structure and produces an unambiguous task graph.
pub struct ActionCompiler;

impl ActionCompiler {
    /// Compile a prescription into a typed CompiledAction.
    ///
    /// `vessel` — 0–15 Calabash vessel index
    /// `opcode` — unique string label (e.g. "execution:deploy_artifact")
    /// `prescription` — the directive's Prescription text
    /// `verify_block` — the directive's Verify: block (list of assertion specs)
    /// `cadence_block` — the directive's Cadence: block (trigger spec)
    pub fn compile(
        vessel: u8,
        opcode: impl Into<String>,
        prescription: &str,
        verify_block: &[VerifySpec],
        cadence_block: &CadenceSpec,
    ) -> Result<CompiledAction, CompileError> {
        let opcode = opcode.into();

        if prescription.trim().is_empty() {
            return Err(CompileError {
                opcode,
                reason: "empty prescription — cannot compile an action with no steps".to_string(),
            });
        }

        let steps = parse_prescription_steps(prescription);
        if steps.is_empty() {
            return Err(CompileError {
                opcode,
                reason: "prescription parsed to zero steps — check directive format".to_string(),
            });
        }

        let assertions = compile_assertions(verify_block, &opcode)?;
        let cadence = compile_cadence(cadence_block);
        let action_intent = infer_intent(vessel, &opcode, &steps);

        Ok(CompiledAction {
            opcode,
            vessel,
            action_intent,
            steps,
            assertions,
            cadence,
            depends_on: None,
        })
    }

    /// Add a cross-vessel dependency: this action waits for `dependency_opcode` to COMMITTED.
    pub fn with_dependency(mut action: CompiledAction, dependency_opcode: impl Into<String>) -> CompiledAction {
        action.depends_on = Some(dependency_opcode.into());
        action
    }
}

/// Raw verify spec from the Calabash corpus before compilation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerifySpec {
    pub kind: String,  // "file_exists" | "file_contains" | "hash_match" | "git_atomic_commit" | "json_field_equals" | "exit_code"
    pub path: Option<String>,
    pub expected: Option<String>,
    pub key: Option<String>,
    pub expected_exit_code: Option<i32>,
    pub actual_exit_code: Option<i32>,
}

/// Raw cadence spec from the Calabash corpus before compilation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CadenceSpec {
    pub trigger: String,  // "immediate" | "event:<name>" | "state:<condition>" | "cron:<expr>"
    pub cooldown_secs: Option<u64>,
    pub max_per_window: Option<u32>,
    pub window_secs: Option<u64>,
    pub deadline_secs: Option<u64>,
}

// ─── Internal helpers ─────────────────────────────────────────────────────────

fn parse_prescription_steps(prescription: &str) -> Vec<CompiledStep> {
    // Parse numbered steps from prescription text.
    // Format expected:
    //   1. <description> via <tool>(<params>)
    //   2. <description> via <tool>
    // Falls back to treating each non-empty line as a description-only step.
    let mut steps = Vec::new();
    for line in prescription.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        // Strip leading "N." or "-" or "*"
        let text = line
            .trim_start_matches(|c: char| c.is_ascii_digit() || c == '.' || c == '-' || c == '*')
            .trim();
        if text.is_empty() {
            continue;
        }
        // Try to extract "via <tool>" suffix
        let (description, tool, params) = if let Some(pos) = text.to_lowercase().rfind(" via ") {
            let desc = text[..pos].trim().to_string();
            let tool_part = text[pos + 5..].trim();
            // Check for parenthesized params: tool(params)
            if let Some(paren) = tool_part.find('(') {
                let tool = tool_part[..paren].trim().to_string();
                let params_str = tool_part[paren + 1..].trim_end_matches(')').trim();
                let params = serde_json::from_str(params_str)
                    .unwrap_or_else(|_| serde_json::json!({"raw": params_str}));
                (desc, tool, params)
            } else {
                (desc, tool_part.to_string(), serde_json::json!({}))
            }
        } else {
            (text.to_string(), "bash".to_string(), serde_json::json!({}))
        };

        steps.push(CompiledStep {
            description,
            tool,
            params,
            expected_artifacts: Vec::new(),
        });
    }
    steps
}

fn compile_assertions(specs: &[VerifySpec], opcode: &str) -> Result<Vec<Assertion>, CompileError> {
    let mut assertions = Vec::new();
    for spec in specs {
        let assertion = match spec.kind.as_str() {
            "file_exists" => {
                let path = spec.path.clone().ok_or_else(|| CompileError {
                    opcode: opcode.to_string(),
                    reason: "file_exists assertion requires a path".to_string(),
                })?;
                Assertion::FileExists { path }
            }
            "file_contains" => {
                let path = spec.path.clone().ok_or_else(|| CompileError {
                    opcode: opcode.to_string(),
                    reason: "file_contains assertion requires a path".to_string(),
                })?;
                let expected = spec.expected.clone().ok_or_else(|| CompileError {
                    opcode: opcode.to_string(),
                    reason: "file_contains assertion requires an expected value".to_string(),
                })?;
                Assertion::FileContains { path, expected }
            }
            "hash_match" => {
                let path = spec.path.clone().ok_or_else(|| CompileError {
                    opcode: opcode.to_string(),
                    reason: "hash_match assertion requires a path".to_string(),
                })?;
                let expected_sha256 = spec.expected.clone().ok_or_else(|| CompileError {
                    opcode: opcode.to_string(),
                    reason: "hash_match assertion requires an expected sha256".to_string(),
                })?;
                Assertion::HashMatch { path, expected_sha256 }
            }
            "git_atomic_commit" => {
                let repo_path = spec.path.clone().unwrap_or_else(|| ".".to_string());
                Assertion::GitAtomicCommit { repo_path }
            }
            "json_field_equals" => {
                let path = spec.path.clone().ok_or_else(|| CompileError {
                    opcode: opcode.to_string(),
                    reason: "json_field_equals requires a path".to_string(),
                })?;
                let key = spec.key.clone().ok_or_else(|| CompileError {
                    opcode: opcode.to_string(),
                    reason: "json_field_equals requires a key".to_string(),
                })?;
                let expected = spec.expected.clone().ok_or_else(|| CompileError {
                    opcode: opcode.to_string(),
                    reason: "json_field_equals requires an expected value".to_string(),
                })?;
                Assertion::JsonFieldEquals { path, key, expected }
            }
            "exit_code" => {
                let expected = spec.expected_exit_code.ok_or_else(|| CompileError {
                    opcode: opcode.to_string(),
                    reason: "exit_code assertion requires expected_exit_code".to_string(),
                })?;
                let actual = spec.actual_exit_code.unwrap_or(0);
                Assertion::ExitCode { expected, actual }
            }
            unknown => {
                return Err(CompileError {
                    opcode: opcode.to_string(),
                    reason: format!("unknown assertion kind: '{unknown}'"),
                });
            }
        };
        assertions.push(assertion);
    }
    Ok(assertions)
}

fn compile_cadence(spec: &CadenceSpec) -> ActionCadence {
    use crate::rhythm::TriggerKind;
    let trigger = if spec.trigger == "immediate" {
        TriggerKind::Immediate
    } else if let Some(event) = spec.trigger.strip_prefix("event:") {
        TriggerKind::EventDriven { event: event.to_string() }
    } else if let Some(condition) = spec.trigger.strip_prefix("state:") {
        TriggerKind::StateDriven { condition: condition.to_string() }
    } else if let Some(cron) = spec.trigger.strip_prefix("cron:") {
        TriggerKind::Scheduled { cron: cron.to_string() }
    } else {
        TriggerKind::Immediate
    };
    ActionCadence {
        trigger,
        cooldown_secs: spec.cooldown_secs,
        max_per_window: spec.max_per_window,
        window_secs: spec.window_secs,
        deadline_secs: spec.deadline_secs,
    }
}

fn infer_intent(vessel: u8, opcode: &str, steps: &[CompiledStep]) -> ActionIntent {
    use crate::gates::{ActionIntent, DataSensitivity, Reversibility};
    // Derive basic intent from opcode and steps — caller can override after compilation.
    let has_write = steps.iter().any(|s| {
        let t = s.tool.to_lowercase();
        t.contains("write") || t.contains("edit") || t.contains("delete") || t.contains("create")
    });
    let has_network = steps.iter().any(|s| {
        let t = s.tool.to_lowercase();
        t.contains("http") || t.contains("fetch") || t.contains("request") || t.contains("vantage")
    });
    ActionIntent {
        purpose: opcode.to_string(),
        target: steps.first().map(|s| s.description.clone()).unwrap_or_default(),
        mutations: if has_write { vec!["state_write".to_string()] } else { vec![] },
        data_sensitivity: DataSensitivity::Internal,
        network_access: has_network,
        consent_required: vessel == 11, // Vessel 11 = Consent
        reversibility: if has_write { Reversibility::PartiallyReversible } else { Reversibility::Reversible },
        expected_effects: steps.iter().map(|s| s.description.clone()).collect(),
        receipt_required: has_write || has_network,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn simple_spec() -> (Vec<VerifySpec>, CadenceSpec) {
        let verify = vec![VerifySpec {
            kind: "file_exists".to_string(),
            path: Some("/tmp/test_output.txt".to_string()),
            expected: None,
            key: None,
            expected_exit_code: None,
            actual_exit_code: None,
        }];
        let cadence = CadenceSpec {
            trigger: "immediate".to_string(),
            cooldown_secs: Some(30),
            max_per_window: None,
            window_secs: None,
            deadline_secs: Some(300),
        };
        (verify, cadence)
    }

    #[test]
    fn compile_simple_prescription() {
        let (verify, cadence) = simple_spec();
        let prescription = "1. Write output file via write_file\n2. Confirm success via bash";
        let result = ActionCompiler::compile(7, "execution:write_output", prescription, &verify, &cadence);
        assert!(result.is_ok());
        let compiled = result.unwrap();
        assert_eq!(compiled.vessel, 7);
        assert_eq!(compiled.steps.len(), 2);
        assert_eq!(compiled.assertions.len(), 1);
        assert!(compiled.cadence.cooldown_secs == Some(30));
    }

    #[test]
    fn empty_prescription_errors() {
        let (verify, cadence) = simple_spec();
        let result = ActionCompiler::compile(7, "execution:empty", "", &verify, &cadence);
        assert!(result.is_err());
    }

    #[test]
    fn with_dependency_sets_depends_on() {
        let (verify, cadence) = simple_spec();
        let compiled = ActionCompiler::compile(7, "execution:step2", "1. Run step 2", &verify, &cadence).unwrap();
        let with_dep = ActionCompiler::with_dependency(compiled, "execution:step1");
        assert_eq!(with_dep.depends_on.as_deref(), Some("execution:step1"));
    }

    #[test]
    fn unknown_assertion_kind_errors() {
        let (_, cadence) = simple_spec();
        let verify = vec![VerifySpec {
            kind: "unknown_check".to_string(),
            path: None, expected: None, key: None,
            expected_exit_code: None, actual_exit_code: None,
        }];
        let result = ActionCompiler::compile(7, "execution:test", "1. Do thing", &verify, &cadence);
        assert!(result.is_err());
    }

    #[test]
    fn network_access_inferred_from_tool_name() {
        let (verify, cadence) = simple_spec();
        let prescription = "1. Fetch remote data via fetch_url";
        let compiled = ActionCompiler::compile(8, "swarm:fetch", prescription, &verify, &cadence).unwrap();
        assert!(compiled.action_intent.network_access);
        assert!(compiled.action_intent.receipt_required);
    }
}
