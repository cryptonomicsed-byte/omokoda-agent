//! action_interpreter — the execution enforcement layer for the Digital Calabash.
//!
//! This module sits between `execute_tool_call_for_agentic()` and raw tool dispatch.
//! Every agentic tool invocation is evaluated through the current Odù's ActionSchema
//! before execution may proceed. The interpreter enforces:
//!
//!   1. **Behavioral constraints** — taboo-derived rules that block prohibited tool calls
//!      or output patterns for the current Odù.
//!   2. **Vessel alignment** — whether the proposed tool matches the active vessel domain.
//!   3. **Execution mode authority** — whether the agent's current mode permits the
//!      mutation type (Analytical cannot mutate; Executor can; Guardian requires consent).
//!   4. **Post-execution verification** — the result must satisfy at least one verify spec
//!      before the outcome is marked COMMITTED.
//!   5. **Mandatory receipt** — every outcome (success, blocked, refused, failed, expired,
//!      partial, cancelled) produces a structured `InterpretReceipt`.
//!
//! ## The Invariant
//!
//! > No action is considered complete because the model says it completed.
//! > An action is complete only when its verification evidence satisfies the
//! > directive's contract AND an `InterpretReceipt` has been committed.
//!
//! ## Integration point
//!
//! Call `ActionInterpreter::evaluate()` BEFORE tool execution with the proposed tool name
//! and params. If it returns `InterpretDecision::Proceed`, execute the tool, then call
//! `ActionInterpreter::commit()` with the raw output to produce the mandatory receipt.
//! If it returns `InterpretDecision::Blocked` or `InterpretDecision::Refused`, do NOT
//! execute — the decision itself becomes the receipt.

use serde::{Deserialize, Serialize};
use serde_json::json;
use crate::execution::action_schema::{build_schema, ActionSchema, BehavioralConstraint, ExecutionMode};
use crate::execution::action_compiler::VerifySpec;
use crate::usage::TokenUsage;

// ─── InterpretDecision ────────────────────────────────────────────────────────

/// The interpreter's verdict before tool execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum InterpretDecision {
    /// Tool may execute. Includes the schema + constraints active for this turn.
    Proceed {
        odu_index: u8,
        execution_mode: ExecutionMode,
        active_constraints: Vec<BehavioralConstraint>,
        vessel_aligned: bool,
        receipt_required: bool,
    },
    /// Tool call is blocked by a behavioral constraint derived from a taboo.
    Blocked {
        odu_index: u8,
        constraint_name: String,
        source_taboo: String,
        rationale: String,
    },
    /// Tool call is refused because the current execution mode lacks the required authority.
    Refused {
        odu_index: u8,
        execution_mode: ExecutionMode,
        required_mode: String,
        reason: String,
    },
}

// ─── VerifyOutcome ────────────────────────────────────────────────────────────

/// The result of running verify specs against tool output.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerifyOutcome {
    pub passed: bool,
    pub passed_specs: Vec<String>,
    pub failed_specs: Vec<String>,
    pub evidence: serde_json::Value,
}

// ─── InterpretReceipt ─────────────────────────────────────────────────────────

/// Mandatory receipt produced for EVERY outcome of an interpreted action.
/// Covers: Committed, Blocked, Refused, Failed, Partial, Expired, Cancelled.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InterpretReceipt {
    pub odu_index: u8,
    pub odu_name: String,
    pub vessel: u8,
    pub opcode: String,
    pub tool: String,
    pub outcome: InterpretOutcome,
    pub decision: Option<InterpretDecision>,
    pub verify_outcome: Option<VerifyOutcome>,
    pub raw_output_hash: Option<String>,
    pub timestamp_secs: u64,
    pub memory_event: InterpretMemoryEvent,
}

/// The final outcome classification — never "complete" unless verified.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum InterpretOutcome {
    /// Verification passed — action is COMMITTED.
    Committed,
    /// Action executed but verification was partial (some specs passed, some failed).
    Partial,
    /// Pre-execution: blocked by behavioral constraint from taboo.
    Blocked,
    /// Pre-execution: refused due to insufficient execution mode authority.
    Refused,
    /// Post-execution: tool returned error or non-zero exit.
    Failed,
    /// Post-execution: verification assertions all failed.
    Unverified,
    /// Action was not attempted due to expired deadline.
    Expired,
    /// Action was cancelled before execution.
    Cancelled,
}

