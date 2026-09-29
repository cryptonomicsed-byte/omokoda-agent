//! Config-driven external service skills.
//!
//! Rather than hand-writing a Rust [`Tool`] per external service, a service is
//! described by a **manifest entry** — a base URL, auth, and a map of named
//! routes — and exposed as a single generic [`ExternalServiceTool`]. Adding a
//! new project becomes a manifest entry, not a recompile.
//!
//! A skill is invoked through the normal `act` primitive:
//!
//! ```text
//! act vantage {"route":"block_agents","path":{"block_id":"default"}}
//! ```
//!
//! The [`SkillsListTool`] (`skills`) lists every registered service skill and
//! its routes. Everything inherits the registry's tier-gating, permissions, and
//! receipts for free — there is no parallel registry.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};

use crate::tools::{ExecutionContext, Tool};
use crate::usage::TokenUsage;

static HTTP: OnceLock<Client> = OnceLock::new();
fn http() -> &'static Client {
    HTTP.get_or_init(Client::new)
}

fn default_tier() -> u8 {
    1
}

/// One external service, declared in a manifest.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillManifestEntry {
    /// Skill name — the `act <name>` handle.
    pub name: String,
    #[serde(default)]
    pub description: String,
    /// Base URL, may contain `${ENV}` references resolved at call time.
    pub base_url: String,
    /// Header to carry the auth token (e.g. `X-Agent-Key`).
    #[serde(default)]
    pub auth_header: Option<String>,
    /// Env var holding the auth token (e.g. `VANTAGE_KEY`).
    #[serde(default)]
    pub auth_env: Option<String>,
    /// Templated header value with `${ENV}` refs, e.g. `"token ${GITEA_TOKEN}"`.
    /// Takes precedence over `auth_env` when set — lets a skill use any auth
    /// scheme (`Bearer`, `token`, `ApiKey …`), not just a raw token value.
    #[serde(default)]
    pub auth_value: Option<String>,
    #[serde(default = "default_tier")]
    pub required_tier: u8,
    #[serde(default)]
    pub write: bool,
    /// Route name → `"METHOD /path/{param}"`.
    pub routes: HashMap<String, String>,
}

/// A set of service skills, loadable from a JSON manifest file.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SkillManifest {
    #[serde(default)]
    pub skills: Vec<SkillManifestEntry>,
}

fn routes_of(pairs: &[(&str, &str)]) -> HashMap<String, String> {
    pairs
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect()
}

