// Action Transaction — the full lifecycle of a single agent action.
//
// Every agent action moves through this state machine:
//   PROPOSED → RESOLVED → PREFLIGHT → AUTHORIZED → EXECUTING →
//   VERIFYING → COMMITTED → RECEIPTED → SCHEDULED | CLOSED
//
// The OS owns state transitions; the LLM only proposes.
// All outcomes (success, partial, blocked, failed, refused, expired, cancelled)
// produce a receipt and a structured memory event.

use std::time::{SystemTime, UNIX_EPOCH};
use serde::{Deserialize, Serialize};
use crate::gates::ActionIntent;

fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Every possible outcome for a completed or terminated action.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ActionOutcome {
    /// Action completed and all verify assertions passed.
    Success,
    /// Action ran but some verify assertions failed; partial state written.
    Partial { assertions_failed: Vec<String> },
    /// A Hermetic gate blocked execution before the action started.
    Blocked { gate: String, reason: String },
    /// The action started but the tool returned an error.
    Failed { error: String },
    /// Agent refused to proceed after LLM proposed (e.g. consent not given).
    Refused { reason: String },
    /// Action was proposed but not executed within the cadence window.
    Expired { proposed_at: u64, window_secs: u64 },
    /// Operator or agent cancelled the transaction before AUTHORIZED.
    Cancelled { by: String },
}

/// The canonical state machine for an action transaction.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ActionState {
    /// LLM has proposed an action; OS has not yet accepted it.
    Proposed,
    /// OS has resolved the action against the Calabash corpus.
    Resolved { vessel: u8, opcode: String },
    /// Pre-flight checks in progress (permission, budget, consent).
    Preflight,
    /// All 7 Hermetic gates passed; action is authorized to run.
    Authorized { gate_score: f64 },
    /// Tool execution is in progress.
    Executing,
    /// Tool has returned; verify assertions are being evaluated.
    Verifying,
    /// All assertions passed; state has been committed.
    Committed,
    /// Receipt has been produced and written to memory.
    Receipted { receipt_id: String },
    /// Action has been scheduled for a future cadence tick.
    Scheduled { next_tick: u64 },
    /// Terminal state — action is complete (Success/Partial/Failed/etc).
    Closed { outcome: ActionOutcome, closed_at: u64 },
}

/// A single step within an action execution, with evidence.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionStep {
    pub description: String,
    pub tool: Option<String>,
    pub evidence: Vec<String>,
    pub completed_at: Option<u64>,
    pub error: Option<String>,
}

/// Artifact produced or consumed by an action (path + SHA-256 hash).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionArtifact {
    pub path: String,
    pub sha256: Option<String>,
    pub role: ArtifactRole,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ArtifactRole {
    Input,
    Output,
    Modified,
}

/// The full typed receipt for an action — structured consequence, not just a log entry.
/// This is the bridge between execution and memory: every receipt becomes a memory event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionReceipt {
    pub receipt_id: String,
    pub transaction_id: String,
    pub agent_id: String,
    /// Which Calabash vessel governed this action (0–15).
    pub vessel: u8,
    /// The opcode / directive label from the Calabash corpus.
    pub opcode: String,
    pub purpose: String,
    pub target: String,
    /// Gate alignment score (0.0–1.0 mean of 7 gate scores).
    pub gate_alignment: f64,
    pub authorized_at: u64,
    pub completed_at: u64,
    pub outcome: ActionOutcome,
    /// Step-by-step execution trace with evidence at each step.
    pub steps: Vec<ActionStep>,
    /// Files/objects created, modified, or consumed.
    pub artifacts: Vec<ActionArtifact>,
    /// Assertions that were verified (name → pass/fail).
    pub verifications: Vec<(String, bool)>,
    /// Forward pointer to any follow-up action (scheduled or spawned).
    pub traces_to: Option<String>,
    /// Structured memory event derived from this receipt.
    pub memory_event: MemoryEvent,
}

/// Consequence record written to the agent's memory after every action outcome.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryEvent {
    pub action: String,
    pub decision: String,
    pub reason: String,
    pub outcome: String,
    /// Evidence used during execution (e.g. file contents read, API responses).
    pub evidence: Vec<String>,
    /// The specific failure description when outcome is not Success.
    pub failure: Option<String>,
    /// Lesson distilled from this action (written on failure/partial).
    pub lesson: Option<String>,
    /// Residual state that persisted after the action (changed files, updated memory, etc).
    pub residue: Vec<String>,
    /// Pointer to the next action if this one triggers a chain.
    pub next_action: Option<String>,
}

