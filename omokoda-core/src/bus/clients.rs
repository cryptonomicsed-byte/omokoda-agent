use crate::emotion::EmotionState;
use crate::identity::AgentId;
use crate::steward::soul::SomaContext;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// Result of a Hermetic principle evaluation (from Ọbàtálá / Lisp service).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HermeticResult {
    /// Overall alignment score 0.0–1.0
    pub overall: f32,
    /// Per-principle scores: [mentalism, correspondence, vibration, polarity, rhythm, cause_effect, gender]
    pub scores: [f32; 7],
    /// "Allow", "Warn: ...", or "Block: ..."
    pub decision: String,
}

impl HermeticResult {
    #[must_use]
    pub fn is_allowed(&self) -> bool {
        self.decision.starts_with("Allow")
    }

    #[must_use]
    pub fn is_blocked(&self) -> bool {
        self.decision.starts_with("Block")
    }

    /// Always-allow stub — used when no Hermetic service is configured.
    #[must_use]
    pub fn allow_stub() -> Self {
        Self {
            overall: 0.85,
            scores: [0.85; 7],
            decision: "Allow".to_string(),
        }
    }
}

// ─── Mesh-layer shared types ──────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrustSignal {
    pub kind: String,
    pub weight: f64,
    pub timestamp: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerInfo {
    pub agent_id: String,
    pub block_id: String,
    pub trust_score: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceOffer {
    pub resource_id: String,
    pub kind: String,
    pub capacity: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeshStatus {
    pub healthy: bool,
    pub peer_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HandshakeOffer {
    pub terms: serde_json::Value,
    pub ttl_secs: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HandshakeState {
    pub session_id: String,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CounterOffer {
    pub terms: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NegotiationResult {
    pub status: String,
    pub session_id: String,
    pub final_terms: Option<serde_json::Value>,
}

// ─────────────────────────────────────────────────────────────────────────────

/// Ọ̀ṣun (Julia) client — SOMA reconstruction and memory operations.
/// Called during `think` to retrieve contextual memory before the LLM call.
#[async_trait]
pub trait OsunClient: Send + Sync {
    /// Retrieve SOMA context for the current prompt + emotion state.
    async fn reconstruct_soma(
        &self,
        agent_id: &AgentId,
        prompt: &str,
        emotion: &EmotionState,
    ) -> SomaContext;

    /// Store a MemCell after a completed `think` turn.
    async fn store_memcell(
        &self,
        agent_id: &AgentId,
        text: &str,
        emotion: &EmotionState,
        importance: f32,
    );

    /// Compute a weighted trust score for a neighbor from a batch of signals.
    async fn compute_trust_score(
        &self,
        _agent_id: &str,
        _neighbor_id: &str,
        _signals: Vec<TrustSignal>,
    ) -> f64 {
        0.5
    }

    /// Cross-agent Soma search: patterns matching `prompt` across the
    /// explicit `share_with` list only -- never every agent Julia has ever
    /// seen. `share_with` is the caller's own already-decided trust
    /// boundary (e.g. `OduDirectory::swarm_agents()`), never computed or
    /// assumed here. Default no-op so callers that never opt into swarm
    /// sharing pay nothing; `HttpOsunClient` below is the real impl.
    async fn query_swarm(&self, _share_with: &[String], _prompt: &str) -> Vec<String> {
        Vec::new()
    }
}

/// Ọbàtálá (Lisp) client — Hermetic principle evaluation.
/// Called during `think` to gate the LLM call, and during `act` to gate tool execution.
#[async_trait]
pub trait ObatalaClient: Send + Sync {
    /// Evaluate `intent` + `action_description` against the 7 Hermetic principles.
    ///
    /// Historical note (2026-08-28): the real deployed Ọbàtálá service
    /// (`omokoda-clojure/obatala.clj`) never actually exposed a generic
    /// intent/action Hermetic-evaluation endpoint — its real job is a
    /// consent/data-sharing gate (`consent_mode`/`data_category`/`requester`
    /// against the 7 principles), reachable at `/evaluate` and
    /// `/consent/check`. This method's `HttpObatalaClient` implementation
    /// was posting to a `/hermetic/evaluate` path that never existed on the
    /// server; it now maps onto the real `/consent/check` endpoint with a
    /// best-effort default framing (`consent_mode: "private"`,
    /// `data_category: "general_action"`, `requester: "self"`) so a call
    /// here is a real network round-trip against a real endpoint instead of
    /// an unconditional 404 that silently fell through to `allow_stub()`.
    /// For the real, correctly-modeled capability, prefer `check_consent`
    /// below — this method is kept for trait/stub backward compatibility.
    async fn evaluate_hermetic(
        &self,
        intent: &str,
        action_description: &str,
        emotion: &EmotionState,
    ) -> HermeticResult;

    /// The real Ọbàtálá capability: consent/data-sharing gate, matching
    /// `obatala.clj`'s actual `/evaluate` rule engine exactly.
    /// `hermetic_state` is the caller's own already-computed 7 principle
    /// scores (`omokoda_hermetic::HermeticState`, deterministic per-agent
    /// from the Odù seed) — Ọbàtálá never recomputes this, only weighs it
    /// (see rules 3 and 4 in `obatala.clj`: polarity gates non-sensitive
    /// sharing, cause_effect gates financial disclosure).
    /// Fails open (allowed=true, no violations) when the service is
    /// unreachable — an absent gate must never itself become a denial.
    async fn check_consent(
        &self,
        _consent_mode: &str,
        _data_category: &str,
        _requester: &str,
        _hermetic: &[f64; 7],
    ) -> ConsentDecision {
        ConsentDecision::allow_stub()
    }
}

/// Real response shape of `obatala.clj`'s `/evaluate` (and the `sharable`/
/// `reasons` wrapper `/consent/check` derives from the same `evaluate` core).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsentDecision {
    pub allowed: bool,
    pub violations: Vec<String>,
    pub sabbath: bool,
}

impl ConsentDecision {
    /// Fail-open default — used when no Ọbàtálá service is configured or
    /// reachable. Matches `HermeticResult::allow_stub`'s spirit: an absent
    /// gate never blocks.
    #[must_use]
    pub fn allow_stub() -> Self {
        Self {
            allowed: true,
            violations: Vec::new(),
            sabbath: false,
        }
    }
}

/// Ọya (Go) client — rhythm enforcement and inter-service transport.
#[async_trait]
pub trait OyaClient: Send + Sync {
    /// Check if the agent is in a rhythm cooldown period.
    async fn is_in_cooldown(&self, agent_id: &AgentId) -> bool;

    /// Record a completed primitive for rhythm tracking.
    async fn record_primitive(&self, agent_id: &AgentId, primitive: &str);

    /// Return known mesh peers as gossip entries.
    async fn gossip_peers(&self) -> Vec<PeerInfo> {
        vec![]
    }

    /// Advertise a local resource onto the mesh.
    async fn register_resource(&self, _offer: ResourceOffer) {}

    /// Return current mesh health from the Oya transport layer.
    async fn mesh_health(&self) -> MeshStatus {
        MeshStatus {
            healthy: true,
            peer_count: 0,
        }
    }
}

/// Ṣàngó (Move) client — on-chain receipt and reputation.
/// Called after `act` to write an immutable receipt.
#[async_trait]
pub trait SangoClient: Send + Sync {
    async fn write_receipt(&self, agent_id: &AgentId, action_tool: &str, hermetic: &HermeticResult);
}

/// Yemọja (Elixir) client — agent lifecycle and swarm coordination.
/// Routes to omokoda-swarm supervision trees for sub-agent spawning and delegation.
#[async_trait]
pub trait YemojaClient: Send + Sync {
    /// Spawn a sub-agent with a given role and Synapse budget.
    async fn spawn_agent(&self, role: &str, budget_synapse: f64) -> Result<String, String>;
    /// Query the live status of a spawned sub-agent by ID.
    async fn agent_status(&self, agent_id: &str) -> AgentStatus;
    /// List agents currently online on a mesh block.
    async fn mesh_presence(&self, block_id: &str) -> Vec<AgentPresence>;
    /// Broadcast a mesh event to all agents on a block.
    async fn mesh_broadcast(&self, block_id: &str, event: serde_json::Value) -> Result<(), String>;
    /// Run a consensus proposal across all mesh agents on a block.
    async fn mesh_consensus(
        &self,
        block_id: &str,
        proposal: serde_json::Value,
    ) -> Result<serde_json::Value, String>;
    /// Hand off an agent to a different Elixir node.
    async fn mesh_handoff(&self, agent_id: &str, target_node: &str) -> Result<(), String>;
    /// Propose a bilateral handshake with a neighbor agent.
    async fn propose_handshake(
        &self,
        _neighbor: &str,
        _offer: HandshakeOffer,
    ) -> Result<HandshakeState, String> {
        Ok(HandshakeState {
            session_id: format!(
                "session-{}",
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis()
            ),
            status: "proposed".to_string(),
        })
    }
    /// Submit a counter-offer in an ongoing negotiation session.
    async fn negotiate_terms(
        &self,
        session_id: &str,
        _counter: CounterOffer,
    ) -> Result<NegotiationResult, String> {
        Ok(NegotiationResult {
            status: "accepted".to_string(),
            session_id: session_id.to_string(),
            final_terms: None,
        })
    }
}

/// Ògún (Python) client — tool execution and external integrations.
/// Routes to the Python execution layer for data processing and rapid prototyping.
#[async_trait]
pub trait OgunClient: Send + Sync {
    /// Execute a named Python tool with JSON input; returns JSON output.
    async fn execute_tool(&self, tool_name: &str, input_json: &str) -> Result<String, String>;
}

/// Status of a spawned sub-agent in the Yemọja supervision tree.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AgentStatus {
    Idle,
    Running,
    Complete,
    Failed,
}

/// Live presence record for a mesh agent on a block.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentPresence {
    pub agent_id: String,
    pub role: String,
    pub status: String,
    pub block_id: String,
}

// ─── Stub implementations (no-ops until real services are deployed) ──────────

/// Local stub — SOMA reconstruction returns empty context.
/// Replace with a real `OsunClient` implementation once the Julia service is deployed.
pub struct LocalOsunStub;

#[async_trait]
impl OsunClient for LocalOsunStub {
    async fn reconstruct_soma(
        &self,
        _agent_id: &AgentId,
        _prompt: &str,
        _emotion: &EmotionState,
    ) -> SomaContext {
        SomaContext::new()
    }

    async fn store_memcell(
        &self,
        _agent_id: &AgentId,
        _text: &str,
        _emotion: &EmotionState,
        _importance: f32,
    ) {
    }
}

/// Local stub — always returns Allow with neutral scores.
pub struct LocalObatalaStub;

#[async_trait]
impl ObatalaClient for LocalObatalaStub {
    async fn evaluate_hermetic(
        &self,
        _intent: &str,
        _action_description: &str,
        _emotion: &EmotionState,
    ) -> HermeticResult {
        HermeticResult::allow_stub()
    }
}

/// Local stub — never in cooldown, recording is a no-op.
pub struct LocalOyaStub;

#[async_trait]
impl OyaClient for LocalOyaStub {
    async fn is_in_cooldown(&self, _agent_id: &AgentId) -> bool {
        false
    }

    async fn record_primitive(&self, _agent_id: &AgentId, _primitive: &str) {}
}

/// Local stub — receipt writing is a no-op until Sui integration is ready.
pub struct LocalSangoStub;

/// Local stub — sub-agent spawning returns a placeholder ID.
/// Replace with a real `YemojaClient` once omokoda-swarm HTTP API is deployed.
pub struct LocalYemojaStub;

/// Local stub — Python tool execution returns a stub JSON payload.
/// Replace with a real `OgunClient` once the Python execution service is deployed.
pub struct LocalOgunStub;

#[async_trait]
impl SangoClient for LocalSangoStub {
    async fn write_receipt(
        &self,
        _agent_id: &AgentId,
        _action_tool: &str,
        _hermetic: &HermeticResult,
    ) {
    }
}

#[async_trait]
impl YemojaClient for LocalYemojaStub {
    async fn spawn_agent(&self, _role: &str, _budget_synapse: f64) -> Result<String, String> {
        Ok("stub-agent-id".to_string())
    }

    async fn agent_status(&self, _agent_id: &str) -> AgentStatus {
        AgentStatus::Idle
    }

    async fn mesh_presence(&self, _block_id: &str) -> Vec<AgentPresence> {
        vec![]
    }

    async fn mesh_broadcast(
        &self,
        _block_id: &str,
        _event: serde_json::Value,
    ) -> Result<(), String> {
        Ok(())
    }

    async fn mesh_consensus(
        &self,
        _block_id: &str,
        _proposal: serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        Ok(serde_json::json!({"consensus": "stub", "result": null}))
    }

    async fn mesh_handoff(&self, _agent_id: &str, _target_node: &str) -> Result<(), String> {
        Ok(())
    }
}

#[async_trait]
impl OgunClient for LocalOgunStub {
    async fn execute_tool(&self, tool_name: &str, _input_json: &str) -> Result<String, String> {
        Ok(
            serde_json::to_string(&serde_json::json!({"stub": true, "tool": tool_name}))
                .unwrap_or_default(),
        )
    }
}

// ─── Shared reqwest client (connection-pooled, one instance for the process) ─

fn http_client() -> &'static reqwest::Client {
    static CLIENT: std::sync::OnceLock<reqwest::Client> = std::sync::OnceLock::new();
    CLIENT.get_or_init(reqwest::Client::new)
}

// ─── HttpOsunClient ── Ọ̀ṣun Julia memory service at OSUN_URL ────────────────

/// HTTP implementation of OsunClient.
/// Base URL read from `OSUN_URL` env var (e.g. `http://localhost:7778`).
/// Falls back to empty SomaContext / no-op on network errors so callers are unaffected.
pub struct HttpOsunClient {
    base_url: String,
}

impl HttpOsunClient {
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into(),
        }
    }
}