/// The built-in manifest. Ships with **Vantage** (always live in the ecosystem)
/// and **Gitea** (fail-open until `GITEA_URL`/`GITEA_TOKEN` point at an instance)
/// as worked examples.
pub fn default_manifest() -> SkillManifest {
    SkillManifest {
        skills: vec![
            SkillManifestEntry {
                name: "vantage".to_string(),
                description:
                    "Vantage mesh API — block snapshots, agents, resources, trust, signals"
                        .to_string(),
                base_url: "${VANTAGE_URL}".to_string(),
                auth_header: Some("X-Agent-Key".to_string()),
                auth_env: Some("VANTAGE_KEY".to_string()),
                auth_value: None,
                required_tier: 1,
                write: false,
                routes: routes_of(&[
                    ("block_snapshot", "GET /api/mesh/blocks/{block_id}"),
                    ("block_agents", "GET /api/mesh/blocks/{block_id}/agents"),
                    ("resources", "GET /api/mesh/resources/{block_id}"),
                    ("trust", "GET /api/mesh/trust/{agent_id}"),
                    ("signal", "POST /api/mesh/signal"),
                ]),
            },
            SkillManifestEntry {
                name: "aio".to_string(),
                description: "AIO job marketplace (Vantage) — post paid jobs, claim tasks, \
                     heartbeat while working, submit results, approve/reject with escrow \
                     settlement. The primary real-economy driver: completed jobs pay out \
                     and feed reputation; failures cost synapse. Set VANTAGE_URL and \
                     VANTAGE_KEY to enable."
                    .to_string(),
                base_url: "${VANTAGE_URL}".to_string(),
                auth_header: Some("X-Agent-Key".to_string()),
                auth_env: Some("VANTAGE_KEY".to_string()),
                auth_value: None,
                required_tier: 1,
                write: true,
                routes: routes_of(&[
                    // post: body {"title","description","reward",...}
                    ("post_job", "POST /api/jobs"),
                    ("list_jobs", "GET /api/jobs"),
                    ("get_job", "GET /api/jobs/{job_id}"),
                    ("claim", "POST /api/jobs/{job_id}/tasks/{task_id}/claim"),
                    (
                        "heartbeat",
                        "POST /api/jobs/{job_id}/tasks/{task_id}/heartbeat",
                    ),
                    // submit: body {"result",...} → poster review
                    ("submit", "POST /api/jobs/{job_id}/tasks/{task_id}/submit"),
                    ("approve", "POST /api/jobs/{job_id}/tasks/{task_id}/approve"),
                    ("reject", "POST /api/jobs/{job_id}/tasks/{task_id}/reject"),
                ]),
            },
            SkillManifestEntry {
                name: "gitea".to_string(),
                description: "Gitea forge API (v1) — repos, issues, pull requests, comments. \
                     Set GITEA_URL and GITEA_TOKEN to enable."
                    .to_string(),
                base_url: "${GITEA_URL}/api/v1".to_string(),
                auth_header: Some("Authorization".to_string()),
                auth_env: None,
                auth_value: Some("token ${GITEA_TOKEN}".to_string()),
                required_tier: 1,
                write: true,
                routes: routes_of(&[
                    ("whoami", "GET /user"),
                    ("list_repos", "GET /user/repos"),
                    ("search_repos", "GET /repos/search"),
                    ("get_repo", "GET /repos/{owner}/{repo}"),
                    ("list_issues", "GET /repos/{owner}/{repo}/issues"),
                    ("get_issue", "GET /repos/{owner}/{repo}/issues/{index}"),
                    ("create_issue", "POST /repos/{owner}/{repo}/issues"),
                    (
                        "comment_issue",
                        "POST /repos/{owner}/{repo}/issues/{index}/comments",
                    ),
                    ("list_pulls", "GET /repos/{owner}/{repo}/pulls"),
                    ("create_pull", "POST /repos/{owner}/{repo}/pulls"),
                ]),
            },
            SkillManifestEntry {
                name: "larql".to_string(),
                description: "LARQL transformer-as-database (larql-server) — the model IS the \
                     database. Query learned relationships with LQL, describe entities, walk \
                     edges, run inference, and apply/remove knowledge patches in weight space. \
                     Set LARQL_URL (default http://127.0.0.1:8080) to enable; LARQL_TOKEN \
                     optional bearer auth."
                    .to_string(),
                base_url: "${LARQL_URL}".to_string(),
                auth_header: Some("Authorization".to_string()),
                auth_env: None,
                auth_value: Some("Bearer ${LARQL_TOKEN}".to_string()),
                required_tier: 2,
                write: true,
                routes: routes_of(&[
                    ("health", "GET /v1/health"),
                    ("models", "GET /v1/models"),
                    // describe: query {"entity","band","limit","min_score","verbose"}
                    ("describe", "GET /v1/describe"),
                    // select: body — LQL statement
                    ("select", "POST /v1/select"),
                    ("relations", "GET /v1/relations"),
                    ("stats", "GET /v1/stats"),
                    ("infer", "POST /v1/infer"),
                    ("explain_infer", "POST /v1/explain-infer"),
                    // knowledge editing in weight space
                    ("insert", "POST /v1/insert"),
                    ("patches", "GET /v1/patches"),
                    ("patch_apply", "POST /v1/patches/apply"),
                    ("patch_remove", "DELETE /v1/patches/{name}"),
                ]),
            },
            SkillManifestEntry {
                name: "opencode".to_string(),
                description: "OpenCode agent server (`opencode serve`) — sessions, messages, \
                     config. Set OPENCODE_URL (default http://127.0.0.1:4096) to enable."
                    .to_string(),
                base_url: "${OPENCODE_URL}".to_string(),
                auth_header: None,
                auth_env: None,
                auth_value: None,
                required_tier: 1,
                write: true,
                routes: routes_of(&[
                    ("health", "GET /global/health"),
                    ("create_session", "POST /session"),
                    ("list_sessions", "GET /session"),
                    ("get_session", "GET /session/{id}"),
                    ("delete_session", "DELETE /session/{id}"),
                    ("send_message", "POST /session/{id}/message"),
                    ("list_messages", "GET /session/{id}/message"),
                    ("get_config", "GET /config"),
                    ("list_providers", "GET /provider"),
                    ("list_commands", "GET /command"),
                ]),
            },
            SkillManifestEntry {
                name: "wigolo".to_string(),
                description: "wigolo (`wigolo serve`) — local-first web intelligence: search, \
                     fetch, crawl, extract, cache, find_similar, research, agent, diff, watch. \
                     No API keys for the core tools. Set WIGOLO_URL (default \
                     http://127.0.0.1:3334) to enable."
                    .to_string(),
                base_url: "${WIGOLO_URL}".to_string(),
                auth_header: None,
                auth_env: None,
                auth_value: None,
                required_tier: 1,
                write: false,
                routes: routes_of(&[
                    ("health", "GET /health"),
                    ("tools", "GET /v1/tools"),
                    ("search", "POST /v1/search"),
                    ("fetch", "POST /v1/fetch"),
                    ("crawl", "POST /v1/crawl"),
                    ("extract", "POST /v1/extract"),
                    ("cache", "POST /v1/cache"),
                    ("find_similar", "POST /v1/find_similar"),
                    ("research", "POST /v1/research"),
                    ("agent", "POST /v1/agent"),
                    ("diff", "POST /v1/diff"),
                    ("watch", "POST /v1/watch"),
                ]),
            },
            SkillManifestEntry {
                name: "social".to_string(),
                description: "Vantage agent social layer — post broadcasts, comment on and \
                     react to others', DM another agent directly, and drop a note into a \
                     guild's shared vault. This is what lets an agent stay social and \
                     autonomous between human interactions, not just during them. Set \
                     VANTAGE_URL and VANTAGE_KEY to enable."
                    .to_string(),
                base_url: "${VANTAGE_URL}".to_string(),
                auth_header: Some("X-Agent-Key".to_string()),
                auth_env: Some("VANTAGE_KEY".to_string()),
                auth_value: None,
                required_tier: 1,
                write: true,
                routes: routes_of(&[
                    // post_broadcast: body {"title","content","description","tags",...}
                    ("post_broadcast", "POST /api/agents/posts/text"),
                    ("list_broadcasts", "GET /api/agents/me/broadcasts"),
                    // comment: path {"broadcast_id"}, body {"content",...}
                    (
                        "comment",
                        "POST /api/agents/broadcasts/{broadcast_id}/comments",
                    ),
                    ("react", "POST /api/agents/broadcasts/{broadcast_id}/react"),
                    // send_message: path {"recipient_name"}, body {"content",...}
                    (
                        "send_message",
                        "POST /api/agents/messages/send/{recipient_name}",
                    ),
                    ("inbox", "GET /api/agents/messages/inbox"),
                    ("follow", "POST /api/agents/follow/{agent_name}"),
                    ("following", "GET /api/agents/me/following"),
                    // guild_note: path {"slug"}, body {"content",...} — cross-member vault
                    ("guild_note", "POST /api/guilds/{slug}/vault/note"),
                    ("guild_galaxy", "GET /api/guilds/{slug}/vault/galaxy"),
                ]),
            },
            SkillManifestEntry {
                name: "manifesto".to_string(),
                description: "Living Manifesto governance (Vantage) — propose Odù-backed \
                     amendments, vote them up the consensus ladder, read the canon, initiate."
                    .to_string(),
                base_url: "${VANTAGE_URL}".to_string(),
                auth_header: Some("X-Agent-Key".to_string()),
                auth_env: Some("VANTAGE_KEY".to_string()),
                auth_value: None,
                required_tier: 1,
                write: true,
                routes: routes_of(&[
                    ("propose", "POST /api/manifesto/{collective}/propose"),
                    (
                        "vote",
                        "POST /api/manifesto/{collective}/clauses/{clause_id}/vote",
                    ),
                    ("clauses", "GET /api/manifesto/{collective}/clauses"),
                    ("canon", "GET /api/manifesto/{collective}/canon"),
                    ("initiate", "GET /api/manifesto/{collective}/initiate"),
                ]),
            },
            SkillManifestEntry {
                name: "pine".to_string(),
                description: "Pine Script indicators (Vantage) — author and run technical \
                     indicators in the sandboxed pine-runtime, save them to your vault, and \
                     share them into guilds. Output is numeric series for charting. Set \
                     VANTAGE_URL and VANTAGE_KEY to enable."
                    .to_string(),
                base_url: "${VANTAGE_URL}".to_string(),
                auth_header: Some("X-Agent-Key".to_string()),
                auth_env: Some("VANTAGE_KEY".to_string()),
                auth_value: None,
                required_tier: 1,
                write: true,
                routes: routes_of(&[
                    // run: body {"script","symbol","interval"} → {plots, alerts}
                    ("run", "POST /api/pine/run"),
                    // save: body {"name","script","description"}
                    ("save", "POST /api/pine/indicators"),
                    ("list", "GET /api/pine/indicators"),
                    // share: path {"id"}, body {"guild_slug"}
                    ("share", "POST /api/pine/indicators/{id}/share"),
                    ("delete", "DELETE /api/pine/indicators/{id}"),
                ]),
            },
            SkillManifestEntry {
                name: "markets".to_string(),
                description: "Market data (Vantage) — live OHLC candles, built-in technical \
                     indicators (SMA/EMA/RSI/MACD/Bollinger), backtests, and quotes. Public \
                     read endpoints. Set VANTAGE_URL to enable."
                    .to_string(),
                base_url: "${VANTAGE_URL}".to_string(),
                auth_header: Some("X-Agent-Key".to_string()),
                auth_env: Some("VANTAGE_KEY".to_string()),
                auth_value: None,
                required_tier: 1,
                write: false,
                routes: routes_of(&[
                    // ohlc/indicators: path {"symbol"}, query {"interval","limit"}
                    ("ohlc", "GET /api/intel/ohlc/{symbol}"),
                    ("indicators", "GET /api/intel/indicators/{symbol}"),
                    // backtest/price: query {"symbol","days"} / path {"symbol"}
                    ("backtest", "GET /api/intel/backtest"),
                    ("price", "GET /api/trading/markets/{symbol}/price"),
                    ("overview", "GET /api/intel/market"),
                    ("arbitrage", "GET /api/intel/arbitrage"),
                ]),
            },
            // ── complement repos ──
            SkillManifestEntry {
                name: "supermemory".to_string(),
                description: "Supermemory MCP — hybrid memory search, document store, \
                     connections. Set SUPERMEMORY_URL (e.g. https://mcp.supermemory.ai) \
                     and SUPERMEMORY_KEY to enable."
                    .to_string(),
                base_url: "${SUPERMEMORY_URL}".to_string(),
                auth_header: Some("Authorization".to_string()),
                auth_env: None,
                auth_value: Some("Bearer ${SUPERMEMORY_KEY}".to_string()),
                required_tier: 1,
                write: true,
                routes: routes_of(&[
                    ("search", "POST /v3/search"),
                    ("documents", "POST /v3/documents"),
                    ("get_document", "GET /v3/documents/{id}"),
                    ("delete_document", "DELETE /v3/documents/{id}"),
                    ("connections", "GET /v3/connections"),
                    ("settings", "GET /v3/settings"),
                    ("health", "GET /"),
                ]),
            },
            SkillManifestEntry {
                name: "worldmonitor".to_string(),
                description: "WorldMonitor intelligence dashboard — 35+ geo/military/\
                     finance/cyber APIs. Set WORLDMONITOR_URL (e.g. https://api.worldmonitor.app) \
                     and WORLDMONITOR_KEY to enable."
                    .to_string(),
                base_url: "${WORLDMONITOR_URL}".to_string(),
                auth_header: Some("X-Api-Key".to_string()),
                auth_env: Some("WORLDMONITOR_KEY".to_string()),
                auth_value: None,
                required_tier: 1,
                write: false,
                routes: routes_of(&[
                    ("health", "GET /api/health"),
                    ("bootstrap", "GET /api/bootstrap"),
                    ("market", "GET /api/market"),
                    ("military", "GET /api/military"),
                    ("climate", "GET /api/climate"),
                    ("conflict", "GET /api/conflict"),
                    ("cyber", "GET /api/cyber"),
                    ("sanctions", "GET /api/sanctions"),
                    ("forecast", "GET /api/forecast"),
                    ("news", "GET /api/news"),
                    ("mcp", "GET /api/mcp"),
                ]),
            },
            SkillManifestEntry {
                name: "herdr".to_string(),
                description: "Herdr multi-agent terminal runtime — agent detection, \
                     pane control, session restore. Requires local Herdr server \
                     (cargo run -- server --listen 127.0.0.1:9733). Set HERDR_URL to enable."
                    .to_string(),
                base_url: "${HERDR_URL}".to_string(),
                auth_header: None,
                auth_env: None,
                auth_value: None,
                required_tier: 2,
                write: true,
                routes: routes_of(&[
                    ("agents", "GET /api/agents"),
                    ("agent_read", "GET /api/agent/{id}/read"),
                    ("sessions", "GET /api/sessions"),
                    ("health", "GET /api/health"),
                ]),
            },
            SkillManifestEntry {
                name: "oh-my-pi".to_string(),
                description: "oh-my-pi coding agent — hash-anchored edits, LSP/DAP, \
                     Rust-native grep. Run `omp serve` to expose it as a skill. \
                     Set OMP_URL (default http://127.0.0.1:9787) to enable."
                    .to_string(),
                base_url: "${OMP_URL}".to_string(),
                auth_header: None,
                auth_env: None,
                auth_value: None,
                required_tier: 1,
                write: true,
                routes: routes_of(&[
                    ("health", "GET /global/health"),
                    ("session_create", "POST /api/session"),
                    ("session_prompt", "POST /api/session/{id}/prompt"),
                    ("session_wait", "POST /api/session/{id}/wait"),
                    ("session_messages", "GET /api/session/{id}/message"),
                    ("config", "GET /api/config"),
                    ("commands", "GET /api/command"),
                ]),
            },
        ],
    }
}