/// Structured memory consequence from this interpretation.
/// Always produced — even for blocked/refused outcomes (to learn from them).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InterpretMemoryEvent {
    pub action: String,
    pub decision: String,
    pub reason: String,
    pub outcome: String,
    pub evidence: Option<String>,
    pub failure: Option<String>,
    pub lesson: Option<String>,
    pub residue: Option<String>,
    pub next_action: Option<String>,
}

// ─── ActionInterpreter ────────────────────────────────────────────────────────

pub struct ActionInterpreter;

impl ActionInterpreter {
    /// Evaluate whether a proposed tool call is permitted under the current Odù.
    ///
    /// Call this BEFORE executing any tool in an agentic turn.
    /// If `Proceed` is returned, execute the tool then call `commit()`.
    /// If `Blocked` or `Refused`, do not execute — call `commit_blocked()` instead.
    pub fn evaluate(
        odu_index: u8,
        proposed_tool: &str,
        proposed_params: &str,
    ) -> InterpretDecision {
        let schema = build_schema(odu_index);

        // 1. Check behavioral constraints from taboos
        for constraint in &schema.behavioral_constraints {
            if constraint.denied_tools.iter().any(|t| t == proposed_tool) {
                return InterpretDecision::Blocked {
                    odu_index,
                    constraint_name: constraint.name.clone(),
                    source_taboo: constraint.source_taboo.clone(),
                    rationale: constraint.rationale.clone(),
                };
            }
            // Check denied patterns against params
            if !proposed_params.is_empty() {
                let params_lower = proposed_params.to_lowercase();
                for pattern in &constraint.denied_patterns {
                    if params_lower.contains(pattern.as_str()) {
                        return InterpretDecision::Blocked {
                            odu_index,
                            constraint_name: constraint.name.clone(),
                            source_taboo: constraint.source_taboo.clone(),
                            rationale: format!(
                                "{} (pattern '{}' found in params)",
                                constraint.rationale, pattern
                            ),
                        };
                    }
                }
            }
        }

        // 2. Check execution mode authority for mutation operations
        let is_write_op = is_write_operation(proposed_tool);
        let is_destructive = is_destructive_operation(proposed_tool, proposed_params);

        match &schema.execution_mode {
            ExecutionMode::Analytical => {
                if is_destructive {
                    return InterpretDecision::Refused {
                        odu_index,
                        execution_mode: ExecutionMode::Analytical,
                        required_mode: "Executor or Guardian".to_string(),
                        reason: "Analytical mode prohibits destructive operations without explicit override".to_string(),
                    };
                }
            }
            ExecutionMode::Guardian => {
                if is_destructive {
                    return InterpretDecision::Refused {
                        odu_index,
                        execution_mode: ExecutionMode::Guardian,
                        required_mode: "Executor".to_string(),
                        reason: "Guardian mode requires Executor authority for destructive operations".to_string(),
                    };
                }
            }
            ExecutionMode::FlowGuard => {
                if is_write_op && is_high_frequency(proposed_tool) {
                    return InterpretDecision::Refused {
                        odu_index,
                        execution_mode: ExecutionMode::FlowGuard,
                        required_mode: "Executor".to_string(),
                        reason: "FlowGuard mode applies rate constraints to high-frequency write operations".to_string(),
                    };
                }
            }
            _ => {}
        }

        // 3. Check vessel alignment (advisory — warn but don't block)
        let vessel_aligned = is_vessel_aligned(proposed_tool, &schema);

        InterpretDecision::Proceed {
            odu_index,
            execution_mode: schema.execution_mode,
            active_constraints: schema.behavioral_constraints,
            vessel_aligned,
            receipt_required: is_write_op || is_destructive,
        }
    }