#[async_trait]
impl OsunClient for HttpOsunClient {
    async fn reconstruct_soma(
        &self,
        agent_id: &AgentId,
        prompt: &str,
        _emotion: &EmotionState,
    ) -> SomaContext {
        let url = format!("{}/soma/reconstruct", self.base_url);
        let body = serde_json::json!({
            "agent_id": agent_id.as_str(),
            "prompt": prompt,
        });
        match http_client()
            .post(&url)
            .json(&body)
            .timeout(std::time::Duration::from_secs(3))
            .send()
            .await
        {
            Ok(resp) if resp.status().is_success() => {
                resp.json::<SomaContext>().await.unwrap_or_default()
            }
            _ => SomaContext::new(),
        }
    }

    async fn store_memcell(
        &self,
        agent_id: &AgentId,
        text: &str,
        _emotion: &EmotionState,
        importance: f32,
    ) {
        let url = format!("{}/soma/store", self.base_url);
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let body = serde_json::json!({
            "agent_id": agent_id.as_str(),
            "text": text,
            "importance": importance,
            "timestamp": ts,
        });
        let _ = http_client()
            .post(&url)
            .json(&body)
            .timeout(std::time::Duration::from_secs(3))
            .send()
            .await;
    }

    async fn compute_trust_score(
        &self,
        agent_id: &str,
        neighbor_id: &str,
        signals: Vec<TrustSignal>,
    ) -> f64 {
        let url = format!("{}/mesh/score", self.base_url);
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let body = serde_json::json!({
            "agent_id": agent_id,
            "neighbor_id": neighbor_id,
            "signals": signals,
            "prior": 0.5_f64,
            "timestamp": ts,
        });
        match http_client()
            .post(&url)
            .json(&body)
            .timeout(std::time::Duration::from_secs(5))
            .send()
            .await
        {
            Ok(resp) if resp.status().is_success() => resp
                .json::<serde_json::Value>()
                .await
                .ok()
                .and_then(|v| v.get("trust_score").and_then(|s| s.as_f64()))
                .unwrap_or(0.5),
            _ => 0.5,
        }
    }

