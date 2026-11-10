// omokoda-core/src/waggle — the powers' connection to the Waggle field.
//
// Connection Map v2 §6:
// - Èṣù: capability-token gate on the field verbs (claim/mark/release/dance)
//   plus per-agent mark throttling folded into the same chokepoint — a
//   looping agent cannot flood the field faster than decay cleans it.
// - Ọbàtálá: a gatekeeper HALT becomes a `taboo` deposit whose meta carries
//   the failing Hermetic gate and reason — the justification trace any agent
//   can retrieve via sniff_explain. A taboo is a judgment with a why, not a
//   silent wall.
// - Ògún: tool-outcome auto-signaling — every tool execution's
//   success/failure can flow through a registered watch, making the whole
//   tool surface stigmergic without per-tool instrumentation.
//
// Everything fails soft: an absent substrate never blocks the steward loop.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde_json::{json, Value};

use crate::steward::gatekeeper::GatekeeperResult;

pub mod taboo_cap;
pub use taboo_cap::{is_obatala_lineage, TabooCapIssuer};

/// waggled (agentic-waggle) listens on 127.0.0.1:7779; 7777 is ares-buzz-relay
/// and 7778 is this kernel.
fn waggle_url() -> String {
    std::env::var("WAGGLE_URL").unwrap_or_else(|_| "http://127.0.0.1:7779".to_string())
}

// ── Èṣù: capability tokens for field verbs ──────────────────────────────────

/// The verbs the capability gate covers. Read verbs (sniff/gradient) stay
/// open — scent is public by design; only writes and leases need capability.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FieldVerb {
    Claim,
    Mark,
    Release,
    Dance,
}

/// One agent's capability record: the bearer token plus its issuance lineage —
/// which minting path (origin) issued it and when. Lineage is what turns a
/// Sybil ring visible: N identities minted from one compromised path share an
/// origin even though each token is individually valid.
struct Capability {
    token: String,
    origin: String,
    issued_at: Instant,
}

/// Session-scoped capability tokens, issued by Èṣù when an agent is born and
/// checked on every field write. Folded into the same dispatch that already
/// gates think/act — the crossroads keeper decides who may leave scent.
///
/// Beyond per-call validity, the gate tracks issuance *lineage* so a
/// same-origin cluster (a Sybil ring minted from one path in a burst) can be
/// flagged even when every individual token is technically valid — the
/// red-team's Sybil scenario is exactly this attack (round 2, #1).
#[derive(Default)]
pub struct CapabilityGate {
    caps: Mutex<HashMap<String, Capability>>, // agent id -> capability
}

impl CapabilityGate {
    pub fn new() -> Self {
        Self::default()
    }

    /// Issue an agent's field capability from an unnamed origin (the agent
    /// itself). Prefer `issue_from` when a minting path is known.
    pub fn issue(&self, agent: &str) -> String {
        self.issue_from(agent, agent)
    }

    /// Issue an agent's field capability, recording the minting `origin` so
    /// lineage analysis can later cluster same-origin identities.
    pub fn issue_from(&self, agent: &str, origin: &str) -> String {
        let token = format!("{:016x}{:016x}", fastrand_u64(), fastrand_u64());
        self.caps.lock().unwrap().insert(
            agent.to_string(),
            Capability {
                token: token.clone(),
                origin: origin.to_string(),
                issued_at: Instant::now(),
            },
        );
        token
    }

    /// Revoke an agent's capability (death, quarantine, misbehavior).
    pub fn revoke(&self, agent: &str) {
        self.caps.lock().unwrap().remove(agent);
    }

    /// Authorize one field verb. All four gated verbs share the one check;
    /// unknown agents and stale tokens are refused.
    pub fn authorize(&self, agent: &str, _verb: FieldVerb, token: &str) -> bool {
        self.caps
            .lock()
            .unwrap()
            .get(agent)
            .is_some_and(|c| constant_time_eq(c.token.as_bytes(), token.as_bytes()))
    }

    /// Flag same-origin clusters: any minting origin that issued at least
    /// `min_ring` capabilities within `window` is a suspected Sybil ring.
    /// Returns (origin, [agent ids]) for each ring, so the caller can discount
    /// their corroboration or quarantine the origin. This is the piece the
    /// bare field can't do — the field sees N independent agents; Èṣù sees
    /// they were all minted from one path in one burst.
    pub fn suspected_rings(&self, window: Duration, min_ring: usize) -> Vec<(String, Vec<String>)> {
        let caps = self.caps.lock().unwrap();
        let now = Instant::now();
        let mut by_origin: HashMap<&str, Vec<(&str, Instant)>> = HashMap::new();
        for (agent, cap) in caps.iter() {
            by_origin
                .entry(cap.origin.as_str())
                .or_default()
                .push((agent.as_str(), cap.issued_at));
        }
        let mut rings = Vec::new();
        for (origin, mut members) in by_origin {
            // an origin issuing to itself only (origin == agent) is not a ring
            if members.len() < min_ring || (members.len() == 1 && members[0].0 == origin) {
                continue;
            }
            let burst: Vec<String> = members
                .iter()
                .filter(|(_, t)| now.duration_since(*t) <= window)
                .map(|(a, _)| a.to_string())
                .collect();
            if burst.len() >= min_ring {
                members.sort_by_key(|(_, t)| *t);
                rings.push((origin.to_string(), burst));
            }
        }
        rings
    }
}

