//! ZÀNGBÉTÒ enforcement client.
//!
//! When `ZANGBETO_URL` is set, the runtime reports anomalies (e.g. a denied
//! capability) to the ZÀNGBÉTÒ enforcement bridge and receives the enforcement
//! action keyed on the same `agent_id` the agent registers on Vantage.
//!
//! Fail-open: a no-op returning `None` when `ZANGBETO_URL` is unset, so runtimes
//! without an enforcer are unaffected. Best-effort — transport errors are
//! swallowed rather than failing the act.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::OnceLock;

use reqwest::Client;
use serde_json::json;

static HTTP: OnceLock<Client> = OnceLock::new();
fn http() -> &'static Client {
    HTTP.get_or_init(Client::new)
}

fn normalize_base(raw: &str) -> Option<String> {
    let url = raw.trim().trim_end_matches('/');
    if url.is_empty() {
        None
    } else {
        Some(url.to_string())
    }
}

fn base_url() -> Option<String> {
    normalize_base(&std::env::var("ZANGBETO_URL").ok()?)
}

/// Last-seen cursor for once-only canary-trip injection. Process-level: it
/// baselines to "now" on first poll, then advances to the newest trip
/// timestamp seen, so each trip is injected into exactly one think() call and
/// not re-spammed every turn forever.
static LAST_INCIDENT_TS: AtomicU64 = AtomicU64::new(0);

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Report an anomaly for `agent_id` to the enforcement bridge and return the
/// enforcement action ZÀNGBÉTÒ decided (the parsed JSON response), or `None`
/// when no enforcer is configured or the call fails.
///
/// `severity` ∈ observational | warning | critical | catastrophic.
/// `classification` ∈ schema_drift | economic_anomaly | temporal_inconsistency |
///   capability_escape | concurrency_conflict.
pub async fn report_anomaly(
    agent_id: &str,
    severity: &str,
    classification: &str,
    detail: &str,
) -> Option<serde_json::Value> {
    let base = base_url()?;
    let resp = http()
        .post(format!("{base}/enforce"))
        .json(&json!({
            "agent_id": agent_id,
            "severity": severity,
            "classification": classification,
            "detail": detail,
        }))
        .send()
        .await
        .ok()?;
    if !resp.status().is_success() {
        return None;
    }
    resp.json::<serde_json::Value>().await.ok()
}

/// Ask ZÀNGBÉTÒ to review a *proposed* act before it runs and return its verdict
/// (parsed JSON), or `None` when no enforcer is configured or the call fails.
/// This is the pre-act enforcement gate: a blocking verdict (see
/// [`verdict_blocks`]) denies an otherwise-allowed act. Fail-open.
pub async fn review_act(agent_id: &str, tool: &str, detail: &str) -> Option<serde_json::Value> {
    let base = base_url()?;
    let resp = http()
        .post(format!("{base}/review"))
        .json(&json!({
            "agent_id": agent_id,
            "tool": tool,
            "detail": detail,
        }))
        .send()
        .await
        .ok()?;
    if !resp.status().is_success() {
        return None;
    }
    resp.json::<serde_json::Value>().await.ok()
}

/// Interpret a ZÀNGBÉTÒ verdict: does it order the act blocked?
///
/// Conservative / fail-open by design — only an *explicit* blocking signal
/// returns `true`; an empty, malformed, or unrecognized verdict does not block.
/// Recognized: a string `action`/`decision`/`enforcement`/`status`/`verdict`
/// naming a blocking action, or `{"block": true}` / `{"allowed": false}`.
pub fn verdict_blocks(verdict: &serde_json::Value) -> bool {
    const BLOCKING: &[&str] = &[
        "block",
        "deny",
        "quarantine",
        "suspend",
        "halt",
        "reject",
        "jail",
    ];
    for key in ["action", "decision", "enforcement", "status", "verdict"] {
        if let Some(s) = verdict.get(key).and_then(|v| v.as_str()) {
            if BLOCKING.iter().any(|b| s.eq_ignore_ascii_case(b)) {
                return true;
            }
        }
    }
    if verdict.get("block").and_then(|v| v.as_bool()) == Some(true) {
        return true;
    }
    if verdict.get("allowed").and_then(|v| v.as_bool()) == Some(false) {
        return true;
    }
    false
}

/// Pull pending canary-trip incidents from Zàngbétò since the last poll.
/// Fail-open: returns `None` when `ZANGBETO_URL` is unset or the call fails,
/// so a down enforcer degrades to "no notice" instead of breaking think().
/// Once-only: the cursor baselines to "now" on first call, then advances to
/// the newest trip timestamp seen, so each trip is injected on exactly one
/// subsequent think() and not re-spammed every turn forever.
///
/// This is Zàngbétò's enforcement layer reaching back into Omo-Koda2's
/// reasoning loop — Canarytokens itself never touches think(); it only fires
/// a webhook at Zàngbétò, which is the sole authority that signs the incident
/// as a `canary_trip` receipt. The kernel then *pulls* those receipts here.
pub async fn pending_incidents() -> Option<Vec<serde_json::Value>> {
    let base = base_url()?;
    let mut since = LAST_INCIDENT_TS.load(Ordering::SeqCst);
    if since == 0 {
        since = now_secs();
        LAST_INCIDENT_TS.store(since, Ordering::SeqCst);
    }
    let resp = http()
        .get(format!("{base}/canary-trips?since={since}"))
        .send()
        .await
        .ok()?;
    if !resp.status().is_success() {
        return None;
    }
    let trips = resp
        .json::<serde_json::Value>()
        .await
        .ok()?
        .get("trips")?
        .as_array()?
        .clone();
    if let Some(max_ts) = trips
        .iter()
        .filter_map(|t| t.get("timestamp").and_then(|v| v.as_u64()))
        .max()
    {
        LAST_INCIDENT_TS.store(max_ts, Ordering::SeqCst);
    }
    // A canary trip is a security event; never silent. Log how many were pulled
    // (think() injects them as system notices into the reasoning context).
    if !trips.is_empty() {
        eprintln!(
            "[zangbeto] canary incident: pulled {} trip(s) since {since}",
            trips.len()
        );
    }
    Some(trips)
}