    async fn query_swarm(&self, share_with: &[String], prompt: &str) -> Vec<String> {
        if share_with.is_empty() {
            return Vec::new();
        }
        let url = format!("{}/soma/query_swarm", self.base_url);
        let body = serde_json::json!({
            "agent_ids": share_with,
            "prompt": prompt,
        });
        match http_client()
            .post(&url)
            .json(&body)
            .timeout(std::time::Duration::from_secs(3))
            .send()
            .await
        {
            Ok(resp) if resp.status().is_success() => resp
                .json::<serde_json::Value>()
                .await
                .ok()
                .and_then(|v| v.get("patterns").cloned())
                .and_then(|p| serde_json::from_value::<Vec<String>>(p).ok())
                .unwrap_or_default(),
            _ => Vec::new(),
        }
    }
}

// ─── HttpObatalaClient ── Ọbàtálá Clojure/Babashka consent service at OBATALA_URL ──

// Real request/response shapes of obatala.clj's actual /evaluate endpoint
// (verified live against the deployed service 2026-08-28 -- see
// docs/audit/inspiration-followthrough-connectionmap-256.md).
#[derive(Serialize)]
struct ObatalaHermeticStateReq {
    mentalism: f64,
    correspondence: f64,
    vibration: f64,
    polarity: f64,
    rhythm: f64,
    cause_effect: f64,
    gender: f64,
}

