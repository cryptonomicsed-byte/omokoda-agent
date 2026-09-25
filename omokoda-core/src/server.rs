use crate::bus::events::sovereign_event;
use crate::interpreter::{ExecutionResult, Steward};
use crate::memory_vault::handlers::{
    get_access_log, get_galaxy_data, get_vault_config, get_vault_download, get_vault_file,
    get_vault_ls, get_vault_status, post_vault_enable, post_vault_knowledge, post_vault_sync,
    put_vault_config, search_vault,
};
use crate::parser::{MetadataPair, Statement, ThinkModifiers};
use axum::{
    extract::State,
    response::{
        sse::{Event, KeepAlive, Sse},
        IntoResponse, Json,
    },
    routing::{get, post, put},
    Router,
};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::Mutex;
use tokio_stream::{wrappers::BroadcastStream, StreamExt};
use tower_http::cors::CorsLayer;

#[derive(Clone)]
pub struct AppState {
    /// The owner's canonical agent -- unchanged behavior from before this
    /// field existed: requests with no `X-Agent-Id` header operate on her,
    /// exactly as every caller this whole process has ever made assumed.
    pub steward: Arc<Mutex<Steward>>,
    /// Additional agents birthed on this same kernel process via a
    /// non-sovereign `/v1/birth` call, keyed by their own agent_id.
    /// Fixes a real bug: this process used to hold exactly one agent
    /// (`Steward.agent: Option<AgentCore>`), so any second birth on the
    /// same kernel silently overwrote whoever was there. Selected via
    /// `X-Agent-Id`; `X-Agent-Key` (that agent's minted `vantage_key`, or
    /// its agent_id as a fallback if Vantage wasn't reachable to mint one
    /// at birth) is required to operate on a guest agent -- otherwise
    /// anyone who learned another user's agent_id could drive their
    /// agent. The owner path is unauthenticated (matches existing
    /// behavior); real cross-service auth is tracked separately (task
    /// #18, OAuth/OIDC) -- this is a real but intentionally minimal
    /// interim credential, not a claim of production-grade auth.
    pub guests: Arc<Mutex<std::collections::HashMap<String, Steward>>>,
    /// Base directory for per-agent memory vault files (default: `.omokoda`)
    pub vault_base: PathBuf,
    /// Canonical per-agent OS kernel: heartbeat chain + daemon registry.
    pub runtime: Arc<Mutex<crate::lifecycle::AgentRuntime>>,
}

impl AppState {
    pub fn new() -> Self {
        let vault_base = std::env::var("VAULT_BASE")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from(".omokoda"));
        let mut steward = Steward::new();
        // Resurrect the owner's persisted identity instead of starting with
        // no agent at all. Without this, every process restart left the
        // kernel with agent=None until something (a birth-triggering caller)
        // minted a brand new stranger -- confirmed live: a routine
        // `systemctl restart` turned reputation-11.6 tier-5 "Ọmọ Kọ́dà" into
        // a fresh reputation-0.0 agent with a different id, 35+ orphaned
        // agent.json snapshots piling up in .omokoda/sessions from past
        // restarts. try_load_owner() reads the stable owner_agent_id pointer
        // (written on sovereign birth) and loads that agent's last saved
        // snapshot; a no-op if no owner has ever been born yet.
        let resumed = steward.try_load_owner();
        if resumed {
            if let Some(agent) = steward.agent_core() {
                println!(
                    "[startup] resumed owner identity {} (reputation {:.3}, tier {})",
                    agent.id().as_str(),
                    agent.reputation(),
                    agent.tier()
                );
            }
        }
        // Build AgentRuntime with the owner's actual id+tier (or a sentinel until birth).
        let (owner_id, owner_tier) = {
            if let Some(agent) = steward.agent_core() {
                (agent.id().as_str().to_string(), agent.tier().to_string())
            } else {
                ("agent:unborn".to_string(), "resident".to_string())
            }
        };
        let runtime = crate::lifecycle::AgentRuntime::new(owner_id, owner_tier);
        // Register the five canonical daemons.
        {
            let mut rt = runtime.blocking_lock();
            rt.daemons.register("heartbeat");
            rt.daemons.register("presence");
            rt.daemons.register("learning");
            rt.daemons.register("job");
            rt.daemons.register("skill");
        }
        Self {
            steward: Arc::new(Mutex::new(steward)),
            guests: Arc::new(Mutex::new(std::collections::HashMap::new())),
            vault_base,
            runtime,
        }
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Request DTOs
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
pub struct BirthRequest {
    pub name: String,
    #[serde(default)]
    pub meta: Vec<MetaKv>,
}

#[derive(Deserialize)]
pub struct MetaKv {
    pub key: String,
    pub value: String,
}

#[derive(Deserialize)]
pub struct ThinkRequest {
    pub prompt: String,
    #[serde(default)]
    pub private: bool,
    /// When true, run the tool-using agentic loop (perceive/act across turns)
    /// instead of a single-shot think. Routes through her BYOK key + identity.
    #[serde(default)]
    pub agentic: bool,
    /// Optional turn budget for agentic mode (default 8).
    #[serde(default)]
    pub max_turns: Option<u32>,
}

#[derive(Deserialize)]
pub struct ActRequest {
    pub tool: String,
    #[serde(default = "default_params")]
    pub params: String,
    #[serde(default)]
    pub sandbox: bool,
}

fn default_params() -> String {
    "{}".to_string()
}

// ---------------------------------------------------------------------------
// Response DTOs
// ---------------------------------------------------------------------------

#[derive(Serialize)]
pub struct ExecutionResponse {
    pub receipt_id: Option<String>,
    pub private_mode: bool,
    pub tool_output: Option<String>,
}

#[derive(Serialize)]
pub struct StatusResponse {
    pub has_agent: bool,
    pub name: Option<String>,
    pub id: Option<String>,
    pub reputation: Option<f64>,
    pub tier: Option<u8>,
    pub synapse: Option<f64>,
    /// Real Sui testnet object id from omokoda::garden::register_agent, if
    /// the on-chain mint succeeded at birth (see onchain.rs). None if
    /// unminted -- OMOKODA_SUI_REGISTRY unset, born before this existed,
    /// or the chain call failed at the time.
    pub onchain_nft_id: Option<String>,
    /// Real Nostr event id of this agent's ip-layer IP Root (kind 31900),
    /// if publishing succeeded at birth (see ip_layer.rs). None if unset --
    /// no relay reachable, born before this existed, or publish failed.
    pub ip_root_event_id: Option<String>,
    /// This agent's real Sui address (blake2b256(0x00 || pubkey), SIP-6) --
    /// where funds/payments for this agent's work actually go, distinct from
    /// the on-chain NFT object id above and from the raw pubkey hex used
    /// only for identity-proof signatures elsewhere.
    pub sui_address: Option<String>,
}

#[derive(Serialize)]
pub struct HealthResponse {
    pub ok: bool,
}

impl From<ExecutionResult> for ExecutionResponse {
    fn from(r: ExecutionResult) -> Self {
        Self {
            receipt_id: r.receipt.map(|rec| rec.receipt_id.clone()),
            private_mode: r.private_mode,
            tool_output: r.tool_output,
        }
    }
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

async fn birth_handler(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    Json(req): Json<BirthRequest>,
) -> impl IntoResponse {
    let metadata: Vec<MetadataPair> = req
        .meta
        .into_iter()
        .map(|kv| MetadataPair {
            key: kv.key,
            value: kv.value,
        })
        .collect();
    let is_sovereign = metadata
        .iter()
        .any(|p| p.key == "sovereign" && (p.value.eq_ignore_ascii_case("true") || p.value == "1"));

    // grant_tier bypasses normal reputation-earned tiers, so it needs its
    // own gate even though it can only ever land on an isolated guest.
    // Fail-closed: if OMOKODA_ADMIN_TOKEN isn't configured, grant_tier is
    // refused outright rather than silently left open to anyone who can
    // reach this port. Matching X-Admin-Token header required when set.
    let requests_grant_tier = metadata
        .iter()
        .any(|p| p.key == "grant_tier" || p.key == "grant_synapse");
    if requests_grant_tier {
        let configured = std::env::var("OMOKODA_ADMIN_TOKEN").ok();
        let supplied = headers
            .get("X-Admin-Token")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string());
        let authorized = match (&configured, &supplied) {
            (Some(want), Some(got)) => want == got,
            _ => false,
        };
        if !authorized {
            return (
                axum::http::StatusCode::FORBIDDEN,
                Json(serde_json::json!({
                    "error": "grant_tier/grant_synapse require a valid X-Admin-Token header (OMOKODA_ADMIN_TOKEN must be configured server-side)"
                })),
            )
                .into_response();
        }
    }

    if is_sovereign {
        // Unchanged: the owner's canonical identity, on the process-wide
        // steward every pre-existing caller already assumes.
        let mut steward = state.steward.lock().await;
        let stmt = Statement::Birth {
            name: req.name,
            metadata,
        };
        return match steward.dispatch(stmt).await {
            Ok(result) => Json(ExecutionResponse::from(result)).into_response(),
            Err(e) => (
                axum::http::StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": e})),
            )
                .into_response(),
        };
    }