/// Active transaction tracking a single action from proposal to close.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionTransaction {
    pub id: String,
    pub agent_id: String,
    pub action_intent: ActionIntent,
    pub state: ActionState,
    pub proposed_at: u64,
    pub steps: Vec<ActionStep>,
    pub artifacts: Vec<ActionArtifact>,
    pub verifications: Vec<(String, bool)>,
    pub gate_alignment: Option<f64>,
    pub vessel: Option<u8>,
    pub opcode: Option<String>,
}

impl ActionTransaction {
    pub fn new(id: String, agent_id: String, action_intent: ActionIntent) -> Self {
        Self {
            id,
            agent_id,
            action_intent,
            state: ActionState::Proposed,
            proposed_at: unix_now(),
            steps: Vec::new(),
            artifacts: Vec::new(),
            verifications: Vec::new(),
            gate_alignment: None,
            vessel: None,
            opcode: None,
        }
    }

    /// Advance to Resolved after OS matches the action to the Calabash corpus.
    pub fn resolve(&mut self, vessel: u8, opcode: String) -> Result<(), String> {
        if self.state != ActionState::Proposed {
            return Err(format!("cannot resolve from state {:?}", self.state));
        }
        self.vessel = Some(vessel);
        self.opcode = Some(opcode.clone());
        self.state = ActionState::Resolved { vessel, opcode };
        Ok(())
    }

    /// Enter preflight (permission + budget + consent checks).
    pub fn begin_preflight(&mut self) -> Result<(), String> {
        if !matches!(self.state, ActionState::Resolved { .. }) {
            return Err(format!("cannot enter preflight from state {:?}", self.state));
        }
        self.state = ActionState::Preflight;
        Ok(())
    }

    /// Authorize after all 7 gates pass.
    pub fn authorize(&mut self, gate_score: f64) -> Result<(), String> {
        if self.state != ActionState::Preflight {
            return Err(format!("cannot authorize from state {:?}", self.state));
        }
        self.gate_alignment = Some(gate_score);
        self.state = ActionState::Authorized { gate_score };
        Ok(())
    }

    /// Block the transaction (a gate rejected it).
    pub fn block(&mut self, gate: String, reason: String) -> ActionReceipt {
        let outcome = ActionOutcome::Blocked { gate, reason };
        self.close_with(outcome)
    }

    /// Begin execution.
    pub fn begin_executing(&mut self) -> Result<(), String> {
        if !matches!(self.state, ActionState::Authorized { .. }) {
            return Err(format!("cannot execute from state {:?}", self.state));
        }
        self.state = ActionState::Executing;
        Ok(())
    }

    /// Record a completed step with evidence.
    pub fn record_step(&mut self, description: String, tool: Option<String>, evidence: Vec<String>) {
        self.steps.push(ActionStep {
            description,
            tool,
            evidence,
            completed_at: Some(unix_now()),
            error: None,
        });
    }

    /// Record an artifact (file produced or modified).
    pub fn record_artifact(&mut self, path: String, sha256: Option<String>, role: ArtifactRole) {
        self.artifacts.push(ActionArtifact { path, sha256, role });
    }

    /// Enter verify phase.
    pub fn begin_verifying(&mut self) -> Result<(), String> {
        if self.state != ActionState::Executing {
            return Err(format!("cannot verify from state {:?}", self.state));
        }
        self.state = ActionState::Verifying;
        Ok(())
    }

    /// Record an assertion result.
    pub fn record_assertion(&mut self, name: String, passed: bool) {
        self.verifications.push((name, passed));
    }

    /// Commit after all verifications pass.
    pub fn commit(&mut self) -> Result<(), String> {
        if self.state != ActionState::Verifying {
            return Err(format!("cannot commit from state {:?}", self.state));
        }
        self.state = ActionState::Committed;
        Ok(())
    }