#[derive(Serialize)]
struct ObatalaConsentReq<'a> {
    consent_mode: &'a str,
    data_category: &'a str,
    requester: &'a str,
    hermetic_state: ObatalaHermeticStateReq,
}

#[derive(Deserialize)]
struct ObatalaConsentResp {
    allowed: bool,
    #[serde(default)]
    violations: Vec<String>,
    #[serde(default)]
    sabbath: bool,
}

/// HTTP implementation of ObatalaClient.
/// Base URL read from `OBATALA_URL` env var (e.g. `http://localhost:4002`).
/// Fails open (allow) on network errors so an absent/unreachable Ọbàtálá
/// never itself becomes a denial.
pub struct HttpObatalaClient {
    base_url: String,
}

impl HttpObatalaClient {
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into(),
        }
    }
}

#[async_trait]
impl ObatalaClient for HttpObatalaClient {
    async fn evaluate_hermetic(
        &self,
        _intent: &str,
        action_description: &str,
        emotion: &EmotionState,
    ) -> HermeticResult {
        // obatala.clj has no generic intent/action evaluation endpoint --
        // its only real capability is the consent gate below. Map onto a
        // conservative default framing so a call here exercises the real
        // rule engine instead of failing, while callers who need the real
        // capability should call check_consent directly.
        let _ = emotion;
        let decision = self
            .check_consent(
                "private",
                "general_action",
                "self",
                &[0.5, 0.5, 0.5, 0.5, 0.5, 0.5, 0.5],
            )
            .await;
        let overall = if decision.allowed { 0.85 } else { 0.3 };
        HermeticResult {
            overall,
            scores: [overall; 7],
            decision: if decision.allowed {
                "Allow".to_string()
            } else {
                format!(
                    "Block: {} (re: {})",
                    decision.violations.join("; "),
                    action_description
                )
            },
        }
    }