/// Std-only randomness for tokens: hash of time + a counter. Session-scoped
/// bearer tokens, not long-lived credentials — rotation is one issue() away.
fn fastrand_u64() -> u64 {
    use std::hash::{BuildHasher, Hasher, RandomState};
    let mut h = RandomState::new().build_hasher();
    h.write_u128(Instant::now().elapsed().as_nanos());
    h.finish()
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

// ── Èṣù: per-agent mark throttling ──────────────────────────────────────────

/// Token-bucket throttle on mark frequency, per agent. Èṣù already owns
/// cooldown enforcement; this extends it to the field so one misbehaving
/// agent cannot out-write evaporation. Defaults: 30 marks burst, refilling
/// at 1 mark/2s (steady-state 30/min).
pub struct MarkThrottle {
    capacity: f64,
    refill_per_s: f64,
    buckets: Mutex<HashMap<String, (f64, Instant)>>,
}

impl MarkThrottle {
    pub fn new(capacity: f64, refill_per_s: f64) -> Self {
        Self {
            capacity,
            refill_per_s,
            buckets: Mutex::new(HashMap::new()),
        }
    }

    /// True if the agent may mark now; spends one token when allowed.
    pub fn allow(&self, agent: &str) -> bool {
        let mut buckets = self.buckets.lock().unwrap();
        let now = Instant::now();
        let (tokens, last) = buckets
            .entry(agent.to_string())
            .or_insert((self.capacity, now));
        let refilled = (*tokens + now.duration_since(*last).as_secs_f64() * self.refill_per_s)
            .min(self.capacity);
        *last = now;
        if refilled >= 1.0 {
            *tokens = refilled - 1.0;
            true
        } else {
            *tokens = refilled;
            false
        }
    }
}

impl Default for MarkThrottle {
    fn default() -> Self {
        Self::new(30.0, 0.5)
    }
}

// ── Field client ─────────────────────────────────────────────────────────────

/// Thin async client for the substrate. Best-effort by contract: every call
/// returns Option and swallows transport errors — no scent, no harm.
/// The authority to leave taboos: Èṣù's signing key plus this agent's proven
/// Ọbàtálá lineage. When present, taboo deposits self-attach a capability the
/// daemon verifies; when absent, taboos go out unauthenticated (accepted only
/// by a daemon not running the gate, or in its observe mode).
struct TabooAuthority {
    issuer: TabooCapIssuer,
    lineage: String,
    ttl_secs: i64,
}

pub struct WaggleField {
    http: reqwest::Client,
    base: String,
    agent: String,
    watch_id: Mutex<Option<String>>,
    taboo_authority: Option<TabooAuthority>,
}

impl WaggleField {
    pub fn new(agent: impl Into<String>) -> Self {
        Self {
            http: reqwest::Client::builder()
                .timeout(Duration::from_secs(5))
                .build()
                .unwrap_or_default(),
            base: waggle_url(),
            agent: agent.into(),
            watch_id: Mutex::new(None),
            taboo_authority: None,
        }
    }

    /// Grant this field the authority to leave authenticated taboos, from
    /// Èṣù's signing seed and this agent's verified lineage. A no-op (leaves
    /// the field unable to mint taboos) unless the lineage clears the Ọbàtálá
    /// bar — the same check the issuer enforces, surfaced early so a
    /// misconfigured identity fails loudly at wiring time, not silently at
    /// deposit time.
    pub fn with_taboo_authority(mut self, seed: &[u8; 32], lineage: impl Into<String>) -> Self {
        let lineage = lineage.into();
        if is_obatala_lineage(&lineage) {
            self.taboo_authority = Some(TabooAuthority {
                issuer: TabooCapIssuer::from_seed(seed),
                lineage,
                ttl_secs: 300,
            });
        }
        self
    }

    async fn post(&self, path: &str, body: Value) -> Option<Value> {
        self.http
            .post(format!("{}{}", self.base, path))
            .json(&body)
            .send()
            .await
            .ok()?
            .json()
            .await
            .ok()
    }

    pub async fn deposit(
        &self,
        resource: &str,
        kind: &str,
        intensity: f64,
        note: &str,
        meta: Value,
    ) -> Option<Value> {
        let mut body = json!({
            "agent": self.agent, "resource": resource, "kind": kind,
            "intensity": intensity, "note": note, "meta": meta,
        });
        // taboo censors an action, so it self-authenticates: mint a fresh
        // Èṣù capability for this deposit when we hold the authority. Other
        // channels never carry one.
        if kind == "taboo" {
            if let Some(auth) = &self.taboo_authority {
                if let Some(cap) = auth.issuer.issue(&self.agent, &auth.lineage, auth.ttl_secs) {
                    body["capability"] = json!(cap);
                }
            }
        }
        self.post("/v1/signals", body).await
    }

    pub async fn sniff(&self, resource: &str, kind: &str) -> Option<Value> {
        self.http
            .get(format!(
                "{}/v1/sniff?resource={}&kind={}",
                self.base, resource, kind
            ))
            .send()
            .await
            .ok()?
            .json()
            .await
            .ok()
    }

    async fn ensure_watch(&self) -> Option<String> {
        if let Some(id) = self.watch_id.lock().unwrap().clone() {
            return Some(id);
        }
        let out = self
            .post(
                "/v1/watches",
                json!({
                    "agent": self.agent,
                    "name": "ogun tool executions",
                    "resource_prefix": "tool://",
                }),
            )
            .await?;
        let id = out.get("watch")?.get("id")?.as_str()?.to_string();
        *self.watch_id.lock().unwrap() = Some(id.clone());
        Some(id)
    }

    // ── Ọbàtálá: HALT → taboo with justification (§6.7, §6.8) ────────────

    /// When the 7-gate evaluation halts an operation, the exclusion becomes
    /// ambient knowledge: a slow-decay taboo on the resource, its meta
    /// carrying which Hermetic gate fired and why. sniff_explain then
    /// answers "why is this territory cold" with the actual reasoning trace.
    pub async fn taboo_from_halt(
        &self,
        resource: &str,
        result: &GatekeeperResult,
    ) -> Option<Value> {
        let GatekeeperResult::Halted {
            failed_gate,
            reason,
            ..
        } = result
        else {
            return None; // approvals leave no taboo
        };
        self.deposit(
            resource,
            "taboo",
            7.0,
            &format!("halted by {:?}", failed_gate),
            json!({
                "principle": format!("{:?}", failed_gate),
                "justification": reason,
                "source": "esu-gatekeeper",
            }),
        )
        .await
    }

    // ── Ògún: tool-outcome auto-signaling (§6.10) ─────────────────────────

    /// Report one tool execution through the watch: success → gold,
    /// failure → dead-end, both at watch-derived trust. Hook this once in
    /// the execution layer and the whole tool surface becomes stigmergic.
    pub async fn tool_outcome(
        &self,
        tool: &str,
        target: &str,
        success: bool,
        note: &str,
    ) -> Option<Value> {
        let id = self.ensure_watch().await?;
        self.post(
            &format!("/v1/ingest/{id}"),
            json!({
                "resource": format!("{tool}/{target}"),
                "outcome": if success { "success" } else { "failure" },
                "note": note,
            }),
        )
        .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capability_tokens_gate_the_verbs() {
        let gate = CapabilityGate::new();
        let token = gate.issue("esu-child-1");
        assert!(gate.authorize("esu-child-1", FieldVerb::Mark, &token));
        assert!(!gate.authorize("esu-child-1", FieldVerb::Mark, "forged"));
        assert!(!gate.authorize("unknown", FieldVerb::Claim, &token));
        gate.revoke("esu-child-1");
        assert!(!gate.authorize("esu-child-1", FieldVerb::Dance, &token));
        // re-issue rotates: the old token dies
        let t2 = gate.issue("esu-child-1");
        assert!(gate.authorize("esu-child-1", FieldVerb::Release, &t2));
        assert!(!gate.authorize("esu-child-1", FieldVerb::Release, &token));
    }

    #[test]
    fn suspected_rings_flags_same_origin_sybil() {
        let gate = CapabilityGate::new();
        // a compromised minting path issues 5 identities in a burst
        for i in 0..5 {
            gate.issue_from(&format!("sybil-{i}"), "compromised-path");
        }
        // plus a handful of honest agents, each its own origin
        gate.issue("honest-a");
        gate.issue("honest-b");

        let rings = gate.suspected_rings(Duration::from_secs(60), 3);
        assert_eq!(rings.len(), 1, "exactly one ring expected: {rings:?}");
        let (origin, members) = &rings[0];
        assert_eq!(origin, "compromised-path");
        assert_eq!(members.len(), 5);
        // honest self-origin agents are never a ring
        assert!(!rings
            .iter()
            .any(|(o, _)| o == "honest-a" || o == "honest-b"));
    }

    #[test]
    fn suspected_rings_ignores_small_or_slow_groups() {
        let gate = CapabilityGate::new();
        // only 2 from one origin: below the ring threshold
        gate.issue_from("a", "shared");
        gate.issue_from("b", "shared");
        assert!(gate.suspected_rings(Duration::from_secs(60), 3).is_empty());
    }

    #[test]
    fn mark_throttle_caps_burst_and_refills() {
        let throttle = MarkThrottle::new(3.0, 1000.0); // fast refill for the test
        assert!(throttle.allow("noisy"));
        assert!(throttle.allow("noisy"));
        assert!(throttle.allow("noisy"));
        // burst spent; immediate 4th mark may or may not refill in time on a
        // fast machine, so drain with zero refill instead
        let strict = MarkThrottle::new(2.0, 0.0);
        assert!(strict.allow("a"));
        assert!(strict.allow("a"));
        assert!(!strict.allow("a"), "burst must be capped");
        // other agents are unaffected
        assert!(strict.allow("b"));
    }
}