    /// Produce the receipt and close the transaction.
    pub fn receipt(&mut self, receipt_id: String, traces_to: Option<String>) -> ActionReceipt {
        let outcome = if self.verifications.iter().all(|(_, p)| *p) {
            ActionOutcome::Success
        } else {
            let failed: Vec<String> = self
                .verifications
                .iter()
                .filter(|(_, p)| !*p)
                .map(|(n, _)| n.clone())
                .collect();
            ActionOutcome::Partial { assertions_failed: failed }
        };
        let closed = self.close_with_receipt(receipt_id.clone(), outcome.clone(), traces_to);
        closed
    }

    /// Close with a failure outcome.
    pub fn fail(&mut self, error: String) -> ActionReceipt {
        self.close_with(ActionOutcome::Failed { error })
    }

    /// Close with a refused outcome.
    pub fn refuse(&mut self, reason: String) -> ActionReceipt {
        self.close_with(ActionOutcome::Refused { reason })
    }

    /// Close with a cancelled outcome.
    pub fn cancel(&mut self, by: String) -> ActionReceipt {
        self.close_with(ActionOutcome::Cancelled { by })
    }

    /// Schedule for a future tick.
    pub fn schedule(&mut self, next_tick: u64) -> Result<(), String> {
        if !matches!(
            self.state,
            ActionState::Authorized { .. } | ActionState::Committed
        ) {
            return Err(format!("cannot schedule from state {:?}", self.state));
        }
        self.state = ActionState::Scheduled { next_tick };
        Ok(())
    }

    fn close_with(&mut self, outcome: ActionOutcome) -> ActionReceipt {
        let receipt_id = format!("rx-{}", self.id);
        self.close_with_receipt(receipt_id, outcome, None)
    }

    fn close_with_receipt(
        &mut self,
        receipt_id: String,
        outcome: ActionOutcome,
        traces_to: Option<String>,
    ) -> ActionReceipt {
        let closed_at = unix_now();
        self.state = ActionState::Closed {
            outcome: outcome.clone(),
            closed_at,
        };
        let memory_event = build_memory_event(&self.action_intent, &outcome, &self.steps);
        ActionReceipt {
            receipt_id: receipt_id.clone(),
            transaction_id: self.id.clone(),
            agent_id: self.agent_id.clone(),
            vessel: self.vessel.unwrap_or(255),
            opcode: self.opcode.clone().unwrap_or_else(|| "unknown".to_string()),
            purpose: self.action_intent.purpose.clone(),
            target: self.action_intent.target.clone(),
            gate_alignment: self.gate_alignment.unwrap_or(0.0),
            authorized_at: self.proposed_at,
            completed_at: closed_at,
            outcome,
            steps: self.steps.clone(),
            artifacts: self.artifacts.clone(),
            verifications: self.verifications.clone(),
            traces_to,
            memory_event,
        }
    }
}