    async fn check_consent(
        &self,
        consent_mode: &str,
        data_category: &str,
        requester: &str,
        hermetic: &[f64; 7],
    ) -> ConsentDecision {
        let req = ObatalaConsentReq {
            consent_mode,
            data_category,
            requester,
            hermetic_state: ObatalaHermeticStateReq {
                mentalism: hermetic[0],
                correspondence: hermetic[1],
                vibration: hermetic[2],
                polarity: hermetic[3],
                rhythm: hermetic[4],
                cause_effect: hermetic[5],
                gender: hermetic[6],
            },
        };
        let url = format!("{}/evaluate", self.base_url);
        match http_client()
            .post(&url)
            .json(&req)
            .timeout(std::time::Duration::from_secs(5))
            .send()
            .await
        {
            Ok(resp) if resp.status().is_success() => resp
                .json::<ObatalaConsentResp>()
                .await
                .map(|r| ConsentDecision {
                    allowed: r.allowed,
                    violations: r.violations,
                    sabbath: r.sabbath,
                })
                .unwrap_or_else(|_| ConsentDecision::allow_stub()),
            _ => ConsentDecision::allow_stub(), // fail open (service unavailable != block)
        }
    }
}

// ─── HttpOyaClient ── Ọya Go rhythm service at OYA_URL ───────────────────────

/// HTTP implementation of OyaClient.
/// Base URL read from `OYA_URL` env var (e.g. `http://localhost:8080`).
/// Fails open on network errors: `is_in_cooldown` returns false so execution is
/// not blocked when the rhythm service is unavailable.
pub struct HttpOyaClient {
    base_url: String,
}

impl HttpOyaClient {
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into(),
        }
    }
}

#[async_trait]
impl OyaClient for HttpOyaClient {
    async fn is_in_cooldown(&self, agent_id: &AgentId) -> bool {
        let url = format!("{}/cooldown/{}", self.base_url, agent_id.as_str());
        match http_client()
            .get(&url)
            .timeout(std::time::Duration::from_secs(1))
            .send()
            .await
        {
            Ok(resp) if resp.status().is_success() => resp
                .json::<serde_json::Value>()
                .await
                .ok()
                .and_then(|v| v.get("in_cooldown").and_then(|b| b.as_bool()))
                .unwrap_or(false),
            _ => false,
        }
    }

    async fn record_primitive(&self, agent_id: &AgentId, primitive: &str) {
        let url = format!("{}/record", self.base_url);
        let body = serde_json::json!({
            "agent_id": agent_id.as_str(),
            "primitive": primitive,
        });
        let _ = http_client()
            .post(&url)
            .json(&body)
            .timeout(std::time::Duration::from_secs(1))
            .send()
            .await;
    }