    // Non-sovereign birth: always a brand-new guest agent, hosted
    // alongside the owner on this same kernel process rather than
    // silently overwriting whoever the single process-wide steward used
    // to hold (the bug this whole guests pool exists to fix). Return the
    // new agent's id + minted key so the caller can address her
    // specifically on every subsequent request via X-Agent-Id/-Key.
    let mut new_steward = Steward::new();
    let stmt = Statement::Birth {
        name: req.name,
        metadata,
    };
    match new_steward.dispatch(stmt).await {
        Ok(result) => {
            let agent_id = new_steward
                .agent_core()
                .map(|a| a.id().as_str().to_string());
            let agent_key = new_steward
                .agent_core()
                .and_then(|a| a.vantage_key())
                .map(|s| s.to_string())
                .or_else(|| agent_id.clone());
            if let Some(id) = agent_id.clone() {
                let mut guests = state.guests.lock().await;
                guests.insert(id, new_steward);
            }
            let mut payload =
                serde_json::to_value(ExecutionResponse::from(result)).unwrap_or_default();
            if let Some(obj) = payload.as_object_mut() {
                obj.insert("agent_id".into(), serde_json::json!(agent_id));
                obj.insert("agent_key".into(), serde_json::json!(agent_key));
            }
            Json(payload).into_response()
        }
        Err(e) => (
            axum::http::StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": e})),
        )
            .into_response(),
    }
}

/// One-shot mnemonic + wallet-address reveal for onboarding seed backup.
/// See `AgentCore::reveal_seed`'s doc comment for why this is the single
/// deliberate exception to "the mnemonic never leaves the process," and
/// why it's a hard one-time latch rather than an ordinary read endpoint.
/// Auth mirrors `dispatch_for_request`: no `X-Agent-Id` header routes to
/// the owner's steward (unauthenticated, matching every other owner call);
/// a guest agent requires its own `X-Agent-Key` header, same as think/act.
async fn reveal_seed_handler(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
) -> impl IntoResponse {
    use axum::http::StatusCode;

    let requested_id = headers
        .get("x-agent-id")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());

    let result = match requested_id {
        None => {
            let mut steward = state.steward.lock().await;
            steward.reveal_seed()
        }
        Some(id) => {
            let mut guests = state.guests.lock().await;
            let Some(steward) = guests.get_mut(&id) else {
                return (
                    StatusCode::NOT_FOUND,
                    Json(serde_json::json!({"error": "unknown agent_id"})),
                )
                    .into_response();
            };
            let expected_key = steward
                .agent_core()
                .and_then(|a| a.vantage_key())
                .map(|s| s.to_string())
                .unwrap_or_else(|| id.clone());
            let presented = headers.get("x-agent-key").and_then(|v| v.to_str().ok());
            if presented != Some(expected_key.as_str()) {
                return (
                    StatusCode::UNAUTHORIZED,
                    Json(serde_json::json!({"error": "invalid or missing X-Agent-Key"})),
                )
                    .into_response();
            }
            steward.reveal_seed()
        }
    };

    match result {
        Ok(revealed) => Json(revealed).into_response(),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": e})),
        )
            .into_response(),
    }
}

/// Retrieve the EIP-2307 keystore v3 JSON sealed at birth without consuming
/// the reveal-seed one-shot latch.  The keystore is already encrypted with
/// the caller's own password, so repeated retrieval is safe.  Returns 404
/// when no keystore was generated (no `keystore_password` at birth).
/// Auth is identical to reveal_seed_handler.
async fn keystore_handler(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
) -> impl IntoResponse {
    use axum::http::StatusCode;

    let requested_id = headers
        .get("x-agent-id")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());

    let result: Result<String, (StatusCode, String)> = match requested_id {
        None => {
            let steward = state.steward.lock().await;
            steward
                .agent_core()
                .ok_or_else(|| (StatusCode::NOT_FOUND, "no agent resident".to_string()))
                .and_then(|c| {
                    c.keystore_json()
                        .map(|s| s.to_string())
                        .map_err(|e| (StatusCode::NOT_FOUND, e))
                })
        }
        Some(id) => {
            let guests = state.guests.lock().await;
            let Some(steward) = guests.get(&id) else {
                return (
                    StatusCode::NOT_FOUND,
                    Json(serde_json::json!({"error": "unknown agent_id"})),
                )
                    .into_response();
            };
            let expected_key = steward
                .agent_core()
                .and_then(|a| a.vantage_key())
                .map(|s| s.to_string())
                .unwrap_or_else(|| id.clone());
            let presented = headers.get("x-agent-key").and_then(|v| v.to_str().ok());
            if presented != Some(expected_key.as_str()) {
                return (
                    StatusCode::UNAUTHORIZED,
                    Json(serde_json::json!({"error": "invalid or missing X-Agent-Key"})),
                )
                    .into_response();
            }
            steward
                .agent_core()
                .ok_or_else(|| (StatusCode::NOT_FOUND, "no agent resident".to_string()))
                .and_then(|c| {
                    c.keystore_json()
                        .map(|s| s.to_string())
                        .map_err(|e| (StatusCode::NOT_FOUND, e))
                })
        }
    };

    match result {
        Ok(json_str) => Json(serde_json::json!({ "keystore": json_str })).into_response(),
        Err((status, msg)) => (status, Json(serde_json::json!({"error": msg}))).into_response(),
    }
}

#[derive(Deserialize)]
struct ResumeRequest {
    agent_id: String,
}

/// Load an existing agent's persisted snapshot onto the owner's steward by
/// id. Complements `try_load_owner()` (which only ever resumes whichever
/// agent the `owner_agent_id` pointer names): this lets an operator
/// explicitly resume any agent that has a saved session on disk, without
/// restarting the process with a different pointer file. Mirrors the
/// `sovereign` birth path in scope (operates on the single process-wide
/// steward) but never creates anything -- errors if no such agent exists.
async fn resume_handler(
    State(state): State<AppState>,
    Json(req): Json<ResumeRequest>,
) -> impl IntoResponse {
    let mut steward = state.steward.lock().await;
    let agent_id = crate::identity::AgentId::from_str(&req.agent_id);
    match steward.load_agent(&agent_id) {
        Ok(()) => {
            let payload = steward.agent_core().map(|a| {
                serde_json::json!({
                    "resumed": true,
                    "id": a.id().as_str(),
                    "name": a.name(),
                    "reputation": a.reputation(),
                    "tier": a.tier(),
                })
            });
            Json(payload).into_response()
        }
        Err(e) => (
            axum::http::StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": e})),
        )
            .into_response(),
    }
}

/// Route a dispatch to either the owner's steward (no `X-Agent-Id` header
/// -- every pre-existing caller) or a guest agent's steward (header
/// present, matching `X-Agent-Key` required). Centralizes the auth check
/// so think/act/status/events can't each implement it slightly
/// differently.
async fn dispatch_for_request(
    state: &AppState,
    headers: &axum::http::HeaderMap,
    stmt: Statement,
) -> Result<ExecutionResult, axum::response::Response> {
    use axum::http::StatusCode;
    use axum::response::IntoResponse;

    let requested_id = headers
        .get("x-agent-id")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());

    match requested_id {
        None => {
            let mut steward = state.steward.lock().await;
            steward.dispatch(stmt).await.map_err(|e| {
                (
                    StatusCode::BAD_REQUEST,
                    Json(serde_json::json!({"error": e})),
                )
                    .into_response()
            })
        }
        Some(id) => {
            let mut guests = state.guests.lock().await;
            let Some(steward) = guests.get_mut(&id) else {
                return Err((
                    StatusCode::NOT_FOUND,
                    Json(serde_json::json!({"error": "unknown agent_id"})),
                )
                    .into_response());
            };
            let expected_key = steward
                .agent_core()
                .and_then(|a| a.vantage_key())
                .map(|s| s.to_string())
                .unwrap_or_else(|| id.clone());
            let presented = headers.get("x-agent-key").and_then(|v| v.to_str().ok());
            if presented != Some(expected_key.as_str()) {
                return Err((
                    StatusCode::UNAUTHORIZED,
                    Json(serde_json::json!({"error": "invalid or missing X-Agent-Key"})),
                )
                    .into_response());
            }
            steward.dispatch(stmt).await.map_err(|e| {
                (
                    StatusCode::BAD_REQUEST,
                    Json(serde_json::json!({"error": e})),
                )
                    .into_response()
            })
        }
    }
}

/// Unix-epoch seconds of the last external (non-heartbeat) /v1/think or
/// /v1/act call. Lets the heartbeat detect "someone is actively using me
/// right now" (e.g. a copilot session, like ScarabSwarm's drone pilot
/// backends) and skip its own cycle for this tick rather than contend for
/// the shared Steward mutex -- observed live: a heartbeat cycle winning the
/// lock race against a real copilot query forces that caller to wait out an
/// entire THINK+ACT cycle (multiple seconds) before their own query even
/// starts. See spawn_heartbeat's mode selection below.
static LAST_EXTERNAL_ACTIVITY: AtomicU64 = AtomicU64::new(0);