/// Render a `canary_trip` receipt as a system-level security notice for the
/// think() prompt. Only safe fields are rendered — `subject` (token_type),
/// `src_ip`, `memo` — and never the canary token secret (the receipt's
/// `actor` is already a sha256; the raw token never reaches the kernel at
/// all). Returns `None` when the trip lacks a `subject`.
pub fn render_incident_notice(trip: &serde_json::Value) -> Option<String> {
    let token_type = trip.get("subject")?.as_str()?;
    let detail = trip.get("detail")?;
    let src_ip = detail
        .get("src_ip")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .unwrap_or("unknown source");
    let memo = detail.get("memo").and_then(|v| v.as_str()).unwrap_or("");
    let mut notice = format!(
        "## Security Notice\nA canary mousetrap tripped ({token_type}) from {src_ip}. \
         This host may be compromised — treat this session and any recent actions as \
         potentially untrusted."
    );
    if !memo.is_empty() {
        notice.push_str(&format!(" Context: {memo}"));
    }
    Some(notice)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn verdict_blocks_on_explicit_signals() {
        assert!(verdict_blocks(&json!({"action": "quarantine"})));
        assert!(verdict_blocks(&json!({"decision": "DENY"})));
        assert!(verdict_blocks(&json!({"enforcement": "block"})));
        assert!(verdict_blocks(&json!({"status": "suspend"})));
        assert!(verdict_blocks(&json!({"block": true})));
        assert!(verdict_blocks(&json!({"allowed": false})));
    }

    #[test]
    fn verdict_allows_by_default() {
        // Fail-open: anything not an explicit block must not gate the act.
        assert!(!verdict_blocks(&json!({})));
        assert!(!verdict_blocks(&json!({"action": "observe"})));
        assert!(!verdict_blocks(&json!({"decision": "allow"})));
        assert!(!verdict_blocks(&json!({"block": false})));
        assert!(!verdict_blocks(&json!({"allowed": true})));
        assert!(!verdict_blocks(&json!("garbage")));
        assert!(!verdict_blocks(&json!(42)));
    }

    #[test]
    fn normalize_base_trims_and_rejects_empty() {
        assert_eq!(
            normalize_base("http://enforcer:8787/"),
            Some("http://enforcer:8787".to_string())
        );
        assert_eq!(
            normalize_base("  http://x:1/  "),
            Some("http://x:1".to_string())
        );
        assert_eq!(normalize_base(""), None);
        assert_eq!(normalize_base("   "), None);
        assert_eq!(normalize_base("/"), None);
    }

    #[test]
    fn render_incident_notice_uses_safe_fields_only() {
        let trip = json!({
            "subject": "aws_keys",
            "actor": "abc123", // already a hash on the wire; irrelevant here
            "detail": {
                "src_ip": "203.0.113.7",
                "memo": "decoy creds on contabo",
                "token": "SUPERSECRET",      // must NOT be rendered
                "manage_url": "https://canarytokens.org/manage/x",
            }
        });
        let notice = render_incident_notice(&trip).unwrap();
        assert!(notice.contains("aws_keys"), "token_type rendered");
        assert!(notice.contains("203.0.113.7"), "src_ip rendered");
        assert!(notice.contains("decoy creds on contabo"), "memo rendered");
        assert!(
            !notice.contains("SUPERSECRET"),
            "raw token must never be rendered"
        );
        assert!(
            !notice.contains("manage_url"),
            "unrelated fields must not leak"
        );
        assert!(
            !notice.contains("canarytokens.org"),
            "manage_url content must not leak"
        );
    }

    #[test]
    fn render_incident_notice_returns_none_without_subject() {
        assert!(render_incident_notice(&json!({ "detail": { "src_ip": "1.2.3.4" } })).is_none());
        assert!(render_incident_notice(&json!({})).is_none());
        assert!(render_incident_notice(&json!("garbage")).is_none());
    }

    #[test]
    fn render_incident_notice_falls_back_when_src_ip_missing() {
        let trip = json!({ "subject": "ms_word", "detail": {} });
        let notice = render_incident_notice(&trip).unwrap();
        assert!(notice.contains("ms_word"));
        assert!(
            notice.contains("unknown source"),
            "empty src_ip → fallback label"
        );
    }

    #[test]
    fn now_secs_is_sane() {
        // Anything after 2020-01-01 (1577836800). Guards against a broken
        // epoch conversion silently zeroing the once-only cursor.
        assert!(now_secs() > 1_577_836_800);
    }
}