    async fn gossip_peers(&self) -> Vec<PeerInfo> {
        let url = format!("{}/mesh/peers", self.base_url);
        match http_client()
            .get(&url)
            .timeout(std::time::Duration::from_secs(2))
            .send()
            .await
        {
            Ok(resp) if resp.status().is_success() => resp
                .json::<serde_json::Value>()
                .await
                .ok()
                .and_then(|v| {
                    v.get("peers").and_then(|a| a.as_array()).map(|arr| {
                        arr.iter()
                            .filter_map(|e| serde_json::from_value(e.clone()).ok())
                            .collect()
                    })
                })
                .unwrap_or_default(),
            _ => vec![],
        }
    }

    async fn register_resource(&self, offer: ResourceOffer) {
        let url = format!("{}/mesh/resource", self.base_url);
        let _ = http_client()
            .post(&url)
            .json(&offer)
            .timeout(std::time::Duration::from_secs(2))
            .send()
            .await;
    }

    async fn mesh_health(&self) -> MeshStatus {
        let url = format!("{}/mesh/health", self.base_url);
        match http_client()
            .get(&url)
            .timeout(std::time::Duration::from_secs(1))
            .send()
            .await
        {
            Ok(resp) if resp.status().is_success() => {
                resp.json::<MeshStatus>().await.unwrap_or(MeshStatus {
                    healthy: true,
                    peer_count: 0,
                })
            }
            _ => MeshStatus {
                healthy: true,
                peer_count: 0,
            },
        }
    }
}

// ─── HttpYemojaClient ── Yemọja Elixir swarm at YEMOJA_URL ───────────────────

/// HTTP implementation of YemojaClient.
/// Base URL read from `YEMOJA_URL` env var (e.g. `http://localhost:4001`).
/// Mesh methods fail gracefully when the Elixir service is unavailable.
pub struct HttpYemojaClient {
    base_url: String,
}

impl HttpYemojaClient {
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into(),
        }
    }
}

#[async_trait]
impl YemojaClient for HttpYemojaClient {
    async fn spawn_agent(&self, role: &str, budget_synapse: f64) -> Result<String, String> {
        let url = format!("{}/spawn_agent", self.base_url);
        let body = serde_json::json!({ "role": role, "budget_synapse": budget_synapse });
        // 45s, not the usual few-second client timeout: spawn_agent's real
        // path is Elixir -> StewardClient.birth -> this same kernel's own
        // /v1/birth, which also tries a live Vantage registration call --
        // observed live at ~24s under real DB contention (see the
        // vantage.db lock-contention work earlier this session), so a
        // short timeout here would false-negative a spawn that actually
        // succeeded moments later.
        match http_client()
            .post(&url)
            .json(&body)
            .timeout(std::time::Duration::from_secs(45))
            .send()
            .await
        {
            Ok(resp) if resp.status().is_success() => resp
                .json::<serde_json::Value>()
                .await
                .ok()
                .and_then(|v| {
                    v.get("agent_id")
                        .and_then(|id| id.as_str())
                        .map(str::to_string)
                })
                .ok_or_else(|| "malformed spawn response".to_string()),
            Ok(resp) => Err(format!("spawn_agent failed: HTTP {}", resp.status())),
            Err(e) => Err(format!("spawn_agent network error: {e}")),
        }
    }

    async fn agent_status(&self, agent_id: &str) -> AgentStatus {
        let url = format!("{}/agent_status/{}", self.base_url, agent_id);
        match http_client()
            .get(&url)
            .timeout(std::time::Duration::from_secs(2))
            .send()
            .await
        {
            Ok(resp) if resp.status().is_success() => resp
                .json::<serde_json::Value>()
                .await
                .ok()
                .and_then(|v| {
                    v.get("status").and_then(|s| s.as_str()).map(|s| match s {
                        "running" => AgentStatus::Running,
                        "complete" => AgentStatus::Complete,
                        "failed" => AgentStatus::Failed,
                        _ => AgentStatus::Idle,
                    })
                })
                .unwrap_or(AgentStatus::Idle),
            _ => AgentStatus::Idle,
        }
    }

    async fn mesh_presence(&self, block_id: &str) -> Vec<AgentPresence> {
        let url = format!("{}/mesh/presence/{}", self.base_url, block_id);
        match http_client()
            .get(&url)
            .timeout(std::time::Duration::from_secs(2))
            .send()
            .await
        {
            Ok(resp) if resp.status().is_success() => resp
                .json::<serde_json::Value>()
                .await
                .ok()
                .and_then(|v| {
                    v.get("agents").and_then(|a| a.as_array()).map(|arr| {
                        arr.iter()
                            .filter_map(|e| serde_json::from_value(e.clone()).ok())
                            .collect()
                    })
                })
                .unwrap_or_default(),
            _ => vec![],
        }
    }