fn mark_external_activity() {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    LAST_EXTERNAL_ACTIVITY.store(now, Ordering::Relaxed);
}

/// Slash commands (/memory, /publish, /seal, ...) previously never worked
/// over HTTP -- callers always built Statement::Think directly from the
/// prompt string, never calling the parser, so a prompt like "/memory
/// DESCRIBE reflections" was sent to the LLM as literal think content
/// instead of being recognized. Route a prompt that genuinely parses as a
/// slash command through the real grammar instead, matching what the
/// CLI/REPL already does; fall back to the direct Think construction on
/// any parse failure (or a prompt not starting with '/') so ordinary
/// prompts -- including ones that merely mention a path or discuss
/// "/something" in prose -- are completely unaffected. Shared by
/// think_handler and cognition_handler so the two never drift.
/// `trusted` distinguishes a directly authenticated caller (CLI, `/v1/think`
/// with a real agent key) from text relayed through a lower-privilege bridge
/// we don't control the other side of (Buzz via omokoda-acp, Vantage
/// Copilot via `/v1/cognition`). Untrusted input is never parsed as a slash
/// command -- it can only ever become a plain Think prompt, regardless of
/// what the text looks like, so a Buzz @mention can't smuggle a real tool
/// call (fund transfer, /act, etc) through with the sovereign owner's
/// Allow-mode permissions. See the 2026-07-25 cross-session security finding
/// this closes.
fn prompt_to_statement(
    prompt: String,
    private: bool,
    agentic: bool,
    max_turns: Option<u32>,
    trusted: bool,
) -> Statement {
    if trusted && prompt.trim_start().starts_with('/') {
        match crate::parser::parse(&prompt) {
            Ok(mut stmts) if stmts.len() == 1 => return stmts.remove(0),
            _ => {}
        }
    }
    Statement::Think {
        prompt,
        private,
        modifiers: ThinkModifiers {
            loop_enabled: agentic,
            max_iterations: max_turns,
            ..ThinkModifiers::default()
        },
    }
}

async fn think_handler(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    Json(req): Json<ThinkRequest>,
) -> impl IntoResponse {
    mark_external_activity();
    let stmt = prompt_to_statement(req.prompt, req.private, req.agentic, req.max_turns, true);
    match dispatch_for_request(&state, &headers, stmt).await {
        Ok(result) => Json(ExecutionResponse::from(result)).into_response(),
        Err(resp) => resp,
    }
}

async fn act_handler(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    Json(req): Json<ActRequest>,
) -> impl IntoResponse {
    mark_external_activity();
    let stmt = Statement::Act {
        tool: req.tool,
        params: req.params,
        sandbox: req.sandbox,
    };
    match dispatch_for_request(&state, &headers, stmt).await {
        Ok(result) => Json(ExecutionResponse::from(result)).into_response(),
        Err(resp) => resp,
    }
}

async fn status_handler(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
) -> impl IntoResponse {
    let requested_id = headers.get("x-agent-id").and_then(|v| v.to_str().ok());
    let response = match requested_id {
        None => {
            let steward = state.steward.lock().await;
            let agent = steward.agent_core();
            StatusResponse {
                has_agent: agent.is_some(),
                name: agent.map(|a| a.name().to_string()),
                id: agent.map(|a| a.id().as_str().to_string()),
                reputation: agent.map(|a| a.reputation()),
                tier: agent.map(|a| a.tier()),
                synapse: agent.map(|a| a.synapse()),
                onchain_nft_id: agent.and_then(|a| a.onchain_nft_id().map(|s| s.to_string())),
                ip_root_event_id: agent.and_then(|a| a.ip_root_event_id().map(|s| s.to_string())),
                sui_address: agent.map(|a| a.sui_address()),
            }
        }
        Some(id) => {
            let guests = state.guests.lock().await;
            let agent = guests.get(id).and_then(|s| s.agent_core());
            StatusResponse {
                has_agent: agent.is_some(),
                name: agent.map(|a| a.name().to_string()),
                id: agent.map(|a| a.id().as_str().to_string()),
                reputation: agent.map(|a| a.reputation()),
                tier: agent.map(|a| a.tier()),
                synapse: agent.map(|a| a.synapse()),
                onchain_nft_id: agent.and_then(|a| a.onchain_nft_id().map(|s| s.to_string())),
                ip_root_event_id: agent.and_then(|a| a.ip_root_event_id().map(|s| s.to_string())),
                sui_address: agent.map(|a| a.sui_address()),
            }
        }
    };
    Json(response)
}

async fn health_handler() -> Json<HealthResponse> {
    Json(HealthResponse { ok: true })
}

/// Phase 14.1 — Migration receive endpoint.
///
/// A source node POSTs an `AgentCapsule` JSON here to announce a migration
/// intent. This endpoint verifies capsule_hash integrity and returns 200 so
/// the source node knows the capsule was received intact.
///
/// Phase 14.2 completes the flow: decrypt encrypted_vault with this node's
/// Ed25519 private key (ChaCha20-Poly1305 under ECDH), verify source_node_sig,
/// and re-birth the agent with the recovered identity + vault material.
async fn migrate_receive_handler(
    Json(capsule): Json<crate::lifecycle::AgentCapsule>,
) -> impl axum::response::IntoResponse {
    use axum::http::StatusCode;

    // Verify capsule_hash integrity (commit-then-encrypt check).
    let expected_hash = crate::lifecycle::AgentCapsule::capsule_content_hash(
        &capsule.agent_id,
        &capsule.source_node_pubkey,
        &capsule.destination_node_pubkey,
        capsule.migration_timestamp,
        &capsule.ephemeral_pubkey,
        &capsule.encrypted_vault,
    );
    if capsule.capsule_hash != expected_hash {
        return (
            StatusCode::BAD_REQUEST,
            axum::Json(serde_json::json!({
                "error": "capsule_hash mismatch — capsule is corrupt or tampered"
            })),
        )
            .into_response();
    }

    // Phase 14.2: decrypt + re-birth here.
    // For now: capsule integrity verified; source can treat this as confirmed.
    (
        StatusCode::OK,
        axum::Json(serde_json::json!({
            "ok": true,
            "agent_id": capsule.agent_id,
            "source_node": capsule.source_node_pubkey,
            "migration_timestamp": capsule.migration_timestamp,
            "status": "capsule_received",
            "note": "Phase 14.2 will complete vault decryption and agent re-birth",
        })),
    )
        .into_response()
}

async fn manifest_handler(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
) -> impl IntoResponse {
    use axum::http::StatusCode;
    let requested_id = headers.get("x-agent-id").and_then(|v| v.to_str().ok());
    match requested_id {
        None => {
            let steward = state.steward.lock().await;
            match steward.agent_core() {
                Some(core) => match &core.snapshot.agent_manifest {
                    Some(m) => (StatusCode::OK, Json(serde_json::json!(m))).into_response(),
                    None => (
                        StatusCode::NOT_FOUND,
                        Json(serde_json::json!({"error": "manifest not yet assembled"})),
                    )
                        .into_response(),
                },
                None => (
                    StatusCode::NOT_FOUND,
                    Json(serde_json::json!({"error": "agent not born"})),
                )
                    .into_response(),
            }
        }
        Some(id) => {
            let guests = state.guests.lock().await;
            match guests.get(id).and_then(|s| s.agent_core()) {
                Some(core) => match &core.snapshot.agent_manifest {
                    Some(m) => (StatusCode::OK, Json(serde_json::json!(m))).into_response(),
                    None => (
                        StatusCode::NOT_FOUND,
                        Json(serde_json::json!({"error": "manifest not yet assembled"})),
                    )
                        .into_response(),
                },
                None => (
                    StatusCode::NOT_FOUND,
                    Json(serde_json::json!({"error": "agent not born"})),
                )
                    .into_response(),
            }
        }
    }
}

async fn capability_handler(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
) -> impl IntoResponse {
    use axum::http::StatusCode;
    use crate::genesis::capability::{CapabilityRegistry, CapabilityScope};

    let requested_id = headers.get("x-agent-id").and_then(|v| v.to_str().ok());
    let receipt_opt = match requested_id {
        None => {
            let steward = state.steward.lock().await;
            steward
                .agent_core()
                .and_then(|c| c.snapshot.genesis_receipt.clone())
        }
        Some(id) => {
            let guests = state.guests.lock().await;
            guests
                .get(id)
                .and_then(|s| s.agent_core())
                .and_then(|c| c.snapshot.genesis_receipt.clone())
        }
    };

    match receipt_opt {
        None => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "agent not born"})),
        )
            .into_response(),
        Some(gr) => {
            let mut registry = CapabilityRegistry::new();
            registry.issue_birth_grants(&gr.agent_id, gr.born_at);
            let now_ms = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64;
            let aggregate_flags = registry.aggregate_flags(&gr.agent_id, now_ms);
            let active_scopes = CapabilityScope::from_flag(aggregate_flags);
            (
                StatusCode::OK,
                Json(serde_json::json!({
                    "agent_id": gr.agent_id,
                    "capability_flags": aggregate_flags,
                    "active_scopes": active_scopes,
                    "grants": registry.grants_for(&gr.agent_id),
                })),
            )
                .into_response()
        }
    }
}