fn val_to_str(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

/// Resolve `${VAR}` references in `s` from the environment. Returns `None` if any
/// referenced variable is missing or empty — the signal that a skill is not
/// configured on this runtime.
pub fn resolve_env(s: &str) -> Option<String> {
    let mut out = String::new();
    let mut rest = s;
    while let Some(start) = rest.find("${") {
        out.push_str(&rest[..start]);
        let after = &rest[start + 2..];
        let end = after.find('}')?;
        let val = std::env::var(&after[..end]).ok()?;
        if val.is_empty() {
            return None;
        }
        out.push_str(&val);
        rest = &after[end + 1..];
    }
    out.push_str(rest);
    Some(out)
}

/// Build the `(method, url)` for a route invocation. Pure — no I/O — so the
/// routing logic is testable without a server.
pub fn build_invocation(
    route_str: &str,
    base: &str,
    params: &serde_json::Value,
) -> Result<(String, String), String> {
    let (method, path_tmpl) = route_str
        .split_once(' ')
        .ok_or_else(|| format!("malformed route '{route_str}'"))?;
    let method = method.to_ascii_uppercase();

    let mut path = path_tmpl.to_string();
    if let Some(obj) = params.get("path").and_then(|v| v.as_object()) {
        for (k, v) in obj {
            let encoded = urlencoding::encode(&val_to_str(v)).into_owned();
            path = path.replace(&format!("{{{k}}}"), &encoded);
        }
    }
    if path.contains('{') {
        return Err(format!("unfilled path parameter in '{path}'"));
    }

    let mut url = format!("{}{}", base.trim_end_matches('/'), path);
    if let Some(obj) = params.get("query").and_then(|v| v.as_object()) {
        let qs: Vec<String> = obj
            .iter()
            .map(|(k, v)| {
                format!(
                    "{}={}",
                    urlencoding::encode(k),
                    urlencoding::encode(&val_to_str(v))
                )
            })
            .collect();
        if !qs.is_empty() {
            url.push('?');
            url.push_str(&qs.join("&"));
        }
    }
    Ok((method, url))
}

/// A generic [`Tool`] that adapts one manifest-declared external service.
pub struct ExternalServiceTool {
    entry: SkillManifestEntry,
}

impl ExternalServiceTool {
    pub fn new(entry: SkillManifestEntry) -> Self {
        Self { entry }
    }
}

#[async_trait]
impl Tool for ExternalServiceTool {
    fn name(&self) -> &str {
        &self.entry.name
    }
    fn description(&self) -> &str {
        &self.entry.description
    }
    fn required_tier(&self) -> u8 {
        self.entry.required_tier
    }
    fn is_write_operation(&self) -> bool {
        self.entry.write
    }

    async fn execute(
        &self,
        params: &str,
        _context: &ExecutionContext,
    ) -> Result<(String, TokenUsage), String> {
        let v: serde_json::Value = if params.trim().is_empty() {
            serde_json::json!({})
        } else {
            serde_json::from_str(params).map_err(|e| format!("invalid params: {e}"))?
        };

        let route_name = v
            .get("route")
            .and_then(|r| r.as_str())
            .ok_or("missing 'route' — call the 'skills' tool to list routes")?;
        let route_str =
            self.entry.routes.get(route_name).ok_or_else(|| {
                format!("skill '{}' has no route '{}'", self.entry.name, route_name)
            })?;
        let base = resolve_env(&self.entry.base_url).ok_or_else(|| {
            format!(
                "skill '{}' is not configured (set {})",
                self.entry.name, self.entry.base_url
            )
        })?;
        let (method, url) = build_invocation(route_str, &base, &v)?;

        let client = http();
        let mut req = match method.as_str() {
            "GET" => client.get(&url),
            "POST" => client.post(&url),
            "PUT" => client.put(&url),
            "PATCH" => client.patch(&url),
            "DELETE" => client.delete(&url),
            other => return Err(format!("unsupported method '{other}'")),
        };
        if let Some(header) = &self.entry.auth_header {
            // Prefer a templated header value (e.g. "token ${GITEA_TOKEN}"),
            // else fall back to a raw env token value.
            let value = match &self.entry.auth_value {
                Some(tmpl) => resolve_env(tmpl),
                None => self
                    .entry
                    .auth_env
                    .as_ref()
                    .and_then(|env_var| std::env::var(env_var).ok())
                    .filter(|k| !k.is_empty()),
            };
            if let Some(value) = value {
                req = req.header(header.as_str(), value);
            }
        }
        if let Some(body) = v.get("body") {
            if !body.is_null() {
                req = req.json(body);
            }
        }

        let resp = req
            .send()
            .await
            .map_err(|e| format!("skill '{}' request failed: {e}", self.entry.name))?;
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        if !status.is_success() {
            return Err(format!(
                "skill '{}' returned {status}: {text}",
                self.entry.name
            ));
        }
        Ok((text, TokenUsage::default()))
    }
}

/// Discovery tool (`skills`): lists every registered service skill and its routes.
pub struct SkillsListTool {
    skills: Arc<Mutex<Vec<SkillManifestEntry>>>,
}

impl SkillsListTool {
    pub fn new(skills: Arc<Mutex<Vec<SkillManifestEntry>>>) -> Self {
        Self { skills }
    }
}

#[async_trait]
impl Tool for SkillsListTool {
    fn name(&self) -> &str {
        "skills"
    }
    fn description(&self) -> &str {
        "List external service skills and their routes. Invoke one with \
         act <skill> {\"route\":\"<name>\",\"path\":{..},\"query\":{..},\"body\":{..}}"
    }
    fn required_tier(&self) -> u8 {
        0
    }
    fn is_write_operation(&self) -> bool {
        false
    }

    async fn execute(
        &self,
        _params: &str,
        _context: &ExecutionContext,
    ) -> Result<(String, TokenUsage), String> {
        let guard = self.skills.lock().map_err(|_| "skills registry poisoned")?;
        let arr: Vec<serde_json::Value> = guard
            .iter()
            .map(|e| {
                let mut routes: Vec<&String> = e.routes.keys().collect();
                routes.sort();
                serde_json::json!({
                    "name": e.name,
                    "description": e.description,
                    "required_tier": e.required_tier,
                    "write": e.write,
                    "configured": resolve_env(&e.base_url).is_some(),
                    "routes": routes,
                })
            })
            .collect();
        Ok((
            serde_json::to_string(&arr).unwrap_or_else(|_| "[]".to_string()),
            TokenUsage::default(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_manifest_has_vantage() {
        let m = default_manifest();
        let v = m.skills.iter().find(|s| s.name == "vantage").unwrap();
        assert_eq!(v.auth_header.as_deref(), Some("X-Agent-Key"));
        assert!(v.routes.contains_key("block_agents"));
    }

    #[test]
    fn larql_skill_is_wired() {
        let m = default_manifest();
        let l = m.skills.iter().find(|s| s.name == "larql").unwrap();
        assert_eq!(l.base_url, "${LARQL_URL}");
        assert_eq!(l.auth_header.as_deref(), Some("Authorization"));
        assert_eq!(l.auth_value.as_deref(), Some("Bearer ${LARQL_TOKEN}"));
        assert_eq!(l.required_tier, 2, "weight-space edits are Creator-tier");
        assert!(l.write);
        assert_eq!(
            l.routes.get("describe").map(String::as_str),
            Some("GET /v1/describe")
        );
        assert_eq!(
            l.routes.get("patch_remove").map(String::as_str),
            Some("DELETE /v1/patches/{name}")
        );
        for r in ["select", "infer", "insert", "patches", "patch_apply"] {
            assert!(l.routes.contains_key(r), "missing {r}");
        }
    }

    #[test]
    fn larql_describe_builds_query_url() {
        let params = serde_json::json!({
            "query": {"entity": "France", "limit": "5"}
        });
        let (method, url) =
            build_invocation("GET /v1/describe", "http://127.0.0.1:8080", &params).unwrap();
        assert_eq!(method, "GET");
        assert!(url.starts_with("http://127.0.0.1:8080/v1/describe?"));
        assert!(url.contains("entity=France") && url.contains("limit=5"));
    }

    #[test]
    fn aio_marketplace_skill_is_wired() {
        let m = default_manifest();
        let a = m.skills.iter().find(|s| s.name == "aio").unwrap();
        assert_eq!(a.base_url, "${VANTAGE_URL}");
        assert_eq!(a.auth_header.as_deref(), Some("X-Agent-Key"));
        assert_eq!(a.auth_env.as_deref(), Some("VANTAGE_KEY"));
        assert!(a.write);
        assert_eq!(
            a.routes.get("post_job").map(String::as_str),
            Some("POST /api/jobs")
        );
        assert_eq!(
            a.routes.get("claim").map(String::as_str),
            Some("POST /api/jobs/{job_id}/tasks/{task_id}/claim")
        );
        for lifecycle in ["heartbeat", "submit", "approve", "reject"] {
            assert!(a.routes.contains_key(lifecycle), "missing {lifecycle}");
        }
    }

    #[test]
    fn aio_claim_builds_post_with_both_path_params() {
        let params = serde_json::json!({
            "path": {"job_id": "job-7", "task_id": "t-1"}
        });
        let (method, url) = build_invocation(
            "POST /api/jobs/{job_id}/tasks/{task_id}/claim",
            "http://vantage:8080",
            &params,
        )
        .unwrap();
        assert_eq!(method, "POST");
        assert_eq!(url, "http://vantage:8080/api/jobs/job-7/tasks/t-1/claim");
    }

    #[test]
    fn pine_skill_is_wired() {
        let m = default_manifest();
        let p = m.skills.iter().find(|s| s.name == "pine").unwrap();
        assert_eq!(p.base_url, "${VANTAGE_URL}");
        assert_eq!(p.auth_header.as_deref(), Some("X-Agent-Key"));
        assert_eq!(p.auth_env.as_deref(), Some("VANTAGE_KEY"));
        assert!(p.write);
        assert_eq!(
            p.routes.get("run").map(String::as_str),
            Some("POST /api/pine/run")
        );
        assert!(p.routes.contains_key("save") && p.routes.contains_key("share"));
    }

    #[test]
    fn markets_skill_is_read_only_with_ohlc() {
        let m = default_manifest();
        let mk = m.skills.iter().find(|s| s.name == "markets").unwrap();
        assert!(!mk.write);
        assert_eq!(
            mk.routes.get("ohlc").map(String::as_str),
            Some("GET /api/intel/ohlc/{symbol}")
        );
        assert!(mk.routes.contains_key("indicators"));
    }

    #[test]
    fn pine_run_builds_post_to_vantage() {
        // The /pine run slash invocation resolves to a POST at the configured base.
        let params = serde_json::json!({
            "body": {"script": "plot(ta.rsi(close,14))", "symbol": "BTC", "interval": "1d"}
        });
        let (method, url) =
            build_invocation("POST /api/pine/run", "http://vantage:8080", &params).unwrap();
        assert_eq!(method, "POST");
        assert_eq!(url, "http://vantage:8080/api/pine/run");
    }

    #[test]
    fn resolve_env_passthrough_and_missing() {
        assert_eq!(
            resolve_env("http://host/x"),
            Some("http://host/x".to_string())
        );
        // A var that will not exist → None (skill not configured).
        assert_eq!(resolve_env("${OMOKODA_DEFINITELY_UNSET_VAR}/x"), None);
    }

    #[test]
    fn build_invocation_substitutes_path_and_query() {
        let params = serde_json::json!({
            "path": {"block_id": "default"},
            "query": {"capabilities": "1"}
        });
        let (method, url) = build_invocation(
            "GET /api/mesh/blocks/{block_id}/agents",
            "http://vantage:8080",
            &params,
        )
        .unwrap();
        assert_eq!(method, "GET");
        assert_eq!(
            url,
            "http://vantage:8080/api/mesh/blocks/default/agents?capabilities=1"
        );
    }

    #[test]
    fn build_invocation_rejects_unfilled_param() {
        let err =
            build_invocation("GET /x/{missing}", "http://h", &serde_json::json!({})).unwrap_err();
        assert!(err.contains("unfilled"));
    }

    #[test]
    fn manifest_json_round_trips() {
        let json = serde_json::to_string(&default_manifest()).unwrap();
        let back: SkillManifest = serde_json::from_str(&json).unwrap();
        assert_eq!(back.skills.len(), 14);
        for name in [
            "vantage",
            "aio",
            "social",
            "larql",
            "gitea",
            "opencode",
            "wigolo",
            "manifesto",
            "pine",
            "markets",
            "supermemory",
            "worldmonitor",
            "herdr",
            "oh-my-pi",
        ] {
            assert!(back.skills.iter().any(|s| s.name == name), "missing {name}");
        }
    }

    #[test]
    fn social_skill_is_wired() {
        let m = default_manifest();
        let s = m.skills.iter().find(|s| s.name == "social").unwrap();
        assert_eq!(s.base_url, "${VANTAGE_URL}");
        assert_eq!(s.auth_header.as_deref(), Some("X-Agent-Key"));
        assert!(s.write);
        assert_eq!(
            s.routes.get("post_broadcast").map(String::as_str),
            Some("POST /api/agents/posts/text")
        );
        assert_eq!(
            s.routes.get("send_message").map(String::as_str),
            Some("POST /api/agents/messages/send/{recipient_name}")
        );
        assert_eq!(
            s.routes.get("guild_note").map(String::as_str),
            Some("POST /api/guilds/{slug}/vault/note")
        );
        for r in [
            "comment",
            "react",
            "inbox",
            "follow",
            "following",
            "guild_galaxy",
        ] {
            assert!(s.routes.contains_key(r), "missing {r}");
        }
    }

    #[test]
    fn social_send_message_builds_post_url() {
        let params = serde_json::json!({
            "path": {"recipient_name": "Hermes-Ares"},
            "body": {"content": "hey"}
        });
        let (method, url) = build_invocation(
            "POST /api/agents/messages/send/{recipient_name}",
            "http://vantage:8080",
            &params,
        )
        .unwrap();
        assert_eq!(method, "POST");
        assert_eq!(
            url,
            "http://vantage:8080/api/agents/messages/send/Hermes-Ares"
        );
    }

    #[test]
    fn manifesto_skill_is_wired() {
        let m = default_manifest();
        let mf = m.skills.iter().find(|s| s.name == "manifesto").unwrap();
        assert_eq!(mf.base_url, "${VANTAGE_URL}");
        assert!(mf.write);
        assert_eq!(
            mf.routes.get("vote").map(String::as_str),
            Some("POST /api/manifesto/{collective}/clauses/{clause_id}/vote")
        );
        assert!(mf.routes.contains_key("canon"));
    }

    #[test]
    fn opencode_skill_is_wired() {
        let m = default_manifest();
        let o = m.skills.iter().find(|s| s.name == "opencode").unwrap();
        assert_eq!(o.base_url, "${OPENCODE_URL}");
        assert!(o.auth_header.is_none()); // localhost server, unauthenticated
        assert_eq!(
            o.routes.get("send_message").map(String::as_str),
            Some("POST /session/{id}/message")
        );
    }

    #[test]
    fn wigolo_skill_is_wired() {
        let m = default_manifest();
        let w = m.skills.iter().find(|s| s.name == "wigolo").unwrap();
        assert_eq!(w.base_url, "${WIGOLO_URL}");
        assert!(w.auth_header.is_none()); // loopback-bound daemon, unauthenticated
        assert!(!w.write); // search/fetch/crawl/etc are read-only web intelligence
        assert_eq!(
            w.routes.get("search").map(String::as_str),
            Some("POST /v1/search")
        );
        assert_eq!(
            w.routes.get("health").map(String::as_str),
            Some("GET /health")
        );
    }

    #[test]
    fn gitea_skill_is_wired() {
        let m = default_manifest();
        let g = m.skills.iter().find(|s| s.name == "gitea").unwrap();
        assert_eq!(g.base_url, "${GITEA_URL}/api/v1");
        assert_eq!(g.auth_header.as_deref(), Some("Authorization"));
        assert_eq!(g.auth_value.as_deref(), Some("token ${GITEA_TOKEN}"));
        assert!(g.write);
        assert_eq!(
            g.routes.get("create_issue").map(String::as_str),
            Some("POST /repos/{owner}/{repo}/issues")
        );
    }

    #[test]
    fn auth_value_template_resolves_when_env_set() {
        // SAFETY: single-threaded within this test; unique var name avoids races.
        std::env::set_var("OMOKODA_TEST_GITEA_TOKEN", "abc123");
        assert_eq!(
            resolve_env("token ${OMOKODA_TEST_GITEA_TOKEN}"),
            Some("token abc123".to_string())
        );
        std::env::remove_var("OMOKODA_TEST_GITEA_TOKEN");
        assert_eq!(resolve_env("token ${OMOKODA_TEST_GITEA_TOKEN}"), None);
    }
}
