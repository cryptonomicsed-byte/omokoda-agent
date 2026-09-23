// H4: Twelve-thrones deliberation integration
//
// Bridges the Rust hive coordinator to the TypeScript Twelve-thrones server
// (~/Twelve-thrones/server.ts, port 3030).
//
// During the Deliberate phase, HiveBreath calls ThronesClient::deliberate()
// which POST /deliberate with a deliberation request and receives ranked verdicts.
// Fail-open: when THRONES_URL is unset, returns an empty verdict set.

use serde::{Deserialize, Serialize};

/// The 12 throne identities (matching Twelve-thrones/server.ts)
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ThroneId {
    OBATALA,
    OGUN,
    SHANGO,
    YEMOJA,
    OSHUN,
    ESHU,
    ORUNMILA,
    ODUDUWA,
    OYA,
    EGUNGON,  // Throne 10: Mistral Large
    OGUN2,    // Throne 11: Claude Haiku 3.5 (Ògún second seat)
    OBALUAYE, // Throne 12: Gemini 2.0 Flash
}

/// A deliberation request sent to the Twelve-thrones server.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeliberationRequest {
    pub tick: u64,
    pub question: String,
    pub context: serde_json::Value,
    pub proposals: Vec<String>,
}

/// A verdict from one throne seat.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThroneVerdict {
    pub throne: ThroneId,
    pub ranking: Vec<String>,
    pub confidence: f64,
    pub reasoning: String,
}

/// Aggregated deliberation result across all 12 thrones.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeliberationResult {
    pub tick: u64,
    pub verdicts: Vec<ThroneVerdict>,
    /// Borda-count ranked proposals (first = most preferred)
    pub consensus_ranking: Vec<String>,
    pub quorum_reached: bool,
}

/// HTTP client for the Twelve-thrones deliberation server.
pub struct ThronesClient {
    pub base_url: String,
}

impl ThronesClient {
    /// Construct from THRONES_URL env var; fail-open if unset.
    pub fn from_env() -> Self {
        let base_url = std::env::var("THRONES_URL")
            .unwrap_or_else(|_| String::new());
        Self { base_url }
    }

    pub fn is_enabled(&self) -> bool {
        !self.base_url.is_empty()
    }

    /// Send a deliberation request and receive ranked verdicts.
    /// Returns Ok(empty) when THRONES_URL is unset (fail-open).
    pub async fn deliberate(
        &self,
        request: DeliberationRequest,
    ) -> Result<DeliberationResult, String> {
        if !self.is_enabled() {
            return Ok(DeliberationResult {
                tick: request.tick,
                verdicts: vec![],
                consensus_ranking: request.proposals.clone(),
                quorum_reached: false,
            });
        }

        let url = format!("{}/deliberate", self.base_url);
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
            return Err(format!("thrones server returned {}", resp.status()));
        }

        resp.json::<DeliberationResult>()
            .await
            .map_err(|e| e.to_string())
    }

    /// Borda count aggregation over verdicts (used locally when server unavailable).
    pub fn borda_aggregate(verdicts: &[ThroneVerdict], proposals: &[String]) -> Vec<String> {
        let n = proposals.len();
        let mut scores: std::collections::HashMap<&str, usize> = proposals
            .iter()
            .map(|p| (p.as_str(), 0))
            .collect();

        for verdict in verdicts {
            for (rank, proposal) in verdict.ranking.iter().enumerate() {
                let points = n.saturating_sub(rank);
                *scores.entry(proposal.as_str()).or_insert(0) += points;
            }
        }

        let mut ranked: Vec<(&str, usize)> = scores.into_iter().collect();
        ranked.sort_by(|a, b| b.1.cmp(&a.1));
        ranked.into_iter().map(|(p, _)| p.to_string()).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fail_open_when_url_unset() {
        std::env::remove_var("THRONES_URL");
        let client = ThronesClient::from_env();
        assert!(!client.is_enabled());
    }

    #[test]
    fn borda_count_aggregates_correctly() {
        let verdicts = vec![
            ThroneVerdict {
                throne: ThroneId::OBATALA,
                ranking: vec!["A".into(), "B".into(), "C".into()],
                confidence: 0.9,
                reasoning: "A is best".into(),
            },
            ThroneVerdict {
                throne: ThroneId::OGUN,
                ranking: vec!["B".into(), "A".into(), "C".into()],
                confidence: 0.7,
                reasoning: "B is best".into(),
            },
        ];
        let proposals = vec!["A".to_string(), "B".to_string(), "C".to_string()];
        let ranking = ThronesClient::borda_aggregate(&verdicts, &proposals);
        // A: 3+2=5, B: 2+3=5, C: 1+1=2 → A or B first, C last
        assert_eq!(ranking.last().unwrap(), "C");
    }

    #[tokio::test]
    async fn fail_open_returns_original_order() {
        std::env::remove_var("THRONES_URL");
        let client = ThronesClient::from_env();
        let req = DeliberationRequest {
            tick: 1,
            question: "what to do?".into(),
            context: serde_json::json!({}),
            proposals: vec!["survive".into(), "learn".into()],
        };
        let result = client.deliberate(req).await.unwrap();
        assert!(!result.quorum_reached);
        assert_eq!(result.consensus_ranking, vec!["survive", "learn"]);
    }
}