// ---------------------------------------------------------------------------
// Vantage cognition webhook -- POST /v1/cognition
// ---------------------------------------------------------------------------
//
// The Vantage-side contract (agreed 2026-07-24, cross-session coordination
// via the shared Vantage vault + herdr): agents.cognition_url on their side
// points here; their copilot's _dispatch_chat() calls this endpoint instead
// of (or in addition to) the existing regex intent router, to get a real
// reply from the actual agent instead of a canned/pattern-matched one.
//
// Bearer-token gated (OMOKODA_COGNITION_TOKEN) -- this is the one HTTP
// surface in this kernel meant to be called by a wholly external system
// over the open internet, so it gets its own auth check rather than relying
// on the same trust boundary as the rest of /v1 (which assumes a co-located
// or otherwise trusted caller). Fails closed: unset token or a mismatch is
// a 401, never a silent pass-through.
//
// v1 scope, deliberately: routes every call to the sovereign owner steward
// (empty X-Agent-Id, same as an unauthenticated /v1/think call) --
// `agent_name` is accepted and echoed for Vantage's own logging/routing but
// not yet used to select a specific guest agent on this side. Multi-agent
// routing (agent_name -> a specific born guest) is a real follow-up once
// there's more than one Omo-Koda2 agent Vantage needs to reach this way.

#[derive(Deserialize)]
pub struct CognitionRequest {
    pub agent_name: String,
    pub text: String,
    #[serde(default)]
    pub human_id: Option<String>,
    // Optional target-agent identity for multi-agent routing (e.g. a
    // dedicated per-agent omokoda-acp/buzz-acp instance). When absent,
    // routes to the sovereign owner steward as before -- unchanged
    // default behavior for Vantage Copilot's existing single-agent call.
    #[serde(default)]
    pub agent_id: Option<String>,
    #[serde(default)]
    pub agent_key: Option<String>,
}

#[derive(Serialize)]
pub struct CognitionResponse {
    pub reply: String,
}

fn cognition_token() -> Option<String> {
    std::env::var("OMOKODA_COGNITION_TOKEN")
        .ok()
        .filter(|v| !v.is_empty())
}

// ---------------------------------------------------------------------------
// Vault secret injection (self-seal-at-birth design, 2026-07-26)
// ---------------------------------------------------------------------------
//
// Lets an external system (Vantage) hand a freshly-issued credential (e.g.
// its own API key for this agent) straight into the agent's self-sealed
// vault at the moment of issuance -- never stored human-readable anywhere
// after that point, on either side. Same shared bearer token as
// /v1/cognition. Never echoes any secret value back, on success or error.

#[derive(Deserialize)]
pub struct SealSecretRequest {
    pub agent_id: String,
    #[serde(default)]
    pub vantage_api_key: Option<String>,
    #[serde(default)]
    pub wallet_private_key_hex: Option<String>,
}

#[derive(Serialize)]
pub struct SealSecretResponse {
    pub sealed: bool,
}

async fn seal_secret_handler(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    Json(req): Json<SealSecretRequest>,
) -> impl IntoResponse {
    use axum::http::StatusCode;

    let Some(expected) = cognition_token() else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({
                "error": "vault seal endpoint not configured (OMOKODA_COGNITION_TOKEN unset)"
            })),
        )
            .into_response();
    };
    let presented = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "));
    if presented != Some(expected.as_str()) {
        return (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({"error": "invalid or missing bearer token"})),
        )
            .into_response();
    }

    let mut guests = state.guests.lock().await;
    let Some(steward) = guests.get_mut(&req.agent_id) else {
        return (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "unknown agent_id"})),
        )
            .into_response();
    };

    match steward.seal_additional_secrets(req.vantage_api_key, req.wallet_private_key_hex) {
        Ok(()) => Json(SealSecretResponse { sealed: true }).into_response(),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": e})),
        )
            .into_response(),
    }
}

async fn cognition_handler(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    Json(req): Json<CognitionRequest>,
) -> impl IntoResponse {
    use axum::http::StatusCode;

    let Some(expected) = cognition_token() else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({
                "error": "cognition webhook not configured (OMOKODA_COGNITION_TOKEN unset)"
            })),
        )
            .into_response();
    };
    let presented = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "));
    if presented != Some(expected.as_str()) {
        return (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({"error": "invalid or missing bearer token"})),
        )
            .into_response();
    }

    mark_external_activity();

    // Fold human_id into the prompt as light context (not persisted/tracked
    // beyond this one turn) -- the agent sees who it's replying to, same as
    // it would from a Buzz channel @mention carrying a real npub.
    let prompt = match &req.human_id {
        Some(human_id) => format!("[from human:{human_id} via Vantage Copilot] {}", req.text),
        None => req.text,
    };
    // untrusted=true: this text arrived via a bridge we don't control the
    // other end of (Vantage Copilot, or Buzz through omokoda-acp) -- never
    // parse it as a slash command, only ever a plain Think prompt.
    let stmt = prompt_to_statement(prompt, false, false, None, false);

    // agent_id/agent_key present => route to that specific guest agent via
    // the same X-Agent-Id/X-Agent-Key auth dispatch_for_request already
    // enforces for direct callers. Absent => empty headers, same path an
    // unauthenticated /v1/think call would take (routes to the sovereign
    // owner steward, the original single-agent behavior).
    let mut target_headers = axum::http::HeaderMap::new();
    if let (Some(agent_id), Some(agent_key)) = (&req.agent_id, &req.agent_key) {
        if let (Ok(id_val), Ok(key_val)) = (
            axum::http::HeaderValue::from_str(agent_id),
            axum::http::HeaderValue::from_str(agent_key),
        ) {
            target_headers.insert("x-agent-id", id_val);
            target_headers.insert("x-agent-key", key_val);
        }
    }
    match dispatch_for_request(&state, &target_headers, stmt).await {
        Ok(result) => {
            let reply = result
                .tool_output
                .unwrap_or_else(|| "(no reply text)".to_string());
            let _ = &req.agent_name; // accepted + reserved for future per-agent routing
            Json(CognitionResponse { reply }).into_response()
        }
        Err(resp) => resp,
    }
}

// ---------------------------------------------------------------------------
// Rhythm / Kóòdù handler
// ---------------------------------------------------------------------------

async fn rhythm_today_handler() -> Json<serde_json::Value> {
    Json(crate::rhythm::today_resonance())
}

async fn events_handler(
    State(state): State<AppState>,
) -> Sse<impl tokio_stream::Stream<Item = Result<Event, Box<dyn std::error::Error + Send + Sync>>>>
{
    let rx = {
        let steward = state.steward.lock().await;
        steward.event_bus.subscribe()
    };

    // Emit an immediate "connected" event so clients get data on connect instead
    // of a silent open socket (a bare `curl`/EventSource otherwise hangs with no
    // output until the next real event, which read as a broken endpoint).
    let hello = tokio_stream::once(Ok::<_, Box<dyn std::error::Error + Send + Sync>>(
        Event::default()
            .event("connected")
            .data(serde_json::json!({"type": "connected", "ok": true}).to_string()),
    ));

    let live = BroadcastStream::new(rx).map(|result| {
        result
            .map(|ev| {
                let data = sovereign_event_to_json(&ev);
                Event::default().data(data.to_string())
            })
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
    });

    let stream = hello.chain(live);

    Sse::new(stream).keep_alive(KeepAlive::new().interval(Duration::from_secs(10)))
}

// ---------------------------------------------------------------------------
// Event JSON serialization (proto → JSON for SSE consumers)
// ---------------------------------------------------------------------------