fn build_memory_event(ai: &ActionIntent, outcome: &ActionOutcome, steps: &[ActionStep]) -> MemoryEvent {
    let evidence: Vec<String> = steps
        .iter()
        .flat_map(|s| s.evidence.iter().cloned())
        .collect();

    let (outcome_str, failure, lesson) = match outcome {
        ActionOutcome::Success => ("success".to_string(), None, None),
        ActionOutcome::Partial { assertions_failed } => (
            "partial".to_string(),
            Some(format!("assertions failed: {}", assertions_failed.join(", "))),
            Some("verify assertions did not all pass — check preconditions before next attempt".to_string()),
        ),
        ActionOutcome::Blocked { gate, reason } => (
            "blocked".to_string(),
            Some(format!("{gate} gate: {reason}")),
            Some(format!("action was blocked by {gate} — review intent alignment")),
        ),
        ActionOutcome::Failed { error } => (
            "failed".to_string(),
            Some(error.clone()),
            Some("tool execution failed — check tool availability and parameters".to_string()),
        ),
        ActionOutcome::Refused { reason } => (
            "refused".to_string(),
            Some(reason.clone()),
            Some("agent refused — consent or constitutional requirement not met".to_string()),
        ),
        ActionOutcome::Expired { .. } => (
            "expired".to_string(),
            Some("cadence window elapsed without execution".to_string()),
            Some("schedule this action earlier in the cadence cycle".to_string()),
        ),
        ActionOutcome::Cancelled { by } => (
            "cancelled".to_string(),
            Some(format!("cancelled by {by}")),
            None,
        ),
    };

    let residue: Vec<String> = steps
        .iter()
        .filter_map(|s| {
            if s.completed_at.is_some() && s.error.is_none() {
                Some(s.description.clone())
            } else {
                None
            }
        })
        .collect();

    MemoryEvent {
        action: ai.purpose.clone(),
        decision: format!("act on target: {}", ai.target),
        reason: ai.expected_effects.join("; "),
        outcome: outcome_str,
        evidence,
        failure,
        lesson,
        residue,
        next_action: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gates::{ActionIntent, DataSensitivity, Reversibility};

    fn test_intent() -> ActionIntent {
        ActionIntent {
            purpose: "write_config".to_string(),
            target: "workspace/config.toml".to_string(),
            mutations: vec!["file_write".to_string()],
            data_sensitivity: DataSensitivity::Internal,
            network_access: false,
            consent_required: false,
            reversibility: Reversibility::Reversible,
            expected_effects: vec!["config.toml updated".to_string()],
            receipt_required: true,
        }
    }

    #[test]
    fn happy_path_state_machine() {
        let mut tx = ActionTransaction::new(
            "tx-001".to_string(),
            "agent-abc".to_string(),
            test_intent(),
        );
        assert!(matches!(tx.state, ActionState::Proposed));

        tx.resolve(7, "execution:write_config".to_string()).unwrap();
        assert!(matches!(tx.state, ActionState::Resolved { .. }));

        tx.begin_preflight().unwrap();
        assert!(matches!(tx.state, ActionState::Preflight));

        tx.authorize(0.88).unwrap();
        assert!(matches!(tx.state, ActionState::Authorized { .. }));

        tx.begin_executing().unwrap();
        assert!(matches!(tx.state, ActionState::Executing));

        tx.record_step(
            "wrote config.toml".to_string(),
            Some("write_file".to_string()),
            vec!["file exists".to_string()],
        );

        tx.begin_verifying().unwrap();
        tx.record_assertion("file_exists:config.toml".to_string(), true);
        tx.commit().unwrap();

        let receipt = tx.receipt("rx-001".to_string(), None);
        assert!(matches!(receipt.outcome, ActionOutcome::Success));
        assert_eq!(receipt.memory_event.outcome, "success");
    }

    #[test]
    fn blocked_produces_receipt() {
        let mut tx = ActionTransaction::new("tx-002".to_string(), "agent-abc".to_string(), test_intent());
        tx.resolve(7, "execution:write_config".to_string()).unwrap();
        tx.begin_preflight().unwrap();

        let receipt = tx.block("Rhythm".to_string(), "active cooldown".to_string());
        assert!(matches!(receipt.outcome, ActionOutcome::Blocked { .. }));
        assert_eq!(receipt.memory_event.outcome, "blocked");
        assert!(receipt.memory_event.lesson.is_some());
    }

    #[test]
    fn partial_outcome_on_failed_assertions() {
        let mut tx = ActionTransaction::new("tx-003".to_string(), "agent-abc".to_string(), test_intent());
        tx.resolve(7, "execution:write_config".to_string()).unwrap();
        tx.begin_preflight().unwrap();
        tx.authorize(0.75).unwrap();
        tx.begin_executing().unwrap();
        tx.begin_verifying().unwrap();
        tx.record_assertion("file_exists:config.toml".to_string(), true);
        tx.record_assertion("hash_match:config.toml".to_string(), false);
        tx.commit().unwrap();

        let receipt = tx.receipt("rx-003".to_string(), None);
        assert!(matches!(receipt.outcome, ActionOutcome::Partial { .. }));
    }

    #[test]
    fn cancel_before_authorized_produces_receipt() {
        let mut tx = ActionTransaction::new("tx-004".to_string(), "agent-abc".to_string(), test_intent());
        let receipt = tx.cancel("operator".to_string());
        assert!(matches!(receipt.outcome, ActionOutcome::Cancelled { .. }));
    }

    #[test]
    fn invalid_state_transition_returns_error() {
        let mut tx = ActionTransaction::new("tx-005".to_string(), "agent-abc".to_string(), test_intent());
        // Cannot authorize from Proposed (must go through Preflight first)
        let result = tx.authorize(0.9);
        assert!(result.is_err());
    }
}
