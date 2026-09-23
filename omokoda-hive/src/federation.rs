// H5: Cross-hive federation via DIP
//
// Multiple Omokoda hive instances federate by exchanging GoalVectors
// and EpistemicDeltas over DIP envelopes (~/DIP/).
//
// Federation protocol:
//   1. Each hive publishes its GoalVector on the DIP mesh after Broadcast phase
//   2. Peer hives receive GoalVectors and merge them into their Receive phase
//   3. EpistemicDeltas from peer hives reduce local cumulative_delta
//      (confirmation from a peer that they witnessed the same thing)
//
// Fail-open: when DIP_URL is unset or unreachable, federation is disabled.

use serde::{Deserialize, Serialize};
use crate::goal_vector::GoalVector;
use crate::epistemic::EpistemicDelta;

/// A federation message exchanged between hive instances.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum HiveFederationMessage {
    /// Publishing this hive's GoalVector to peers
    GoalVectorPublish {
        hive_id: String,
        goal_vector: GoalVector,
    },
    /// Sharing epistemic delta so peers can confirm shared beliefs
    EpistemicShare {
        hive_id: String,
        delta: EpistemicDelta,
    },
    /// Requesting a peer's current GoalVector (pull model)
    GoalVectorRequest {
        from_hive: String,
        tick: u64,
    },
}

/// A DIP-based federation client for cross-hive communication.
pub struct HiveFederationClient {
    pub hive_id: String,
    pub dip_url: String,
}

impl HiveFederationClient {
    pub fn from_env(hive_id: String) -> Self {
        let dip_url = std::env::var("DIP_URL")
            .unwrap_or_else(|_| String::new());
        Self { hive_id, dip_url }
    }

    pub fn is_enabled(&self) -> bool {
        !self.dip_url.is_empty()
    }

    /// Publish this hive's GoalVector to peers via DIP.
    /// Fail-open: returns Ok(()) when DIP_URL is unset.
    pub async fn publish_goal_vector(&self, gv: &GoalVector) -> Result<(), String> {
        if !self.is_enabled() {
            return Ok(());
        }
        let msg = HiveFederationMessage::GoalVectorPublish {
            hive_id: self.hive_id.clone(),
            goal_vector: gv.clone(),
        };
        self.send_dip_message(msg).await
    }

    /// Share this hive's EpistemicDelta with peers.
    pub async fn share_epistemic_delta(&self, delta: &EpistemicDelta) -> Result<(), String> {
        if !self.is_enabled() {
            return Ok(());
        }
        let msg = HiveFederationMessage::EpistemicShare {
            hive_id: self.hive_id.clone(),
            delta: delta.clone(),
        };
        self.send_dip_message(msg).await
    }

    async fn send_dip_message(&self, msg: HiveFederationMessage) -> Result<(), String> {
        let url = format!("{}/api/dip/outbound", self.dip_url);
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(5))
            .build()
            .map_err(|e| e.to_string())?;

        let payload = serde_json::json!({
            "kind": "hive_federation",
            "source_hive": self.hive_id,
            "payload": serde_json::to_value(&msg).map_err(|e| e.to_string())?,
        });

        let resp = client
            .post(&url)
            .json(&payload)
            .send()
            .await
            .map_err(|e| e.to_string())?;

        if resp.status().is_success() {
            Ok(())
        } else {
            Err(format!("DIP returned {}", resp.status()))
        }
    }
}

/// Merges a peer's GoalVector proposals into the local Receive phase buffer.
/// Returns the number of new proposals added.
pub fn merge_peer_goal_vector(
    local_proposals: &mut Vec<crate::goal_vector::LobeGoalProposal>,
    peer_gv: &GoalVector,
    peer_hive_weight: f32,
) -> usize {
    use crate::goal_vector::LobeGoalProposal;
    use crate::lobe::OrisaLobe;

    let mut added = 0;
    for goal in &peer_gv.goals {
        // Represent peer goals as Orunmila (prophecy/external) proposals
        // discounted by peer_hive_weight
        let proposal = LobeGoalProposal {
            lobe: OrisaLobe::Orunmila,
            topic: format!("peer::{}", goal.topic),
            description: format!("[peer hive] {}", goal.description),
            urgency: goal.weighted_urgency * peer_hive_weight,
            alignment_score: goal.alignment_score,
            tick_proposed: peer_gv.tick,
        };
        local_proposals.push(proposal);
        added += 1;
    }
    added
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::goal_vector::{AggregatedGoal, GoalVector};
    use crate::lobe::OrisaLobe;

    #[test]
    fn fail_open_when_dip_url_unset() {
        std::env::remove_var("DIP_URL");
        let client = HiveFederationClient::from_env("hive-1".into());
        assert!(!client.is_enabled());
    }

    #[test]
    fn merge_peer_gv_adds_proposals() {
        let peer_gv = GoalVector {
            id: "gv-1".into(),
            tick: 5,
            goals: vec![AggregatedGoal {
                topic: "survival".into(),
                description: "keep running".into(),
                weighted_urgency: 0.8,
                alignment_score: 0.9,
                proposing_lobes: vec![OrisaLobe::Obatala],
            }],
        };
        let mut local = Vec::new();
        let added = merge_peer_goal_vector(&mut local, &peer_gv, 0.5);
        assert_eq!(added, 1);
        assert_eq!(local[0].urgency, 0.4); // 0.8 × 0.5
        assert!(local[0].topic.starts_with("peer::"));
    }
}