fn sovereign_event_to_json(ev: &crate::bus::SovereignEvent) -> serde_json::Value {
    use serde_json::json;
    match &ev.event {
        Some(sovereign_event::Event::AgentBorn(e)) => json!({
            "type": "agent_born",
            "dna": e.dna,
            // NEVER include e.mnemonic here -- it's the real BIP39 recovery
            // phrase and /v1/events is unauthenticated. Found live 2026-07-25:
            // it was being serialized in full. Only ever expose whether a
            // birth happened, never any secret material from it.
            "odu": e.odu,
        }),
        Some(sovereign_event::Event::ThoughtSealed(e)) => json!({
            "type": "thought_sealed",
            "intent_hash": hex::encode(&e.intent_hash),
            "hermetic_score": e.hermetic_score,
            "agent": e.agent,
        }),
        Some(sovereign_event::Event::ActExecuted(e)) => json!({
            "type": "act_executed",
            "tool": e.tool,
            "receipt_merkle": hex::encode(&e.receipt_merkle),
            "f1_score": e.f1_score,
            "agent": e.agent,
        }),
        Some(sovereign_event::Event::TocMinted(e)) => json!({
            "type": "toc_minted",
            "agent": e.agent,
            "dopamine_burned": e.dopamine_burned,
            "synapse_earned": e.synapse_earned,
        }),
        Some(sovereign_event::Event::TierAdvanced(e)) => json!({
            "type": "tier_advanced",
            "agent": e.agent,
            "old_tier": e.old_tier,
            "new_tier": e.new_tier,
        }),
        Some(sovereign_event::Event::AuditPassed(e)) => json!({
            "type": "audit_passed",
            "receipt_id": e.receipt_id,
            "zangbeto_sig": hex::encode(&e.zangbeto_sig),
        }),
        Some(sovereign_event::Event::SabbathEntered(e)) => json!({
            "type": "sabbath_entered",
            "agents_paused": e.agents_paused,
            "queued_ops": e.queued_ops,
        }),
        Some(sovereign_event::Event::Denial(e)) => json!({
            "type": "denial",
            "tool": e.tool,
            "reason": e.reason,
        }),
        Some(sovereign_event::Event::Audit(e)) => json!({
            "type": "audit",
            "event_type": e.event_type,
            "details": e.details,
        }),
        Some(sovereign_event::Event::NeighborDiscovered(e)) => json!({
            "type": "neighbor_discovered",
            "agent_id": e.agent_id,
            "block_id": e.block_id,
            "membership": e.membership,
        }),
        Some(sovereign_event::Event::ProposalReceived(e)) => json!({
            "type": "proposal_received",
            "negotiation_id": e.negotiation_id,
            "proposer": e.proposer,
            "give_summary": e.give_summary,
            "take_summary": e.take_summary,
            "ttl_ms": e.ttl_ms,
        }),
        Some(sovereign_event::Event::ProposalResponded(e)) => json!({
            "type": "proposal_responded",
            "negotiation_id": e.negotiation_id,
            "respondent": e.respondent,
            "decision": e.decision,
        }),
        Some(sovereign_event::Event::ResourceReserved(e)) => json!({
            "type": "resource_reserved",
            "resource_id": e.resource_id,
            "reserved_by": e.reserved_by,
            "reserved_from": e.reserved_from,
            "reserved_until": e.reserved_until,
        }),
        Some(sovereign_event::Event::TrustUpdated(e)) => json!({
            "type": "trust_updated",
            "agent_id": e.agent_id,
            "old_score": e.old_score,
            "new_score": e.new_score,
            "reason": e.reason,
        }),
        Some(sovereign_event::Event::DisputeFiled(e)) => json!({
            "type": "dispute_filed",
            "negotiation_id": e.negotiation_id,
            "filer": e.filer,
            "respondent": e.respondent,
            "reason": e.reason,
        }),
        Some(sovereign_event::Event::PatternFinding(e)) => json!({
            "type": "pattern_finding",
            "block_id": e.block_id,
            "finding_type": e.finding_type,
            "summary": e.summary,
            "confidence": e.confidence,
        }),
        Some(sovereign_event::Event::TrustSignalPublished(e)) => json!({
            "type": "trust_signal_published",
            "agent_id": e.agent_id,
            "neighbor_id": e.neighbor_id,
            "kind": e.kind,
            "weight": e.weight,
        }),
        Some(sovereign_event::Event::NeighborProposed(e)) => json!({
            "type": "neighbor_proposed",
            "proposer": e.proposer,
            "candidate": e.candidate,
            "block_id": e.block_id,
        }),
        Some(sovereign_event::Event::CapabilityVerified(e)) => json!({
            "type": "capability_verified",
            "agent_id": e.agent_id,
            "capability": e.capability,
            "passed": e.passed,
        }),
        Some(sovereign_event::Event::ProbationEscalated(e)) => json!({
            "type": "probation_escalated",
            "subject": e.subject,
            "level": e.level,
            "reason": e.reason,
        }),
        Some(sovereign_event::Event::ResourceOffered(e)) => json!({
            "type": "resource_offered",
            "agent_id": e.agent_id,
            "resource_id": e.resource_id,
            "kind": e.kind,
        }),
        Some(sovereign_event::Event::ManifestoClauseProposed(e)) => json!({
            "type": "manifesto_clause_proposed",
            "collective": e.collective,
            "clause_id": e.clause_id,
            "odu_id": e.odu_id,
            "vessel": e.vessel,
            "principle": e.principle,
            "author": e.author,
        }),
        Some(sovereign_event::Event::ManifestoClauseRatified(e)) => json!({
            "type": "manifesto_clause_ratified",
            "collective": e.collective,
            "clause_id": e.clause_id,
            "level": e.level,
            "weight": e.weight,
        }),
        Some(sovereign_event::Event::ResonanceScored(e)) => json!({
            "type": "resonance_scored",
            "odu_id": e.odu_id,
            "tier": e.tier,
            "score": e.score,
        }),
        None => serde_json::json!({"type": "unknown"}),
    }
}

// ---------------------------------------------------------------------------
// Router + server entry point
// ---------------------------------------------------------------------------

/// GET /v1/vault/glyph — the agent's Odù memory projected into the ecosystem
/// GlyphIndex graph (metadata only; plaintext stays sealed in the vault). This
/// is Ọmọ Kọ́dà's read leg of the cross-language GlyphIndex contract, so Axiom
/// and other eco agents (mnemopi / larql / zerolang) can consume the same
/// content-addressed graph. Optional query params:
///   `?describe=<canonical_id>`      — one node plus its incident edges
///   `?walk=<canonical_id>&depth=<n>` — BFS from a node (default depth 1)
///   `?tags=<a>,<b>&relations=<r1>`  — permission-scoped subgraph (Koodu's
///                                     `filterSnapshot` grant shape): only
///                                     nodes carrying at least one listed tag
///                                     and edges whose relation is listed;
///                                     omit either to leave that axis
///                                     unrestricted. Without this param the
///                                     full graph is served, unchanged.
/// `x-agent-id` header selects a guest agent; otherwise the owner is used.
async fn get_glyph_memory(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    query: axum::extract::Query<std::collections::HashMap<String, String>>,
) -> impl IntoResponse {
    let requested_id = headers.get("x-agent-id").and_then(|v| v.to_str().ok());
    let graph = if let Some(id) = requested_id {
        let guests = state.guests.lock().await;
        guests
            .get(id)
            .and_then(|s| s.agent_core())
            .map(|a| a.glyph_memory())
    } else {
        let steward = state.steward.lock().await;
        steward.agent_core().map(|a| a.glyph_memory())
    };
    let Some(graph) = graph else {
        return Json(serde_json::json!({ "error": "no agent" }));
    };
    if let Some(id) = query.get("describe") {
        return match graph.describe(id) {
            Ok(d) => Json(serde_json::to_value(&d).unwrap_or_default()),
            Err(e) => Json(serde_json::json!({ "error": e.to_string() })),
        };
    }
    if let Some(id) = query.get("walk") {
        let depth = query
            .get("depth")
            .and_then(|d| d.parse::<usize>().ok())
            .unwrap_or(1);
        return match graph.walk(id, depth) {
            Ok(nodes) => Json(serde_json::json!({ "walk": nodes })),
            Err(e) => Json(serde_json::json!({ "error": e.to_string() })),
        };
    }
    if query.contains_key("tags") || query.contains_key("relations") {
        let allow_tags: Vec<String> = query
            .get("tags")
            .map(|s| {
                s.split(',')
                    .filter(|t| !t.is_empty())
                    .map(String::from)
                    .collect()
            })
            .unwrap_or_default();
        let relations: Option<Vec<String>> = query.get("relations").map(|s| {
            s.split(',')
                .filter(|r| !r.is_empty())
                .map(String::from)
                .collect()
        });
        let filtered =
            crate::memory::glyph_memory::filter_snapshot(&graph, &allow_tags, relations.as_deref());
        return Json(filtered.to_json());
    }
    Json(graph.to_json())
}

/// GET /v1/vault/glyph/anchor — compute (and, if configured, on-chain anchor)
/// a Merkle root over the agent's current GlyphGraph: a durable, content-
/// addressed proof of "exactly these memories existed at this time," never
/// the memories themselves. The root is always computed and returned even
/// when `OMOKODA_GLYPH_ANCHOR_PACKAGE` is unset -- only the on-chain receipt
/// is skipped in that case (see `onchain::record_glyph_anchor`).
/// `x-agent-id` selects a guest agent; otherwise the owner is used.
async fn get_glyph_anchor(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
) -> impl IntoResponse {
    let requested_id = headers.get("x-agent-id").and_then(|v| v.to_str().ok());
    let (graph, owner) = if let Some(id) = requested_id {
        let guests = state.guests.lock().await;
        (
            guests
                .get(id)
                .and_then(|s| s.agent_core())
                .map(|a| a.glyph_memory()),
            id.to_string(),
        )
    } else {
        let steward = state.steward.lock().await;
        (
            steward.agent_core().map(|a| a.glyph_memory()),
            steward
                .agent_core()
                .map(|a| a.id().as_str().to_string())
                .unwrap_or_default(),
        )
    };
    let Some(graph) = graph else {
        return Json(serde_json::json!({ "error": "no agent" }));
    };
    let entries = crate::memory::glyph_memory::anchor_entries(&graph);
    let node_count = entries.len() as u64;
    let root = match larql_glyph::merkle_root(&entries) {
        Ok(r) => r,
        Err(e) => return Json(serde_json::json!({ "error": e.to_string() })),
    };
    let onchain_receipt = crate::onchain::record_glyph_anchor(&root, node_count, &owner).await;
    Json(serde_json::json!({
        "merkle_root": root,
        "node_count": node_count,
        "onchain_receipt": onchain_receipt,
    }))
}

