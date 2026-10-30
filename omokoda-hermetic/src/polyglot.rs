//! Polyglot service integration for hermetic scoring.
//!
//! Provides `wisdom_hermetic_score(text)` — a fire-and-forget async helper
//! that asks the Clojure Wisdom service (WISDOM_ENGINE_URL, default :7704)
//! for an alignment score and blends it with the local Rust hermetic score.
//!
//! ## Blend formula
//! `final_score = rust_score * 0.6 + wisdom_score.unwrap_or(rust_score) * 0.4`
//!
//! Fail-open: if the Wisdom service is unreachable or returns an unexpected
//! payload, `wisdom_score` is `None` and the blend collapses to `rust_score`.

use serde_json::Value;
use std::time::Duration;

/// Ask the Clojure Wisdom service for a hermetic alignment score for `text`.
///
/// Returns `Some(score)` in [0.0, 1.0] when the service responds, or `None`
/// on any error (network, timeout, bad JSON) — always fail-open.
pub async fn wisdom_hermetic_score(text: &str) -> Option<f64> {
    let url = std::env::var("WISDOM_ENGINE_URL")
        .unwrap_or_else(|_| "http://localhost:7704".to_string());
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(3))
        .build()
        .ok()?;
    let body = serde_json::json!({ "text": text });
    match client
        .post(&format!("{}/hermetic", url))
        .json(&body)
        .send()
        .await
    {
        Ok(resp) => {
            if let Ok(json) = resp.json::<Value>().await {
                json["alignment"].as_f64()
            } else {
                None // fail-open
            }
        }
        Err(_) => None, // fail-open
    }
}

/// Blend a locally-computed `rust_score` with the Clojure Wisdom service score.
///
/// `final_score = rust_score * 0.6 + wisdom_score.unwrap_or(rust_score) * 0.4`
///
/// The blend is asymmetric on purpose: the Rust deterministic score is the
/// majority contributor; Wisdom enriches but never overrides.
pub async fn blend_hermetic_score(rust_score: f64, text: &str) -> f64 {
    let wisdom = wisdom_hermetic_score(text).await;
    rust_score * 0.6 + wisdom.unwrap_or(rust_score) * 0.4
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn blend_falls_back_gracefully_when_service_absent() {
        // With no WISDOM_ENGINE_URL pointing at a live service, the blend
        // must collapse to the rust_score unchanged.
        std::env::remove_var("WISDOM_ENGINE_URL");
        // Force an unreachable URL so no accidental live service is hit.
        std::env::set_var("WISDOM_ENGINE_URL", "http://127.0.0.1:19999");
        let blended = blend_hermetic_score(0.75, "test text").await;
        // 0.75 * 0.6 + 0.75 * 0.4 = 0.75
        assert!((blended - 0.75).abs() < 1e-9);
    }
}