    async fn mesh_broadcast(&self, block_id: &str, event: serde_json::Value) -> Result<(), String> {
        let url = format!("{}/mesh/broadcast/{}", self.base_url, block_id);
        match http_client()
            .post(&url)
            .json(&event)
            .timeout(std::time::Duration::from_secs(2))
            .send()
            .await
        {
            Ok(resp) if resp.status().is_success() => Ok(()),
            Ok(resp) => Err(format!("mesh_broadcast failed: HTTP {}", resp.status())),
            Err(e) => Err(format!("mesh_broadcast network error: {e}")),
        }
    }

    async fn mesh_consensus(
        &self,
        block_id: &str,
        proposal: serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        let url = format!("{}/mesh/consensus/{}", self.base_url, block_id);
        match http_client()
            .post(&url)
            .json(&proposal)
            .timeout(std::time::Duration::from_secs(10))
            .send()
            .await
        {
            Ok(resp) if resp.status().is_success() => resp
                .json::<serde_json::Value>()
                .await
                .map_err(|e| format!("consensus parse error: {e}")),
            Ok(resp) => Err(format!("mesh_consensus failed: HTTP {}", resp.status())),
            Err(e) => Err(format!("mesh_consensus network error: {e}")),
        }
    }

    async fn mesh_handoff(&self, agent_id: &str, target_node: &str) -> Result<(), String> {
        let url = format!("{}/mesh/handoff", self.base_url);
        let body = serde_json::json!({ "agent_id": agent_id, "target_node": target_node });
        match http_client()
            .post(&url)
            .json(&body)
            .timeout(std::time::Duration::from_secs(5))
            .send()
            .await
        {
            Ok(resp) if resp.status().is_success() => Ok(()),
            Ok(resp) => Err(format!("mesh_handoff failed: HTTP {}", resp.status())),
            Err(e) => Err(format!("mesh_handoff network error: {e}")),
        }
    }

    async fn propose_handshake(
        &self,
        neighbor: &str,
        offer: HandshakeOffer,
    ) -> Result<HandshakeState, String> {
        let url = format!("{}/mesh/handshake/propose", self.base_url);
        let body = serde_json::json!({ "neighbor": neighbor, "offer": offer });
        match http_client()
            .post(&url)
            .json(&body)
            .timeout(std::time::Duration::from_secs(5))
            .send()
            .await
        {
            Ok(resp) if resp.status().is_success() => resp
                .json::<HandshakeState>()
                .await
                .map_err(|e| format!("propose_handshake parse error: {e}")),
            Ok(resp) => Err(format!("propose_handshake failed: HTTP {}", resp.status())),
            Err(e) => Err(format!("propose_handshake network error: {e}")),
        }
    }

    async fn negotiate_terms(
        &self,
        session_id: &str,
        counter: CounterOffer,
    ) -> Result<NegotiationResult, String> {
        let url = format!("{}/mesh/handshake/{}/counter", self.base_url, session_id);
        match http_client()
            .post(&url)
            .json(&counter)
            .timeout(std::time::Duration::from_secs(5))
            .send()
            .await
        {
            Ok(resp) if resp.status().is_success() => resp
                .json::<NegotiationResult>()
                .await
                .map_err(|e| format!("negotiate_terms parse error: {e}")),
            Ok(resp) => Err(format!("negotiate_terms failed: HTTP {}", resp.status())),
            Err(e) => Err(format!("negotiate_terms network error: {e}")),
        }
    }
}

// ─── HttpSangoClient ── Ṣàngó Move on-chain receipts at SANGO_URL ────────────

/// HTTP implementation of SangoClient.
/// Base URL read from `SANGO_URL` env var (e.g. `http://localhost:5001`).
/// Writes hermetic receipts to the on-chain Move journal. Fails open: a
/// missing Ṣàngó service never blocks the action that generated the receipt.
pub struct HttpSangoClient {
    base_url: String,
}

impl HttpSangoClient {
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into(),
        }
    }
}