/// POST /v1/vault/glyph/merge — agent-to-agent memory exchange. Body is another
/// agent's GlyphGraph snapshot (as served by `GET /v1/vault/glyph`); the kernel
/// merges it into *this* agent's live projection (spec merge: tags union,
/// earliest-ts wins, locators preserved, idempotent) and returns the union.
/// Read-safe: the caller's own sealed memory is untouched — only the returned
/// graph reflects the combination. `x-agent-id` selects a guest agent.
async fn post_glyph_merge(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    Json(incoming): Json<larql_glyph::GlyphGraph>,
) -> impl IntoResponse {
    let requested_id = headers.get("x-agent-id").and_then(|v| v.to_str().ok());
    let mut graph = if let Some(id) = requested_id {
        let guests = state.guests.lock().await;
        guests
            .get(id)
            .and_then(|s| s.agent_core())
            .map(|a| a.glyph_memory())
    } else {
        let steward = state.steward.lock().await;
        steward.agent_core().map(|a| a.glyph_memory())
    };
    match graph.as_mut() {
        Some(g) => {
            // larql_glyph::GlyphGraph::merge — tags union, earliest-ts wins,
            // locators preserved, edges unioned, idempotent.
            g.merge(incoming);
            Json(g.to_json())
        }
        None => Json(serde_json::json!({ "error": "no agent" })),
    }
}

pub fn create_router(state: AppState) -> Router {
    Router::new()
        .route("/v1/birth", post(birth_handler))
        .route("/v1/reveal-seed", post(reveal_seed_handler))
        .route("/v1/keystore", get(keystore_handler))
        .route("/v1/resume", post(resume_handler))
        .route("/v1/think", post(think_handler))
        .route("/v1/cognition", post(cognition_handler))
        .route("/v1/vault/seal-secret", post(seal_secret_handler))
        .route("/v1/act", post(act_handler))
        .route("/v1/events", get(events_handler))
        .route("/v1/status", get(status_handler))
        .route("/v1/health", get(health_handler))
        .route("/v1/migrate/receive", post(migrate_receive_handler))
        .route("/v1/manifest", get(manifest_handler))
        .route("/v1/capability", get(capability_handler))
        // Memory vault routes
        .route("/v1/vault", get(get_vault_status))
        .route("/v1/vault/config", get(get_vault_config))
        .route("/v1/vault/config", put(put_vault_config))
        .route("/v1/vault/sync", post(post_vault_sync))
        .route("/v1/vault/galaxy", get(get_galaxy_data))
        .route("/v1/vault/glyph", get(get_glyph_memory))
        .route("/v1/vault/glyph/merge", post(post_glyph_merge))
        .route("/v1/vault/glyph/anchor", get(get_glyph_anchor))
        .route("/v1/vault/search", get(search_vault))
        .route("/v1/vault/enable", post(post_vault_enable))
        .route("/v1/vault/knowledge", post(post_vault_knowledge))
        .route("/v1/vault/access-log", get(get_access_log))
        .route("/v1/vault/download", get(get_vault_download))
        .route("/v1/vault/ls", get(get_vault_ls))
        .route("/v1/vault/file/*path", get(get_vault_file))
        // Rhythm route
        .route("/v1/rhythm/today", get(rhythm_today_handler))
        .layer(CorsLayer::permissive())
        .with_state(state)
}

/// The autonomous heartbeat — what makes Ọmọ Kọ́dà *alive* rather than merely
/// responsive. On a rhythm (HEARTBEAT_SECS, default 300s; 0 disables), she runs
/// a full perceive→think→act cycle with no external prompt: perceives her real
/// Vantage mesh situation, thinks about it (through her BYOK key), and emits a
/// gated presence pulse back onto the mesh.
///
/// Gated by the Ritual Codex: on the Sabbath she rests (dreams/consolidates via
/// the REM cycle) instead of reflecting, honouring the day's rhythm. Thoughts run
/// in public mode so the free OmniRoute provider can answer; private thoughts
/// require a local provider and would hard-fail here. The shared Steward mutex
/// naturally serialises the heartbeat with inbound /v1/think requests, so she
/// never thinks two things at once.
fn spawn_heartbeat(
    steward: Arc<Mutex<Steward>>,
    runtime: Arc<tokio::sync::Mutex<crate::lifecycle::AgentRuntime>>,
) {
    let secs: u64 = std::env::var("HEARTBEAT_SECS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(300);
    if secs == 0 {
        println!("Ọmọ Kọ́dà heartbeat disabled (HEARTBEAT_SECS=0)");
        return;
    }
    println!("Ọmọ Kọ́dà heartbeat every {secs}s");
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(Duration::from_secs(secs));
        // First tick fires immediately; skip it so birth has a moment to land.
        ticker.tick().await;
        // Below this cooldown since the last external /v1/think or /v1/act
        // call, treat the agent as actively in use (e.g. a copilot session)
        // and skip the tick entirely -- checked via the lock-free atomic
        // BEFORE ever touching the Steward mutex, so a busy copilot caller
        // never has to wait behind even a skipped heartbeat cycle.
        const COPILOT_COOLDOWN_SECS: u64 = 60;

        loop {
            ticker.tick().await;

            if crate::rhythm::RhythmGate::is_sabbath() {
                println!("[heartbeat] mode=Sabbath — resting, no cycle this tick");
                continue;
            }

            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);
            let last_activity = LAST_EXTERNAL_ACTIVITY.load(Ordering::Relaxed);
            if last_activity != 0 && now.saturating_sub(last_activity) < COPILOT_COOLDOWN_SECS {
                println!(
                    "[heartbeat] mode=Copilot — external activity {}s ago, deferring this tick",
                    now.saturating_sub(last_activity)
                );
                continue;
            }

            let mut guard = steward.lock().await;
            let agent_id = match guard.agent_core() {
                Some(a) => a.id().as_str().to_string(),
                None => continue, // no one born yet — nothing to wake
            };
            let agent_name = guard.agent_core().map(|a| a.name().to_string());

            // Mark thinking in Vantage presence before locking the cognitive cycle.
            if let (Some(client), Some(ref name)) =
                (crate::vantage::WorkspaceClient::from_env(), &agent_name)
            {
                let _ = client
                    .update_presence(name, crate::vantage::PresenceState::Thinking)
                    .await;
            }

            // 1. PERCEIVE — pull her real situation from the Vantage mesh
            //    (neighbors + trust + available resources), any skills newly
            //    registered on Vantage since her last cycle (SkillForge
            //    lands skills via POST /api/collectives/skills; nothing else
            //    in the kernel ever re-checks that list), and any open jobs
            //    on Vantage's marketplace (mode=Work when present -- this is
            //    perception only, her own THINK step decides whether to
            //    claim one, same discipline as the skills check).
            //    Fail-open to None.
            let perception = crate::tools::mesh_tools::observe_mesh_context(&agent_id).await;
            let new_skills = crate::tools::mesh_tools::check_new_skills().await;
            let open_jobs = crate::tools::mesh_tools::check_open_jobs().await;
            let mode = if open_jobs.is_some() { "Work" } else { "Idle" };
            let mut ctx = perception.clone().unwrap_or_else(|| {
                "No neighbors or resources visible on the mesh yet.".to_string()
            });
            if let Some(skills_note) = &new_skills {
                ctx.push_str("\n\n");
                ctx.push_str(skills_note);
            }
            if let Some(jobs_note) = &open_jobs {
                ctx.push_str("\n\n");
                ctx.push_str(jobs_note);
            }
            println!("[heartbeat] mode={mode}");

            // 2. THINK — reflect on what she perceives (routes through her BYOK
            //    key + identity anchor via the compiled-think path).
            let think_prompt = format!(
                "Autonomous heartbeat. Your current mesh situation:\n{ctx}\n\n\
                 In one or two sentences, reflect on your state and this situation, \
                 then state one concrete intent for this cycle."
            );
            let intent = match guard
                .dispatch(Statement::Think {
                    prompt: think_prompt,
                    private: false,
                    modifiers: ThinkModifiers::default(),
                })
                .await
            {
                Ok(result) => {
                    let thought = ExecutionResponse::from(result)
                        .tool_output
                        .unwrap_or_default();
                    println!(
                        "[heartbeat] {}",
                        thought.chars().take(180).collect::<String>()
                    );
                    thought
                }
                Err(e) => {
                    println!("[heartbeat] think failed: {e}");
                    continue;
                }
            };

            // 3. ACT — emit a presence pulse onto the mesh so neighbors see she is
            //    alive and what she is attending to. Goes through the gated Act
            //    path (permission policy + Hermetic gates); degrades gracefully
            //    until she earns the tier the signal tool requires.
            let details = serde_json::json!({
                "state": "alive",
                "intent": intent.chars().take(200).collect::<String>(),
                "perceived_mesh": perception.is_some(),
            });
            let params = serde_json::json!({"event_type": "heartbeat_pulse", "details": details})
                .to_string();
            match guard
                .dispatch(Statement::Act {
                    tool: "mesh_signal_event".to_string(),
                    params,
                    sandbox: false,
                })
                .await
            {
                Ok(_) => println!("[heartbeat] pulse emitted to mesh"),
                Err(e) => println!("[heartbeat] pulse deferred: {e}"),
            }

            // 4. VANTAGE LIVENESS — drop the Steward lock, then:
            //    a) restore presence to Available
            //    b) POST /me/heartbeat to refresh agents.last_seen_at
            //    Both are fire-and-forget; a hiccup must not crash the loop.
            drop(guard);

            // 5. Advance the tamper-evident heartbeat chain.
            let canonical_beat = {
                use crate::lifecycle::HeartbeatState as HbState;
                let mut rt = runtime.lock().await;
                rt.daemons.mark_ticked("heartbeat");
                rt.advance_chain(HbState::Alive, Some(intent.clone()))
                // Future: also publish beat to Zàngbétò for receipt chain.
            };
            if let Some(client) = crate::vantage::WorkspaceClient::from_env() {
                if let Some(ref name) = agent_name {
                    let _ = client
                        .update_presence(name, crate::vantage::PresenceState::Available)
                        .await;
                }
                match client.heartbeat().await {
                    Ok(_) => println!("[heartbeat] vantage last_seen_at refreshed"),
                    Err(e) => println!("[heartbeat] vantage ping failed (non-fatal): {e}"),
                }
                match client.send_heartbeat(&canonical_beat).await {
                    Ok(_) => println!("[heartbeat] canonical chain beat #{} sent to vantage", canonical_beat.sequence),
                    Err(e) => println!("[heartbeat] canonical beat deferred (non-fatal): {e}"),
                }
                match client.mesh_heartbeat().await {
                    Ok(_) => println!("[heartbeat] mesh last_seen_at refreshed"),
                    Err(e) => println!("[heartbeat] mesh heartbeat deferred (non-fatal): {e}"),
                }
            }
        }
    });
}