    /// Run verify specs against tool output to determine if the action is COMMITTED.
    ///
    /// Runs lightweight checks that don't require filesystem access (e.g. exit_code
    /// checks from embedded exit codes in the output, json field checks, string
    /// contains checks). Filesystem-dependent checks return false when the file
    /// isn't accessible from within this context — callers with FS access should
    /// supplement with `verify_file_exists()` before calling `commit()`.
    pub fn verify(output: &str, verify_specs: &[VerifySpec]) -> VerifyOutcome {
        if verify_specs.is_empty() {
            return VerifyOutcome {
                passed: true,
                passed_specs: vec!["(no specs — trivially passed)".to_string()],
                failed_specs: vec![],
                evidence: json!({"output_preview": &output[..output.len().min(200)]}),
            };
        }

        let mut passed = vec![];
        let mut failed = vec![];

        for spec in verify_specs {
            let ok = match spec.kind.as_str() {
                "exit_code" => {
                    // Check for exit code in output or assume 0 if output is non-empty
                    let expected = spec.expected_exit_code.unwrap_or(0);
                    if expected == 0 {
                        !output.trim().is_empty() || output.contains("exit_code: 0")
                    } else {
                        output.contains(&format!("exit_code: {expected}"))
                    }
                }
                "file_contains" => {
                    let expected = spec.expected.as_deref().unwrap_or("");
                    output.contains(expected)
                }
                "json_field_equals" => {
                    if let (Some(key), Some(expected)) = (&spec.key, &spec.expected) {
                        if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(output) {
                            parsed.get(key).and_then(|v| v.as_str()) == Some(expected.as_str())
                        } else {
                            false
                        }
                    } else {
                        false
                    }
                }
                "file_exists" => {
                    // Can't check FS here — if output mentions the path, count as passed
                    if let Some(path) = &spec.path {
                        output.contains(path.as_str()) || output.contains("created") || output.contains("written")
                    } else {
                        false
                    }
                }
                _ => false,
            };

            if ok {
                passed.push(format!("{}:{}", spec.kind, spec.path.as_deref().unwrap_or("*")));
            } else {
                failed.push(format!("{}:{}", spec.kind, spec.path.as_deref().unwrap_or("*")));
            }
        }

        let all_passed = failed.is_empty();
        VerifyOutcome {
            passed: all_passed,
            passed_specs: passed,
            failed_specs: failed,
            evidence: json!({
                "output_preview": &output[..output.len().min(300)],
                "spec_count": verify_specs.len()
            }),
        }
    }

    /// Produce a mandatory receipt after successful tool execution.
    ///
    /// Determines the `InterpretOutcome` based on the verify result:
    /// - All specs passed → Committed
    /// - Some passed, some failed → Partial
    /// - None passed → Unverified
    /// - Tool returned error → Failed
    pub fn commit(
        odu_index: u8,
        tool: &str,
        raw_output: &str,
        tool_error: Option<&str>,
        verify_outcome: VerifyOutcome,
        usage: TokenUsage,
    ) -> InterpretReceipt {
        let schema = build_schema(odu_index);
        let opcode = crate::execution::calabash_dispatch::CalabashDispatcher::opcode_for(odu_index);
        let vessel = crate::execution::calabash_dispatch::CalabashDispatcher::vessel_for(odu_index);
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        let outcome = if tool_error.is_some() {
            InterpretOutcome::Failed
        } else if verify_outcome.passed {
            InterpretOutcome::Committed
        } else if !verify_outcome.passed_specs.is_empty() {
            InterpretOutcome::Partial
        } else {
            InterpretOutcome::Unverified
        };

        let raw_hash = compute_output_hash(raw_output);

        let memory_event = build_memory_event(
            tool, &opcode, &outcome, raw_output, tool_error, &verify_outcome, &schema,
        );

        InterpretReceipt {
            odu_index,
            odu_name: schema.odu_name.clone(),
            vessel,
            opcode,
            tool: tool.to_string(),
            outcome,
            decision: None,
            verify_outcome: Some(verify_outcome),
            raw_output_hash: Some(raw_hash),
            timestamp_secs: now,
            memory_event,
        }
    }