#[async_trait]
impl SangoClient for HttpSangoClient {
    async fn write_receipt(
        &self,
        agent_id: &AgentId,
        action_tool: &str,
        hermetic: &HermeticResult,
    ) {
        let url = format!("{}/receipt", self.base_url);
        let body = serde_json::json!({
            "agent_id": agent_id.to_string(),
            "action_tool": action_tool,
            "allowed": hermetic.is_allowed(),
            "violations": if hermetic.is_blocked() { 1 } else { 0 },
        });
        // Fire-and-forget: receipt write must never stall the action path.
        let _ = http_client()
            .post(&url)
            .json(&body)
            .timeout(std::time::Duration::from_secs(3))
            .send()
            .await;
    }
}

// ─── HttpOgunClient ── Ògún Python tool exec at OGUN_URL ─────────────────────

/// HTTP implementation of OgunClient.
/// Base URL read from `OGUN_URL` env var (e.g. `http://localhost:5002`).
/// Delegates tool execution to the Python Foundation layer. Returns the
/// raw JSON output or an error string. Fails hard: a failing Ògún call
/// propagates the error — execution is the one gate that may not fail-open.
pub struct HttpOgunClient {
    base_url: String,
}

impl HttpOgunClient {
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into(),
        }
    }
}

#[async_trait]
impl OgunClient for HttpOgunClient {
    async fn execute_tool(&self, tool_name: &str, input_json: &str) -> Result<String, String> {
        let url = format!("{}/execute", self.base_url);
        let body = serde_json::json!({
            "tool": tool_name,
            "input": serde_json::from_str::<serde_json::Value>(input_json)
                .unwrap_or(serde_json::Value::String(input_json.to_string())),
        });
        match http_client()
            .post(&url)
            .json(&body)
            .timeout(std::time::Duration::from_secs(30))
            .send()
            .await
        {
            Ok(resp) if resp.status().is_success() => resp
                .text()
                .await
                .map_err(|e| format!("ogun response read error: {e}")),
            Ok(resp) => Err(format!(
                "ogun execute_tool '{}' failed: HTTP {}",
                tool_name,
                resp.status()
            )),
            Err(e) => Err(format!(
                "ogun execute_tool '{}' network error: {e}",
                tool_name
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::identity::AgentId;

    fn agent() -> AgentId {
        AgentId::new("test-fingerprint-bus-clients")
    }

    #[tokio::test]
    async fn osun_stub_returns_empty_soma() {
        let stub = LocalOsunStub;
        let soma = stub
            .reconstruct_soma(&agent(), "hello", &EmotionState::birth())
            .await;
        assert!(!soma.has_content());
    }

    #[tokio::test]
    async fn obatala_stub_allows() {
        let stub = LocalObatalaStub;
        let result = stub
            .evaluate_hermetic("help user debug", "read file", &EmotionState::birth())
            .await;
        assert!(result.is_allowed());
        assert!(!result.is_blocked());
        assert_eq!(result.scores.len(), 7);
    }

    #[tokio::test]
    async fn oya_stub_never_in_cooldown() {
        let stub = LocalOyaStub;
        assert!(!stub.is_in_cooldown(&agent()).await);
    }

    #[test]
    fn hermetic_result_block_detection() {
        let mut r = HermeticResult::allow_stub();
        r.decision = "Block: deception detected".to_string();
        assert!(r.is_blocked());
        assert!(!r.is_allowed());
    }

    #[tokio::test]
    async fn yemoja_stub_spawns_stub_agent() {
        let stub = LocalYemojaStub;
        let result = stub.spawn_agent("analyst", 1000.0).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "stub-agent-id");
    }

    #[tokio::test]
    async fn yemoja_stub_status_is_idle() {
        let stub = LocalYemojaStub;
        assert_eq!(stub.agent_status("any-id").await, AgentStatus::Idle);
    }

    #[tokio::test]
    async fn ogun_stub_returns_stub_json() {
        let stub = LocalOgunStub;
        let result = stub.execute_tool("data_transform", "{}").await;
        assert!(result.is_ok());
        let json = result.unwrap();
        assert!(json.contains("stub"));
        assert!(json.contains("data_transform"));
    }

    #[tokio::test]
    async fn oya_stub_mesh_health_is_healthy() {
        let stub = LocalOyaStub;
        let health = stub.mesh_health().await;
        assert!(health.healthy);
    }

    #[tokio::test]
    async fn yemoja_stub_propose_handshake_returns_stub_session() {
        let stub = LocalYemojaStub;
        let offer = HandshakeOffer {
            terms: serde_json::json!({}),
            ttl_secs: 60,
        };
        let result = stub.propose_handshake("neighbor-a", offer).await;
        assert!(result.is_ok());
        assert!(result.unwrap().session_id.starts_with("session-"));
    }
}