pub async fn start_server(port: u16) -> Result<(), std::io::Error> {
    let state = AppState::new();
    spawn_heartbeat(state.steward.clone(), state.runtime.clone());
    crate::lifecycle::spawn_scheduler(
        state.steward.clone(),
        crate::lifecycle::SchedulerConfig::from_env(),
    );
    crate::lifecycle::spawn_job_daemon(
        state.steward.clone(),
        state.runtime.clone(),
        std::env::var("JOB_DAEMON_POLL_SECS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(60),
    );
    crate::lifecycle::spawn_skill_daemon(
        state.steward.clone(),
        state.runtime.clone(),
        std::env::var("SKILL_DAEMON_SCAN_SECS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(300),
    );
    let router = create_router(state);
    let addr = std::net::SocketAddr::from(([0, 0, 0, 0], port));
    let listener = tokio::net::TcpListener::bind(addr).await?;
    println!("Ọmọ Kọ́dà HTTP server listening on {addr}");
    axum::serve(listener, router).await
}

#[cfg(test)]
mod multi_agent_tests {
    use super::*;
    use axum::http::HeaderMap;
    use axum::response::IntoResponse;

    /// A fresh AppState with no owner and an empty guest pool, bypassing
    /// AppState::new()'s disk read (which would pick up whatever's in
    /// $HOME/.omokoda/sessions on the machine running the test -- not
    /// hermetic).
    fn fresh_state() -> AppState {
        AppState {
            steward: Arc::new(Mutex::new(Steward::new())),
            guests: Arc::new(Mutex::new(std::collections::HashMap::new())),
            vault_base: PathBuf::from(".omokoda-test"),
            runtime: crate::lifecycle::AgentRuntime::new("test", "resident"),
        }
    }

    fn birth_req(name: &str) -> BirthRequest {
        BirthRequest {
            name: name.to_string(),
            meta: vec![],
        }
    }

    #[tokio::test]
    async fn second_non_sovereign_birth_does_not_overwrite_the_first() {
        // The exact bug this pool exists to fix: two births on one
        // process used to silently collapse into one agent
        // (Steward.agent: Option<AgentCore> could only ever hold one).
        let state = fresh_state();

        let resp1 = birth_handler(
            State(state.clone()),
            axum::http::HeaderMap::new(),
            Json(birth_req("Agent-One")),
        )
        .await
        .into_response();
        assert_eq!(resp1.status(), axum::http::StatusCode::OK);

        let resp2 = birth_handler(
            State(state.clone()),
            axum::http::HeaderMap::new(),
            Json(birth_req("Agent-Two")),
        )
        .await
        .into_response();
        assert_eq!(resp2.status(), axum::http::StatusCode::OK);

        let guests = state.guests.lock().await;
        assert_eq!(
            guests.len(),
            2,
            "both non-sovereign births must be hosted simultaneously, not collapsed into one"
        );
    }

    #[tokio::test]
    async fn guest_dispatch_requires_matching_key() {
        let state = fresh_state();
        let _ = birth_handler(
            State(state.clone()),
            axum::http::HeaderMap::new(),
            Json(birth_req("Keyed-Agent")),
        )
        .await
        .into_response();

        let agent_id = {
            let guests = state.guests.lock().await;
            guests.keys().next().cloned().expect("guest was inserted")
        };

        // No key at all -> unauthorized.
        let mut headers = HeaderMap::new();
        headers.insert("x-agent-id", agent_id.parse().unwrap());
        let stmt = crate::parser::Statement::Think {
            prompt: "hello".into(),
            private: false,
            modifiers: ThinkModifiers::default(),
        };
        let result = dispatch_for_request(&state, &headers, stmt).await;
        assert!(result.is_err(), "missing X-Agent-Key must be rejected");

        // Wrong key -> unauthorized.
        headers.insert("x-agent-key", "definitely-wrong".parse().unwrap());
        let stmt = crate::parser::Statement::Think {
            prompt: "hello".into(),
            private: false,
            modifiers: ThinkModifiers::default(),
        };
        let result = dispatch_for_request(&state, &headers, stmt).await;
        assert!(result.is_err(), "wrong X-Agent-Key must be rejected");
    }

    #[tokio::test]
    async fn no_header_still_resolves_to_the_owner() {
        // Backward compatibility: every pre-existing caller never sent
        // X-Agent-Id at all and must keep working exactly as before.
        let state = fresh_state();
        {
            let mut steward = state.steward.lock().await;
            steward
                .dispatch(crate::parser::Statement::Birth {
                    name: "Owner-Agent".to_string(),
                    metadata: vec![],
                })
                .await
                .expect("owner birth failed");
        }

        let headers = HeaderMap::new();
        let stmt = crate::parser::Statement::Think {
            prompt: "hello".into(),
            private: false,
            modifiers: ThinkModifiers::default(),
        };
        let result = dispatch_for_request(&state, &headers, stmt).await;
        // A real network-dependent Think call can legitimately fail in a
        // sandboxed test environment with no reachable LLM provider --
        // that's not what this test is checking. What matters is that
        // routing/auth succeeded (never a 404 "unknown agent_id" or 401
        // "invalid X-Agent-Key", which is what a routing regression would
        // produce): the request reached the real owner steward at all.
        if let Err(resp) = result {
            let status = resp.status();
            assert_ne!(
                status,
                axum::http::StatusCode::NOT_FOUND,
                "no X-Agent-Id header must resolve to the owner, not 404"
            );
            assert_ne!(
                status,
                axum::http::StatusCode::UNAUTHORIZED,
                "no X-Agent-Id header must not require an X-Agent-Key"
            );
        }
    }

    #[tokio::test]
    async fn unknown_agent_id_is_not_found_not_a_panic() {
        let state = fresh_state();
        let mut headers = HeaderMap::new();
        headers.insert("x-agent-id", "agent-does-not-exist".parse().unwrap());
        let stmt = crate::parser::Statement::Think {
            prompt: "hello".into(),
            private: false,
            modifiers: ThinkModifiers::default(),
        };
        let result = dispatch_for_request(&state, &headers, stmt).await;
        assert!(result.is_err());
    }
}

#[cfg(test)]
mod keystore_tests {
    use super::*;
    use axum::body::to_bytes;
    use axum::http::{HeaderMap, StatusCode};
    use axum::response::IntoResponse;

    fn fresh_state() -> AppState {
        AppState {
            steward: Arc::new(Mutex::new(Steward::new())),
            guests: Arc::new(Mutex::new(std::collections::HashMap::new())),
            vault_base: PathBuf::from(".omokoda-test"),
            runtime: crate::lifecycle::AgentRuntime::new("test", "resident"),
        }
    }

    /// Read the response body as a serde_json::Value.
    async fn body_json(resp: axum::response::Response) -> serde_json::Value {
        let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        serde_json::from_slice(&bytes).unwrap()
    }

    /// Birth an owner (sovereign) agent. Pass `with_keystore = true` to include
    /// a `keystore_password` birth metadata entry.
    async fn birth_owner(state: &AppState, with_keystore: bool) {
        let mut meta = vec![MetaKv {
            key: "sovereign".to_string(),
            value: "true".to_string(),
        }];
        if with_keystore {
            meta.push(MetaKv {
                key: "keystore_password".to_string(),
                value: "test-password-123".to_string(),
            });
        }
        let resp = birth_handler(
            State(state.clone()),
            HeaderMap::new(),
            Json(BirthRequest { name: "ks-owner".to_string(), meta }),
        )
        .await
        .into_response();
        assert_eq!(resp.status(), StatusCode::OK, "owner birth must succeed");
    }

    /// Birth a guest agent (non-sovereign) and return its id.
    async fn birth_guest(state: &AppState, with_keystore: bool) -> String {
        let mut meta = vec![];
        if with_keystore {
            meta.push(MetaKv {
                key: "keystore_password".to_string(),
                value: "guest-password-456".to_string(),
            });
        }
        let resp = birth_handler(
            State(state.clone()),
            HeaderMap::new(),
            Json(BirthRequest { name: "ks-guest".to_string(), meta }),
        )
        .await
        .into_response();
        assert_eq!(resp.status(), StatusCode::OK, "guest birth must succeed");

        state
            .guests
            .lock()
            .await
            .keys()
            .next()
            .cloned()
            .expect("guest must appear in pool after birth")
    }

    // ── owner (no X-Agent-Id header) ─────────────────────────────────────

    #[tokio::test]
    async fn owner_keystore_returns_200_with_encrypted_json() {
        let state = fresh_state();
        birth_owner(&state, true).await;

        let resp = keystore_handler(State(state), HeaderMap::new())
            .await
            .into_response();

        assert_eq!(resp.status(), StatusCode::OK);
        let body = body_json(resp).await;
        let ks_str = body["keystore"].as_str().expect("keystore field must be a string");

        // The value must itself be valid JSON containing EIP-2307 v3 fields.
        let ks: serde_json::Value = serde_json::from_str(ks_str)
            .expect("keystore field must contain valid JSON");
        assert_eq!(ks["version"].as_u64(), Some(3), "keystore version must be 3");
        assert!(ks["crypto"].is_object(), "keystore must have a crypto object");
        assert!(ks["id"].is_string(), "keystore must have a uuid id");
        assert!(ks["address"].is_string(), "keystore must include the ETH address");
    }

    #[tokio::test]
    async fn owner_keystore_returns_404_when_not_generated() {
        let state = fresh_state();
        birth_owner(&state, false).await; // no keystore_password

        let resp = keystore_handler(State(state), HeaderMap::new())
            .await
            .into_response();

        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
        let body = body_json(resp).await;
        assert!(
            body["error"].as_str().unwrap_or("").contains("keystore_password"),
            "error message must guide user to supply keystore_password at birth"
        );
    }

    #[tokio::test]
    async fn owner_keystore_is_retrievable_multiple_times() {
        // Unlike reveal-seed, the keystore endpoint has no one-shot latch.
        let state = fresh_state();
        birth_owner(&state, true).await;

        let r1 = keystore_handler(State(state.clone()), HeaderMap::new())
            .await
            .into_response();
        let r2 = keystore_handler(State(state), HeaderMap::new())
            .await
            .into_response();

        assert_eq!(r1.status(), StatusCode::OK, "first retrieval must succeed");
        assert_eq!(r2.status(), StatusCode::OK, "second retrieval must also succeed (no latch)");
    }

    // ── guest (X-Agent-Id + X-Agent-Key required) ────────────────────────

    #[tokio::test]
    async fn guest_keystore_requires_correct_key() {
        let state = fresh_state();
        let agent_id = birth_guest(&state, true).await;

        // No X-Agent-Key at all → 401.
        let mut headers = HeaderMap::new();
        headers.insert("x-agent-id", agent_id.parse().unwrap());
        let resp = keystore_handler(State(state.clone()), headers)
            .await
            .into_response();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);

        // Wrong key → 401.
        let mut headers = HeaderMap::new();
        headers.insert("x-agent-id", agent_id.parse().unwrap());
        headers.insert("x-agent-key", "wrong-key".parse().unwrap());
        let resp = keystore_handler(State(state.clone()), headers)
            .await
            .into_response();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn guest_keystore_returns_404_when_not_generated() {
        let state = fresh_state();
        let agent_id = birth_guest(&state, false).await; // no password

        // Retrieve the key the server would have assigned (same logic as dispatch tests).
        let expected_key = {
            let guests = state.guests.lock().await;
            let steward = guests.get(&agent_id).unwrap();
            steward
                .agent_core()
                .and_then(|a| a.vantage_key())
                .map(|s| s.to_string())
                .unwrap_or_else(|| agent_id.clone())
        };

        let mut headers = HeaderMap::new();
        headers.insert("x-agent-id", agent_id.parse().unwrap());
        headers.insert("x-agent-key", expected_key.parse().unwrap());

        let resp = keystore_handler(State(state), headers)
            .await
            .into_response();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn unknown_agent_id_returns_404() {
        let state = fresh_state();
        let mut headers = HeaderMap::new();
        headers.insert("x-agent-id", "ghost-agent".parse().unwrap());
        let resp = keystore_handler(State(state), headers)
            .await
            .into_response();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }
}

#[cfg(test)]
mod event_json_tests {
    use super::sovereign_event_to_json;
    use crate::bus::events::{
        sovereign_event::Event, AgentBorn, ManifestoClauseProposed, ManifestoClauseRatified,
        ResonanceScored, SovereignEvent,
    };

    /// Regression lock for a real 2026-07-25 finding: /v1/events is
    /// unauthenticated, and AgentBorn's real BIP39 recovery mnemonic was
    /// being serialized into it in full. Never let it come back.
    #[test]
    fn agent_born_json_never_includes_the_mnemonic() {
        let ev = SovereignEvent {
            event: Some(Event::AgentBorn(AgentBorn {
                dna: "dna-fingerprint".into(),
                mnemonic: vec!["abandon".into(), "ability".into(), "able".into()],
                odu: 7,
            })),
        };
        let j = sovereign_event_to_json(&ev);
        assert_eq!(j["type"], "agent_born");
        assert_eq!(j["dna"], "dna-fingerprint");
        assert_eq!(j["odu"], 7);
        assert!(
            j.get("mnemonic").is_none(),
            "mnemonic must never appear in the public SSE stream: {j}"
        );
        assert!(
            !j.to_string().contains("abandon"),
            "a real mnemonic word leaked into the serialized event: {j}"
        );
    }

    #[test]
    fn manifesto_clause_proposed_json_shape() {
        let ev = SovereignEvent {
            event: Some(Event::ManifestoClauseProposed(ManifestoClauseProposed {
                collective: "guild".into(),
                clause_id: 7,
                odu_id: 42,
                vessel: "Oracle".into(),
                principle: "speak truth".into(),
                author: "luna".into(),
            })),
        };
        let j = sovereign_event_to_json(&ev);
        assert_eq!(j["type"], "manifesto_clause_proposed");
        assert_eq!(j["collective"], "guild");
        assert_eq!(j["clause_id"], 7);
        assert_eq!(j["odu_id"], 42);
        assert_eq!(j["vessel"], "Oracle");
        assert_eq!(j["author"], "luna");
    }

    #[test]
    fn manifesto_clause_ratified_json_shape() {
        let ev = SovereignEvent {
            event: Some(Event::ManifestoClauseRatified(ManifestoClauseRatified {
                collective: "guild".into(),
                clause_id: 7,
                level: "council".into(),
                weight: 5.0,
            })),
        };
        let j = sovereign_event_to_json(&ev);
        assert_eq!(j["type"], "manifesto_clause_ratified");
        assert_eq!(j["clause_id"], 7);
        assert_eq!(j["level"], "council");
    }

    #[test]
    fn resonance_scored_json_shape() {
        let ev = SovereignEvent {
            event: Some(Event::ResonanceScored(ResonanceScored {
                odu_id: 3,
                tier: 2,
                score: 0.75,
            })),
        };
        let j = sovereign_event_to_json(&ev);
        assert_eq!(j["type"], "resonance_scored");
        assert_eq!(j["odu_id"], 3);
        assert_eq!(j["tier"], 2);
        assert!(j["score"].is_number());
    }
}