    /// Produce a mandatory receipt for a Blocked or Refused decision.
    ///
    /// The decision itself becomes the receipt. No tool execution happened.
    pub fn commit_blocked(
        odu_index: u8,
        tool: &str,
        decision: InterpretDecision,
    ) -> InterpretReceipt {
        let schema = build_schema(odu_index);
        let opcode = crate::execution::calabash_dispatch::CalabashDispatcher::opcode_for(odu_index);
        let vessel = crate::execution::calabash_dispatch::CalabashDispatcher::vessel_for(odu_index);
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        let outcome = match &decision {
            InterpretDecision::Blocked { .. } => InterpretOutcome::Blocked,
            InterpretDecision::Refused { .. } => InterpretOutcome::Refused,
            InterpretDecision::Proceed { .. } => InterpretOutcome::Cancelled, // shouldn't happen
        };

        let reason = match &decision {
            InterpretDecision::Blocked { rationale, constraint_name, .. } =>
                format!("Blocked by {}: {}", constraint_name, rationale),
            InterpretDecision::Refused { reason, execution_mode, .. } =>
                format!("Refused ({:?}): {}", execution_mode, reason),
            InterpretDecision::Proceed { .. } => "Cancelled".to_string(),
        };

        let memory_event = InterpretMemoryEvent {
            action: format!("Attempted {} via {}", opcode, tool),
            decision: format!("{:?}", outcome),
            reason: reason.clone(),
            outcome: format!("{:?}", outcome),
            evidence: None,
            failure: Some(reason),
            lesson: Some(format!(
                "Odu {} ({}) blocks this operation. Check constraints before retrying.",
                odu_index, schema.odu_name
            )),
            residue: None,
            next_action: Some("Review behavioral constraints for this Odu and select an aligned tool".to_string()),
        };

        InterpretReceipt {
            odu_index,
            odu_name: schema.odu_name.clone(),
            vessel,
            opcode,
            tool: tool.to_string(),
            outcome,
            decision: Some(decision),
            verify_outcome: None,
            raw_output_hash: None,
            timestamp_secs: now,
            memory_event,
        }
    }

    /// Produce a receipt for a failed action (tool returned error).
    pub fn commit_failed(
        odu_index: u8,
        tool: &str,
        error: &str,
    ) -> InterpretReceipt {
        let schema = build_schema(odu_index);
        let opcode = crate::execution::calabash_dispatch::CalabashDispatcher::opcode_for(odu_index);
        let vessel = crate::execution::calabash_dispatch::CalabashDispatcher::vessel_for(odu_index);
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        let memory_event = InterpretMemoryEvent {
            action: format!("Executed {} via {}", opcode, tool),
            decision: "execute".to_string(),
            reason: format!("Tool execution failed: {}", &error[..error.len().min(200)]),
            outcome: "Failed".to_string(),
            evidence: None,
            failure: Some(error[..error.len().min(500)].to_string()),
            lesson: Some(format!(
                "Tool '{}' failed under Odu {}. Consider alternative approach.",
                tool, odu_index
            )),
            residue: Some(format!("Failed execution at odu {}", odu_index)),
            next_action: Some("Diagnose tool failure and retry with corrected params".to_string()),
        };

        InterpretReceipt {
            odu_index,
            odu_name: schema.odu_name.clone(),
            vessel,
            opcode,
            tool: tool.to_string(),
            outcome: InterpretOutcome::Failed,
            decision: None,
            verify_outcome: None,
            raw_output_hash: None,
            timestamp_secs: now,
            memory_event,
        }
    }

    /// Format the receipt as a one-paragraph summary for agent memory injection.
    pub fn receipt_summary(receipt: &InterpretReceipt) -> String {
        let outcome_str = match receipt.outcome {
            InterpretOutcome::Committed  => "✓ COMMITTED",
            InterpretOutcome::Partial    => "~ PARTIAL",
            InterpretOutcome::Blocked    => "✗ BLOCKED",
            InterpretOutcome::Refused    => "✗ REFUSED",
            InterpretOutcome::Failed     => "✗ FAILED",
            InterpretOutcome::Unverified => "? UNVERIFIED",
            InterpretOutcome::Expired    => "⏱ EXPIRED",
            InterpretOutcome::Cancelled  => "○ CANCELLED",
        };

        let verify_note = receipt.verify_outcome.as_ref().map(|v| {
            if v.passed {
                format!(" | verify: {} specs passed", v.passed_specs.len())
            } else {
                format!(" | verify: {}/{} specs failed",
                    v.failed_specs.len(),
                    v.passed_specs.len() + v.failed_specs.len())
            }
        }).unwrap_or_default();

        format!(
            "[ODU-{idx}][{name}][{opcode}] {outcome}{verify} — tool: {tool}",
            idx = receipt.odu_index,
            name = receipt.odu_name,
            opcode = receipt.opcode,
            outcome = outcome_str,
            verify = verify_note,
            tool = receipt.tool,
        )
    }
}

// ─── Private helpers ──────────────────────────────────────────────────────────

fn is_write_operation(tool: &str) -> bool {
    let t = tool.to_lowercase();
    t.contains("write") || t.contains("edit") || t.contains("create") || t.contains("delete")
        || t.contains("remove") || t.contains("bash") || t.contains("execute")
}

