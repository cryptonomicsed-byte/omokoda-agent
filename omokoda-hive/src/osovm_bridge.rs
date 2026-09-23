// H8: OSOVM simulation ↔ hive belief feedback loop
//
// The DNA double-helix architecture:
//   Axis A (OSOVM sim strand): hive submits GoalVectors as simulation inputs
//   Axis B (Omo-Koda real strand): hive receives Zàngbétò receipts as real evidence
//   EpistemicDelta = divergence between the two strands
//
// This bridge:
//   1. Submits GoalVectors to OSOVM as simulation runs
//   2. Receives simulation outputs as Belief updates (Axis A)
//   3. Receives Zàngbétò receipt confirmations as witnessed beliefs (Axis B)
//   4. Computes and emits the resulting EpistemicDelta
//
// Fail-open: OSOVM_URL unset → all simulation requests return empty results.

use serde::{Deserialize, Serialize};
use crate::epistemic::{BeliefSource, EpistemicState};
use crate::goal_vector::GoalVector;

/// A simulation run request sent to OSOVM.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimulationRequest {
    pub run_id: String,
    pub goal_vector_id: String,
    pub parameters: serde_json::Value,
    pub tick: u64,
}

/// Simulation output received from OSOVM.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimulationResult {
    pub run_id: String,
    pub beliefs: Vec<SimulationBelief>,
    pub osovm_proof: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimulationBelief {
    pub topic: String,
    pub confidence: f64,
    pub evidence: serde_json::Value,
}

/// A Zàngbétò receipt confirming real-world evidence for a topic.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZangbetoReceipt {
    pub receipt_id: String,
    pub topic: String,
    pub agent_id: String,
    pub tick: u64,
    pub signature: Option<String>,
}

/// Bridge between HiveBreath and OSOVM.
pub struct OsovmHiveBridge {
    pub osovm_url: String,
}

impl OsovmHiveBridge {
    pub fn from_env() -> Self {
        let osovm_url = std::env::var("OSOVM_URL")
            .unwrap_or_else(|_| String::new());
        Self { osovm_url }
    }

    pub fn is_enabled(&self) -> bool {
        !self.osovm_url.is_empty()
    }

    /// Submit a GoalVector to OSOVM as a simulation run.
    /// Returns the run_id to poll for results.
    pub async fn submit_simulation(
        &self,
        gv: &GoalVector,
        tick: u64,
    ) -> Result<String, String> {
        if !self.is_enabled() {
            return Ok(format!("sim-noop-tick-{tick}"));
        }

        let request = SimulationRequest {
            run_id: format!("sim-tick-{tick}"),
            goal_vector_id: gv.id.clone(),
            parameters: serde_json::json!({
                "goals": gv.goals.iter().map(|g| &g.topic).collect::<Vec<_>>(),
                "tick": tick,
            }),
            tick,
        };

        let url = format!("{}/api/osovm/run", self.osovm_url);
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .map_err(|e| e.to_string())?;

        let resp = client
            .post(&url)
            .json(&request)
            .send()
            .await
            .map_err(|e| e.to_string())?;

        if !resp.status().is_success() {
            return Err(format!("OSOVM returned {}", resp.status()));
        }

        let result: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;
        Ok(result["run_id"].as_str().unwrap_or(&request.run_id).to_string())
    }

    /// Apply simulation results to the hive's EpistemicState (Axis A update).
    pub fn apply_simulation_result(
        epistemic: &mut EpistemicState,
        result: &SimulationResult,
    ) {
        for belief in &result.beliefs {
            epistemic.update_belief(
                &belief.topic,
                belief.confidence,
                BeliefSource::Simulation {
                    run_id: result.run_id.clone(),
                },
            );
        }
    }

    /// Apply a Zàngbétò receipt to the hive's EpistemicState (Axis B update).
    pub fn apply_zangbeto_receipt(
        epistemic: &mut EpistemicState,
        receipt: &ZangbetoReceipt,
    ) {
        epistemic.witness_belief(&receipt.topic, &receipt.receipt_id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::epistemic::EpistemicState;

    #[test]
    fn fail_open_when_osovm_unset() {
        std::env::remove_var("OSOVM_URL");
        let bridge = OsovmHiveBridge::from_env();
        assert!(!bridge.is_enabled());
    }

    #[test]
    fn simulation_result_updates_beliefs() {
        let mut state = EpistemicState::new();
        let result = SimulationResult {
            run_id: "run-1".into(),
            beliefs: vec![
                SimulationBelief {
                    topic: "agent_health".into(),
                    confidence: 0.85,
                    evidence: serde_json::json!({}),
                },
            ],
            osovm_proof: None,
        };
        OsovmHiveBridge::apply_simulation_result(&mut state, &result);
        assert!(state.beliefs.contains_key("agent_health"));
        assert!((state.beliefs["agent_health"].confidence - 0.85).abs() < 0.001);
    }

    #[test]
    fn zangbeto_receipt_witnesses_belief() {
        let mut state = EpistemicState::new();
        // First add a simulated belief
        state.update_belief(
            "agent_health",
            0.6,
            BeliefSource::Simulation { run_id: "r".into() },
        );
        let before_delta = state.cumulative_delta;

        // Now witness it with a Zàngbétò receipt
        let receipt = ZangbetoReceipt {
            receipt_id: "zr-001".into(),
            topic: "agent_health".into(),
            agent_id: "agent-1".into(),
            tick: 5,
            signature: None,
        };
        OsovmHiveBridge::apply_zangbeto_receipt(&mut state, &receipt);

        // Delta should have decreased (real evidence confirming sim)
        assert!(state.cumulative_delta <= before_delta);
        // Confidence should have increased
        assert!(state.beliefs["agent_health"].confidence > 0.6);
    }

    #[tokio::test]
    async fn noop_run_returns_ok_when_disabled() {
        std::env::remove_var("OSOVM_URL");
        let bridge = OsovmHiveBridge::from_env();
        let gv = crate::goal_vector::GoalVector {
            id: "gv-1".into(),
            tick: 1,
            goals: vec![],
        };
        let result = bridge.submit_simulation(&gv, 1).await;
        assert!(result.is_ok());
    }
}