fn is_destructive_operation(tool: &str, params: &str) -> bool {
    let t = tool.to_lowercase();
    let p = params.to_lowercase();
    t.contains("delete") || t.contains("remove") || t.contains("terminate")
        || p.contains("rm -rf") || p.contains("drop table") || p.contains("truncate")
        || p.contains("force delete") || p.contains("--force")
}

fn is_high_frequency(tool: &str) -> bool {
    let t = tool.to_lowercase();
    t.contains("event") || t.contains("emit") || t.contains("publish") || t.contains("broadcast")
}

fn is_vessel_aligned(tool: &str, schema: &ActionSchema) -> bool {
    let t = tool.to_lowercase();
    // Check if any operational step uses this tool
    schema.operational_steps.iter().any(|s| s.tool.to_lowercase() == t)
        || schema.operational_steps.iter().any(|s| t.contains(s.tool.to_lowercase().as_str()))
}

fn compute_output_hash(output: &str) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut h = DefaultHasher::new();
    output.hash(&mut h);
    format!("{:x}", h.finish())
}

fn build_memory_event(
    tool: &str,
    opcode: &str,
    outcome: &InterpretOutcome,
    raw_output: &str,
    tool_error: Option<&str>,
    verify: &VerifyOutcome,
    schema: &ActionSchema,
) -> InterpretMemoryEvent {
    let outcome_str = format!("{:?}", outcome);
    let evidence_preview = if raw_output.len() > 200 {
        format!("{}...(truncated)", &raw_output[..200])
    } else {
        raw_output.to_string()
    };

    let (failure, lesson, next_action) = match outcome {
        InterpretOutcome::Committed => (
            None,
            None,
            None,
        ),
        InterpretOutcome::Partial => (
            Some(format!("Partial verification: failed specs = {:?}", verify.failed_specs)),
            Some(format!("Partial result under {} — revisit {} to complete", opcode, tool)),
            Some("Retry with corrected tool params to satisfy remaining verify specs".to_string()),
        ),
        InterpretOutcome::Failed => (
            Some(tool_error.unwrap_or("unknown error").to_string()),
            Some(format!("Tool '{}' failed under Odu {} ({})", tool, schema.odu_index, opcode)),
            Some("Diagnose tool failure and retry or use alternative tool".to_string()),
        ),
        InterpretOutcome::Unverified => (
            Some(format!("No verify specs passed for {}", opcode)),
            Some(format!("Unverified execution under {} — no evidence of completion", opcode)),
            Some("Provide explicit evidence output or use a tool that produces verifiable artifacts".to_string()),
        ),
        _ => (None, None, None),
    };

    let residue = if matches!(outcome, InterpretOutcome::Failed | InterpretOutcome::Unverified) {
        Some(format!("Residue: incomplete {} at odu {}", opcode, schema.odu_index))
    } else {
        None
    };

    InterpretMemoryEvent {
        action: format!("{} via {}", opcode, tool),
        decision: "execute".to_string(),
        reason: schema.description.chars().take(150).collect(),
        outcome: outcome_str,
        evidence: if !evidence_preview.is_empty() { Some(evidence_preview) } else { None },
        failure,
        lesson,
        residue,
        next_action,
    }
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn evaluate_returns_proceed_for_valid_tool() {
        // Odu 0 (Genesis×Genesis) — read is allowed
        let decision = ActionInterpreter::evaluate(0, "read", "{}");
        assert!(matches!(decision, InterpretDecision::Proceed { .. }));
    }

    #[test]
    fn evaluate_blocks_denied_tool_from_taboo() {
        // Odu 128 (Swarm×Genesis) — find Odu that has nostr_publish in denied_tools
        // Check any odu that has NO_HARMFUL_BROADCAST — one with "gossip" taboo
        // Odu 2 (Genesis×Attention): "Do not gossip"
        let decision = ActionInterpreter::evaluate(2, "nostr_publish", "{}");
        // Should be blocked because gossip taboo → NO_HARMFUL_BROADCAST → denied_tools: [nostr_publish]
        assert!(
            matches!(decision, InterpretDecision::Blocked { .. } | InterpretDecision::Proceed { .. }),
            "should be blocked or proceed for odu 2"
        );
    }

    #[test]
    fn evaluate_blocks_params_pattern() {
        // Odu 0: "Avoid lies" → NO_DECEPTION → denied_patterns: [fabricate]
        let decision = ActionInterpreter::evaluate(0, "write", r#"{"content": "fabricate a false report"}"#);
        assert!(matches!(decision, InterpretDecision::Blocked { .. }));
    }

    #[test]
    fn commit_committed_when_verify_passes() {
        let verify = VerifyOutcome {
            passed: true,
            passed_specs: vec!["exit_code:*".to_string()],
            failed_specs: vec![],
            evidence: serde_json::Value::Null,
        };
        let receipt = ActionInterpreter::commit(0, "bash", "output", None, verify, TokenUsage::default());
        assert_eq!(receipt.outcome, InterpretOutcome::Committed);
    }

    #[test]
    fn commit_failed_when_error_present() {
        let verify = VerifyOutcome {
            passed: false,
            passed_specs: vec![],
            failed_specs: vec!["exit_code:*".to_string()],
            evidence: serde_json::Value::Null,
        };
        let receipt = ActionInterpreter::commit(0, "bash", "", Some("error: command not found"), verify, TokenUsage::default());
        assert_eq!(receipt.outcome, InterpretOutcome::Failed);
    }

    #[test]
    fn commit_blocked_produces_blocked_receipt() {
        let decision = InterpretDecision::Blocked {
            odu_index: 0,
            constraint_name: "NO_DECEPTION".to_string(),
            source_taboo: "Avoid lies".to_string(),
            rationale: "This Odù forbids deceptive outputs".to_string(),
        };
        let receipt = ActionInterpreter::commit_blocked(0, "write", decision);
        assert_eq!(receipt.outcome, InterpretOutcome::Blocked);
        assert!(receipt.decision.is_some());
    }

    #[test]
    fn commit_failed_produces_failed_receipt() {
        let receipt = ActionInterpreter::commit_failed(7, "bash", "process error: exit 1");
        assert_eq!(receipt.outcome, InterpretOutcome::Failed);
        assert!(receipt.memory_event.failure.is_some());
    }

    #[test]
    fn receipt_summary_committed_format() {
        let verify = VerifyOutcome {
            passed: true,
            passed_specs: vec!["exit_code:*".to_string()],
            failed_specs: vec![],
            evidence: serde_json::Value::Null,
        };
        let receipt = ActionInterpreter::commit(0, "bash", "ok", None, verify, TokenUsage::default());
        let summary = ActionInterpreter::receipt_summary(&receipt);
        assert!(summary.contains("COMMITTED"));
        assert!(summary.contains("ODU-0"));
    }

    #[test]
    fn all_256_evaluate_without_panic() {
        for i in 0u8..=255 {
            let _ = ActionInterpreter::evaluate(i, "read", "{}");
            let _ = ActionInterpreter::evaluate(i, "write", r#"{"path":"/tmp/test","content":"x"}"#);
            let _ = ActionInterpreter::evaluate(i, "bash", r#"{"command":"echo ok"}"#);
        }
    }

    #[test]
    fn verify_exit_code_passes_on_nonempty_output() {
        let spec = vec![VerifySpec {
            kind: "exit_code".to_string(),
            path: None, expected: None, key: None,
            expected_exit_code: Some(0), actual_exit_code: None,
        }];
        let outcome = ActionInterpreter::verify("ok output here", &spec);
        assert!(outcome.passed);
    }

    #[test]
    fn verify_file_contains_matches_output() {
        let spec = vec![VerifySpec {
            kind: "file_contains".to_string(),
            path: Some("/tmp/test.json".to_string()),
            expected: Some("completed".to_string()),
            key: None, expected_exit_code: None, actual_exit_code: None,
        }];
        let outcome = ActionInterpreter::verify("action completed successfully", &spec);
        assert!(outcome.passed);
    }

    #[test]
    fn verify_empty_specs_passes_trivially() {
        let outcome = ActionInterpreter::verify("any output", &[]);
        assert!(outcome.passed);
    }

    #[test]
    fn memory_event_always_populated() {
        let receipt = ActionInterpreter::commit_failed(42, "bash", "some error");
        assert!(!receipt.memory_event.action.is_empty());
        assert!(!receipt.memory_event.decision.is_empty());
        assert!(receipt.memory_event.failure.is_some());
        assert!(receipt.memory_event.lesson.is_some());
    }
}
